// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::cgroup::is_paused;
use crate::container::get_fifo_path;
use crate::utils::*;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use libc::pid_t;
use nix::{
    errno::Errno,
    sys::{signal::kill, stat::Mode},
    unistd::Pid,
};
use procfs::process::ProcState;
use runtime_spec::{ContainerState, State as OCIState};
use rustjail::{cgroups::fs::Manager as CgroupManager, specconv::CreateOpts};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    time::SystemTime,
};

const STATUS_FILE: &str = "status.json";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub oci_version: String,
    pub id: String,
    pub pid: pid_t,
    pub root: PathBuf,
    pub bundle: PathBuf,
    pub rootfs: String,
    pub process_start_time: u64,
    pub created: DateTime<Utc>,
    // Methods of Manager traits in rustjail are invisible, and CgroupManager.cgroup can't be serialized.
    // So it is cumbersome to manage cgroups by this field. Instead, we use cgroups-rs::cgroup directly in Container to manager cgroups.
    // Another solution is making some methods public outside rustjail and adding getter/setter for CgroupManager.cgroup.
    // Temporarily keep this field for compatibility.
    pub cgroup_manager: CgroupManager,
    pub config: CreateOpts,
}

impl Status {
    pub fn new(
        root: &Path,
        bundle: &Path,
        oci_state: OCIState,
        process_start_time: u64,
        created_time: SystemTime,
        cgroup_mg: CgroupManager,
        config: CreateOpts,
    ) -> Result<Self> {
        let created = DateTime::from(created_time);
        let rootfs = config
            .clone()
            .spec
            .ok_or_else(|| anyhow!("spec config was not present"))?
            .root()
            .as_ref()
            .ok_or_else(|| anyhow!("root config was not present in the spec"))?
            .path()
            .clone();

        Ok(Self {
            oci_version: oci_state.version,
            id: oci_state.id,
            pid: oci_state.pid,
            root: root.to_path_buf(),
            bundle: bundle.to_path_buf(),
            rootfs: rootfs.display().to_string(),
            process_start_time,
            created,
            cgroup_manager: cgroup_mg,
            config,
        })
    }

    pub fn save(&self) -> Result<()> {
        let state_file_path = Self::get_file_path(&self.root, &self.id);

        if !&self.root.exists() {
            create_dir_with_mode(&self.root, Mode::S_IRWXU, true)?;
        }

        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(state_file_path)?;

        serde_json::to_writer(&file, self)?;

        Ok(())
    }

    pub fn load(state_root: &Path, id: &str) -> Result<Self> {
        let state_file_path = Self::get_file_path(state_root, id);
        if !state_file_path.exists() {
            return Err(anyhow!("container \"{}\" does not exist", id));
        }

        let file = File::open(&state_file_path)?;
        let state: Self = serde_json::from_reader(&file)?;

        Ok(state)
    }

    pub fn create_dir(state_root: &Path, id: &str) -> Result<()> {
        let state_dir_path = Self::get_dir_path(state_root, id);
        if !state_dir_path.exists() {
            create_dir_with_mode(state_dir_path, Mode::S_IRWXU, true)?;
        } else {
            return Err(anyhow!("container with id exists: \"{}\"", id));
        }

        Ok(())
    }

    pub fn remove_dir(&self) -> Result<()> {
        let state_dir_path = Self::get_dir_path(&self.root, &self.id);
        fs::remove_dir_all(state_dir_path)?;

        Ok(())
    }

    pub fn get_dir_path(state_root: &Path, id: &str) -> PathBuf {
        state_root.join(id)
    }

    pub fn get_file_path(state_root: &Path, id: &str) -> PathBuf {
        state_root.join(id).join(STATUS_FILE)
    }
}

pub fn is_process_running(pid: Pid) -> Result<bool> {
    match kill(pid, None) {
        Err(errno) => {
            if errno != Errno::ESRCH {
                return Err(anyhow!("failed to kill process {}: {:?}", pid, errno));
            }
            Ok(false)
        }
        Ok(()) => Ok(true),
    }
}

// Returns the current state of a container. It will read cgroupfs and procfs to determine the state.
// https://github.com/opencontainers/runc/blob/86d6898f3052acba1ebcf83aa2eae3f6cc5fb471/libcontainer/container_linux.go#L1953
pub fn get_current_container_state(
    status: &Status,
    cgroup: &cgroups::Cgroup,
) -> Result<ContainerState> {
    if is_paused(cgroup)? {
        return Ok(ContainerState::Paused);
    }
    let proc = procfs::process::Process::new(status.pid);
    // if reading /proc/<pid> occurs error, then the process is not running
    if proc.is_err() {
        return Ok(ContainerState::Stopped);
    }
    let proc_stat = proc.unwrap().stat()?;
    // if start time is not equal, then the pid is reused, and the process is not running
    if proc_stat.starttime != status.process_start_time {
        return Ok(ContainerState::Stopped);
    }
    match proc_stat.state()? {
        ProcState::Zombie | ProcState::Dead => Ok(ContainerState::Stopped),
        _ => {
            let fifo = get_fifo_path(status);
            if fifo.exists() {
                return Ok(ContainerState::Created);
            }
            Ok(ContainerState::Running)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::*;
    use ::test_utils::skip_if_not_root;
    use chrono::{DateTime, Utc};
    use nix::unistd::getpid;
    use runtime_spec::ContainerState;
    use rustjail::cgroups::fs::Manager as CgroupManager;
    use scopeguard::defer;
    use std::path::Path;
    use std::time::SystemTime;

    #[test]
    fn test_status() {
        let cgm: CgroupManager = serde_json::from_str(TEST_CGM_DATA).unwrap();
        let oci_state = create_dummy_oci_state();
        let created = SystemTime::now();
        let status = Status::new(
            Path::new(TEST_STATE_ROOT_PATH),
            Path::new(TEST_BUNDLE_PATH),
            oci_state.clone(),
            1,
            created,
            cgm,
            create_dummy_opts(),
        )
        .unwrap();

        assert_eq!(status.id, oci_state.id);
        assert_eq!(status.pid, oci_state.pid);
        assert_eq!(status.process_start_time, 1);
        assert_eq!(status.created, DateTime::<Utc>::from(created));
    }

    #[test]
    fn test_is_process_running() {
        let pid = getpid();
        let ret = is_process_running(pid).unwrap();
        assert!(ret);
    }

    #[test]
    fn test_get_current_container_state() {
        skip_if_not_root!();
        let mut status = create_dummy_status();
        status.id = "test_get_current_container_state".to_string();
        // crete a dummy cgroup to make sure is_pause doesn't return error
        let cgroup = create_dummy_cgroup(Path::new(&status.id));
        defer!(cgroup.delete().unwrap());
        let state = get_current_container_state(&status, &cgroup).unwrap();
        assert_eq!(state, ContainerState::Running);
    }
}
