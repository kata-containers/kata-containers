// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! Utilities to support Kubernetes (K8s).
//!
//! This module depends on kubelet internal implementation details, a better way is needed
//! to detect K8S EmptyDir medium type from `oci::spec::Mount` objects.

use std::fs;

use kata_types::mount;
use oci_spec::runtime::{Mount, Spec};

use crate::mount::{get_linux_mount_info, LinuxMountInfo};

pub use kata_types::k8s::is_empty_dir;

fn get_mount_info(path: &str) -> crate::mount::Result<LinuxMountInfo> {
    // Kubelet paths can be symlinks or relocated by bind mounts. Resolve the
    // path first because /proc/mounts reports the mounted target path.
    if let Ok(real_path) = fs::canonicalize(path) {
        if let Some(real_path) = real_path.to_str() {
            if real_path != path {
                match get_linux_mount_info(real_path) {
                    Ok(info) => return Ok(info),
                    Err(crate::mount::Error::NoMountEntry(_)) => {}
                    Err(err) => return Err(err),
                }
            }
        }
    }

    get_linux_mount_info(path)
}

/// Check whether a given volume is an ephemeral volume.
///
/// For k8s, there are generally two types of ephemeral volumes: one is the
/// volume used as /dev/shm of the container, and the other is the
/// emptydir volume based on the memory type. Both types of volumes
/// are based on tmpfs mount volumes, so we classify them as ephemeral
/// volumes and can be setup in the guest; For the other volume based on tmpfs
/// which would contain some initial files we cound't deal them as ephemeral and
/// should be passed using share fs.
pub fn is_ephemeral_volume(mount: &Mount) -> bool {
    matches!(
        (
            mount.typ().as_deref(),
            mount.source().as_deref().and_then(|s| s.to_str()),
            mount.destination(),

        ),
        (Some("bind"), Some(source), _dest) if get_mount_info(source).is_ok_and(|info| info.fs_type == "tmpfs") &&
            is_empty_dir(source))
}

/// Check whether the given path is a kubernetes empty-dir volume of medium "default".
///
/// K8s `EmptyDir` volumes are directories on the host. If the fs type is tmpfs, it's a ephemeral
/// volume instead of a `EmptyDir` volume.
pub fn is_host_empty_dir(path: &str) -> bool {
    if !is_empty_dir(path) {
        return false;
    }

    match get_mount_info(path) {
        Ok(info) => info.fs_type != "tmpfs",
        Err(crate::mount::Error::NoMountEntry(_)) => true,
        Err(_) => false,
    }
}

// update_ephemeral_storage_type sets the mount type to 'ephemeral'
// if the mount source path is provisioned by k8s for ephemeral storage.
// For the given pod ephemeral volume is created only once
// backed by tmpfs inside the VM. For successive containers
// of the same pod the already existing volume is reused.
pub fn update_ephemeral_storage_type(oci_spec: &mut Spec, disable_guest_empty_dir: bool) {
    if let Some(mounts) = oci_spec.mounts_mut() {
        for m in mounts.iter_mut() {
            if let Some(typ) = &m.typ() {
                if mount::is_kata_guest_mount_volume(typ) {
                    continue;
                }
            }

            if let Some(source) = &m.source() {
                let mnt_src = &source.display().to_string();
                // We only care about the "bind" mount volume here.
                if is_ephemeral_volume(m) {
                    m.set_typ(Some(String::from(mount::KATA_EPHEMERAL_VOLUME_TYPE)));
                }
                if is_host_empty_dir(mnt_src) && !disable_guest_empty_dir {
                    m.set_typ(Some(mount::KATA_K8S_LOCAL_STORAGE_TYPE.to_string()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    use super::*;

    #[cfg(unix)]
    #[test]
    fn detects_ephemeral_empty_dir_from_resolved_mount_source() {
        if !matches!(get_linux_mount_info("/dev/shm"), Ok(info) if info.fs_type == "tmpfs") {
            return;
        }

        let dir = tempdir().unwrap();
        let empty_dir_parent = dir.path().join("kubernetes.io~empty-dir");
        fs::create_dir(&empty_dir_parent).unwrap();

        let volume = empty_dir_parent.join("memory-empty-vol");
        std::os::unix::fs::symlink("/dev/shm", &volume).unwrap();

        let mut mount = Mount::default();
        mount.set_typ(Some("bind".to_string()));
        mount.set_source(Some(volume.clone()));
        mount.set_destination(PathBuf::from("/tmp/cache"));

        assert!(is_ephemeral_volume(&mount));
        assert!(!is_host_empty_dir(volume.to_str().unwrap()));
    }
}
