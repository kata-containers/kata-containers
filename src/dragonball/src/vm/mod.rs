// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use serde_derive::{Deserialize, Serialize};

/// Configuration information for user defined NUMA nodes.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct NumaRegionInfo {
    /// memory size for this region (unit: MiB)
    pub size: u64,
    /// numa node id on host for this region
    pub host_numa_node_id: Option<u32>,
    /// numa node id on guest for this region
    pub guest_numa_node_id: Option<u32>,
    /// vcpu ids belonging to this region
    pub vcpu_ids: Vec<u32>,
}
