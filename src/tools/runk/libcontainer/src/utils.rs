// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use nix::sys::stat::Mode;
use oci::Process;
use std::{
    fs::{DirBuilder, File},
    io::{prelude::*, BufReader},
    os::unix::fs::DirBuilderExt,
    path::Path,
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
    use nix::unistd::getpid;
    use oci::Process;
    use oci::State as OCIState;
    use oci::{ContainerState, Root, Spec};
    use rustjail::cgroups::fs::Manager as CgroupManager;
    use rustjail::specconv::CreateOpts;
    use std::path::Path;
    use std::time::SystemTime;

    pub const TEST_CONTAINER_ID: &str = "test";
    pub const TEST_STATE_ROOT_PATH: &str = "/state";
    pub const TEST_BUNDLE_PATH: &str = "/bundle";
    pub const TEST_ANNOTATION: &str = "test";
    pub const TEST_CGM_DATA: &str = r#"{
        "paths": {
            "devices": "/sys/fs/cgroup/devices"
        },
        "mounts": {
            "devices": "/sys/fs/cgroup/devices"
        },
        "cpath": "test"
    }"#;
    pub const TEST_ROOTFS_PATH: &str = "rootfs";

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
            version: "1.0.0".to_string(),
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

    pub fn create_dummy_cgroup(cpath: &Path) -> cgroups::Cgroup {
        cgroups::Cgroup::new(cgroups::hierarchies::auto(), cpath)
    }

    pub fn clean_up_cgroup(cpath: &Path) {
        let cgroup = cgroups::Cgroup::load(cgroups::hierarchies::auto(), cpath);
        cgroup.delete().unwrap();
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
