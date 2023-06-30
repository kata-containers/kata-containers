// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[derive(Copy, Clone, Debug)]
pub enum ShareFsOperation {
    Mount,
    Umount,
    Update,
}

#[derive(Debug, Clone)]
pub enum ShareFsMountType {
    PASSTHROUGH,
    RAFS,
}

/// ShareFsMountConfig: share fs mount config
#[derive(Debug, Clone)]
pub struct ShareFsMountConfig {
    /// source: the passthrough fs exported dir or rafs meta file of rafs
    pub source: String,

    /// fstype: specifies the type of this sub-fs, could be passthrough-fs or rafs
    pub fstype: ShareFsMountType,

    /// mount_point: the mount point inside guest
    pub mount_point: String,

    /// config: the rafs backend config file
    pub config: Option<String>,

    /// tag: is the tag used inside the kata guest.
    pub tag: String,

    /// op: the operation to take, e.g. mount, umount or update
    pub op: ShareFsOperation,

    /// prefetch_list_path: path to file that contains file lists that should be prefetched by rafs
    pub prefetch_list_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ShareFsMountDevice {
    pub config: ShareFsMountConfig,
}

/// ShareFsDeviceConfig: share fs device config
#[derive(Debug, Clone)]
pub struct ShareFsDeviceConfig {
    /// fs_type: virtiofs or inline-virtiofs
    pub fs_type: String,

    /// socket_path: socket path for virtiofs
    pub sock_path: String,

    /// mount_tag: a label used as a hint to the guest.
    pub mount_tag: String,

    /// host_path: the host filesystem path for this volume.
    pub host_path: String,

    /// queue_size: queue size
    pub queue_size: u64,

    /// queue_num: queue number
    pub queue_num: u64,

    /// options: virtiofs device's config options.
    pub options: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ShareFsDevice {
    pub config: ShareFsDeviceConfig,
}
