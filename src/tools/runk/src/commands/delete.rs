// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use libcontainer::{
    cgroup,
    status::{get_current_container_state, Status},
};
use liboci_cli::Delete;
use nix::{
    errno::Errno,
    sys::signal::{kill, Signal},
    unistd::Pid,
};
use oci::{ContainerState, State as OCIState};
use rustjail::container;
use slog::{info, Logger};
use std::{fs, path::Path};

pub async fn run(opts: Delete, root: &Path, logger: &Logger) -> Result<()> {
    let container_id = &opts.container_id;
    let status_dir = Status::get_dir_path(root, container_id);
    if !status_dir.exists() {
        return Err(anyhow!("container {} does not exist", container_id));
    }

    let status = if let Ok(value) = Status::load(root, container_id) {
        value
    } else {
        fs::remove_dir_all(status_dir)?;
        return Ok(());
    };

    let spec = status
        .config
        .spec
        .as_ref()
        .ok_or_else(|| anyhow!("spec config was not present in the status"))?;

    let oci_state = OCIState {
        version: status.oci_version.clone(),
        id: status.id.clone(),
        status: get_current_container_state(&status)?,
        pid: status.pid,
        bundle: status
            .bundle
            .to_str()
            .ok_or_else(|| anyhow!("invalid bundle path"))?
            .to_string(),
        annotations: spec.annotations.clone(),
    };

    if spec.hooks.is_some() {
        let hooks = spec
            .hooks
            .as_ref()
            .ok_or_else(|| anyhow!("hooks config was not present"))?;
        for h in hooks.poststop.iter() {
            container::execute_hook(logger, h, &oci_state).await?;
        }
    }

    match oci_state.status {
        ContainerState::Stopped => {
            destroy_container(&status)?;
        }
        ContainerState::Created => {
            kill(Pid::from_raw(status.pid), Some(Signal::SIGKILL))?;
            destroy_container(&status)?;
        }
        _ => {
            if opts.force {
                match kill(Pid::from_raw(status.pid), Some(Signal::SIGKILL)) {
                    Err(errno) => {
                        if errno != Errno::ESRCH {
                            return Err(anyhow!("{}", errno));
                        }
                    }
                    Ok(()) => {}
                }
                destroy_container(&status)?;
            } else {
                return Err(anyhow!(
                    "cannot delete container {} that is not stopped",
                    container_id
                ));
            }
        }
    }

    info!(&logger, "delete command finished successfully");

    Ok(())
}

fn destroy_container(status: &Status) -> Result<()> {
    cgroup::destroy_cgroup(&status.cgroup_manager)?;
    status.remove_dir()?;

    Ok(())
}
