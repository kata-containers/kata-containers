// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod physical_endpoint;
pub use physical_endpoint::PhysicalEndpoint;
mod veth_endpoint;
pub use veth_endpoint::VethEndpoint;
mod ipvlan_endpoint;
pub use ipvlan_endpoint::IPVlanEndpoint;
mod vlan_endpoint;
pub use vlan_endpoint::VlanEndpoint;
mod macvlan_endpoint;
pub use macvlan_endpoint::MacVlanEndpoint;
pub mod endpoint_persist;
mod endpoints_test;
mod tap_endpoint;
pub use tap_endpoint::TapEndpoint;
mod vhost_user_endpoint;
pub use vhost_user_endpoint::VhostUserEndpoint;

use anyhow::Result;
use async_trait::async_trait;
use hypervisor::device::device_manager::{do_handle_device, DeviceManager};
use hypervisor::device::driver::NetworkConfig;
use hypervisor::device::{DeviceConfig, DeviceType};
use hypervisor::Hypervisor;
use tokio::sync::RwLock;
use std::sync::Arc;

use super::EndpointState;

pub(crate) async fn attach_network_device(
    d: &Arc<RwLock<DeviceManager>>,
    config: NetworkConfig,
) -> Result<Option<String>> {
    let device_info = do_handle_device(d, &DeviceConfig::NetworkCfg(config)).await?;
    match device_info {
        DeviceType::Network(net) => Ok(net.config.pci_path.map(|p| p.to_string())),
        _ => Ok(None),
    }
}

#[async_trait]
pub trait Endpoint: std::fmt::Debug + Send + Sync {
    async fn name(&self) -> String;
    async fn hardware_addr(&self) -> String;
    async fn attach(&self) -> Result<Option<String>>;
    async fn detach(&self, hypervisor: &dyn Hypervisor) -> Result<()>;
    async fn save(&self) -> Option<EndpointState>;
    /// Returns the guest PCI path for this endpoint if it is a cold-plugged
    /// physical (VFIO) device, e.g. `"05/00"`. Used to populate
    /// `Interface.device_path` in `update_interface` so the agent can do
    /// PCI-path-based MAC reconciliation for IB/RoCE VFs.
    /// Default: `None` (non-physical endpoints do not have a PCI path).
    async fn guest_pci_path(&self) -> Option<String> {
        None
    }
    /// Returns the QEMU device ID for the cold-plugged VF (e.g.
    /// `"physical_nic__346_0"`), or `None` for non-physical endpoints.
    /// Used to resolve the actual guest PCI path via QMP after VM start.
    async fn vfio_hostdev_id(&self) -> Option<String> {
        None
    }
    /// Update the guest PCI path (called after QMP resolution).
    async fn set_guest_pci_path(&self, _path: String) {}
    /// Returns the host BDF for this endpoint if it is a cold-plugged
    /// physical (VFIO) device, e.g. `"0000:06:02.5"`. Used to build the
    /// VFIO device option string for the agent `CreateContainer` request.
    /// Default: `None`.
    async fn host_bdf(&self) -> Option<String> {
        None
    }
}
