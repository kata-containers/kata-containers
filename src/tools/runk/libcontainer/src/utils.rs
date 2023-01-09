// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use nix::sys::stat::Mode;
use oci::{Process, Spec};
use std::{
    fs::{DirBuilder, File},
    io::{prelude::*, BufReader},
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};

pub fn lines_from_file<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
    let file = File::open(&path)?;
    let buf = BufReader::new(file);
    Ok(buf
        .lines()
        .map(|v| v.expect("could not parse line"))
        .collect())
}

pub fn create_dir_with_mode<P: AsRef<Path>>(path: P, mode: Mode, recursive: bool) -> Result<()> {
    let path = path.as_ref();
    if path.exists() {
        return Err(anyhow!("{} already exists", path.display()));
    }

    Ok(DirBuilder::new()
        .recursive(recursive)
        .mode(mode.bits())
        .create(path)?)
}

/// If root in spec is a relative path, make it absolute.
pub fn canonicalize_spec_root(spec: &mut Spec, bundle_canon: &Path) -> Result<()> {
    let mut spec_root = spec
        .root
        .as_mut()
        .ok_or_else(|| anyhow!("root config was not present in the spec file"))?;
    let rootfs_path = Path::new(&spec_root.path);
    if !rootfs_path.is_absolute() {
        spec_root.path = bundle_canon
            .join(rootfs_path)
            .canonicalize()?
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("failed to convert a rootfs path into a canonical path"))?;
    }
    Ok(())
}

/// Check whether spec is valid. Now runk only support detach mode.
pub fn validate_spec(spec: &Spec, console_socket: &Option<PathBuf>) -> Result<()> {
    validate_process_spec(&spec.process)?;
    if let Some(process) = spec.process.as_ref() {
        // runk always launches containers with detached mode, so users have to
        // use a console socket with run or create operation when a terminal is used.
        if process.terminal && console_socket.is_none() {
            return Err(anyhow!(
                "cannot allocate a pseudo-TTY without setting a console socket"
            ));
        }
    }
    Ok(())
}

// Validate process just like runc, https://github.com/opencontainers/runc/pull/623
pub fn validate_process_spec(process: &Option<Process>) -> Result<()> {
    let process = process
        .as_ref()
        .ok_or_else(|| anyhow!("process property must not be empty"))?;
    if process.cwd.is_empty() {
        return Err(anyhow!("cwd property must not be empty"));
    }
    let cwd = Path::new(process.cwd.as_str());
    if !cwd.is_absolute() {
        return Err(anyhow!("cwd must be an absolute path"));
    }
    if process.args.is_empty() {
        return Err(anyhow!("args must not be empty"));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_utils {
    use super::*;
    use crate::status::Status;
    use chrono::DateTime;
    use nix::unistd::getpid;
    use oci::{ContainerState, LinuxNamespace, Process, Root, Spec, State as OCIState};
    use rustjail::{
        cgroups::fs::Manager as CgroupManager, container::TYPETONAME, specconv::CreateOpts,
    };
    use std::{fs::create_dir_all, path::Path, time::SystemTime};
    use tempfile::tempdir;

    pub const TEST_CONTAINER_ID: &str = "test";
    pub const TEST_STATE_ROOT_PATH: &str = "/state";
    pub const TEST_BUNDLE_PATH: &str = "/bundle";
    pub const TEST_ROOTFS_PATH: &str = "rootfs";
    pub const TEST_ANNOTATION: &str = "test-annotation";
    pub const TEST_CONSOLE_SOCKET_PATH: &str = "/test-console-sock";
    pub const TEST_PROCESS_FILE_NAME: &str = "process.json";
    pub const TEST_PID_FILE_PATH: &str = "/test-pid";
    pub const TEST_HOST_NAME: &str = "test-host";
    pub const TEST_OCI_SPEC_VERSION: &str = "1.0.2";
    pub const TEST_CGM_DATA: &str = r#"{
        "paths": {
            "devices": "/sys/fs/cgroup/devices"
        },
        "mounts": {
            "devices": "/sys/fs/cgroup/devices"
        },
        "cpath": "test"
    }"#;

    #[derive(Debug)]
    pub struct TestContainerData {
        pub id: String,
        pub bundle: PathBuf,
        pub root: PathBuf,
        pub console_socket: Option<PathBuf>,
        pub pid_file: Option<PathBuf>,
        pub config: CreateOpts,
    }

    pub fn create_dummy_spec() -> Spec {
        let linux = oci::Linux {
            namespaces: TYPETONAME
                .iter()
                .filter(|&(_, &name)| name != "user")
                .map(|ns| LinuxNamespace {
                    r#type: ns.0.to_string(),
                    path: "".to_string(),
                })
                .collect(),
            ..Default::default()
        };
        Spec {
            version: TEST_OCI_SPEC_VERSION.to_string(),
            process: Some(Process {
                args: vec!["sleep".to_string(), "10".to_string()],
                env: vec!["PATH=/bin:/usr/bin".to_string()],
                cwd: "/".to_string(),
                ..Default::default()
            }),
            hostname: TEST_HOST_NAME.to_string(),
            root: Some(Root {
                path: TEST_ROOTFS_PATH.to_string(),
                readonly: false,
            }),
            linux: Some(linux),
            ..Default::default()
        }
    }

    pub fn create_dummy_opts() -> CreateOpts {
        let spec = Spec {
            root: Some(Root::default()),
            ..Default::default()
        };
        CreateOpts {
            cgroup_name: "".to_string(),
            use_systemd_cgroup: false,
            no_pivot_root: false,
            no_new_keyring: false,
            spec: Some(spec),
            rootless_euid: false,
            rootless_cgroup: false,
        }
    }

    pub fn create_dummy_oci_state() -> OCIState {
        OCIState {
            version: TEST_OCI_SPEC_VERSION.to_string(),
            id: TEST_CONTAINER_ID.to_string(),
            status: ContainerState::Running,
            pid: getpid().as_raw(),
            bundle: TEST_BUNDLE_PATH.to_string(),
            annotations: [(TEST_ANNOTATION.to_string(), TEST_ANNOTATION.to_string())]
                .iter()
                .cloned()
                .collect(),
        }
    }

    pub fn create_dummy_status() -> Status {
        let cgm: CgroupManager = serde_json::from_str(TEST_CGM_DATA).unwrap();
        let oci_state = create_dummy_oci_state();
        let created = SystemTime::now();
        let start_time = procfs::process::Process::new(oci_state.pid)
            .unwrap()
            .stat()
            .unwrap()
            .starttime;
        let status = Status::new(
            Path::new(TEST_STATE_ROOT_PATH),
            Path::new(TEST_BUNDLE_PATH),
            oci_state,
            start_time,
            created,
            cgm,
            create_dummy_opts(),
        )
        .unwrap();

        status
    }

    pub fn create_custom_dummy_status(id: &str, pid: i32, root: &Path, spec: &Spec) -> Status {
        let start_time = procfs::process::Process::new(pid)
            .unwrap()
            .stat()
            .unwrap()
            .starttime;
        Status {
            oci_version: spec.version.clone(),
            id: id.to_string(),
            pid,
            root: root.to_path_buf(),
            bundle: PathBuf::from(TEST_BUNDLE_PATH),
            rootfs: TEST_ROOTFS_PATH.to_string(),
            process_start_time: start_time,
            created: DateTime::from(SystemTime::now()),
            cgroup_manager: serde_json::from_str(TEST_CGM_DATA).unwrap(),
            config: CreateOpts {
                spec: Some(spec.clone()),
                ..Default::default()
            },
        }
    }

    pub fn create_dummy_cgroup(cpath: &Path) -> cgroups::Cgroup {
        cgroups::Cgroup::new(cgroups::hierarchies::auto(), cpath)
    }

    pub fn clean_up_cgroup(cpath: &Path) {
        let cgroup = cgroups::Cgroup::load(cgroups::hierarchies::auto(), cpath);
        cgroup.delete().unwrap();
    }

    #[test]
    fn test_canonicalize_spec_root() {
        let gen_spec = |p: &str| -> Spec {
            Spec {
                root: Some(Root {
                    path: p.to_string(),
                    readonly: false,
                }),
                ..Default::default()
            }
        };

        let rootfs_name = TEST_ROOTFS_PATH;
        let temp_dir = tempdir().unwrap();
        let bundle_dir = temp_dir.path();
        let abs_root = bundle_dir.join(rootfs_name);
        create_dir_all(abs_root.clone()).unwrap();
        let mut spec = gen_spec(abs_root.to_str().unwrap());
        assert!(canonicalize_spec_root(&mut spec, bundle_dir).is_ok());
        assert_eq!(spec.root.unwrap().path, abs_root.to_str().unwrap());
        let mut spec = gen_spec(rootfs_name);
        assert!(canonicalize_spec_root(&mut spec, bundle_dir).is_ok());
        assert_eq!(spec.root.unwrap().path, abs_root.to_str().unwrap());
    }

    #[test]
    pub fn test_validate_process_spec() {
        let valid_process = Process {
            args: vec!["test".to_string()],
            cwd: "/".to_string(),
            ..Default::default()
        };
        assert!(validate_process_spec(&None).is_err());
        assert!(validate_process_spec(&Some(valid_process.clone())).is_ok());
        let mut invalid_process = valid_process.clone();
        invalid_process.args = vec![];
        assert!(validate_process_spec(&Some(invalid_process)).is_err());
        let mut invalid_process = valid_process.clone();
        invalid_process.cwd = "".to_string();
        assert!(validate_process_spec(&Some(invalid_process)).is_err());
        let mut invalid_process = valid_process;
        invalid_process.cwd = "test/".to_string();
        assert!(validate_process_spec(&Some(invalid_process)).is_err());
    }
}
