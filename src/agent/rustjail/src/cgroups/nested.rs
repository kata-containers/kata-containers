// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

// Resolve the cgroup v2 leaf used to retry attaching an exec process.
// After systemd (init.scope) or DinD (init) nest PID 1, the container
// cgroup is an inner node and cgroup.procs writes to it fail with EBUSY.

use crate::cgroups_rs as cgroups;
use libc::pid_t;
use std::fs;

fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "cgroups"))
}

// Hierarchy-relative init cgroup to retry after attaching to the configured
// container cgroup fails.
pub fn init_cgroup(pid: pid_t, init_pid: pid_t) -> Option<String> {
    if !cgroups::hierarchies::is_cgroup2_unified_mode() {
        return None;
    }

    if init_pid <= 0 || pid == init_pid {
        return None;
    }

    let init_cgroup = cgroup_from_cgroup_file(init_pid)?;
    let path = normalize_cgroup_path(&init_cgroup).to_string();

    debug!(sl(), "init cgroup from pid {}: {}", init_pid, path);
    Some(path)
}

// Return the unified hierarchy path as recorded in /proc/<pid>/cgroup.
fn cgroup_from_cgroup_file(pid: pid_t) -> Option<String> {
    let contents = match fs::read_to_string(format!("/proc/{pid}/cgroup")) {
        Ok(contents) => contents,
        Err(err) => {
            debug!(sl(), "read /proc/{}/cgroup: {}", pid, err);
            return None;
        }
    };

    contents
        .lines()
        .find_map(|line| line.strip_prefix("0::").map(String::from))
}

// /proc and OCI cgroup paths are relative to the hierarchy; a leading
// '/' is the cgroup root, not the filesystem root.
pub(crate) fn normalize_cgroup_path(path: &str) -> &str {
    path.trim().trim_matches('/')
}
