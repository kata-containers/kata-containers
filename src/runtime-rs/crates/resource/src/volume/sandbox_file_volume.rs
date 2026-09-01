// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::get_mount_path;
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

use super::Volume;

/// Container mount destinations the agent can satisfy from a file it wrote
/// during create_sandbox, paired with where in the guest it wrote it.
///
/// resolv.conf comes from the `dns` field via setup_guest_dns(), and hostname
/// from the `hostname` field via setup_guest_hostname().
const SANDBOX_FILES: &[(&str, &str)] = &[
    (
        "/etc/resolv.conf",
        "/run/kata-containers/sandbox/resolv.conf",
    ),
    ("/etc/hostname", "/run/kata-containers/sandbox/hostname"),
];

fn guest_source(destination: &str) -> Option<&'static str> {
    SANDBOX_FILES
        .iter()
        .find(|(dst, _)| *dst == destination)
        .map(|(_, src)| *src)
}

pub(crate) struct SandboxFileVolume {
    mount: oci::Mount,
}

impl SandboxFileVolume {
    pub fn new(mount: &oci::Mount) -> Result<Self> {
        let destination = get_mount_path(&Some(mount.destination().clone()));
        let source = guest_source(&destination)
            .ok_or_else(|| anyhow!("{destination} is not a sandbox file"))?;

        let mut mount = mount.clone();
        mount.set_source(Some(Path::new(source).to_path_buf()));

        Ok(Self { mount })
    }
}

#[async_trait]
impl Volume for SandboxFileVolume {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(vec![])
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        Ok(())
    }
}

/// The host gives every container in the pod the same sandbox-scoped file, so
/// pointing them all at one guest copy reproduces that.
///
/// `available` is the subset the agent actually wrote, since it only writes
/// what the request gave it.
pub(crate) fn is_sandbox_file_mount(m: &oci::Mount, available: &[String]) -> bool {
    let destination = get_mount_path(&Some(m.destination().clone()));

    guest_source(&destination).is_some() && available.contains(&destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oci_spec::runtime as oci;

    fn mount_at(destination: &str) -> oci::Mount {
        let mut m = oci::Mount::default();
        m.set_destination(Path::new(destination).to_path_buf());
        m.set_source(Some(
            Path::new("/var/lib/containerd/sandboxes/x/whatever").to_path_buf(),
        ));
        m
    }

    fn all() -> Vec<String> {
        SANDBOX_FILES
            .iter()
            .map(|(dst, _)| dst.to_string())
            .collect()
    }

    #[test]
    fn only_matches_known_sandbox_files() {
        assert!(is_sandbox_file_mount(&mount_at("/etc/resolv.conf"), &all()));
        assert!(is_sandbox_file_mount(&mount_at("/etc/hostname"), &all()));

        // Not in the table: needs its own transport before it can be shared.
        assert!(!is_sandbox_file_mount(&mount_at("/etc/hosts"), &all()));
        assert!(!is_sandbox_file_mount(
            &mount_at("/dev/termination-log"),
            &all()
        ));
    }

    #[test]
    fn skips_what_the_agent_did_not_write() {
        let only_dns = vec!["/etc/resolv.conf".to_string()];

        assert!(is_sandbox_file_mount(
            &mount_at("/etc/resolv.conf"),
            &only_dns
        ));
        assert!(!is_sandbox_file_mount(
            &mount_at("/etc/hostname"),
            &only_dns
        ));
        assert!(!is_sandbox_file_mount(&mount_at("/etc/resolv.conf"), &[]));
    }

    #[test]
    fn rewrites_source_to_the_guest_copy() {
        for (destination, expected) in SANDBOX_FILES {
            let volume = SandboxFileVolume::new(&mount_at(destination)).unwrap();
            let mounts = volume.get_volume_mount().unwrap();

            assert_eq!(mounts.len(), 1);
            assert_eq!(
                mounts[0].source().as_ref().unwrap().as_path(),
                Path::new(expected)
            );
            assert_eq!(mounts[0].destination().as_path(), Path::new(destination));
            assert!(volume.get_storage().unwrap().is_empty());
            assert!(volume.get_device_id().unwrap().is_none());
        }
    }

    #[test]
    fn rejects_an_unknown_destination() {
        assert!(SandboxFileVolume::new(&mount_at("/etc/hosts")).is_err());
    }
}
