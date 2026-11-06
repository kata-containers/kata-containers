// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use serde::{Deserialize, Serialize};

/// The cgroups, as the recovery of a dead shim needs them: the paths to
/// destroy, and whether an overhead cgroup exists at all, which is what
/// `sandbox_cgroup_only` decides. Settings only a live sandbox consumes,
/// such as vCPU pinning, are not recorded.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct CgroupState {
    pub path: Option<String>,
    pub overhead_path: Option<String>,
    pub sandbox_cgroup_only: bool,
}
