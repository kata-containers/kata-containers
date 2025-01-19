// Copyright 2024 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use libcontainer::container::Container;
use liboci_cli::Events;
use oci::ContainerState;
use slog::{info, Logger};
use std::path::Path;

pub fn run(opts: Events, root: &Path, logger: &Logger) -> Result<()> {
    let container = Container::load(root, &opts.container_id)?;

    if container.state != ContainerState::Running {
        info!(&logger, "events command failed");
        return Err(anyhow!(
            "Failed to run events command: current status of container '{}' is: {:?}",
            opts.container_id,
            container.state
        ));
    }

    container.events(logger, opts.stats, opts.interval)?;

    info!(&logger, "events command finished successfully");

    Ok(())
}
