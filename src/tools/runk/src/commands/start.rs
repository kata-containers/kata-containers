// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::commands::state::get_container_state_name;
use anyhow::{anyhow, Result};
use libcontainer::container::{get_fifo_path, Container};
use liboci_cli::Start;
use nix::unistd::unlink;
use oci::ContainerState;
use slog::{info, Logger};
use std::{fs::OpenOptions, io::prelude::*, path::Path};

pub fn run(opts: Start, state_root: &Path, logger: &Logger) -> Result<()> {
    let container = Container::load(state_root, &opts.container_id)?;
    if container.state != ContainerState::Created {
        return Err(anyhow!(
            "cannot start a container in the {} state",
            get_container_state_name(container.state)
        ));
    };

    let fifo_path = get_fifo_path(&container.status);
    let mut file = OpenOptions::new().write(true).open(&fifo_path)?;

    file.write_all("0".as_bytes())?;

    info!(&logger, "container started");

    if fifo_path.exists() {
        unlink(&fifo_path)?;
    }

    info!(&logger, "start command finished successfully");

    Ok(())
}
