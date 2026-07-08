// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::{self, Error};
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures_lite::stream::StreamExt;
use hypervisor::device::device_manager::{do_handle_device, DeviceManager};
use hypervisor::device::driver::NetworkConfig;
use hypervisor::device::{DeviceConfig, DeviceType};
use hypervisor::{device::driver, Hypervisor, NetworkDevice};
use hypervisor::{get_vfio_device, VfioConfig};
use kata_sys_util::netns::NetnsGuard;
use netlink_packet_core::{NetlinkMessage, NLM_F_ACK, NLM_F_CREATE, NLM_F_EXCL, NLM_F_REQUEST};
use netlink_packet_route::{
    link::{LinkAttribute, LinkMessage, LinkVfInfo, VfInfo, VfInfoMac},
    RouteNetlinkMessage,
};
use rtnetlink::new_connection;
use tokio::sync::RwLock;

use super::endpoint_persist::{EndpointState, PhysicalEndpointState};
use super::Endpoint;
use crate::network::network_pair::NetworkPair;
use crate::network::utils::{self, link};

#[derive(Debug)]
pub struct VendorDevice {
    vendor_id: String,
    device_id: String,
}

impl VendorDevice {
    pub fn new(vendor_id: &str, device_id: &str) -> Result<Self> {
        if vendor_id.is_empty() || device_id.is_empty() {
            return Err(anyhow!(
                "invalid parameters vendor_id {} device_id {}",
                vendor_id,
                device_id
            ));
        }
        Ok(Self {
            vendor_id: vendor_id.to_string(),
            device_id: device_id.to_string(),
        })
    }

    pub fn vendor_device_id(&self) -> String {
        format!("{}_{}", &self.vendor_id, &self.device_id)
    }
}

#[derive(Debug)]
pub struct PhysicalEndpoint {
    iface_name: String,
    hard_addr: String,
    bdf: String,
    driver: String,
    vendor_device_id: VendorDevice,
    is_vfio: bool,
    bus_type: link::BusType,
    net_pair: NetworkPair,
    d: Arc<RwLock<DeviceManager>>,
    /// Guest PCI path — populated after QMP resolution in setup_after_start_vm.
    /// The pre-computed topology path from attach() is WRONG for physical
    /// endpoints because the root port has no explicit addr; the correct path
    /// requires QMP query-pci after VM boots.
    guest_pci_path: std::sync::Mutex<Option<String>>,
    /// QEMU device ID for the cold-plugged VF (e.g. "physical_nic__346_0").
    /// Stored during attach() for use in QMP-based path resolution.
    hostdev_id: std::sync::Mutex<Option<String>>,
}

impl PhysicalEndpoint {
    /// Create a new PhysicalEndpoint.
    ///
    /// For VFIO-capable (PCI) devices, the NIC will be passed through
    /// to the VM via VFIO — no tap/bridge pair is needed, so we create a minimal
    /// stub NetworkPair.
    ///
    /// For non-VFIO physical NICs (e.g. VMBus), we need a real tap+bridge pair
    /// (like veth endpoints use) so that traffic can flow between the host NIC
    /// and the VM through the hypervisor's TAP backend. The `handle`, `idx`,
    /// `model`, and `queues` parameters are required to create this pair.
    pub async fn new(
        handle: &rtnetlink::Handle,
        name: &str,
        hardware_addr: &[u8],
        idx: u32,
        model: &str,
        queues: usize,
        d: Arc<RwLock<DeviceManager>>,
    ) -> Result<Self> {
        // Determine bus type (PCI or VMBus) and resolve the sysfs device path.
        // For PCI: uses ethtool to get BDF, path = /sys/bus/pci/devices/<bdf>
        // For VMBus: resolves device symlink, path = /sys/bus/vmbus/devices/<guid>
        let (sys_iface_device_path, bdf, bus_type) =
            link::get_iface_device_path(name).context("get iface device path")?;
        let sys_device_path = Path::new(&sys_iface_device_path);

        // Get driver by following symlink <device_path>/driver
        let driver_path = sys_device_path.join("driver");
        let link_target = driver_path.read_link().context("read link")?;
        let driver = link_target
            .file_name()
            .map_or(String::new(), |v| v.to_str().unwrap().to_owned());

        // Get vendor and device id from sysfs device path
        let iface_device_path = sys_device_path.join("device");
        let device_id = std::fs::read_to_string(&iface_device_path)
            .with_context(|| format!("read device path {:?}", &iface_device_path))?;

        let iface_vendor_path = sys_device_path.join("vendor");
        let vendor_id = std::fs::read_to_string(&iface_vendor_path)
            .with_context(|| format!("read vendor path {:?}", &iface_vendor_path))?;
        let is_vfio = bus_type == link::BusType::Pci;

        // VFIO devices (PCI) use VFIO passthrough — no real tap/bridge needed.
        // Non-VFIO devices (VMBus) need a real tap+bridge pair for the hypervisor to
        // connect the host NIC traffic into the VM, matching Go's
        // createNetworkInterfacePair() call in createPhysicalEndpoint().
        let net_pair = if is_vfio {
            NetworkPair::new_for_physical(name, hardware_addr, is_vfio)
                .context("new network pair for physical vfio endpoint")?
        } else {
            NetworkPair::new(handle, idx, name, model, queues)
                .await
                .context("new network pair for physical non-vfio endpoint")?
        };

        Ok(Self {
            iface_name: name.to_string(),
            hard_addr: utils::get_mac_addr(hardware_addr).context("get mac addr")?,
            vendor_device_id: VendorDevice::new(&vendor_id, &device_id)
                .context("new vendor device")?,
            driver,
            bdf,
            is_vfio,
            bus_type,
            net_pair,
            d,
            guest_pci_path: std::sync::Mutex::new(None),
            hostdev_id: std::sync::Mutex::new(None),
        })
    }

    #[allow(dead_code)]
    pub fn network_pair(&self) -> &NetworkPair {
        &self.net_pair
    }

    fn get_network_config(&self) -> Result<NetworkConfig> {
        let iface = &self.net_pair.tap.tap_iface;
        let guest_mac = utils::parse_mac(&iface.hard_addr).ok_or_else(|| {
            Error::new(
                io::ErrorKind::InvalidData,
                format!("hard_addr {}", &iface.hard_addr),
            )
        })?;

        Ok(NetworkConfig {
            host_dev_name: iface.name.clone(),
            virt_iface_name: self.net_pair.virt_iface.name.clone(),
            guest_mac: Some(guest_mac),
            ..Default::default()
        })
    }
}

#[async_trait]
impl Endpoint for PhysicalEndpoint {
    async fn name(&self) -> String {
        self.iface_name.clone()
    }

    async fn hardware_addr(&self) -> String {
        self.hard_addr.clone()
    }

    async fn attach(&self) -> Result<()> {
        if self.is_vfio {
            // Push the desired netdev MAC down to the VF as an "admin MAC" via the
            // PF before we rebind to vfio-pci. Without this the guest mlx5_core
            // inherits the VF firmware default MAC, which differs from the
            // IB port HCA MAC, causing mlx5_ib's GID cache to not populate and
            // all RoCE verbs needing a GID to fail.
            // Best-effort: on error we warn and fall back to agent-side MAC
            // reconciliation (update_interface in rpc.rs).
            if !self.hard_addr.is_empty() && !self.bdf.is_empty() {
                let bdf = self.bdf.clone();
                let mac = self.hard_addr.clone();
                match tokio::task::spawn_blocking(move || set_vf_admin_mac_sync(&bdf, &mac))
                    .await
                    .context("spawn_blocking set_vf_admin_mac")?
                {
                    Ok(()) => {}
                    Err(e) => {
                        warn!(
                            sl!(),
                            "set_vf_admin_mac: skipped for {} ({}), \
                            falling back to in-guest MAC reconciliation",
                            self.bdf,
                            e
                        );
                    }
                }
            }
            // bind physical interface from host driver and bind to vfio
            driver::bind_device_to_vfio(
                &self.bdf,
                &self.driver,
                &self.vendor_device_id.vendor_device_id(),
            )
            .with_context(|| format!("bind physical endpoint from {} to vfio", &self.driver))?;

            let vfio_device =
                get_vfio_device(self.bdf.clone()).context("get vfio device failed.")?;
            let vfio_dev_config = &mut VfioConfig {
                host_path: vfio_device.clone(),
                dev_type: "pci".to_string(),
                hostdev_prefix: "physical_nic_".to_owned(),
                ..Default::default()
            };

            // create and insert VFIO device into Kata VM; do_handle_device returns
            // the DeviceType with guest_pci_path already computed by
            // do_add_pcie_endpoint() inside VfioDevice::register().
            let device_type =
                do_handle_device(&self.d, &DeviceConfig::VfioCfg(vfio_dev_config.clone()))
                    .await
                    .context("do handle device failed.")?;

            // Store the QEMU hostdev_id for later QMP-based PCI path resolution.
            // The topology-computed guest_pci_path from do_add_pcie_endpoint() is
            // WRONG for physical endpoints (root port has no explicit addr so QEMU
            // auto-assigns its slot; the correct path requires QMP after VM boot).
            if let hypervisor::device::DeviceType::Vfio(vfio_dev) = device_type {
                if let Some(hostdev) = vfio_dev.devices.first() {
                    if let Ok(mut guard) = self.hostdev_id.lock() {
                        *guard = Some(hostdev.hostdev_id.clone());
                    }
                }
            }

            Ok(())
        } else {
            self.net_pair
                .add_network_model()
                .await
                .context("add network model")?;
            let config = self.get_network_config().context("get network config")?;
            do_handle_device(&self.d, &DeviceConfig::NetworkCfg(config))
                .await
                .context("do handle network Physical endpoint device failed.")?;
            Ok(())
        }
    }

    // detach for physical endpoint unbinds the physical network interface from vfio-pci
    // and binds it back to the saved host driver.
    async fn detach(&self, _hypervisor: &dyn Hypervisor) -> Result<()> {
        if self.is_vfio {
            driver::bind_device_to_host(
                &self.bdf,
                &self.driver,
                &self.vendor_device_id.vendor_device_id(),
            )
            .with_context(|| {
                format!(
                    "bind physical endpoint device from vfio to {}",
                    &self.driver
                )
            })?;
            Ok(())
        } else {
            self.net_pair
                .del_network_model()
                .await
                .context("del network model")?;
            let config = self.get_network_config().context("get network config")?;
            _hypervisor
                .remove_device(DeviceType::Network(NetworkDevice {
                    config,
                    ..Default::default()
                }))
                .await
                .context("remove Physical endpoint device by hypervisor failed.")?;
            Ok(())
        }
    }

    async fn save(&self) -> Option<EndpointState> {
        Some(EndpointState {
            physical_endpoint: Some(PhysicalEndpointState {
                bdf: self.bdf.clone(),
                driver: self.driver.clone(),
                vendor_id: self.vendor_device_id.vendor_id.clone(),
                device_id: self.vendor_device_id.device_id.clone(),
                hard_addr: self.hard_addr.clone(),
                is_vfio: self.is_vfio,
                iface_name: self.iface_name.clone(),
                bus_type: match self.bus_type {
                    link::BusType::Pci => "pci".to_string(),
                    link::BusType::Vmbus => "vmbus".to_string(),
                },
            }),
            ..Default::default()
        })
    }

    async fn guest_pci_path(&self) -> Option<String> {
        self.guest_pci_path.lock().ok()?.clone()
    }

    async fn host_bdf(&self) -> Option<String> {
        Some(self.bdf.clone())
    }

    async fn vfio_hostdev_id(&self) -> Option<String> {
        self.hostdev_id.lock().ok()?.clone()
    }

    async fn set_guest_pci_path(&self, path: String) {
        if let Ok(mut guard) = self.guest_pci_path.lock() {
            *guard = Some(path);
        }
    }
}

// ---------------------------------------------------------------------------
// VF admin MAC helpers — mirror of Go's setVfAdminMAC / resolveVfPfPath /
// pfNetdevName in src/runtime/virtcontainers/physical_endpoint.go
// ---------------------------------------------------------------------------

/// Synchronous VF admin MAC setter, called via `spawn_blocking`.
/// Uses `NetnsGuard` to enter the host netns before opening the netlink
/// socket (attach() runs inside the pod netns).
fn set_vf_admin_mac_sync(vf_bdf: &str, mac: &str) -> Result<()> {
    let mac_bytes = parse_mac_str(mac)?;
    let (pf_bdf, vf_index) = resolve_vf_pf_path(vf_bdf)?;
    let pf_netdev = pf_netdev_name(&pf_bdf)?;

    // The caller runs inside the pod netns. The PF lives in the host netns.
    // Enter the host netns for the duration of the netlink RTM_SETLINK call.
    let _host_ns =
        NetnsGuard::new("/proc/1/ns/net").context("enter host netns for VF admin MAC")?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()
        .context("build runtime for VF admin MAC")?;

    rt.block_on(async {
        let (connection, mut handle, _) = new_connection().context("rtnetlink new_connection")?;
        tokio::spawn(connection);

        let pf_index = {
            let mut stream = handle.link().get().match_name(pf_netdev.clone()).execute();
            // try_next() yields Result<Option<LinkMessage>, rtnetlink::Error>
            let msg = stream
                .try_next()
                .await
                .map_err(|e| anyhow!("get PF link {}: {}", pf_netdev, e))?
                .ok_or_else(|| anyhow!("PF netdev {} not found", pf_netdev))?;
            msg.header.index
        };

        // Build RTM_SETLINK with IFLA_VFINFO_LIST — the netlink equivalent of
        //   ip link set <PF> vf <N> mac <MAC>
        let mut link_msg = LinkMessage::default();
        link_msg.header.index = pf_index;
        link_msg
            .attributes
            .push(LinkAttribute::VfInfoList(vec![LinkVfInfo(vec![
                VfInfo::Mac(VfInfoMac::new(vf_index as u32, &mac_bytes)),
            ])]));

        let mut req: NetlinkMessage<RouteNetlinkMessage> =
            NetlinkMessage::from(RouteNetlinkMessage::SetLink(link_msg));
        req.header.flags = NLM_F_REQUEST | NLM_F_ACK | NLM_F_EXCL | NLM_F_CREATE;
        let mut response = handle.request(req).context("send RTM_SETLINK")?;
        while response.next().await.is_some() {
            // drain the response; errors in the netlink ACK will have
            // caused handle.request() to return Err above.
        }
        Ok::<_, anyhow::Error>(())
    })
}

fn parse_mac_str(mac: &str) -> Result<[u8; 6]> {
    let parts: Vec<&str> = mac.split(':').collect();
    if parts.len() != 6 {
        return Err(anyhow!("invalid MAC {:?}", mac));
    }
    let mut out = [0u8; 6];
    for (i, p) in parts.iter().enumerate() {
        out[i] = u8::from_str_radix(p, 16).with_context(|| format!("invalid MAC octet {:?}", p))?;
    }
    Ok(out)
}

fn resolve_vf_pf_path(vf_bdf: &str) -> Result<(String, usize)> {
    use std::fs;
    let physfn = format!("/sys/bus/pci/devices/{}/physfn", vf_bdf);
    let pf_target = std::fs::read_link(&physfn).with_context(|| format!("readlink {}", physfn))?;
    let pf_bdf = pf_target
        .file_name()
        .ok_or_else(|| anyhow!("no file_name in physfn target"))?
        .to_string_lossy()
        .into_owned();

    let pf_dir = format!("/sys/bus/pci/devices/{}", pf_bdf);
    let entries = fs::read_dir(&pf_dir).with_context(|| format!("read_dir {}", pf_dir))?;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("virtfn") {
            continue;
        }
        let idx: usize = match name["virtfn".len()..].parse() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let link_path = format!("{}/{}", pf_dir, name);
        let target = match std::fs::read_link(&link_path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if target
            .file_name()
            .map(|n| n.to_string_lossy() == vf_bdf)
            .unwrap_or(false)
        {
            return Ok((pf_bdf, idx));
        }
    }
    Err(anyhow!("no virtfn under {} links to {}", pf_dir, vf_bdf))
}

fn pf_netdev_name(pf_bdf: &str) -> Result<String> {
    use std::fs;
    let net_dir = format!("/sys/bus/pci/devices/{}/net", pf_bdf);
    let mut names: Vec<String> = fs::read_dir(&net_dir)
        .with_context(|| format!("read_dir {}", net_dir))?
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    match names.len() {
        0 => Err(anyhow!(
            "no netdev under {} (PF not bound to driver?)",
            net_dir
        )),
        1 => Ok(names.remove(0)),
        _ => {
            warn!(
                sl!(),
                "PF {} has multiple netdevs {:?}, picking first", pf_bdf, names
            );
            Ok(names.remove(0))
        }
    }
}
