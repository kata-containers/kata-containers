// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod vfio;
mod vhost_user;
pub mod vhost_user_blk;
mod vhost_user_net;
mod virtio_blk;
mod virtio_fs;
mod virtio_net;
mod virtio_vsock;

pub use vfio::{
    bind_device_to_host, bind_device_to_vfio, get_vfio_device, HostDevice, VfioBusMode, VfioConfig,
    VfioDevice,
};
pub use vhost_user::{VhostUserConfig, VhostUserDevice, VhostUserType};
pub use vhost_user_net::VhostUserNetDevice;
pub use virtio_blk::{
    BlockConfig, BlockDevice, KATA_BLK_DEV_TYPE, KATA_CCW_DEV_TYPE, KATA_MMIO_BLK_DEV_TYPE,
    KATA_NVDIMM_DEV_TYPE, VIRTIO_BLOCK_CCW, VIRTIO_BLOCK_MMIO, VIRTIO_BLOCK_PCI, VIRTIO_PMEM,
};
pub use virtio_fs::{
    ShareFsConfig, ShareFsDevice, ShareFsMountConfig, ShareFsMountOperation, ShareFsMountType,
};
pub use virtio_net::{Address, NetworkConfig, NetworkDevice};
pub use virtio_vsock::{
    HybridVsockConfig, HybridVsockDevice, VsockConfig, VsockDevice, DEFAULT_GUEST_VSOCK_CID,
};
