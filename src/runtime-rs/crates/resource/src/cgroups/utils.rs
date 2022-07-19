// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// When the Kata overhead threads (I/O, VMM, etc) are not
// placed in the sandbox resource controller (A cgroup on Linux),
// they are moved to a specific, unconstrained resource controller.
// On Linux, assuming the cgroup mount point is at /sys/fs/cgroup/,
// on a cgroup v1 system, the Kata overhead memory cgroup will be at
// /sys/fs/cgroup/memory/kata_overhead/$CGPATH where $CGPATH is
// defined by the orchestrator.
pub(crate) fn gen_overhead_path(path: &str) -> String {
    format!("kata_overhead/{}", path.trim_start_matches('/'))
}
