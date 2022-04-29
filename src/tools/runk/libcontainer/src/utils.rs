// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use nix::sys::stat::Mode;
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

#[cfg(test)]
pub(crate) mod test_utils {
    use crate::status::Status;
    use nix::unistd::getpid;
    use oci::State as OCIState;
    use oci::{ContainerState, Root, Spec};
    use rustjail::cgroups::fs::Manager as CgroupManager;
    use rustjail::specconv::CreateOpts;
    use std::path::Path;
    use std::time::SystemTime;

    pub const TEST_CONTAINER_ID: &str = "test";
    pub const TEST_BUNDLE_PATH: &str = "/test";
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
        let status = Status::new(
            Path::new(TEST_BUNDLE_PATH),
            oci_state.clone(),
            1,
            created,
            cgm,
            create_dummy_opts(),
        )
        .unwrap();

        status
    }
}
