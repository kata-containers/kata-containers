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
use std::path::PathBuf;
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
use crate::network::endpoint::{TapEndpoint, VhostUserEndpoint};
use crate::network::network_info::network_info_from_dan::NetworkInfoFromDan;
use crate::network::utils::generate_private_mac_addr;
use crate::network::Endpoint;

/// Directly attachable network
pub struct Dan {
    inner: Arc<RwLock<DanInner>>,
}

pub struct DanInner {
    netns: Option<String>,
    entity_list: Vec<NetworkEntity>,
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
        let config: DanConfig = serde_json::from_str(&json_str).context("Invalid DAN config")?;
        info!(sl!(), "Dan config is loaded = {:?}", config);

        let (connection, handle, _) = rtnetlink::new_connection().context("New connection")?;
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });

        let mut entity_list = Vec::with_capacity(config.devices.len());
        for (idx, device) in config.devices.iter().enumerate() {
            let name = format!("eth{}", idx);
            let endpoint: Arc<dyn Endpoint> = match &device.device {
                Device::VhostUser {
                    path,
                    queue_num,
                    queue_size,
                } => Arc::new(
                    VhostUserEndpoint::new(
                        dev_mgr,
                        &name,
                        &device.guest_mac,
                        path,
                        *queue_num,
                        *queue_size,
                    )
                    .await
                    .with_context(|| format!("create a vhost user endpoint, path: {}", path))?,
                ),
                Device::HostTap {
                    tap_name,
                    queue_num,
                    queue_size,
                } => Arc::new(
                    TapEndpoint::new(
                        &handle,
                        &name,
                        tap_name,
                        &device.guest_mac,
                        *queue_num,
                        *queue_size,
                        dev_mgr,
                    )
                    .await
                    .with_context(|| format!("create a {} tap endpoint", tap_name))?,
                ),
            };

            let network_info = Arc::new(
                NetworkInfoFromDan::new(device)
                    .await
                    .context("Network info from DAN")?,
            );

            entity_list.push(NetworkEntity {
                endpoint,
                network_info,
            })
        }

        Ok(Self {
            netns: config.netns,
            entity_list,
        })
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
        for e in inner.entity_list.iter() {
            e.endpoint.attach().await.context("Attach")?;
        }
        Ok(())
    }

    async fn interfaces(&self) -> Result<Vec<agent::Interface>> {
        let inner = self.inner.read().await;
        let mut interfaces = vec![];
        for e in inner.entity_list.iter() {
            interfaces.push(e.network_info.interface().await.context("Interface")?);
        }
        Ok(interfaces)
    }

    async fn routes(&self) -> Result<Vec<agent::Route>> {
        let inner = self.inner.read().await;
        let mut routes = vec![];
        for e in inner.entity_list.iter() {
            let mut list = e.network_info.routes().await.context("Routes")?;
            routes.append(&mut list);
        }
        Ok(routes)
    }

    async fn neighs(&self) -> Result<Vec<agent::ARPNeighbor>> {
        let inner = self.inner.read().await;
        let mut neighs = vec![];
        for e in &inner.entity_list {
            let mut list = e.network_info.neighs().await.context("Neighs")?;
            neighs.append(&mut list);
        }
        Ok(neighs)
    }

    async fn save(&self) -> Option<Vec<EndpointState>> {
        let inner = self.inner.read().await;
        let mut ep_states = vec![];
        for e in &inner.entity_list {
            if let Some(state) = e.endpoint.save().await {
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
            e.endpoint.detach(h).await.context("Detach")?;
        }
        Ok(())
    }
}

/// Directly attachable network config
#[derive(Debug)]
pub struct DanNetworkConfig {
    pub dan_conf_path: PathBuf,
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
    PathBuf::from(config.runtime.dan_conf.as_str()).join(format!("{}.json", sandbox_id))
}

#[cfg(test)]
mod tests {
    use crate::network::dan::{ARPNeighbor, DanDevice, Device, Interface, NetworkInfo, Route};

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
                    "flags": 0
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
}
