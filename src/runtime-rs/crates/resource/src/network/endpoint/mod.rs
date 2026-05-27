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
use hypervisor::Hypervisor;

use super::EndpointState;

#[async_trait]
pub trait Endpoint: std::fmt::Debug + Send + Sync {
    async fn name(&self) -> String;
    async fn hardware_addr(&self) -> String;
    async fn attach(&self) -> Result<()>;
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
}
