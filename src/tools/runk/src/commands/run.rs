// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use libcontainer::{builder::ContainerBuilder, container::ContainerAction};
use liboci_cli::Run;
use slog::{info, Logger};
use std::path::Path;

pub async fn run(opts: Run, root: &Path, logger: &Logger) -> Result<()> {
    let ctx = ContainerBuilder::default()
        .id(opts.container_id)
        .bundle(opts.bundle)
        .root(root.to_path_buf())
        .console_socket(opts.console_socket)
        .build()?
        .create_ctx()?;

    ctx.launch(ContainerAction::Run, logger).await?;

    info!(&logger, "run command finished successfully");

    Ok(())
}
