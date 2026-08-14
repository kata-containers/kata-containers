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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use test_utils::skip_if_not_root;

    #[test]
    fn test_cgroup_from_cgroup_file() {
        let test_cases = vec![
            (
                "0::/system.slice/cri-containerd-abc.scope/init.scope\n",
                "system.slice/cri-containerd-abc.scope",
                Some("system.slice/cri-containerd-abc.scope/init.scope"),
            ),
            (
                "0::/system.slice/cri-containerd-abc.scope/init.scope\n",
                "/system.slice/cri-containerd-abc.scope",
                Some("system.slice/cri-containerd-abc.scope/init.scope"),
            ),
            (
                "0::/\n",
                "system.slice/cri-containerd-abc.scope",
                Some("system.slice/cri-containerd-abc.scope"),
            ),
            (
                "0::/kubepods/besteffort/podx/ctry/init\n",
                "/kubepods/besteffort/podx/ctry",
                Some("kubepods/besteffort/podx/ctry/init"),
            ),
            (
                "0::/system.slice/foo.scope\n",
                "system.slice/foo.scope",
                Some("system.slice/foo.scope"),
            ),
            (
                "0::/system.slice/foo.scope/\n",
                "system.slice/foo.scope",
                Some("system.slice/foo.scope"),
            ),
            (
                "0::/a/b.slice/init.scope/payload.service\n",
                "/a/b.slice",
                Some("a/b.slice/init.scope/payload.service"),
            ),
            (
                "12:memory:/\n11:cpu,cpuacct:/\n",
                "system.slice/foo.scope",
                None,
            ),
            ("", "system.slice/foo.scope", None),
            (
                "12:memory:/\n0::/a/b/init.scope\n",
                "/a/b",
                Some("a/b/init.scope"),
            ),
        ];

        for (contents, container_cgroup, expected) in test_cases {
            assert_eq!(
                cgroup_from_cgroup_file(contents, normalize_cgroup_path(container_cgroup))
                    .as_deref(),
                expected,
                "contents={:?} container={}",
                contents,
                container_cgroup
            );
        }
    }

    #[test]
    fn test_live_cgroup_v2_init_scope_lookup() {
        use std::process::{Command, Stdio};

        skip_if_not_root!();
        if !cgroups::hierarchies::is_cgroup2_unified_mode() {
            println!("INFO: skipping live cgroup test; not cgroup v2");
            return;
        }

        let root = cgroups::hierarchies::auto().root();
        let name = format!(
            "kata-nest-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        let parent = root.join(&name);
        if let Err(err) = fs::create_dir(&parent) {
            println!(
                "INFO: skipping live cgroup test; cannot create {:?}: {}",
                parent, err
            );
            return;
        }

        let init_scope = parent.join("init.scope");
        fs::create_dir(&init_scope).unwrap();

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

        let looked_up = exec_cgroup(i32::MAX, child_pid, &name);
        assert_eq!(looked_up, format!("{name}/init.scope"));

        if fs::write(parent.join("cgroup.subtree_control"), "+pids").is_ok() {
            let mut extra = Command::new("sleep")
                .arg("5")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let extra_pid = extra.id() as i32;
            let err = fs::write(parent.join("cgroup.procs"), extra_pid.to_string()).unwrap_err();
            assert_eq!(err.raw_os_error(), Some(libc::EBUSY));
            fs::write(init_scope.join("cgroup.procs"), extra_pid.to_string()).unwrap();
            extra.kill().ok();
            let _ = extra.wait();
        }
    }

    #[test]
    fn test_live_cgroup_v2_init_moved_outside() {
        use std::process::{Command, Stdio};

        skip_if_not_root!();
        if !cgroups::hierarchies::is_cgroup2_unified_mode() {
            println!("INFO: skipping live cgroup test; not cgroup v2");
            return;
        }

        let root = cgroups::hierarchies::auto().root();
        let stamp = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        let container = root.join(format!("kata-nest-test-{stamp}"));
        let outside = root.join(format!("kata-nest-out-{stamp}"));
        if let Err(err) = fs::create_dir(&container) {
            println!(
                "INFO: skipping live cgroup test; cannot create {:?}: {}",
                container, err
            );
            return;
        }
        if let Err(err) = fs::create_dir(&outside) {
            let _ = fs::remove_dir(&container);
            println!(
                "INFO: skipping live cgroup test; cannot create {:?}: {}",
                outside, err
            );
            return;
        }

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

        if let Err(err) = fs::write(outside.join("cgroup.procs"), child_pid.to_string()) {
            println!(
                "INFO: skipping live cgroup test; cannot move pid into {:?}: {}",
                outside, err
            );
            return;
        }

        let looked_up = exec_cgroup(
            i32::MAX,
            child_pid,
            container.file_name().unwrap().to_str().unwrap(),
        );
        assert_eq!(looked_up, outside.file_name().unwrap().to_str().unwrap());
    }
}
