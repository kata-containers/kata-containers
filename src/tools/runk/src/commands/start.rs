// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::commands::state::get_container_state_name;
use anyhow::{anyhow, Result};
use libcontainer::{
    container::get_fifo_path,
    status::{get_current_container_state, Status},
};
use liboci_cli::Start;
use nix::unistd::unlink;
use oci::ContainerState;
use slog::{info, Logger};
use std::{fs::OpenOptions, io::prelude::*, path::Path, time::SystemTime};

pub fn run(opts: Start, state_root: &Path, logger: &Logger) -> Result<()> {
    let mut status = Status::load(state_root, &opts.container_id)?;
    let state = get_current_container_state(&status)?;
    if state != ContainerState::Created {
        return Err(anyhow!(
            "cannot start a container in the {} state",
            get_container_state_name(state)
        ));
    };

    let fifo_path = get_fifo_path(&status);
    let mut file = OpenOptions::new().write(true).open(&fifo_path)?;

    file.write_all("0".as_bytes())?;

    info!(&logger, "container started");

    status.process_start_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();

    status.save()?;

    if fifo_path.exists() {
        unlink(&fifo_path)?;
    }

    info!(&logger, "start command finished successfully");

    Ok(())
}
