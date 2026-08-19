// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

// Resolve the cgroup v2 leaf used to retry attaching an exec process.
// After systemd (init.scope) or DinD (init) nest PID 1, the container
// cgroup is an inner node and cgroup.procs writes to it fail with EBUSY.
//
// The same nesting has to be accounted for when a container goes away: the
// leaves it created are not visible in the container cgroup and nothing else
// removes them.

use crate::cgroups_rs as cgroups;
use libc::pid_t;
use std::fs;
use std::path::{Path, PathBuf};

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

// Processes of a container, including the ones it moved into nested cgroups.
// Reading the container cgroup alone misses them, so a container that nests
// PID 1 would keep processes running after being signalled or destroyed.
pub fn subtree_pids(cpath: &str) -> Vec<pid_t> {
    if !cgroups::hierarchies::is_cgroup2_unified_mode() {
        return Vec::new();
    }

    let cgroup = unified_path(cpath);
    let mut pids = procs(&cgroup);
    for nested in nested_cgroups(&cgroup) {
        pids.extend(procs(&nested));
    }
    pids
}

// Remove the cgroups a container nested below its own. They outlive the
// container and keep the container cgroup itself from being removed.
pub fn remove_nested_cgroups(cpath: &str) {
    if !cgroups::hierarchies::is_cgroup2_unified_mode() {
        return;
    }

    for nested in nested_cgroups(&unified_path(cpath)) {
        if let Err(err) = fs::remove_dir(&nested) {
            warn!(sl(), "remove nested cgroup {}: {}", nested.display(), err);
        }
    }
}

// Absolute path of a hierarchy-relative cgroup in the unified hierarchy.
fn unified_path(cpath: &str) -> PathBuf {
    cgroups::hierarchies::auto()
        .root()
        .join(normalize_cgroup_path(cpath))
}

// Cgroups below `cgroup`, deepest first so that they can be removed in order.
fn nested_cgroups(cgroup: &Path) -> Vec<PathBuf> {
    let entries = match fs::read_dir(cgroup) {
        Ok(entries) => entries,
        Err(err) => {
            debug!(sl(), "read cgroup {}: {}", cgroup.display(), err);
            return Vec::new();
        }
    };

    let mut nested = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|typ| typ.is_dir()).unwrap_or(false) {
            continue;
        }

        let path = entry.path();
        nested.extend(nested_cgroups(&path));
        nested.push(path);
    }
    nested
}

fn procs(cgroup: &Path) -> Vec<pid_t> {
    let path = cgroup.join("cgroup.procs");
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            debug!(sl(), "read {}: {}", path.display(), err);
            return Vec::new();
        }
    };

    contents
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

// /proc and OCI cgroup paths are relative to the hierarchy; a leading
// '/' is the cgroup root, not the filesystem root.
pub(crate) fn normalize_cgroup_path(path: &str) -> &str {
    path.trim().trim_matches('/')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgroups::fs::Manager as FsManager;
    use crate::cgroups::Manager as CgroupManager;
    use std::fs;
    use test_utils::skip_if_not_root;

    #[test]
    #[serial(cgroup_v2_nesting)]
    fn test_live_cgroup_v2_init_scope_lookup() {
        use std::process::{Command, Stdio};

        skip_if_not_root!();

        let root = cgroups::hierarchies::auto().root();
        let root_subtree_control = fs::read_to_string(root.join("cgroup.subtree_control")).unwrap();
        let root_memory_enabled = root_subtree_control
            .split_whitespace()
            .any(|controller| controller == "memory");
        if !root_memory_enabled {
            fs::write(root.join("cgroup.subtree_control"), "+memory").unwrap();
        }
        defer!({
            if !root_memory_enabled {
                let _ = fs::write(root.join("cgroup.subtree_control"), "-memory");
            }
        });

        let name = "kata-nest-test-init-scope";
        let parent = root.join(name);
        let init_scope = parent.join("init.scope");
        let _ = fs::remove_dir(&init_scope);
        let _ = fs::remove_dir(&parent);
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&init_scope).unwrap();
        let manager = FsManager::load_for_test(name);

        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let child_pid = child.id() as i32;

        defer!({
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_dir(&init_scope);
            let _ = fs::remove_dir(&parent);
        });

        fs::write(init_scope.join("cgroup.procs"), child_pid.to_string()).unwrap();

        let looked_up = init_cgroup(i32::MAX, child_pid);
        assert_eq!(looked_up, Some(format!("{name}/init.scope")));

        fs::write(parent.join("cgroup.subtree_control"), "+memory").unwrap();

        let mut extra = Command::new("sleep")
            .arg("5")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let extra_pid = extra.id() as i32;
        defer!({
            let _ = extra.kill();
            let _ = extra.wait();
        });

        let err = fs::write(parent.join("cgroup.procs"), extra_pid.to_string()).unwrap_err();
        assert_eq!(err.raw_os_error(), Some(libc::EBUSY));
        CgroupManager::apply(&manager, extra_pid, child_pid).unwrap();
        let procs = fs::read_to_string(init_scope.join("cgroup.procs")).unwrap();
        let extra_pid = extra_pid.to_string();
        assert!(procs.lines().any(|line| line == extra_pid.as_str()));
    }

    #[test]
    #[serial(cgroup_v2_nesting)]
    fn test_live_cgroup_v2_destroy_removes_nested_cgroups() {
        use std::process::{Command, Stdio};

        skip_if_not_root!();

        let root = cgroups::hierarchies::auto().root();
        let name = "kata-nest-test-destroy";
        let container = root.join(name);
        let leaf = container.join("init");
        let _ = fs::remove_dir(&leaf);
        let _ = fs::remove_dir(&container);
        fs::create_dir(&container).unwrap();
        fs::create_dir(&leaf).unwrap();

        defer!({
            let _ = fs::remove_dir(&leaf);
            let _ = fs::remove_dir(&container);
        });

        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let child_pid = child.id() as i32;

        fs::write(leaf.join("cgroup.procs"), child_pid.to_string()).unwrap();

        // The container cgroup is empty, so only walking the nested cgroups
        // finds the process that has to be killed.
        let mut manager = FsManager::load_for_test(name);
        assert!(fs::read_to_string(container.join("cgroup.procs"))
            .unwrap()
            .is_empty());
        assert_eq!(CgroupManager::get_pids(&manager).unwrap(), vec![child_pid]);

        child.kill().unwrap();
        child.wait().unwrap();

        CgroupManager::destroy(&mut manager).unwrap();
        assert!(!leaf.exists());
        assert!(!container.exists());
    }

    #[test]
    #[serial(cgroup_v2_nesting)]
    fn test_live_cgroup_v2_init_moved_outside() {
        use std::process::{Command, Stdio};

        skip_if_not_root!();

        let root = cgroups::hierarchies::auto().root();
        let container = root.join("kata-nest-test-container");
        let outside = root.join("kata-nest-test-outside");
        let _ = fs::remove_dir(&outside);
        let _ = fs::remove_dir(&container);
        fs::create_dir(&container).unwrap();
        fs::create_dir(&outside).unwrap();

        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let child_pid = child.id() as i32;

        defer!({
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_dir(&outside);
            let _ = fs::remove_dir(&container);
        });

        fs::write(outside.join("cgroup.procs"), child_pid.to_string()).unwrap();

        let looked_up = init_cgroup(i32::MAX, child_pid);
        assert_eq!(looked_up.as_deref(), outside.file_name().unwrap().to_str());
    }
}
