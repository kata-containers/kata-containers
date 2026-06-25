// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! Utilities to support Kubernetes (K8s).
//!
//! This module depends on kubelet internal implementation details, a better way is needed
//! to detect K8S EmptyDir medium type from `oci::spec::Mount` objects.

use kata_types::mount;
use oci_spec::runtime::{Mount, Spec};

use crate::mount::get_linux_mount_info;

pub use kata_types::k8s::is_empty_dir;

/// Returns true for tmpfs-backed emptyDirs (medium: Memory).
pub fn is_tmpfs_empty_dir(mount: &Mount) -> bool {
    matches!(
        (
            mount.typ().as_deref(),
            mount.source().as_deref().and_then(|s| s.to_str()),
            mount.destination(),

        ),
        (
            Some("bind"),
            Some(source),
            _dest,
        )
        if is_empty_dir(source) && get_linux_mount_info(source).is_ok_and(|info| info.fs_type == "tmpfs")
    )
}

/// Returns true for non-tmpfs-backed emptyDirs.
/// This includes disk-backed (medium: "", default) and hugepage-backed (medium: HugePages).
pub fn is_non_tmpfs_empty_dir(path: &str) -> bool {
    if !is_empty_dir(path) {
        return false;
    }

    match get_linux_mount_info(path) {
        Ok(info) => info.fs_type != "tmpfs",
        Err(crate::mount::Error::NoMountEntry(_)) => true,
        Err(_) => false,
    }
}

/// Returns true for hugepage-backed emptyDirs (medium: HugePages).
pub fn is_hugepage_empty_dir(path: &str) -> bool {
    is_empty_dir(path) && get_linux_mount_info(path).is_ok_and(|info| info.fs_type == "hugetlbfs")
}

/// Returns true for disk-backed emptyDirs (medium: "", default).
pub fn is_disk_empty_dir(path: &str) -> bool {
    is_non_tmpfs_empty_dir(path) && !is_hugepage_empty_dir(path)
}

// update_ephemeral_storage_type sets the mount type to 'ephemeral'
// if the mount source path is provisioned by k8s for ephemeral storage.
// For the given pod ephemeral volume is created only once
// backed by tmpfs inside the VM. For successive containers
// of the same pod the already existing volume is reused.
pub fn update_ephemeral_storage_type(
    oci_spec: &mut Spec,
    disable_guest_empty_dir: bool,
    emptydir_mode: &str,
) {
    use kata_types::config::EMPTYDIR_MODE_BLOCK_ENCRYPTED;

    if let Some(mounts) = oci_spec.mounts_mut() {
        for m in mounts.iter_mut() {
            if let Some(typ) = &m.typ() {
                if mount::is_kata_guest_mount_volume(typ) {
                    continue;
                }
            }

            if let Some(source) = &m.source() {
                let mnt_src = &source.display().to_string();
                if is_tmpfs_empty_dir(m) {
                    m.set_typ(Some(String::from(mount::KATA_EPHEMERAL_VOLUME_TYPE)));
                } else if is_non_tmpfs_empty_dir(mnt_src) {
                    // Among non-tmpfs emptyDirs:
                    // * For hugepage-backed emptyDirs, do nothing here
                    //   and offload to the later HugePage handler.
                    //   Contrary to runtime-go, adding the LOCAL type
                    //   here would wrongly circumvent the HugePage
                    //   handler.
                    // * For disk-backed emptyDirs, instead of adding
                    //   the LOCAL type here, we'll do this down the
                    //   line:
                    //   - disable_guest_empty_dir=true: FS sharing.
                    //   - emptyDirMode=block-encrypted: Leverage the
                    //     EncryptedEmptyDirVolume handler.
                    if is_hugepage_empty_dir(mnt_src) {
                        // No-op as explained above. Keeping this branch
                        // for now for clarity and easier comparison
                        // with runtime-go.
                    } else if !disable_guest_empty_dir
                        && emptydir_mode != EMPTYDIR_MODE_BLOCK_ENCRYPTED
                    {
                        // This is a disk-backed emptyDir.
                        m.set_typ(Some(String::from(mount::KATA_K8S_LOCAL_STORAGE_TYPE)));
                    }
                }
            }
        }
    }
}
