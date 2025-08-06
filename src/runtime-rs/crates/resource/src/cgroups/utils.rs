// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};

// When the Kata overhead threads (I/O, VMM, etc) are not
// placed in the sandbox resource controller (A cgroup on Linux),
// they are moved to a specific, unconstrained resource controller.
// On Linux, assuming the cgroup mount point is at /sys/fs/cgroup/,
// on a cgroup v1 system, the Kata overhead memory cgroup will be at
// /sys/fs/cgroup/memory/kata_overhead/$CGPATH where $CGPATH is
// defined by the orchestrator.
pub(crate) fn gen_overhead_path(systemd: bool, path: &str) -> String {
    if systemd {
        format!("kata-overhead.slice:runtime-rs:{}", path)
    } else {
        format!("kata_overhead/{}", path.trim_start_matches('/'))
    }
}

/// Get the thread group ID (TGID) from `/proc/{pid}/status`.
pub(crate) fn get_tgid_from_pid(pid: i32) -> Result<i32> {
    let status = std::fs::read_to_string(format!("/proc/{}/status", pid))
        .map_err(|e| anyhow!("failed to read /proc/{}/status: {}", pid, e))?;
    status
        .lines()
        .find_map(|line| {
            if line.starts_with("Tgid") {
                let part = line.split(":").nth(1)?;
                part.trim().parse::<i32>().ok()
            } else {
                None
            }
        })
        .ok_or(anyhow!("tgid not found"))
        .with_context(|| anyhow!("failed to parse tgid"))
}

#[cfg(test)]
mod tests {
    use crate::cgroups::utils::*;

    #[test]
    fn test_gen_overhead_path() {
        let systemd = true;
        let path = "kata_sandboxed_pod";
        let expected = "kata-overhead.slice:runtime-rs:kata_sandboxed_pod";
        let actual = gen_overhead_path(systemd, path);
        assert_eq!(actual, expected);

        let systemd = false;
        let expected = "kata_overhead/kata_sandboxed_pod";
        let actual = gen_overhead_path(systemd, path);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_get_tgid_from_pid() {
        let pid = unsafe { libc::gettid() };
        let expected = unsafe { libc::getpid() };
        let actual = get_tgid_from_pid(pid).unwrap();
        assert_eq!(actual, expected);
    }
}
