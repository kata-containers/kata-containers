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
use std::path::Path;

use crate::mount::get_linux_mount_info;

pub use kata_types::k8s::is_empty_dir;

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
        (Some("bind"), Some(source), dest) if get_linux_mount_info(source)
            .map_or(false, |info| info.fs_type == "tmpfs") &&
            (is_empty_dir(source) || dest.as_path() == Path::new("/dev/shm"))
    )
}

/// Check whether the given path is a kubernetes empty-dir volume of medium "default".
///
/// K8s `EmptyDir` volumes are directories on the host. If the fs type is tmpfs, it's a ephemeral
/// volume instead of a `EmptyDir` volume.
pub fn is_host_empty_dir(path: &str) -> bool {
    if is_empty_dir(path) {
        if let Ok(info) = get_linux_mount_info(path) {
            if info.fs_type != "tmpfs" {
                return true;
            }
        }
    }

    false
}

// update_ephemeral_storage_type sets the mount type to 'ephemeral'
// if the mount source path is provisioned by k8s for ephemeral storage.
// For the given pod ephemeral volume is created only once
// backed by tmpfs inside the VM. For successive containers
// of the same pod the already existing volume is reused.
pub fn update_ephemeral_storage_type(oci_spec: &mut Spec) {
    if let Some(mounts) = oci_spec.mounts_mut() {
        for m in mounts.iter_mut() {
            if let Some(typ) = &m.typ() {
                if mount::is_kata_guest_mount_volume(typ) {
                    continue;
                }
            }

            if let Some(source) = &m.source() {
                let mnt_src = &source.display().to_string();
                //here we only care about the "bind" mount volume.
                if is_ephemeral_volume(m) {
                    m.set_typ(Some(String::from(mount::KATA_EPHEMERAL_VOLUME_TYPE)));
                } else if is_host_empty_dir(mnt_src) {
                    // FIXME support disable_guest_empty_dir
                    // https://github.com/kata-containers/kata-containers/blob/02a51e75a7e0c6fce5e8abe3b991eeac87e09645/src/runtime/pkg/katautils/create.go#L105
                    m.set_typ(Some(String::from(mount::KATA_HOST_DIR_VOLUME_TYPE)));
                }
            }
        }
    }
}
