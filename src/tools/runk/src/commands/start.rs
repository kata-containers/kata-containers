// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use libcontainer::{container::ContainerAction, created_builder::CreatedContainerBuilder};
use liboci_cli::Start;
use slog::{info, Logger};
use std::path::Path;

pub async fn run(opts: Start, root: &Path, logger: &Logger) -> Result<()> {
    let mut launcher = CreatedContainerBuilder::default()
        .id(opts.container_id)
        .root(root.to_path_buf())
        .build()?
        .create_launcher(logger)?;

    launcher.launch(ContainerAction::Start, logger).await?;

    info!(&logger, "start command finished successfully");

    Ok(())
}
