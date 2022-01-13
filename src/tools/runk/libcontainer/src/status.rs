// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

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
use oci::{ContainerState, State as OCIState};
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
    pub cgroup_manager: CgroupManager,
    pub config: CreateOpts,
}

impl Status {
    pub fn new(
        root: &Path,
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
            .root
            .as_ref()
            .ok_or_else(|| anyhow!("root config was not present in the spec"))?
            .path
            .clone();

        Ok(Self {
            oci_version: oci_state.version,
            id: oci_state.id,
            pid: oci_state.pid,
            root: root.to_path_buf(),
            bundle: PathBuf::from(&oci_state.bundle),
            rootfs,
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
                return Err(anyhow!("no such process"));
            }
            Ok(false)
        }
        Ok(()) => Ok(true),
    }
}

pub fn get_current_container_state(status: &Status) -> Result<ContainerState> {
    let running = is_process_running(Pid::from_raw(status.pid))?;
    let mut has_fifo = false;

    if running {
        let fifo = get_fifo_path(status);
        if fifo.exists() {
            has_fifo = true
        }
    }

    if running && !has_fifo {
        // TODO: Check paused status.
        // runk does not support pause command currently.
    }

    if !running {
        Ok(ContainerState::Stopped)
    } else if has_fifo {
        Ok(ContainerState::Created)
    } else {
        Ok(ContainerState::Running)
    }
}

pub fn get_all_pid(cgm: &CgroupManager) -> Result<Vec<Pid>> {
    let cgroup_path = cgm.paths.get("devices");
    match cgroup_path {
        Some(v) => {
            let path = Path::new(v);
            if !path.exists() {
                return Err(anyhow!("cgroup devices file does not exist"));
            }

            let procs_path = path.join("cgroup.procs");
            let pids: Vec<Pid> = lines_from_file(&procs_path)?
                .into_iter()
                .map(|v| {
                    Pid::from_raw(
                        v.parse::<pid_t>()
                            .expect("failed to parse string into pid_t"),
                    )
                })
                .collect();
            Ok(pids)
        }
        None => Err(anyhow!("cgroup devices file dose not exist")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::*;
    use chrono::{DateTime, Utc};
    use nix::unistd::getpid;
    use oci::ContainerState;
    use rustjail::cgroups::fs::Manager as CgroupManager;
    use std::path::Path;
    use std::time::SystemTime;

    #[test]
    fn test_status() {
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
        let status = create_dummy_status();
        let state = get_current_container_state(&status).unwrap();
        assert_eq!(state, ContainerState::Running);
    }

    #[test]
    fn test_get_all_pid() {
        let cgm: CgroupManager = serde_json::from_str(TEST_CGM_DATA).unwrap();
        assert!(get_all_pid(&cgm).is_ok());
    }
}
