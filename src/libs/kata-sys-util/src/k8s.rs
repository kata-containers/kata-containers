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
use oci::Spec;

use crate::mount::get_linux_mount_info;

pub use kata_types::k8s::is_empty_dir;

/// Check whether the given path is a kubernetes ephemeral volume.
///
/// This method depends on a specific path used by k8s to detect if it's type of ephemeral.
/// As of now, this is a very k8s specific solution that works but in future there should be a
/// better way for this method to determine if the path is for ephemeral volume type.
pub fn is_ephemeral_volume(path: &str) -> bool {
    if is_empty_dir(path) {
        if let Ok(info) = get_linux_mount_info(path) {
            if info.fs_type == "tmpfs" {
                return true;
            }
        }
    }

    false
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

// set_ephemeral_storage_type sets the mount type to 'ephemeral'
// if the mount source path is provisioned by k8s for ephemeral storage.
// For the given pod ephemeral volume is created only once
// backed by tmpfs inside the VM. For successive containers
// of the same pod the already existing volume is reused.
pub fn update_ephemeral_storage_type(oci_spec: &mut Spec) {
    for m in oci_spec.mounts.iter_mut() {
        if mount::is_kata_guest_mount_volume(&m.r#type) {
            continue;
        }

        if is_ephemeral_volume(&m.source) {
            m.r#type = String::from(mount::KATA_EPHEMERAL_VOLUME_TYPE);
        } else if is_host_empty_dir(&m.source) {
            m.r#type = String::from(mount::KATA_HOST_DIR_VOLUME_TYPE);
        }
    }
}
