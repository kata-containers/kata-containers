// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::HypervisorConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Serialize, Deserialize, Default)]
pub struct HypervisorState {
    // Type of hypervisor, E.g. dragonball/qemu/firecracker/acrn.
    pub hypervisor_type: String,
    pub pid: Option<i32>,
    pub uuid: String,
    // clh sepcific: refer to 'virtcontainers/clh.go:CloudHypervisorState'
    pub api_socket: String,
    /// sandbox id
    pub id: String,
    /// vm path
    pub vm_path: String,
    /// jailed flag
    pub jailed: bool,
    /// chroot base for the jailer
    pub jailer_root: String,
    /// netns
    pub netns: Option<String>,
    /// hypervisor config
    pub config: HypervisorConfig,
    /// hypervisor run dir
    pub run_dir: String,
    /// cached block device
    pub cached_block_devices: HashSet<String>,
    pub virtiofs_daemon_pid: i32,
}
