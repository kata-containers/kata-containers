// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use libcontainer::activated_builder::ActivatedContainerBuilder;
use libcontainer::container::ContainerAction;
use liboci_cli::Exec;
use slog::{info, Logger};
use std::path::Path;

pub async fn run(opts: Exec, root: &Path, logger: &Logger) -> Result<()> {
    let mut launcher = ActivatedContainerBuilder::default()
        .id(opts.container_id)
        .root(root.to_path_buf())
        .console_socket(opts.console_socket)
        .pid_file(opts.pid_file)
        .tty(opts.tty)
        .cwd(opts.cwd)
        .env(opts.env)
        .no_new_privs(opts.no_new_privs)
        .process(opts.process)
        .args(opts.command)
        .build()?
        .create_launcher(logger)?;

    launcher.launch(ContainerAction::Run, logger).await?;

    info!(&logger, "exec command finished successfully");
    Ok(())
}
