// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    collections::HashSet,
    fs::File,
    os::fd::{AsRawFd, RawFd},
};

use anyhow::{anyhow, Context, Result};
use dbs_utils::net::Tap;
use kata_types::config::KATA_PATH;
use nix::{
    fcntl,
    sched::{setns, CloneFlags},
};

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

// Clear the O_CLOEXEC which is set by default by Rust standard library on
// file descriptors that it opens.  This function is mostly meant to be
// called on descriptors to be passed to a child (hypervisor) process as
// O_CLOEXEC would obviously prevent that.
pub fn clear_cloexec(rawfd: RawFd) -> Result<()> {
    let cur_flags = fcntl::fcntl(rawfd, fcntl::FcntlArg::F_GETFD)?;
    let mut new_flags = fcntl::FdFlag::from_bits(cur_flags).ok_or(anyhow!(
        "couldn't construct FdFlag from flags value {:?}",
        cur_flags
    ))?;
    new_flags.remove(fcntl::FdFlag::FD_CLOEXEC);
    if let Err(err) = fcntl::fcntl(rawfd, fcntl::FcntlArg::F_SETFD(new_flags)) {
        info!(sl!(), "couldn't clear O_CLOEXEC on fd: {:?}", err);
        return Err(err.into());
    }

    Ok(())
}

pub fn enter_netns(netns_path: &str) -> Result<()> {
    if !netns_path.is_empty() {
        let netns =
            File::open(netns_path).context(anyhow!("open netns path {:?} failed.", netns_path))?;
        setns(netns.as_raw_fd(), CloneFlags::CLONE_NEWNET).context("set netns failed")?;
    }

    Ok(())
}

pub fn open_named_tuntap(if_name: &str, queues: u32) -> Result<Vec<File>> {
    let (multi_vq, vq_pairs) = if queues > 1 {
        (true, queues as usize)
    } else {
        (false, 1_usize)
    };

    let tap: Tap = Tap::open_named(if_name, multi_vq).context("open named tuntap device failed")?;
    let taps: Vec<Tap> = tap.into_mq_taps(vq_pairs).context("into mq taps failed.")?;

    let mut tap_files: Vec<std::fs::File> = Vec::new();
    for tap in taps {
        tap_files.push(tap.tap_file);
    }

    Ok(tap_files)
}
