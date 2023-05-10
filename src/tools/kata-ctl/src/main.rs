// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

mod arch;
mod args;
mod check;
mod ops;
mod types;
mod utils;

use anyhow::Result;
use clap::Parser;
use std::process::exit;

use args::{Commands, KataCtlCli};

use ops::check_ops::{
    handle_check, handle_factory, handle_iptables, handle_metrics, handle_version,
};
use ops::env_ops::handle_env;
use ops::exec_ops::handle_exec;
use ops::volume_ops::handle_direct_volume;

fn real_main() -> Result<()> {
    let args = KataCtlCli::parse();

    match args.command {
        Commands::Check(args) => handle_check(args),
        Commands::DirectVolume(args) => handle_direct_volume(args),
        Commands::Exec(args) => handle_exec(args),
        Commands::Env(args) => handle_env(args),
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
