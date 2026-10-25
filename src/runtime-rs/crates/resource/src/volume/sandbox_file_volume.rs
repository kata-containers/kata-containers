// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::path::Path;
use std::sync::Arc;

use agent::{Agent, SetSandboxHostsRequest};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::get_mount_path;
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

use super::Volume;

/// How the guest copy of a sandbox-scoped file comes to exist.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Origin {
    /// The agent wrote it during create_sandbox, so there is nothing left to
    /// hand over.
    CreateSandbox,
    /// The host only names this one in a container spec, too late for
    /// create_sandbox, so we read it and hand it over ourselves.
    PushedByUs,
}

/// Container mount destinations that one guest file can satisfy for the whole
/// pod, paired with where in the guest that file lives.
///
/// resolv.conf comes from the `dns` field via setup_guest_dns(), hostname from
/// the `hostname` field via setup_guest_hostname(), and hosts from the
/// SetSandboxHosts RPC.
const SANDBOX_FILES: &[(&str, &str, Origin)] = &[
    (
        "/etc/resolv.conf",
        "/run/kata-containers/sandbox/resolv.conf",
        Origin::CreateSandbox,
    ),
    (
        "/etc/hostname",
        "/run/kata-containers/sandbox/hostname",
        Origin::CreateSandbox,
    ),
    (
        "/etc/hosts",
        "/run/kata-containers/sandbox/hosts",
        Origin::PushedByUs,
    ),
];

fn sandbox_file(destination: &str) -> Option<(&'static str, Origin)> {
    SANDBOX_FILES
        .iter()
        .find(|(dst, _, _)| *dst == destination)
        .map(|(_, src, origin)| (*src, *origin))
}

fn read_host_file(path: &str) -> Result<Vec<String>> {
    let content = fs::read_to_string(path).context("read host file")?;

    // The agent rejoins these with a newline, so drop the trailing empty
    // element a final newline would produce rather than growing the file by a
    // blank line every round trip.
    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        return Err(anyhow!("{path} is empty"));
    }

    Ok(lines)
}

pub(crate) struct SandboxFileVolume {
    mount: oci::Mount,
}

impl SandboxFileVolume {
    pub async fn new(mount: &oci::Mount, agent: Arc<dyn Agent>) -> Result<Self> {
        let destination = get_mount_path(&Some(mount.destination().clone()));
        let (guest_source, origin) =
            sandbox_file(&destination).ok_or_else(|| anyhow!("{destination} is not shareable"))?;

        if origin == Origin::PushedByUs {
            let host_source = get_mount_path(mount.source());
            let hosts = read_host_file(&host_source)
                .with_context(|| format!("read {destination} from {host_source}"))?;

            agent
                .set_sandbox_hosts(SetSandboxHostsRequest { hosts })
                .await
                .context("set sandbox hosts")?;
        }

        let mut mount = mount.clone();
        mount.set_source(Some(Path::new(guest_source).to_path_buf()));

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
/// `available` lists the destinations the agent already wrote for itself, since
/// it only writes the fields the request gave it. The ones we push instead are
/// gated on the host file being there to read.
pub(crate) fn is_sandbox_file_mount(m: &oci::Mount, available: &[String]) -> bool {
    let destination = get_mount_path(&Some(m.destination().clone()));

    match sandbox_file(&destination) {
        Some((_, Origin::CreateSandbox)) => available.contains(&destination),
        Some((_, Origin::PushedByUs)) => Path::new(&get_mount_path(m.source())).is_file(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn mount_at(destination: &str, source: &str) -> oci::Mount {
        let mut m = oci::Mount::default();
        m.set_destination(Path::new(destination).to_path_buf());
        m.set_source(Some(Path::new(source).to_path_buf()));
        m
    }

    fn agent_written() -> Vec<String> {
        vec!["/etc/resolv.conf".to_string(), "/etc/hostname".to_string()]
    }

    #[test]
    fn matches_what_the_agent_wrote() {
        let available = agent_written();

        assert!(is_sandbox_file_mount(
            &mount_at("/etc/resolv.conf", "/host/resolv.conf"),
            &available
        ));
        assert!(is_sandbox_file_mount(
            &mount_at("/etc/hostname", "/host/hostname"),
            &available
        ));

        // Given nothing for that field, the agent wrote nothing to share.
        assert!(!is_sandbox_file_mount(
            &mount_at("/etc/hostname", "/host/hostname"),
            &["/etc/resolv.conf".to_string()]
        ));
        assert!(!is_sandbox_file_mount(
            &mount_at("/etc/resolv.conf", "/host/resolv.conf"),
            &[]
        ));
    }

    #[test]
    fn hosts_is_gated_on_the_host_file_being_readable() {
        // Nothing the agent wrote, and a source that does not exist.
        assert!(!is_sandbox_file_mount(
            &mount_at("/etc/hosts", "/does/not/exist"),
            &[]
        ));

        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "127.0.0.1\tlocalhost").unwrap();
        let path = f.path().to_str().unwrap();

        // Readable, and notably without needing to be in `available`.
        assert!(is_sandbox_file_mount(&mount_at("/etc/hosts", path), &[]));
    }

    #[test]
    fn leaves_unknown_destinations_alone() {
        assert!(!is_sandbox_file_mount(
            &mount_at("/dev/termination-log", "/host/termination-log"),
            &agent_written()
        ));
        assert!(!is_sandbox_file_mount(
            &mount_at("/etc/passwd", "/host/passwd"),
            &agent_written()
        ));
    }

    #[test]
    fn reads_host_file_without_growing_it() {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "127.0.0.1\tlocalhost\n10.244.0.5\tmy-pod\n").unwrap();

        let lines = read_host_file(f.path().to_str().unwrap()).unwrap();

        assert_eq!(lines, vec!["127.0.0.1\tlocalhost", "10.244.0.5\tmy-pod"]);
    }

    #[test]
    fn rejects_an_empty_host_file() {
        let f = NamedTempFile::new().unwrap();

        assert!(read_host_file(f.path().to_str().unwrap()).is_err());
        assert!(read_host_file("/does/not/exist").is_err());
    }

    #[test]
    fn table_covers_the_three_destinations() {
        assert_eq!(
            sandbox_file("/etc/resolv.conf"),
            Some((
                "/run/kata-containers/sandbox/resolv.conf",
                Origin::CreateSandbox
            ))
        );
        assert_eq!(
            sandbox_file("/etc/hostname"),
            Some((
                "/run/kata-containers/sandbox/hostname",
                Origin::CreateSandbox
            ))
        );
        assert_eq!(
            sandbox_file("/etc/hosts"),
            Some(("/run/kata-containers/sandbox/hosts", Origin::PushedByUs))
        );
        assert_eq!(sandbox_file("/dev/termination-log"), None);
    }
}
