// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::status::Status;
use anyhow::{anyhow, Result};
use nix::unistd::{chdir, unlink};
use rustjail::{
    container::{BaseContainer, LinuxContainer, EXEC_FIFO_FILENAME},
    process::{Process, ProcessOperations},
};
use scopeguard::defer;
use slog::{debug, Logger};
use std::{
    env::current_dir,
    fs,
    path::{Path, PathBuf},
};

pub const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ContainerAction {
    Create,
    Run,
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
            if action != ContainerAction::Run {
                return Err(anyhow!(
                    "ContainerAction::Create is used for init-container only"
                ));
            }
            self.spawn_process(ContainerAction::Run, logger).await?;
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

        self.spawn_process(action, logger).await?;
        let status = self.get_status()?;
        status.save()?;
        debug!(logger, "saved status is {:?}", status);

        // Clean up the fifo file created by LinuxContainer, which is used for block the created process.
        if action == ContainerAction::Run {
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
        if spec.process.is_some() {
            Ok(Process::new(
                logger,
                spec.process
                    .as_ref()
                    .ok_or_else(|| anyhow!("process config was not present in the spec file"))?,
                // rustjail::LinuxContainer use the exec_id to identify processes in a container,
                // so we can get the spawned process by ctr.get_process(exec_id) later.
                // Since LinuxContainer is temporarily created to spawn one process in each runk invocation,
                // we can use arbitrary string as the exec_id. Here we choose the container id.
                &self.id,
                self.init,
                0,
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
            ContainerAction::Run => {
                self.runner.run(process).await?;
            }
        }
        Ok(())
    }

    /// Generate runk specified Status
    fn get_status(&self) -> Result<Status> {
        let oci_state = self.runner.oci_state()?;
        Status::new(
            &self.state_root,
            &self.bundle,
            oci_state,
            self.runner.init_process_start_time,
            self.runner.created,
            self.runner
                .cgroup_manager
                .clone()
                .ok_or_else(|| anyhow!("cgroup manager was not present"))?,
            self.runner.config.clone(),
        )
    }
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
