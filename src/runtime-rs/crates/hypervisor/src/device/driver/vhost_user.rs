// Copyright (c) 2022-2023 Alibaba Cloud
// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::pci_path::PciPath;

#[derive(Debug, Clone, Default)]
pub enum VhostUserType {
    /// Blk - represents a block vhostuser device type
    #[default]
    Blk,

    /// SCSI - represents SCSI based vhost-user type
    SCSI,

    /// Net - represents Net based vhost-user type
    Net,

    /// FS - represents a virtio-fs vhostuser device type
    FS,
}

#[derive(Debug, Clone, Default)]
/// VhostUserConfig represents data shared by most vhost-user devices
pub struct VhostUserConfig {
    /// device id
    pub dev_id: String,
    /// socket path
    pub socket_path: String,
    /// mac_address is only meaningful for vhost user net device
    pub mac_address: String,

    /// vhost-user-fs is only meaningful for vhost-user-fs device
    pub tag: String,
    /// vhost-user-fs cache mode
    pub cache_mode: String,
    /// vhost-user-fs cache size in MB
    pub cache_size: u32,

    /// vhost user device type
    pub device_type: VhostUserType,
    /// guest block driver
    pub driver_option: String,
    /// pci_path is the PCI Path used to identify the slot at which the device is attached.
    pub pci_path: Option<PciPath>,

    /// Block index of the device if assigned
    /// type u64 is not OK
    pub index: u64,

    /// Virtio queue size. Size: byte
    pub queue_size: u32,
    /// Block device multi-queue
    pub num_queues: usize,

    /// device path in guest
    pub virt_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct VhostUserDevice {
    pub device_id: String,
    pub config: VhostUserConfig,
}
