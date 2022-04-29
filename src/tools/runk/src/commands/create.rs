// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use libcontainer::{builder::ContainerBuilder, container::ContainerAction};
use liboci_cli::Create;
use nix::unistd::Pid;
use slog::{info, Logger};
use std::{fs, path::Path};

pub async fn run(opts: Create, root: &Path, logger: &Logger) -> Result<()> {
    let ctx = ContainerBuilder::default()
        .id(opts.container_id)
        .bundle(opts.bundle)
        .root(root.to_path_buf())
        .console_socket(opts.console_socket)
        .build()?
        .create_ctx()?;

    let pid = ctx.launch(ContainerAction::Create, logger).await?;

    if let Some(ref pid_file) = opts.pid_file {
        create_pid_file(pid_file, pid)?;
    }

    info!(&logger, "create command finished successfully");

    Ok(())
}

fn create_pid_file<P: AsRef<Path>>(pid_file: P, pid: Pid) -> Result<()> {
    fs::write(pid_file.as_ref(), format!("{}", pid))?;

    Ok(())
}
