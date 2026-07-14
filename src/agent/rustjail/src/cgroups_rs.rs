// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! Re-exports for cgroups-rs 0.5.x.
//!
//! The legacy cgroupfs API moved under `cgroups::fs` in 0.5.x. Agent code
//! was written against the 0.3.x layout where those types lived at the crate
//! root; this module preserves the old paths.

pub use cgroups::fs::{
    blkio, cpu, cpuacct, cpuset, devices, hierarchies, hugetlb, memory, pid,
    BlkIoDeviceResource, BlkIoDeviceThrottleResource, Cgroup, Controller, DeviceResource,
    DeviceResources, Hierarchy, HugePageResource, MaxValue, NetworkPriority, Resources,
};
pub use cgroups::CgroupPid;
pub use cgroups::FreezerState;

pub mod freezer {
    pub use cgroups::fs::freezer::FreezerController;
    pub use cgroups::FreezerState;
}
