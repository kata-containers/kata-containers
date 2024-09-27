// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt;

use crate::device::driver::vhost_user_blk::VhostUserBlkDevice;
use crate::{
    BlockConfig, BlockDevice, HybridVsockConfig, HybridVsockDevice, Hypervisor as hypervisor,
    NetworkConfig, NetworkDevice, ShareFsConfig, ShareFsDevice, VfioConfig, VfioDevice,
    VhostUserConfig, VhostUserNetDevice, VsockConfig, VsockDevice,
};
use anyhow::Result;
use async_trait::async_trait;

use self::topology::PCIeTopology;

pub mod device_manager;
pub mod driver;
pub mod pci_path;
mod tap;
pub use self::tap::{Error as TapError, Tap};
pub mod topology;
pub mod util;

#[derive(Debug)]
pub enum DeviceConfig {
    BlockCfg(BlockConfig),
    VhostUserBlkCfg(VhostUserConfig),
    NetworkCfg(NetworkConfig),
    VhostUserNetworkCfg(VhostUserConfig),
    ShareFsCfg(ShareFsConfig),
    VfioCfg(VfioConfig),
    VsockCfg(VsockConfig),
    HybridVsockCfg(HybridVsockConfig),
}

#[derive(Debug, Clone)]
pub enum DeviceType {
    Block(BlockDevice),
    VhostUserBlk(VhostUserBlkDevice),
    Vfio(VfioDevice),
    Network(NetworkDevice),
    VhostUserNetwork(VhostUserNetDevice),
    ShareFs(ShareFsDevice),
    HybridVsock(HybridVsockDevice),
    Vsock(VsockDevice),
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[async_trait]
pub trait Device: std::fmt::Debug + Send + Sync {
    // attach is to plug device into VM
    async fn attach(
        &mut self,
        pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<()>;
    // detach is to unplug device from VM
    async fn detach(
        &mut self,
        pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<Option<u64>>;
    // update is to do update for some device
    async fn update(&mut self, h: &dyn hypervisor) -> Result<()>;
    // get_device_info returns device config
    async fn get_device_info(&self) -> DeviceType;
    // increase_attach_count is used to increase the attach count for a device
    // return values:
    // * true: no need to do real attach when current attach count is zero, skip following actions.
    // * err error: error while do increase attach count
    async fn increase_attach_count(&mut self) -> Result<bool>;
    // decrease_attach_count is used to decrease the attach count for a device
    // return values:
    // * false: no need to do real dettach when current attach count is not zero, skip following actions.
    // * err error: error while do decrease attach count
    async fn decrease_attach_count(&mut self) -> Result<bool>;
}

#[async_trait]
pub trait PCIeDevice: std::fmt::Debug + Send + Sync {
    // register pcie device into PCIe Topology for virtio-pci device or PCI/PCIe device.
    async fn register(&mut self, topology: &mut PCIeTopology) -> Result<()>;
    // unregister pcie device from PCIe Topology
    async fn unregister(&mut self, topology: &mut PCIeTopology) -> Result<()>;
}
