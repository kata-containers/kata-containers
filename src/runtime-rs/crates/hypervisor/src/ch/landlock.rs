// Copyright (c) 2026 The Kata Containers Authors
//
// SPDX-License-Identifier: Apache-2.0

//! Disk staging for a Landlock-confined Cloud Hypervisor.
//!
//! Cloud Hypervisor fixes its Landlock rules at vm.create, and a disk path
//! that no rule covers then fails to open with EACCES. The rules name two
//! directories under the sandbox path, and every disk this VM is given is
//! bind-mounted onto a file below one of them before the VMM opens it.

use std::fs;
use std::io;
use std::os::unix::fs::{chown, PermissionsExt};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use ch_config::landlock_stage_dir;
use kata_sys_util::mount;
use nix::mount::{umount2, MntFlags, MsFlags};
use nix::unistd::{Gid, Uid};

use crate::utils::create_dir_all_with_inherit_owner;

/// The path a disk with this device id is staged at.
pub(crate) fn staged_disk_path(
    sandbox_path: &str,
    device_id: &str,
    readonly: bool,
) -> Result<PathBuf> {
    if device_id.is_empty()
        || !device_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(anyhow!(
            "cannot stage a disk under device id {:?}",
            device_id
        ));
    }
    Ok(landlock_stage_dir(sandbox_path, readonly).join(device_id))
}

/// Search permission only: the VMM can reach a staged disk but cannot list or
/// change the directory.
const STAGE_DIR_MODE: u32 = 0o711;

/// Create both staging directories, owned by the shim and not writable by the
/// VMM. They must exist before the first disk is staged and when vm.create
/// runs: Cloud Hypervisor drops a rule whose path is missing, without an error.
/// The VMM owns nothing in them, so it cannot plant a symlink that the bind
/// mount in `stage_disk` would follow.
pub(crate) fn create_stage_dirs(sandbox_path: &str) -> Result<()> {
    for readonly in [true, false] {
        let dir = landlock_stage_dir(sandbox_path, readonly);
        create_dir_all_with_inherit_owner(&dir, STAGE_DIR_MODE)
            .with_context(|| format!("failed to create disk staging directory {:?}", dir))?;
        chown(
            &dir,
            Some(Uid::current().as_raw()),
            Some(Gid::current().as_raw()),
        )
        .with_context(|| format!("failed to take ownership of {:?}", dir))?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(STAGE_DIR_MODE))
            .with_context(|| format!("failed to set the mode of {:?}", dir))?;
    }
    Ok(())
}

/// Bind-mount the disk at `source` below the staging directory and return
/// the staged path, which is the path the VMM is given.
pub(crate) fn stage_disk(
    sandbox_path: &str,
    device_id: &str,
    source: &str,
    readonly: bool,
) -> Result<PathBuf> {
    let staged = staged_disk_path(sandbox_path, device_id, readonly)?;
    mount::bind_mount_unchecked(source, &staged, readonly, MsFlags::MS_SLAVE)
        .with_context(|| format!("failed to stage disk {} at {:?}", source, staged))?;
    Ok(staged)
}

/// Unmount and remove a staged disk. A path that is not staged is not an
/// error.
pub(crate) fn unstage_disk(staged: &Path) -> Result<()> {
    match umount2(staged, MntFlags::MNT_DETACH) {
        Ok(()) | Err(nix::Error::EINVAL) | Err(nix::Error::ENOENT) => {}
        Err(e) => return Err(anyhow!("failed to unmount staged disk {:?}: {}", staged, e)),
    }
    match fs::remove_file(staged) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow!("failed to remove staged disk {:?}: {}", staged, e)),
    }
}

/// Unstage the disk with this device id, whichever directory holds it.
pub(crate) fn unstage_device(sandbox_path: &str, device_id: &str) -> Result<()> {
    for readonly in [true, false] {
        unstage_disk(&staged_disk_path(sandbox_path, device_id, readonly)?)?;
    }
    Ok(())
}

/// Unstage every disk left in either staging directory. The sandbox cleanup
/// calls this before it removes the sandbox path, which a mount would block.
/// Returns each path it could not unstage.
pub(crate) fn unstage_all(sandbox_path: &str) -> Vec<String> {
    let mut failed = Vec::new();
    for readonly in [true, false] {
        let entries = match fs::read_dir(landlock_stage_dir(sandbox_path, readonly)) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => {
                error!(sl!(), "failed to list disk staging directory: {:?}", e);
                failed.push(
                    landlock_stage_dir(sandbox_path, readonly)
                        .display()
                        .to_string(),
                );
                continue;
            }
        };
        for entry in entries.flatten() {
            if let Err(e) = unstage_disk(&entry.path()) {
                error!(sl!(), "{:?}", e);
                failed.push(entry.path().display().to_string());
            }
        }
    }
    failed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_stage_dirs_leaves_the_directories_closed_to_other_users() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempfile::tempdir().unwrap();
        let sandbox = dir.path().to_str().unwrap();
        create_stage_dirs(sandbox).unwrap();
        for readonly in [true, false] {
            let meta = fs::metadata(landlock_stage_dir(sandbox, readonly)).unwrap();
            assert_eq!(meta.mode() & 0o7777, STAGE_DIR_MODE);
            assert_eq!(meta.uid(), Uid::current().as_raw());
            assert_eq!(meta.gid(), Gid::current().as_raw());
        }
    }

    #[test]
    fn test_staged_disk_path_picks_the_directory_by_access() {
        let ro = staged_disk_path("/run/kata/sb", "dev-1", true).unwrap();
        let rw = staged_disk_path("/run/kata/sb", "dev-1", false).unwrap();
        assert_eq!(ro, PathBuf::from("/run/kata/sb/disks-ro/dev-1"));
        assert_eq!(rw, PathBuf::from("/run/kata/sb/disks-rw/dev-1"));
    }

    #[test]
    fn test_staged_disk_path_refuses_an_id_that_leaves_the_directory() {
        for id in ["", "..", "../x", "a/b", "a b"] {
            assert!(
                staged_disk_path("/run/kata/sb", id, false).is_err(),
                "{:?}",
                id
            );
        }
    }

    #[test]
    fn test_unstage_disk_accepts_a_path_that_is_not_staged() {
        let dir = tempfile::tempdir().unwrap();
        unstage_disk(&dir.path().join("absent")).unwrap();
    }
}
