// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use libcontainer::{container::Container, status::Status};
use liboci_cli::Delete;
use slog::{info, Logger};
use std::{fs, path::Path};

pub async fn run(opts: Delete, root: &Path, logger: &Logger) -> Result<()> {
    let container_id = &opts.container_id;
    let status_dir = Status::get_dir_path(root, container_id);
    if !status_dir.exists() {
        return Err(anyhow!("container {} does not exist", container_id));
    }

    let container = if let Ok(value) = Container::load(root, container_id) {
        value
    } else {
        fs::remove_dir_all(status_dir)?;
        return Ok(());
    };
    container.delete(opts.force, logger).await?;

    info!(&logger, "delete command finished successfully");

    Ok(())
}
