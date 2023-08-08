// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::Result;
use kata_sys_util::mount;
use nix::mount::MsFlags;

use super::*;

pub(crate) fn mkdir_with_permissions(path_target: PathBuf, mode: u32) -> Result<()> {
    let new_path = &path_target;
    std::fs::create_dir_all(new_path)
        .context(format!("unable to create new path: {:?}", new_path))?;

    // mode format: 0o750, ...
    std::fs::set_permissions(new_path, std::fs::Permissions::from_mode(mode))?;

    Ok(())
}

pub(crate) fn ensure_dir_exist(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path).context(format!("failed to create directory {:?}", path))?;
    }
    Ok(())
}

/// Bind mount the original path to the runtime directory.
pub(crate) fn share_to_guest(
    // absolute path for source
    source: &str,
    // relative path for target
    target: &str,
    sid: &str,
    cid: &str,
    readonly: bool,
    is_volume: bool,
    is_rafs: bool,
) -> Result<String> {
    let host_dest = do_get_host_path(target, sid, cid, is_volume, false);
    mount::bind_mount_unchecked(source, &host_dest, readonly, MsFlags::MS_SLAVE)
        .with_context(|| format!("failed to bind mount {} to {}", source, &host_dest))?;

    // bind mount remount event is not propagated to mount subtrees, so we have
    // to remount the read only dir mount point directly.
    if readonly {
        let dst = do_get_host_path(target, sid, cid, is_volume, true);
        mount::bind_remount(dst, readonly).context("bind remount readonly")?;
    }

    Ok(do_get_guest_path(target, cid, is_volume, is_rafs))
}
// Shared path handling:
// 1. create two directories for each sandbox:
// -. /run/kata-containers/shared/sandboxes/$sbx_id/rw/, a host/guest shared directory which is rw
// -. /run/kata-containers/shared/sandboxes/$sbx_id/ro/, a host/guest shared directory (virtiofs source dir) which is ro
//
// 2. /run/kata-containers/shared/sandboxes/$sbx_id/rw/ is bind mounted readonly to /run/kata-containers/shared/sandboxes/$sbx_id/ro/, so guest cannot modify it
//
// 3. host-guest shared files/directories are mounted one-level under /run/kata-containers/shared/sandboxes/$sbx_id/rw/passthrough and thus present to guest at one level under run/kata-containers/shared/containers/passthrough.
pub(crate) fn get_host_ro_shared_path(id: &str) -> PathBuf {
    Path::new(KATA_HOST_SHARED_DIR).join(id).join("ro")
}

pub fn get_host_rw_shared_path(sid: &str) -> PathBuf {
    Path::new(KATA_HOST_SHARED_DIR).join(sid).join("rw")
}

pub fn get_host_shared_path(sid: &str) -> PathBuf {
    Path::new(KATA_HOST_SHARED_DIR).join(sid)
}

fn do_get_guest_any_path(
    target: &str,
    cid: &str,
    is_volume: bool,
    is_rafs: bool,
    is_virtiofs: bool,
) -> String {
    let dir = if is_rafs {
        RAFS_DIR
    } else {
        PASSTHROUGH_FS_DIR
    };
    let guest_share_dir = if is_virtiofs {
        Path::new("/").to_path_buf()
    } else {
        Path::new(KATA_GUEST_SHARE_DIR).to_path_buf()
    };

    let path = if is_volume && !is_virtiofs {
        guest_share_dir.join(dir).join(target)
    } else {
        guest_share_dir.join(dir).join(cid).join(target)
    };
    path.to_str().unwrap().to_string()
}

pub fn do_get_guest_path(target: &str, cid: &str, is_volume: bool, is_rafs: bool) -> String {
    do_get_guest_any_path(target, cid, is_volume, is_rafs, false)
}

pub fn do_get_guest_share_path(target: &str, cid: &str, is_rafs: bool) -> String {
    do_get_guest_any_path(target, cid, false, is_rafs, true)
}

pub fn do_get_host_path(
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
