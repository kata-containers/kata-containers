// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use libcontainer::{container::ContainerAction, init_builder::InitContainerBuilder};

use liboci_cli::Create;
use slog::{info, Logger};
use std::path::Path;

pub async fn run(opts: Create, root: &Path, logger: &Logger) -> Result<()> {
    let mut launcher = InitContainerBuilder::default()
        .id(opts.container_id)
        .bundle(opts.bundle)
        .root(root.to_path_buf())
        .console_socket(opts.console_socket)
        .pid_file(opts.pid_file)
        .build()?
        .create_launcher(logger)?;

    launcher.launch(ContainerAction::Create, logger).await?;

    info!(&logger, "create command finished successfully");

    Ok(())
}
