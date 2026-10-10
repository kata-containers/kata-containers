// Copyright (c) 2022-2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::os::unix::fs::{chown, MetadataExt, PermissionsExt};
use std::path::Path;

use anyhow::{Context, Ok, Result};
use kata_types::prefix_with_rootless_dir;

use crate::{utils::get_sandbox_path, JAILER_ROOT};

// The socket used to connect to CH. This is used for CH API communications.
const CH_API_SOCKET_NAME: &str = "ch-api.sock";

// The socket that allows runtime-rs to connect direct through to the Kata
// Containers agent running inside the CH hosted VM.
const CH_VM_SOCKET_NAME: &str = "ch-vm.sock";

// Return the path for a _hypothetical_ API socket path:
// the path does *not* exist yet, and for this reason safe-path cannot be
// used.
pub fn get_api_socket_path(id: &str) -> Result<String> {
    let sandbox_path = get_sandbox_path(id);

    let path = [&sandbox_path, CH_API_SOCKET_NAME].join("/");

    Ok(path)
}

// Return the path for a _hypothetical_ sandbox specific VSOCK socket path:
// the path does *not* exist yet, and for this reason safe-path cannot be
// used.
pub fn get_vsock_path(id: &str) -> Result<String> {
    let sandbox_path = get_sandbox_path(id);

    let path = [&sandbox_path, CH_VM_SOCKET_NAME].join("/");

    Ok(path)
}

/// Returns the symlink path of the sandbox for the virtio-fs socket in rootless mode.
pub fn get_rootless_symlink_sandbox_path(id: &str) -> String {
    Path::new(prefix_with_rootless_dir(id).as_str())
        .to_string_lossy()
        .to_string()
}

/// Returns the symlink path of the sandbox's jailer root for the virtio-fs socket in rootless mode.
pub fn get_rootless_symlink_sandbox_jailer_root(id: &str) -> String {
    let sandbox_path = get_rootless_symlink_sandbox_path(id);

    [&sandbox_path, JAILER_ROOT].join("/")
}

/// Owner and permission bits a disk had before it was handed to a rootless VMM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiskOwner {
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
}

/// Make `path` readable and writable by the VMM user alone.
///
/// Every rootless VMM user shares the /dev/kvm group, so a disk the host
/// opened to that group is reachable from every VMM on the node. Returns the
/// previous owner and mode, which `restore_disk_owner` puts back.
pub fn give_disk_to_vmm_user(path: &str, uid: u32, gid: u32) -> Result<DiskOwner> {
    let meta = fs::metadata(path).with_context(|| format!("stat disk {path}"))?;
    let previous = DiskOwner {
        uid: meta.uid(),
        gid: meta.gid(),
        mode: meta.mode() & 0o7777,
    };

    chown(path, Some(uid), Some(gid)).with_context(|| format!("chown disk {path}"))?;
    if let Err(e) = fs::set_permissions(path, fs::Permissions::from_mode(0o600)) {
        // No caller records a disk this returns an error for, so undo the
        // chown here or the disk stays with the VMM user.
        let rollback = restore_disk_owner(path, &previous);
        return Err(
            anyhow::Error::new(e).context(format!("chmod disk {path} (rollback: {rollback:?})"))
        );
    }

    Ok(previous)
}

/// Give `path` back the owner and mode `give_disk_to_vmm_user` replaced.
///
/// This must run before the VMM user is deleted: the next VMM user can be
/// created with the same uid and would otherwise inherit the disk.
pub fn restore_disk_owner(path: &str, owner: &DiskOwner) -> Result<()> {
    chown(path, Some(owner.uid), Some(owner.gid))
        .with_context(|| format!("restore owner of disk {path}"))?;
    fs::set_permissions(path, fs::Permissions::from_mode(owner.mode))
        .with_context(|| format!("restore mode of disk {path}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::unistd::{getgid, getuid};

    fn disk_with_mode(mode: u32) -> tempfile::NamedTempFile {
        let disk = tempfile::NamedTempFile::new().unwrap();
        fs::set_permissions(disk.path(), fs::Permissions::from_mode(mode)).unwrap();
        disk
    }

    fn owner_of(path: &Path) -> DiskOwner {
        let meta = fs::metadata(path).unwrap();
        DiskOwner {
            uid: meta.uid(),
            gid: meta.gid(),
            mode: meta.mode() & 0o7777,
        }
    }

    #[test]
    fn test_give_disk_to_vmm_user_drops_group_access() {
        let disk = disk_with_mode(0o660);
        let path = disk.path().to_str().unwrap();
        let (uid, gid) = (getuid().as_raw(), getgid().as_raw());

        let previous = give_disk_to_vmm_user(path, uid, gid).unwrap();

        assert_eq!(
            previous,
            DiskOwner {
                uid,
                gid,
                mode: 0o660
            }
        );
        assert_eq!(
            owner_of(disk.path()),
            DiskOwner {
                uid,
                gid,
                mode: 0o600
            }
        );
    }

    #[test]
    fn test_restore_disk_owner_puts_back_previous_mode() {
        let disk = disk_with_mode(0o640);
        let path = disk.path().to_str().unwrap();
        let (uid, gid) = (getuid().as_raw(), getgid().as_raw());

        let previous = give_disk_to_vmm_user(path, uid, gid).unwrap();
        restore_disk_owner(path, &previous).unwrap();

        assert_eq!(
            owner_of(disk.path()),
            DiskOwner {
                uid,
                gid,
                mode: 0o640
            }
        );
    }

    #[test]
    fn test_give_disk_to_vmm_user_missing_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing");

        assert!(give_disk_to_vmm_user(path.to_str().unwrap(), 0, 0).is_err());
    }
}
