// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod block;
pub use block::BlockConfig;
mod network;
pub use network::{Address, NetworkConfig};
mod share_fs_device;
pub use share_fs_device::ShareFsDeviceConfig;
mod vfio;
pub use vfio::{bind_device_to_host, bind_device_to_vfio, VfioBusMode, VfioConfig};
mod share_fs_mount;
pub use share_fs_mount::{ShareFsMountConfig, ShareFsMountType, ShareFsOperation};
mod vsock;
pub use vsock::{HybridVsockConfig, VsockConfig};

use std::fmt;

#[derive(Debug)]
pub enum Device {
    Block(BlockConfig),
    Network(NetworkConfig),
    ShareFsDevice(ShareFsDeviceConfig),
    Vfio(VfioConfig),
    ShareFsMount(ShareFsMountConfig),
    Vsock(VsockConfig),
    HybridVsock(HybridVsockConfig),
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
