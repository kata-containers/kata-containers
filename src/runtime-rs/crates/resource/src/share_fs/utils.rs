// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};

use anyhow::Result;
use kata_sys_util::mount;

use super::*;

pub(crate) fn ensure_dir_exist(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path).context(format!("failed to create directory {:?}", path))?;
    }
    Ok(())
}

pub(crate) fn share_to_guest(
    // absolute path for source
    source: &str,
    // relative path for target
    target: &str,
    sid: &str,
    cid: &str,
    readonly: bool,
    is_volume: bool,
) -> Result<String> {
    let host_dest = do_get_host_path(target, sid, cid, is_volume, false);
    mount::bind_mount_unchecked(source, &host_dest, readonly)
        .with_context(|| format!("failed to bind mount {} to {}", source, &host_dest))?;

    // bind mount remount event is not propagated to mount subtrees, so we have
    // to remount the read only dir mount point directly.
    if readonly {
        let dst = do_get_host_path(target, sid, cid, is_volume, true);
        mount::bind_remount_read_only(&dst).context("bind remount readonly")?;
    }

    Ok(do_get_guest_path(target, cid, is_volume))
}

pub(crate) fn get_host_ro_shared_path(id: &str) -> PathBuf {
    Path::new(KATA_HOST_SHARED_DIR).join(id).join("ro")
}

pub(crate) fn get_host_rw_shared_path(id: &str) -> PathBuf {
    Path::new(KATA_HOST_SHARED_DIR).join(id).join("rw")
}

fn do_get_guest_any_path(target: &str, cid: &str, is_volume: bool, is_virtiofs: bool) -> String {
    let dir = PASSTHROUGH_FS_DIR;
    let guest_share_dir = if is_virtiofs {
        Path::new("/")
    } else {
        Path::new(KATA_GUEST_SHARE_DIR)
    };

    let path = if is_volume && !is_virtiofs {
        guest_share_dir.join(dir).join(target)
    } else {
        guest_share_dir.join(dir).join(cid).join(target)
    };
    path.to_str().unwrap().to_string()
}

fn do_get_guest_path(target: &str, cid: &str, is_volume: bool) -> String {
    do_get_guest_any_path(target, cid, is_volume, false)
}

fn do_get_host_path(
    target: &str,
    sid: &str,
    cid: &str,
    is_volume: bool,
    read_only: bool,
) -> String {
    let dir = PASSTHROUGH_FS_DIR;

    let get_host_path = if read_only {
        get_host_ro_shared_path
    } else {
        get_host_rw_shared_path
    };

    let path = if is_volume {
        get_host_path(sid).join(dir).join(target)
    } else {
        get_host_path(sid).join(dir).join(cid).join(target)
    };
    path.to_str().unwrap().to_string()
}
