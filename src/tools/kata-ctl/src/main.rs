// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

mod arch;
mod args;
mod check;
mod ops;
mod utils;

use std::io;
use anyhow::{anyhow, Result};
use clap::{crate_name, Parser};
use std::process::exit;

use args::{Commands, KataCtlCli};

use ops::check_ops::{
    handle_check, handle_check_volume, handle_env, handle_exec, handle_factory, handle_iptables,
    handle_metrics, handle_version,
};

#[macro_export]
macro_rules! sl {
    () => {
        slog_scope::logger()
    };
}

fn real_main() -> Result<()> {
    //create logger
    let log_level_name = global_args
        .value_of("log-level")
        .ok_or_else(|| anyhow!("cannot get log level"))?;

    let log_level = logging::level_name_to_slog_level(log_level_name).map_err(|e| anyhow!(e))?;

    let writer = io::stdout();
    let (logger, _guard) = logging::create_logger(name, crate_name!(), log_level, writer);

    let args = KataCtlCli::parse();

    match args.command {
        Commands::Check(args) => handle_check(args),
        Commands::DirectVolume => handle_check_volume(),
        Commands::Env => handle_env(),
        Commands::Exec => handle_exec(),
        Commands::Factory => handle_factory(),
        Commands::Iptables(args) => handle_iptables(args),
        Commands::Metrics(args) => handle_metrics(args),
        Commands::Version => handle_version(),
    }
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("ERROR: {:#?}", e);
        exit(1);
    }
}
