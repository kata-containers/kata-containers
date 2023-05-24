// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod vhost_user;
mod virtio_blk;
pub use virtio_blk::{
    BlockConfig, BlockDevice, KATA_BLK_DEV_TYPE, KATA_MMIO_BLK_DEV_TYPE, VIRTIO_BLOCK_MMIO,
    VIRTIO_BLOCK_PCI,
};
mod virtio_net;
pub use virtio_net::{Address, NetworkConfig, NetworkDevice};
mod vfio;
pub use vfio::{bind_device_to_host, bind_device_to_vfio, VfioBusMode, VfioConfig, VfioDevice};
mod virtio_fs;
pub use virtio_fs::{
    ShareFsDevice, ShareFsDeviceConfig, ShareFsMountConfig, ShareFsMountDevice, ShareFsMountType,
    ShareFsOperation,
};
mod virtio_vsock;
pub use virtio_vsock::{HybridVsockConfig, HybridVsockDevice, VsockConfig, VsockDevice};
