// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{fs::File, os::unix::io::AsRawFd};

use anyhow::{Context, Result};
use nix::sched::{setns, CloneFlags};
use nix::unistd::{getpid, gettid};

pub(crate) struct NetnsGuard {
    old_netns: Option<File>,
}

impl NetnsGuard {
    pub(crate) fn new(new_netns_path: &str) -> Result<Self> {
        let old_netns = if !new_netns_path.is_empty() {
            let current_netns_path = format!("/proc/{}/task/{}/ns/{}", getpid(), gettid(), "net");
            let old_netns = File::open(&current_netns_path)
                .with_context(|| format!("open current netns path {}", &current_netns_path))?;
            let new_netns = File::open(new_netns_path)
                .with_context(|| format!("open new netns path {}", &new_netns_path))?;
            setns(new_netns.as_raw_fd(), CloneFlags::CLONE_NEWNET)
                .with_context(|| "set netns to new netns")?;
            info!(
                sl!(),
                "set netns from old {:?} to new {:?} tid {}",
                old_netns,
                new_netns,
                gettid().to_string()
            );
            Some(old_netns)
        } else {
            warn!(sl!(), "skip to set netns for empty netns path");
            None
        };
        Ok(Self { old_netns })
    }
}

impl Drop for NetnsGuard {
    fn drop(&mut self) {
        if let Some(old_netns) = self.old_netns.as_ref() {
            let old_netns_fd = old_netns.as_raw_fd();
            setns(old_netns_fd, CloneFlags::CLONE_NEWNET).unwrap();
            info!(sl!(), "set netns to old {:?}", old_netns_fd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_utils::skip_if_not_root;

    #[test]
    fn test_new_netns_guard() {
        // test run under root
        skip_if_not_root!();

        let new_netns_path = "/proc/1/task/1/ns/net"; // systemd, always exists
        let netns_guard = NetnsGuard::new(new_netns_path).unwrap();
        drop(netns_guard);

        let empty_path = "";
        assert!(NetnsGuard::new(empty_path).unwrap().old_netns.is_none());
    }
}
