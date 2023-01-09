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

#[derive(Debug)]
pub enum ShareFsMountType {
    PASSTHROUGH,
    RAFS,
}

/// ShareFsMountConfig: share fs mount config
#[derive(Debug)]
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
