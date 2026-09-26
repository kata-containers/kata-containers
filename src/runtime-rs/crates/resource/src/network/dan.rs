// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! Directly Attachable Network (DAN) is a type of network that runs in the host
//! netns. It supports host-tap, vhost-user (DPDK), etc.
//! The device information is retrieved from a JSON file, the type of which is
//! `Vec<DanDevice>`.
//! In this module, `IPAddress`, `Interface`, etc., are duplicated mostly from
//! `agent::IPAddress`, `agent::Interface`, and so on. They can't be referenced
//! directly because the former represents the structure of the JSON file written
//! by CNI plugins. They might have some slight differences, and may be revised in
//! the future.

use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use agent::IPFamily;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use hypervisor::Hypervisor;
use kata_sys_util::netns::NetnsGuard;
use kata_types::config::TomlConfig;
use scopeguard::defer;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::RwLock;

use super::network_entity::NetworkEntity;
use super::utils::address::{ip_family_from_ip_addr, parse_ip_cidr};
use super::{EndpointState, Network};
use crate::network::endpoint::{PhysicalEndpoint, TapEndpoint, VhostUserEndpoint};
use crate::network::network_info::network_info_from_dan::NetworkInfoFromDan;
use crate::network::utils::generate_private_mac_addr;
use crate::network::Endpoint;

/// Directly attachable network
pub struct Dan {
    inner: Arc<RwLock<DanInner>>,
}

struct DanEntity {
    entity: NetworkEntity,
    /// VFIO NICs are attached once the VM is up, the rest before boot.
    hotplug: bool,
}

pub struct DanInner {
    netns: Option<String>,
    entity_list: Vec<DanEntity>,
}

impl Dan {
    pub async fn new(
        config: &DanNetworkConfig,
        dev_mgr: Arc<RwLock<DeviceManager>>,
    ) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(DanInner::new(config, &dev_mgr).await?)),
        })
    }
}

impl DanInner {
    /// DanInner initialization deserializes DAN devices from a file writen
    /// by CNI plugins. Respective endpoint and network_info are retrieved
    /// from the devices, and compose NetworkEntity.
    async fn new(config: &DanNetworkConfig, dev_mgr: &Arc<RwLock<DeviceManager>>) -> Result<Self> {
        let json_str = fs::read_to_string(&config.dan_conf_path)
            .await
            .context("Read DAN config from file")?;
        let dan_config: DanConfig =
            serde_json::from_str(&json_str).context("Invalid DAN config")?;
        info!(sl!(), "Dan config is loaded = {:?}", dan_config);

        let (connection, handle, _) = rtnetlink::new_connection().context("New connection")?;
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });

        let mut entity_list = Vec::with_capacity(dan_config.devices.len());
        for (idx, device) in dan_config.devices.iter().enumerate() {
            let name = format!("eth{idx}");
            // The `network_queues` is a queue *pair* count.
            // Keep `queue_num` as a pair count and the hypervisor backend converts pairs into the actual virtqueue count.
            // A JSON-provided non-zero `queue_num` (also a pair count) with a higher priority always wins.
            let (qnum, qsize) = device.device.get_effective_queues(config.network_queues);
            let (endpoint, hotplug): (Arc<dyn Endpoint>, bool) = match &device.device {
                Device::VhostUser { path, .. } => (
                    Arc::new(
                        VhostUserEndpoint::new(
                            dev_mgr,
                            &name,
                            &device.guest_mac,
                            path,
                            qnum,
                            qsize,
                        )
                        .await
                        .with_context(|| format!("create a vhost user endpoint, path: {path}"))?,
                    ),
                    false,
                ),
                Device::HostTap { tap_name, .. } => (
                    Arc::new(
                        TapEndpoint::new(
                            &handle,
                            &name,
                            tap_name,
                            &device.guest_mac,
                            qnum,
                            qsize,
                            dev_mgr,
                        )
                        .await
                        .with_context(|| format!("create a {tap_name} tap endpoint"))?,
                    ),
                    false,
                ),
                Device::Vfio { pci_device_id } => (
                    Arc::new(
                        PhysicalEndpoint::new_dan(
                            &name,
                            &device.guest_mac,
                            pci_device_id,
                            dev_mgr.clone(),
                        )
                        .with_context(|| {
                            format!("create a vfio endpoint for device {pci_device_id}")
                        })?,
                    ),
                    true,
                ),
            };

            let network_info = Arc::new(
                NetworkInfoFromDan::new(device)
                    .await
                    .context("Network info from DAN")?,
            );

            entity_list.push(DanEntity {
                entity: NetworkEntity {
                    endpoint,
                    network_info,
                },
                hotplug,
            })
        }

        Ok(Self {
            netns: dan_config.netns,
            entity_list,
        })
    }
}

impl DanInner {
    async fn attach_entities(&self, hotplug: bool) -> Result<()> {
        for e in self.entity_list.iter().filter(|e| e.hotplug == hotplug) {
            if let Some(device_path) = e.entity.endpoint.attach().await.context("Attach")? {
                e.entity
                    .network_info
                    .set_device_path(device_path)
                    .await
                    .context("set device path")?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Network for Dan {
    async fn setup(&self) -> Result<()> {
        let inner = self.inner.read().await;
        let _netns_guard;
        if let Some(netns) = inner.netns.as_ref() {
            _netns_guard = NetnsGuard::new(netns).context("New netns guard")?;
        }
        inner.attach_entities(false).await
    }

    async fn setup_after_start_vm(&self) -> Result<()> {
        let inner = self.inner.read().await;
        // No netns guard: vfio-pci binding and QMP are netns independent, and
        // unlike setup() this runs on the shared multi-threaded runtime, where
        // a guard held across an await would leak into another task.
        inner.attach_entities(true).await
    }

    async fn interfaces(&self) -> Result<Vec<agent::Interface>> {
        let inner = self.inner.read().await;
        let mut interfaces = vec![];
        for e in inner.entity_list.iter() {
            let mut iface = e
                .entity
                .network_info
                .interface()
                .await
                .context("Interface")?;
            // A physical endpoint learns its path from the QMP pass rather than
            // from attach().
            if iface.device_path.is_empty() {
                if let Some(pci_path) = e.entity.endpoint.guest_pci_path().await {
                    iface.device_path = pci_path;
                }
            }
            interfaces.push(iface);
        }
        Ok(interfaces)
    }

    async fn routes(&self) -> Result<Vec<agent::Route>> {
        let inner = self.inner.read().await;
        let mut routes = vec![];
        for e in inner.entity_list.iter() {
            let mut list = e.entity.network_info.routes().await.context("Routes")?;
            routes.append(&mut list);
        }
        Ok(routes)
    }

    async fn neighs(&self) -> Result<Vec<agent::ARPNeighbor>> {
        let inner = self.inner.read().await;
        let mut neighs = vec![];
        for e in &inner.entity_list {
            let mut list = e.entity.network_info.neighs().await.context("Neighs")?;
            neighs.append(&mut list);
        }
        Ok(neighs)
    }

    async fn save(&self) -> Option<Vec<EndpointState>> {
        let inner = self.inner.read().await;
        let mut ep_states = vec![];
        for e in &inner.entity_list {
            if let Some(state) = e.entity.endpoint.save().await {
                ep_states.push(state);
            }
        }
        Some(ep_states)
    }

    async fn remove(&self, h: &dyn Hypervisor) -> Result<()> {
        let inner = self.inner.read().await;
        let _netns_guard;
        if let Some(netns) = inner.netns.as_ref() {
            _netns_guard = NetnsGuard::new(netns).context("New netns guard")?;
        }
        for e in inner.entity_list.iter() {
            e.entity.endpoint.detach(h).await.context("Detach")?;
        }
        Ok(())
    }

    async fn endpoints(&self) -> Vec<Arc<dyn Endpoint>> {
        let inner = self.inner.read().await;
        inner
            .entity_list
            .iter()
            .map(|e| e.entity.endpoint.clone())
            .collect()
    }
}

/// Directly attachable network config
#[derive(Debug)]
pub struct DanNetworkConfig {
    pub dan_conf_path: PathBuf,
    /// Number of virtio queue pairs (each pair = 1 RX + 1 TX).
    /// Derived from `network_queues` in the hypervisor TOML config.
    pub network_queues: usize,
}

/// Directly attachable network config written by CNI plugins
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DanConfig {
    netns: Option<String>,
    devices: Vec<DanDevice>,
}

/// Directly attachable network device
/// This struct is serilized from a file containing devices information,
/// sent from CNI plugins.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct DanDevice {
    // Name of device (interface name on the guest)
    pub(crate) name: String,
    // Mac address of interface on the guest, if it is not specified, a
    // private address is generated as default.
    #[serde(default = "generate_private_mac_addr")]
    pub(crate) guest_mac: String,
    // Device
    pub(crate) device: Device,
    // Network info
    pub(crate) network_info: NetworkInfo,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum Device {
    #[serde(rename = "vhost-user")]
    VhostUser {
        // Vhost-user socket path
        path: String,
        #[serde(default)]
        queue_num: usize,
        #[serde(default)]
        queue_size: usize,
    },
    #[serde(rename = "host-tap")]
    HostTap {
        tap_name: String,
        #[serde(default)]
        queue_num: usize,
        #[serde(default)]
        queue_size: usize,
    },
    /// `pci_device_id` is the host BDF. It is named as the Go runtime names it,
    /// so that one config file works for both runtimes.
    #[serde(rename = "vfio")]
    Vfio { pci_device_id: String },
}

impl Device {
    /// get the effective queue-pair count and queue size.
    pub(crate) fn get_effective_queues(&self, network_queues: usize) -> (usize, usize) {
        // The `network_queues` comes from hypervisor configurations, and we need to ensure that it is at least 1,
        // otherwise the network device will not work.
        let network_queues = network_queues.max(1);
        let (queue_num, queue_size) = match self {
            Device::VhostUser {
                queue_num,
                queue_size,
                ..
            }
            | Device::HostTap {
                queue_num,
                queue_size,
                ..
            } => (*queue_num, *queue_size),
            // A passed-through NIC has no virtio queues to size.
            Device::Vfio { .. } => (0, 0),
        };
        let qnum = if queue_num == 0 {
            network_queues
        } else {
            queue_num
        };
        let qsize = if queue_size == 0 { 256 } else { queue_size };
        (qnum, qsize)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct NetworkInfo {
    pub(crate) interface: Interface,
    #[serde(default)]
    pub(crate) routes: Vec<Route>,
    #[serde(default)]
    pub(crate) neighbors: Vec<ARPNeighbor>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Interface {
    // IP addresses in the format of CIDR
    pub ip_addresses: Vec<String>,
    #[serde(default = "default_mtu")]
    pub mtu: u64,
    #[serde(default)]
    // Link type
    pub ntype: String,
    #[serde(default)]
    pub flags: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Route {
    #[serde(default)]
    // Destination(CIDR), an empty string denotes no destination
    pub dest: String,
    #[serde(default)]
    // Gateway(IP Address), an empty string denotes no gateway
    pub gateway: String,
    // Source(IP Address), an empty string denotes no gateway
    #[serde(default)]
    pub source: String,
    // Scope
    #[serde(default)]
    pub scope: u32,
    #[serde(default)]
    pub flags: u32,
    #[serde(default)]
    pub mtu: u32,
}

impl Route {
    pub(crate) fn ip_family(&self) -> Result<IPFamily> {
        if !self.dest.is_empty() {
            return Ok(ip_family_from_ip_addr(
                &parse_ip_cidr(&self.dest)
                    .context("Parse ip addr from dest")?
                    .0,
            ));
        }

        if !self.gateway.is_empty() {
            return Ok(ip_family_from_ip_addr(
                &IpAddr::from_str(&self.gateway).context("Parse ip addr from gateway")?,
            ));
        }

        if !self.source.is_empty() {
            return Ok(ip_family_from_ip_addr(
                &IpAddr::from_str(&self.source).context("Parse ip addr from source")?,
            ));
        }

        Err(anyhow!("Failed to retrieve IP family from {:?}", self))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ARPNeighbor {
    // IP address in the format of CIDR
    pub ip_address: Option<String>,
    #[serde(default)]
    pub hardware_addr: String,
    #[serde(default)]
    pub state: u32,
    #[serde(default)]
    pub flags: u32,
}

fn default_mtu() -> u64 {
    1500
}

/// Path of DAN config, the file contains an array of DanDevices.
#[inline]
pub fn dan_config_path(config: &TomlConfig, sandbox_id: &str) -> PathBuf {
    PathBuf::from(config.runtime.dan_conf.as_str()).join(format!("{sandbox_id}.json"))
}

pub async fn dan_vfio_device_count(dan_conf_path: &Path) -> Result<usize> {
    let json_str = fs::read_to_string(dan_conf_path)
        .await
        .context("Read DAN config from file")?;
    let dan_config: DanConfig = serde_json::from_str(&json_str).context("Invalid DAN config")?;

    Ok(dan_config
        .devices
        .iter()
        .filter(|d| matches!(d.device, Device::Vfio { .. }))
        .count())
}

#[cfg(test)]
mod tests {
    use agent::IPFamily;

    use crate::network::dan::{
        ARPNeighbor, DanConfig, DanDevice, Device, Interface, NetworkInfo, Route,
    };

    #[test]
    fn test_dan_json() {
        let json_str = r#"{
            "name": "eth0",
            "guest_mac": "xx:xx:xx:xx:xx",
            "device": {
                "type": "vhost-user",
                "path": "/tmp/test",
                "queue_num": 1,
                "queue_size": 1
            },
            "network_info": {
                "interface": {
                    "ip_addresses": ["192.168.0.1/24"],
                    "mtu": 1500,
                    "ntype": "tuntap",
                    "flags": 0
                },
                "routes": [{
                    "dest": "172.18.0.0/16",
                    "source": "172.18.0.1",
                    "gateway": "172.18.31.1",
                    "scope": 0,
                    "flags": 0,
                    "mtu": 1450
                }],
                "neighbors": [{
                    "ip_address": "192.168.0.3/16",
                    "device": "",
                    "state": 0,
                    "flags": 0,
                    "hardware_addr": "xx:xx:xx:xx:xx"
                }]
            }
        }"#;
        let dev_from_json: DanDevice = serde_json::from_str(json_str).unwrap();
        let dev = DanDevice {
            name: "eth0".to_owned(),
            guest_mac: "xx:xx:xx:xx:xx".to_owned(),
            device: Device::VhostUser {
                path: "/tmp/test".to_owned(),
                queue_num: 1,
                queue_size: 1,
            },
            network_info: NetworkInfo {
                interface: Interface {
                    ip_addresses: vec!["192.168.0.1/24".to_owned()],
                    mtu: 1500,
                    ntype: "tuntap".to_owned(),
                    flags: 0,
                },
                routes: vec![Route {
                    dest: "172.18.0.0/16".to_owned(),
                    source: "172.18.0.1".to_owned(),
                    gateway: "172.18.31.1".to_owned(),
                    scope: 0,
                    flags: 0,
                    mtu: 1450,
                }],
                neighbors: vec![ARPNeighbor {
                    ip_address: Some("192.168.0.3/16".to_owned()),
                    hardware_addr: "xx:xx:xx:xx:xx".to_owned(),
                    state: 0,
                    flags: 0,
                }],
            },
        };

        assert_eq!(dev_from_json, dev);
    }

    #[test]
    fn test_dan_vfio_json() {
        let json_str = r#"{
            "netns": "/var/run/netns/cni-xxx",
            "devices": [{
                "name": "eth0",
                "guest_mac": "0a:58:0a:0a:00:05",
                "device": {
                    "type": "vfio",
                    "pci_device_id": "0000:85:02.5"
                },
                "network_info": {
                    "interface": {
                        "ip_addresses": ["10.10.0.5/24"],
                        "mtu": 1500
                    }
                }
            }]
        }"#;

        let config: DanConfig = serde_json::from_str(json_str).unwrap();
        assert_eq!(config.netns.as_deref(), Some("/var/run/netns/cni-xxx"));
        assert_eq!(
            config.devices[0].device,
            Device::Vfio {
                pci_device_id: "0000:85:02.5".to_owned()
            }
        );
    }

    /// A config as written by a CNI plugin in the field, with every optional
    /// member omitted.
    #[test]
    fn test_dan_vfio_json_from_cni_plugin() {
        let json_str = r#"{
          "netns": "/var/run/netns/cni-60318bd9-a55d-43c9-ad77-60c8ba270e76",
          "devices": [
            {
              "name": "eth0",
              "guest_mac": "0a:58:0a:c0:03:35",
              "device": {
                "type": "vfio",
                "pci_device_id": "0000:41:08.0"
              },
              "network_info": {
                "interface": {
                  "ip_addresses": [
                    "10.192.3.53/24"
                  ],
                  "mtu": 1500
                },
                "routes": [
                  {
                    "gateway": "10.192.3.1"
                  },
                  {
                    "dest": "10.192.0.0/15",
                    "gateway": "10.192.3.1"
                  }
                ]
              }
            }
          ]
        }"#;

        let config: DanConfig = serde_json::from_str(json_str).unwrap();
        assert_eq!(config.devices.len(), 1);

        let device = &config.devices[0];
        assert_eq!(
            device.device,
            Device::Vfio {
                pci_device_id: "0000:41:08.0".to_owned()
            }
        );
        assert_eq!(device.guest_mac, "0a:58:0a:c0:03:35");
        assert_eq!(device.network_info.interface.mtu, 1500);
        assert!(device.network_info.interface.ntype.is_empty());
        assert_eq!(device.network_info.interface.flags, 0);
        assert!(device.network_info.neighbors.is_empty());

        // A default route has no dest, so its family comes from the gateway.
        let default_route = &device.network_info.routes[0];
        assert!(default_route.dest.is_empty());
        assert_eq!(default_route.ip_family().unwrap(), IPFamily::V4);
        assert_eq!(
            device.network_info.routes[1].ip_family().unwrap(),
            IPFamily::V4
        );
    }

    #[test]
    fn test_vfio_device_has_no_virtio_queues() {
        let device = Device::Vfio {
            pci_device_id: "0000:85:02.5".to_owned(),
        };
        // Falls back to the defaults rather than rejecting the device.
        assert_eq!(device.get_effective_queues(4), (4, 256));
    }
}
