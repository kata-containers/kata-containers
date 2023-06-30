// Copyright 2021-2022 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use clap::{crate_description, crate_name, Parser};
use liboci_cli::{CommonCmd, GlobalOpts};
use liboci_cli::{Create, Delete, Kill, Start, State};
use slog::{o, Logger};
use slog_async::AsyncGuard;
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    process::exit,
};

const DEFAULT_ROOT_DIR: &str = "/run/runk";
const DEFAULT_LOG_LEVEL: slog::Level = slog::Level::Info;

mod commands;

#[derive(Parser, Debug)]
enum SubCommand {
    #[clap(flatten)]
    Standard(StandardCmd),
    #[clap(flatten)]
    Common(CommonCmd),
    /// Launch an init process (do not call it outside of runk)
    Init {},
}

// Copy from https://github.com/containers/youki/blob/v0.0.3/crates/liboci-cli/src/lib.rs#L38-L44
#[derive(Parser, Debug)]
pub enum StandardCmd {
    Create(Create),
    Start(Start),
    State(State),
    Delete(Delete),
    Kill(Kill),
}

#[derive(Parser, Debug)]
#[clap(version, author, about = crate_description!())]
struct Cli {
    #[clap(flatten)]
    global: GlobalOpts,
    #[clap(subcommand)]
    subcmd: SubCommand,
}

async fn cmd_run(subcmd: SubCommand, root_path: &Path, logger: &Logger) -> Result<()> {
    match subcmd {
        SubCommand::Standard(cmd) => match cmd {
            StandardCmd::Create(create) => commands::create::run(create, root_path, logger).await,
            StandardCmd::Start(start) => commands::start::run(start, root_path, logger).await,
            StandardCmd::Delete(delete) => commands::delete::run(delete, root_path, logger).await,
            StandardCmd::State(state) => commands::state::run(state, root_path, logger),
            StandardCmd::Kill(kill) => commands::kill::run(kill, root_path, logger),
        },
        SubCommand::Common(cmd) => match cmd {
            CommonCmd::Run(run) => commands::run::run(run, root_path, logger).await,
            CommonCmd::Spec(spec) => commands::spec::run(spec, logger),
            CommonCmd::List(list) => commands::list::run(list, root_path, logger),
            CommonCmd::Exec(exec) => commands::exec::run(exec, root_path, logger).await,
            CommonCmd::Ps(ps) => commands::ps::run(ps, root_path, logger),
            CommonCmd::Pause(pause) => commands::pause::run(pause, root_path, logger),
            CommonCmd::Resume(resume) => commands::resume::run(resume, root_path, logger),
            _ => Err(anyhow!("command is not implemented yet")),
        },
        _ => unreachable!(),
    }
}

fn setup_logger(
    log_file: Option<PathBuf>,
    log_level: slog::Level,
) -> Result<(Logger, Option<AsyncGuard>)> {
    if let Some(ref file) = log_file {
        let log_writer = OpenOptions::new()
            .write(true)
            .read(true)
            .create(true)
            .truncate(true)
            .open(file)?;

        // TODO: Support 'text' log format.
        let (logger_local, logger_async_guard_local) =
            logging::create_logger(crate_name!(), crate_name!(), log_level, log_writer);

        Ok((logger_local, Some(logger_async_guard_local)))
    } else {
        let logger = slog::Logger::root(slog::Discard, o!());
        Ok((logger, None))
    }
}

async fn real_main() -> Result<()> {
    let cli = Cli::parse();

    if let SubCommand::Init {} = cli.subcmd {
        rustjail::container::init_child();
        exit(0);
    }

    let root_path = if let Some(path) = cli.global.root {
        path
    } else {
        PathBuf::from(DEFAULT_ROOT_DIR)
    };

    let log_level = if cli.global.debug {
        slog::Level::Debug
    } else {
        DEFAULT_LOG_LEVEL
    };

    let (logger, _async_guard) = setup_logger(cli.global.log, log_level)?;

    cmd_run(cli.subcmd, &root_path, &logger).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = real_main().await {
        eprintln!("ERROR: {}", e);
        exit(1);
    }

    exit(0);
}
