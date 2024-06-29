// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::cgroup::{freeze, remove_cgroup_dir};
use crate::status::{self, get_current_container_state, Status};
use anyhow::{anyhow, Result};
use cgroups;
use cgroups::freezer::FreezerState;
use cgroups::hierarchies::is_cgroup2_unified_mode;
use nix::sys::signal::kill;
use nix::{
    sys::signal::Signal,
    sys::signal::SIGKILL,
    unistd::{chdir, unlink, Pid},
};
use procfs;
use runtime_spec::{ContainerState, State as OCIState};
use rustjail::cgroups::fs::Manager as CgroupManager;
use rustjail::{
    container::{BaseContainer, LinuxContainer, EXEC_FIFO_FILENAME},
    process::{Process, ProcessOperations},
    specconv::CreateOpts,
};
use scopeguard::defer;
use slog::{debug, info, Logger};
use std::{
    env::current_dir,
    fs,
    path::{Path, PathBuf},
};

use kata_sys_util::hooks::HookStates;

pub const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ContainerAction {
    Create,
    Start,
    Run,
}

#[derive(Debug)]
pub struct Container {
    pub status: Status,
    pub state: ContainerState,
    pub cgroup: cgroups::Cgroup,
}

// Container represents a container that is created by the container runtime.
impl Container {
    pub fn load(state_root: &Path, id: &str) -> Result<Self> {
        let status = Status::load(state_root, id)?;
        let spec = status
            .config
            .spec
            .as_ref()
            .ok_or_else(|| anyhow!("spec config was not present"))?;
        let linux = spec
            .linux()
            .as_ref()
            .ok_or_else(|| anyhow!("linux config was not present"))?;
        let cpath = if linux.cgroups_path().is_none() {
            id.to_string()
        } else {
            linux
                .cgroups_path()
                .clone()
                .unwrap_or_default()
                .display()
                .to_string()
                .trim_start_matches('/')
                .to_string()
        };
        let cgroup = cgroups::Cgroup::load(cgroups::hierarchies::auto(), cpath);
        let state = get_current_container_state(&status, &cgroup)?;
        Ok(Self {
            status,
            state,
            cgroup,
        })
    }

    pub fn processes(&self) -> Result<Vec<Pid>> {
        let pids = self.cgroup.tasks();
        let result = pids.iter().map(|x| Pid::from_raw(x.pid as i32)).collect();
        Ok(result)
    }

    pub fn kill(&self, signal: Signal, all: bool) -> Result<()> {
        if all {
            let pids = self.processes()?;
            for pid in pids {
                if !status::is_process_running(pid)? {
                    continue;
                }
                kill(pid, signal)?;
            }
        } else {
            // If --all option is not specified and the container is stopped,
            // kill operation generates an error in accordance with the OCI runtime spec.
            if self.state == ContainerState::Stopped {
                return Err(anyhow!(
                    "container {} can't be killed because it is {:?}",
                    self.status.id,
                    self.state
                )
                // This error message mustn't be chagned because the containerd integration tests
                // expect that OCI container runtimes return the message.
                // Ref. https://github.com/containerd/containerd/blob/release/1.7/pkg/process/utils.go#L135
                .context("container not running"));
            }

            let pid = Pid::from_raw(self.status.pid);
            if status::is_process_running(pid)? {
                kill(pid, signal)?;
            }
        }
        // For cgroup v1, killing a process in a frozen cgroup does nothing until it's thawed.
        // Only thaw the cgroup for SIGKILL.
        // Ref: https://github.com/opencontainers/runc/pull/3217
        if !is_cgroup2_unified_mode() && self.state == ContainerState::Paused && signal == SIGKILL {
            freeze(&self.cgroup, FreezerState::Thawed)?;
        }
        Ok(())
    }

    pub async fn delete(&self, force: bool, logger: &Logger) -> Result<()> {
        let status = &self.status;
        let spec = status
            .config
            .spec
            .as_ref()
            .ok_or_else(|| anyhow!("spec config was not present in the status"))?;

        let oci_state = OCIState {
            version: status.oci_version.clone(),
            id: status.id.clone(),
            status: self.state,
            pid: status.pid,
            bundle: status
                .bundle
                .to_str()
                .ok_or_else(|| anyhow!("invalid bundle path"))?
                .to_string(),
            annotations: spec.annotations().clone().unwrap_or_default(),
        };

        if let Some(hooks) = spec.hooks().as_ref() {
            info!(&logger, "Poststop Hooks");
            let mut poststop_hookstates = HookStates::new();
            poststop_hookstates.execute_hooks(
                &hooks.poststop().clone().unwrap_or_default(),
                Some(oci_state.clone()),
            )?;
        }

        match oci_state.status {
            ContainerState::Stopped => {
                self.destroy()?;
            }
            ContainerState::Created => {
                // Kill an init process
                self.kill(SIGKILL, false)?;
                self.destroy()?;
            }
            _ => {
                if force {
                    self.kill(SIGKILL, true)?;
                    self.destroy()?;
                } else {
                    return Err(anyhow!(
                        "cannot delete container {} that is not stopped",
                        &status.id
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        if self.state != ContainerState::Running && self.state != ContainerState::Created {
            return Err(anyhow!(
                "failed to pause container: current status is: {:?}",
                self.state
            ));
        }
        freeze(&self.cgroup, FreezerState::Frozen)?;
        Ok(())
    }

    pub fn resume(&self) -> Result<()> {
        if self.state != ContainerState::Paused {
            return Err(anyhow!(
                "failed to resume container: current status is: {:?}",
                self.state
            ));
        }
        freeze(&self.cgroup, FreezerState::Thawed)?;
        Ok(())
    }

    pub fn destroy(&self) -> Result<()> {
        remove_cgroup_dir(&self.cgroup)?;
        self.status.remove_dir()
    }
}

/// Used to run a process. If init is set, it will create a container and run the process in it.
/// If init is not set, it will run the process in an existing container.
#[derive(Debug)]
pub struct ContainerLauncher {
    pub id: String,
    pub bundle: PathBuf,
    pub state_root: PathBuf,
    pub init: bool,
    pub runner: LinuxContainer,
    pub pid_file: Option<PathBuf>,
}

impl ContainerLauncher {
    pub fn new(
        id: &str,
        bundle: &Path,
        state_root: &Path,
        init: bool,
        runner: LinuxContainer,
        pid_file: Option<PathBuf>,
    ) -> Self {
        ContainerLauncher {
            id: id.to_string(),
            bundle: bundle.to_path_buf(),
            state_root: state_root.to_path_buf(),
            init,
            runner,
            pid_file,
        }
    }

    /// Launch a process. For init containers, we will create a container. For non-init, it will join an existing container.
    pub async fn launch(&mut self, action: ContainerAction, logger: &Logger) -> Result<()> {
        if self.init {
            self.spawn_container(action, logger).await?;
        } else {
            if action == ContainerAction::Create {
                return Err(anyhow!(
                    "ContainerAction::Create is used for init-container only"
                ));
            }
            self.spawn_process(action, logger).await?;
        }
        if let Some(pid_file) = self.pid_file.as_ref() {
            fs::write(
                pid_file,
                format!("{}", self.runner.get_process(self.id.as_str())?.pid()),
            )?;
        }
        Ok(())
    }

    /// Create the container by invoking runner to spawn the first process and save status.
    async fn spawn_container(&mut self, action: ContainerAction, logger: &Logger) -> Result<()> {
        // State root path root/id has been created in LinuxContainer::new(),
        // so we don't have to create it again.

        // Spawn a new process in the container by using the agent's codes.
        self.spawn_process(action, logger).await?;

        let status = self.get_status()?;
        status.save()?;
        debug!(logger, "saved status is {:?}", status);

        // Clean up the fifo file created by LinuxContainer, which is used for block the created process.
        if action == ContainerAction::Run || action == ContainerAction::Start {
            let fifo_path = get_fifo_path(&status);
            if fifo_path.exists() {
                unlink(&fifo_path)?;
            }
        }
        Ok(())
    }

    /// Generate rustjail::Process from OCI::Process
    fn get_process(&self, logger: &Logger) -> Result<Process> {
        let spec = self.runner.config.spec.as_ref().unwrap();
        if spec.process().is_some() {
            Ok(Process::new(
                logger,
                spec.process().as_ref().unwrap(),
                // rustjail::LinuxContainer use the exec_id to identify processes in a container,
                // so we can get the spawned process by ctr.get_process(exec_id) later.
                // Since LinuxContainer is temporarily created to spawn one process in each runk invocation,
                // we can use arbitrary string as the exec_id. Here we choose the container id.
                &self.id,
                self.init,
                0,
                None,
            )?)
        } else {
            Err(anyhow!("no process configuration"))
        }
    }

    /// Spawn a new process in the container by invoking runner.
    async fn spawn_process(&mut self, action: ContainerAction, logger: &Logger) -> Result<()> {
        // Agent will chdir to bundle_path before creating LinuxContainer. Just do the same as agent.
        let current_dir = current_dir()?;
        chdir(&self.bundle)?;
        defer! {
            chdir(&current_dir).unwrap();
        }

        let process = self.get_process(logger)?;
        match action {
            ContainerAction::Create => {
                self.runner.start(process).await?;
            }
            ContainerAction::Start => {
                self.runner.exec().await?;
            }
            ContainerAction::Run => {
                self.runner.run(process).await?;
            }
        }
        Ok(())
    }

    /// Generate runk specified Status
    fn get_status(&self) -> Result<Status> {
        let oci_state = self.runner.oci_state()?;
        // read start time from /proc/<pid>/stat
        let proc = procfs::process::Process::new(self.runner.init_process_pid)?;
        let process_start_time = proc.stat()?.starttime;
        Status::new(
            &self.state_root,
            &self.bundle,
            oci_state,
            process_start_time,
            self.runner.created,
            self.runner
                .cgroup_manager
                .as_ref()
                .as_any()?
                .downcast_ref::<CgroupManager>()
                .unwrap()
                .clone(),
            self.runner.config.clone(),
        )
    }
}

pub fn create_linux_container(
    id: &str,
    root: &Path,
    config: CreateOpts,
    console_socket: Option<PathBuf>,
    logger: &Logger,
) -> Result<LinuxContainer> {
    let mut container = LinuxContainer::new(
        id,
        root.to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("failed to convert bundle path"))?
            .as_str(),
        None,
        config,
        logger,
    )?;
    if let Some(socket_path) = console_socket.as_ref() {
        container.set_console_socket(socket_path)?;
    }
    Ok(container)
}

// Load rustjail's Linux container.
// "uid_map_path" and "gid_map_path" are always empty, so they are not set.
pub fn load_linux_container(
    status: &Status,
    console_socket: Option<PathBuf>,
    logger: &Logger,
) -> Result<LinuxContainer> {
    let mut container = LinuxContainer::new(
        &status.id,
        &status
            .root
            .to_str()
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("failed to convert a root path"))?,
        None,
        status.config.clone(),
        logger,
    )?;
    if let Some(socket_path) = console_socket.as_ref() {
        container.set_console_socket(socket_path)?;
    }

    container.init_process_pid = status.pid;
    container.init_process_start_time = status.process_start_time;
    container.created = status.created.into();
    Ok(container)
}

pub fn get_config_path<P: AsRef<Path>>(bundle: P) -> PathBuf {
    bundle.as_ref().join(CONFIG_FILE_NAME)
}

pub fn get_fifo_path(status: &Status) -> PathBuf {
    status.root.join(&status.id).join(EXEC_FIFO_FILENAME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::*;
    use rustjail::container::EXEC_FIFO_FILENAME;
    use std::path::PathBuf;

    #[test]
    fn test_get_config_path() {
        let test_data = PathBuf::from(TEST_BUNDLE_PATH).join(CONFIG_FILE_NAME);
        assert_eq!(get_config_path(TEST_BUNDLE_PATH), test_data);
    }

    #[test]
    fn test_get_fifo_path() {
        let test_data = PathBuf::from(TEST_STATE_ROOT_PATH)
            .join(TEST_CONTAINER_ID)
            .join(EXEC_FIFO_FILENAME);
        let status = create_dummy_status();

        assert_eq!(get_fifo_path(&status), test_data);
    }
}
