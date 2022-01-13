// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::status::Status;
use anyhow::{anyhow, Result};
use nix::unistd::{chdir, unlink, Pid};
use oci::Spec;
use rustjail::{
    container::{BaseContainer, LinuxContainer, EXEC_FIFO_FILENAME},
    process::Process,
    specconv::CreateOpts,
};
use slog::Logger;
use std::{
    env::current_dir,
    path::{Path, PathBuf},
};

pub const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ContainerAction {
    Create,
    Run,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerContext {
    pub id: String,
    pub bundle: PathBuf,
    pub state_root: PathBuf,
    pub spec: Spec,
    pub no_pivot_root: bool,
    pub console_socket: Option<PathBuf>,
}

impl ContainerContext {
    pub async fn launch(&self, action: ContainerAction, logger: &Logger) -> Result<Pid> {
        Status::create_dir(&self.state_root, &self.id)?;

        let current_dir = current_dir()?;
        chdir(&self.bundle)?;

        let create_opts = CreateOpts {
            cgroup_name: "".to_string(),
            use_systemd_cgroup: false,
            no_pivot_root: self.no_pivot_root,
            no_new_keyring: false,
            spec: Some(self.spec.clone()),
            rootless_euid: false,
            rootless_cgroup: false,
        };

        let mut ctr = LinuxContainer::new(
            &self.id,
            &self
                .state_root
                .to_str()
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("failed to convert bundle path"))?,
            create_opts.clone(),
            logger,
        )?;

        let process = if self.spec.process.is_some() {
            Process::new(
                logger,
                self.spec
                    .process
                    .as_ref()
                    .ok_or_else(|| anyhow!("process config was not present in the spec file"))?,
                &self.id,
                true,
                0,
            )?
        } else {
            return Err(anyhow!("no process configuration"));
        };

        if let Some(ref csocket_path) = self.console_socket {
            ctr.set_console_socket(csocket_path)?;
        }

        match action {
            ContainerAction::Create => {
                ctr.start(process).await?;
            }
            ContainerAction::Run => {
                ctr.run(process).await?;
            }
        }

        let oci_state = ctr.oci_state()?;
        let status = Status::new(
            &self.state_root,
            oci_state,
            ctr.init_process_start_time,
            ctr.created,
            ctr.cgroup_manager
                .ok_or_else(|| anyhow!("cgroup manager was not present"))?,
            create_opts,
        )?;

        status.save()?;

        if action == ContainerAction::Run {
            let fifo_path = get_fifo_path(&status);
            if fifo_path.exists() {
                unlink(&fifo_path)?;
            }
        }

        chdir(&current_dir)?;

        Ok(Pid::from_raw(ctr.init_process_pid))
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
        let test_data = PathBuf::from(TEST_BUNDLE_PATH)
            .join(TEST_CONTAINER_ID)
            .join(EXEC_FIFO_FILENAME);
        let status = create_dummy_status();

        assert_eq!(get_fifo_path(&status), test_data);
    }
}
