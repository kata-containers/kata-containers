// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt;

use crate::device::driver::vhost_user_blk::VhostUserBlkDevice;
use crate::{
    BlockConfig, BlockDevice, HybridVsockConfig, HybridVsockDevice, Hypervisor as hypervisor,
    NetworkConfig, NetworkDevice, ShareFsDevice, ShareFsDeviceConfig, ShareFsMountConfig,
    ShareFsMountDevice, VfioConfig, VfioDevice, VhostUserConfig, VsockConfig,
};
use anyhow::Result;
use async_trait::async_trait;

pub mod device_manager;
pub mod driver;
pub mod util;

#[derive(Debug)]
pub enum DeviceConfig {
    BlockCfg(BlockConfig),
    VhostUserBlkCfg(VhostUserConfig),
    NetworkCfg(NetworkConfig),
    ShareFsCfg(ShareFsDeviceConfig),
    VfioCfg(VfioConfig),
    ShareFsMountCfg(ShareFsMountConfig),
    VsockCfg(VsockConfig),
    HybridVsockCfg(HybridVsockConfig),
}

#[derive(Debug, Clone)]
pub enum DeviceType {
    Block(BlockDevice),
    VhostUserBlk(VhostUserBlkDevice),
    Vfio(VfioDevice),
    Network(NetworkDevice),
    ShareFs(ShareFsDevice),
    ShareFsMount(ShareFsMountDevice),
    HybridVsock(HybridVsockDevice),
}

impl DeviceType {
    pub fn try_clone(&self) -> Result<DeviceType> {
        match self {
            DeviceType::Block(block) => Ok(DeviceType::Block(block.clone())),
            DeviceType::VhostUserBlk(vhost_user_blk) => {
                Ok(DeviceType::VhostUserBlk(vhost_user_blk.clone()))
            }
            DeviceType::Vfio(vfio) => Ok(DeviceType::Vfio(vfio.clone())),
            DeviceType::Network(network) => Ok(DeviceType::Network(network.clone())),
            DeviceType::ShareFs(share_fs) => Ok(DeviceType::ShareFs(share_fs.clone())),
            DeviceType::ShareFsMount(share_fs_mount) => {
                Ok(DeviceType::ShareFsMount(share_fs_mount.clone()))
            }
            DeviceType::HybridVsock(hvsock) => Ok(DeviceType::HybridVsock(hvsock.clone())),
        }
    }
}

impl fmt::Display for DeviceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[async_trait]
pub trait Device: std::fmt::Debug + Send + Sync {
    // attach is to plug device into VM
    async fn attach(&mut self, h: &dyn hypervisor) -> Result<()>;
    // detach is to unplug device from VM
    async fn detach(&mut self, h: &dyn hypervisor) -> Result<Option<u64>>;
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
