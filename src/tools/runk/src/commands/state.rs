// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use chrono::{DateTime, Utc};
use libcontainer::{container::Container, status::Status};
use liboci_cli::State;
use runtime_spec::ContainerState;
use serde::{Deserialize, Serialize};
use slog::{info, Logger};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    pub oci_version: String,
    pub id: String,
    pub pid: i32,
    pub status: String,
    pub bundle: PathBuf,
    pub created: DateTime<Utc>,
}

impl RuntimeState {
    pub fn new(status: Status, state: ContainerState) -> Self {
        Self {
            oci_version: status.oci_version,
            id: status.id,
            pid: status.pid,
            status: get_container_state_name(state),
            bundle: status.bundle,
            created: status.created,
        }
    }
}

pub fn run(opts: State, state_root: &Path, logger: &Logger) -> Result<()> {
    let container = Container::load(state_root, &opts.container_id)?;
    let oci_state = RuntimeState::new(container.status, container.state);
    let json_state = &serde_json::to_string_pretty(&oci_state)?;

    println!("{}", json_state);

    info!(&logger, "state command finished successfully");

    Ok(())
}

pub fn get_container_state_name(state: ContainerState) -> String {
    match state {
        ContainerState::Creating => "creating",
        ContainerState::Created => "created",
        ContainerState::Running => "running",
        ContainerState::Stopped => "stopped",
        ContainerState::Paused => "paused",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_spec::ContainerState;

    #[test]
    fn test_get_container_state_name() {
        assert_eq!(
            "creating",
            get_container_state_name(ContainerState::Creating)
        );
        assert_eq!("created", get_container_state_name(ContainerState::Created));
        assert_eq!("running", get_container_state_name(ContainerState::Running));
        assert_eq!("stopped", get_container_state_name(ContainerState::Stopped));
        assert_eq!("paused", get_container_state_name(ContainerState::Paused));
    }
}
