// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::HypervisorConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(default)]
pub struct HypervisorState {
    // Type of hypervisor, E.g. dragonball/qemu/firecracker.
    pub hypervisor_type: String,
    /// sandbox id
    pub id: String,
    /// vm path
    pub vm_path: String,
    /// jailed flag
    pub jailed: bool,
    /// chroot base for the jailer
    pub jailer_root: String,
    /// hypervisor config
    pub config: HypervisorConfig,
    /// cached block device
    pub cached_block_devices: HashSet<String>,
}
