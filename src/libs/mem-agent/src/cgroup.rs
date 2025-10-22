// Copyright (C) 2025 Kylin Soft. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use nix::sys::statfs::statfs;
use std::path::Path;

#[cfg(target_env = "musl")]
const CGROUP2_SUPER_MAGIC: nix::sys::statfs::FsType = nix::sys::statfs::FsType(0x63677270);
#[cfg(not(target_env = "musl"))]
use nix::sys::statfs::CGROUP2_SUPER_MAGIC;

pub const CGROUP_PATH: &str = "/sys/fs/cgroup/";
pub const MEMCGS_V1_PATH: &str = "/sys/fs/cgroup/memory";

pub fn is_cgroup_v2() -> Result<bool> {
    let cgroup_path = Path::new("/sys/fs/cgroup");

    let stat =
        statfs(cgroup_path).map_err(|e| anyhow!("statfs {:?} failed: {}", cgroup_path, e))?;
    Ok(stat.filesystem_type() == CGROUP2_SUPER_MAGIC)
}
