// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::network::EndpointState;
use serde::{Deserialize, Serialize};

use crate::cgroups::cgroup_persist::CgroupState;
#[derive(Serialize, Deserialize, Default)]
pub struct ResourceState {
    pub endpoint: Vec<EndpointState>,
    pub cgroup_state: Option<CgroupState>,
}
