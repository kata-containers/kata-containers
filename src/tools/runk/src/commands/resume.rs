// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use libcontainer::container::Container;
use liboci_cli::Resume;
use slog::{info, Logger};
use std::path::Path;

pub fn run(opts: Resume, root: &Path, logger: &Logger) -> Result<()> {
    let container = Container::load(root, &opts.container_id)?;
    container.resume()?;

    info!(&logger, "pause command finished successfully");
    Ok(())
}
