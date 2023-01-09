// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use libcontainer::container::Container;
use liboci_cli::Pause;
use slog::{info, Logger};
use std::path::Path;

pub fn run(opts: Pause, root: &Path, logger: &Logger) -> Result<()> {
    let container = Container::load(root, &opts.container_id)?;
    container.pause()?;

    info!(&logger, "pause command finished successfully");
    Ok(())
}
