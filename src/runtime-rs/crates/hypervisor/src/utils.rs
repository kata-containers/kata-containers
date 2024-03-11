// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashSet, os::fd::RawFd};

use anyhow::Result;
use kata_types::config::KATA_PATH;
use nix::fcntl;

use crate::{DEFAULT_HYBRID_VSOCK_NAME, JAILER_ROOT};

pub fn get_child_threads(pid: u32) -> HashSet<u32> {
    let mut result = HashSet::new();
    let path_name = format!("/proc/{}/task", pid);
    let path = std::path::Path::new(path_name.as_str());
    if path.is_dir() {
        if let Ok(dir) = path.read_dir() {
            for entity in dir {
                if let Ok(entity) = entity.as_ref() {
                    let file_name = entity.file_name();
                    let file_name = file_name.to_str().unwrap_or_default();
                    if let Ok(tid) = file_name.parse::<u32>() {
                        result.insert(tid);
                    }
                }
            }
        }
    }
    result
}

// Return the path for a _hypothetical_ sandbox: the path does *not* exist
// yet, and for this reason safe-path cannot be used.
pub fn get_sandbox_path(sid: &str) -> String {
    [KATA_PATH, sid].join("/")
}

pub fn get_hvsock_path(sid: &str) -> String {
    let jailer_root_path = get_jailer_root(sid);

    [jailer_root_path, DEFAULT_HYBRID_VSOCK_NAME.to_owned()].join("/")
}

pub fn get_jailer_root(sid: &str) -> String {
    let sandbox_path = get_sandbox_path(sid);

    [&sandbox_path, JAILER_ROOT].join("/")
}

// Clear the O_CLOEXEC which is set by default by Rust standard library
// as it would obviously prevent passing the descriptor to the hypervisor process.
pub fn clear_fd_flags(rawfd: RawFd) -> Result<()> {
    if let Err(err) = fcntl::fcntl(rawfd, fcntl::FcntlArg::F_SETFD(fcntl::FdFlag::empty())) {
        info!(
            sl!(),
            "couldn't clear O_CLOEXEC on device's fd, communication with agent will not work: {:?}",
            err
        );
        return Err(err.into());
    }

    Ok(())
}
