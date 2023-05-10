// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod virtio_blk;
pub use virtio_blk::{
    BlockConfig, KATA_BLK_DEV_TYPE, KATA_MMIO_BLK_DEV_TYPE, VIRTIO_BLOCK_MMIO, VIRTIO_BLOCK_PCI,
};
mod virtio_net;
pub use virtio_net::{Address, NetworkConfig};
mod vfio;
pub use vfio::{bind_device_to_host, bind_device_to_vfio, VfioBusMode, VfioConfig};
mod virtio_fs;
pub use virtio_fs::{ShareFsDeviceConfig, ShareFsMountConfig, ShareFsMountType, ShareFsOperation};
mod virtio_vsock;
use std::fmt;
pub use virtio_vsock::{HybridVsockConfig, VsockConfig};

#[derive(Debug)]
pub enum DeviceConfig {
    Block(BlockConfig),
    Network(NetworkConfig),
    ShareFsDevice(ShareFsDeviceConfig),
    Vfio(VfioConfig),
    ShareFsMount(ShareFsMountConfig),
    Vsock(VsockConfig),
    HybridVsock(HybridVsockConfig),
}

impl fmt::Display for DeviceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
