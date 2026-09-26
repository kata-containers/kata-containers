// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use hypervisor::hypervisor_persist::HypervisorState;
use resource::resource_persist::ResourceState;
use serde::{Deserialize, Serialize};

use crate::vmm_process::VmmProcessIdentity;

#[derive(Serialize, Deserialize)]
pub struct SandboxState {
    pub sandbox_type: String,
    pub resource: Option<ResourceState>,
    pub hypervisor: Option<HypervisorState>,
    #[serde(default)]
    pub vmm_process: Option<VmmProcessIdentity>,
    #[serde(default)]
    pub vmm_exit_confirmed: bool,
}
