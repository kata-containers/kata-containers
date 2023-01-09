// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct CgroupState {
    pub path: Option<String>,
    pub overhead_path: Option<String>,
    pub sandbox_cgroup_only: bool,
}
