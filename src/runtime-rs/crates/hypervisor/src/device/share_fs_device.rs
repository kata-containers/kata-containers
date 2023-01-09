// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

/// ShareFsDeviceConfig: share fs device config
#[derive(Debug)]
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
}
