// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

// Resolve the cgroup v2 leaf used to attach an exec process.
// After systemd (init.scope) or DinD (init) nest PID 1, the container
// cgroup is an inner node and cgroup.procs writes to it fail with EBUSY.

use crate::cgroups_rs as cgroups;
use libc::pid_t;
use std::fs;

fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "cgroups"))
}

// Hierarchy-relative path the exec process should join.
pub fn exec_cgroup(pid: pid_t, init_pid: pid_t, container_cgroup: &str) -> String {
    let container_cgroup = normalize_cgroup_path(container_cgroup);

    if !cgroups::hierarchies::is_cgroup2_unified_mode() {
        return container_cgroup.to_string();
    }

    if init_pid <= 0 || pid == init_pid {
        return container_cgroup.to_string();
    }

    let contents = match fs::read_to_string(format!("/proc/{init_pid}/cgroup")) {
        Ok(contents) => contents,
        Err(err) => {
            debug!(sl(), "read /proc/{}/cgroup: {}", init_pid, err);
            return container_cgroup.to_string();
        }
    };

    if let Some(path) = cgroup_from_cgroup_file(&contents, container_cgroup) {
        debug!(
            sl(),
            "exec cgroup from init pid {}: {} (container {})", init_pid, path, container_cgroup
        );
        return path;
    }

    debug!(
        sl(),
        "no exec cgroup from init pid {}, using container", init_pid
    );
    container_cgroup.to_string()
}

// /proc/<pid>/cgroup is either the full hierarchy path (--cgroupns=host)
// or already relative to the container cgroup (--cgroupns=private).
// If the path is not under the container, exists() tells a private-ns
// view from init having moved elsewhere.
fn cgroup_from_cgroup_file(contents: &str, container_cgroup: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let init = normalize_cgroup_path(line.strip_prefix("0::")?);
        if init == container_cgroup || init.is_empty() {
            return Some(container_cgroup.to_string());
        }
        if let Some(rel) = init.strip_prefix(container_cgroup) {
            if let Some(rel) = rel.strip_prefix('/') {
                return Some(join_cgroup(container_cgroup, rel));
            }
        }
        let under_container = join_cgroup(container_cgroup, init);
        if cgroup_exists(&under_container) || !cgroup_exists(init) {
            Some(under_container)
        } else {
            Some(init.to_string())
        }
    })
}

fn join_cgroup(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else if child.is_empty() {
        parent.to_string()
    } else {
        format!("{parent}/{child}")
    }
}

fn cgroup_exists(path: &str) -> bool {
    cgroups::Cgroup::load(cgroups::hierarchies::auto(), path).exists()
}

// /proc and OCI cgroup paths are relative to the hierarchy; a leading
// '/' is the cgroup root, not the filesystem root.
pub(crate) fn normalize_cgroup_path(path: &str) -> &str {
    path.trim().trim_matches('/')
}
