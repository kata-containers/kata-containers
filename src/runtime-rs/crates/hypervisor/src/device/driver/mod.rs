// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod port_device;
mod protection_device;
mod vfio;
pub mod vfio_device;

/// Host Bridge (0x0600), PCI-to-PCI Bridge (0x0604) and Audio (0x0403) classes
/// cannot be passed through when sharing an IOMMU group. Matched exactly rather
/// than by base class so passthrough-capable bridges like NVSwitches (0x0680)
/// are not filtered out.
pub(crate) fn is_iommu_ignored_pci_class(class_code: u64) -> bool {
    matches!(class_code, 0x0600 | 0x0604 | 0x0403)
}
mod vhost_user;
pub mod vhost_user_blk;
mod vhost_user_net;
pub mod virtio_blk_modern;
mod virtio_fs;
mod virtio_net;
mod virtio_vsock;

pub use port_device::{PCIePortDevice, PortDeviceConfig};
pub use protection_device::{ProtectionDevice, ProtectionDeviceConfig, SevSnpConfig, TdxConfig};
pub use vfio::{
    bind_device_to_host, bind_device_to_vfio, get_vfio_device, HostDevice, VfioBusMode, VfioConfig,
    VfioDevice, VfioDeviceType,
};
pub use vfio_device::{
    is_vfio_ap_device, VfioDeviceBase, VfioDeviceModern, VfioDeviceModernHandle,
};
pub use vhost_user::{VhostUserConfig, VhostUserDevice, VhostUserType};
pub use vhost_user_net::VhostUserNetDevice;
pub use virtio_blk_modern::{BlockConfigModern, BlockDeviceFormat, BlockDeviceAio, BlockDeviceModern, BlockDeviceModernHandle, VIRTIO_BLOCK_CCW, VIRTIO_BLOCK_MMIO, VIRTIO_BLOCK_PCI, VIRTIO_PMEM,};
pub use virtio_fs::{
    ShareFsConfig, ShareFsDevice, ShareFsMountConfig, ShareFsMountOperation, ShareFsMountType,
};
pub use virtio_net::{Address, NetworkConfig, NetworkDevice};
pub use virtio_vsock::{
    HybridVsockConfig, HybridVsockDevice, VsockConfig, VsockDevice, DEFAULT_GUEST_VSOCK_CID,
};
pub use kata_types::device::{
    DRIVER_BLK_CCW_TYPE as KATA_CCW_DEV_TYPE, DRIVER_BLK_MMIO_TYPE as KATA_MMIO_BLK_DEV_TYPE,
    DRIVER_BLK_PCI_TYPE as KATA_BLK_DEV_TYPE, DRIVER_NVDIMM_TYPE as KATA_NVDIMM_DEV_TYPE,
    DRIVER_SCSI_TYPE as KATA_SCSI_DEV_TYPE,
};
