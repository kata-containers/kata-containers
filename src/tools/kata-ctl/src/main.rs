// Copyright (c) 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate slog;

mod arch;
mod args;
mod check;
mod monitor;
mod ops;
mod types;
mod utils;

use anyhow::Result;
use clap::{crate_name, Parser};
use std::io;
use std::process::exit;

use args::{Commands, KataCtlCli};

use ops::check_ops::{
    handle_check, handle_factory, handle_iptables, handle_metrics, handle_monitor, handle_version,
};
use ops::env_ops::handle_env;
use ops::exec_ops::handle_exec;
use ops::volume_ops::handle_direct_volume;
use slog::{error, o};

macro_rules! sl {
    () => {
        slog_scope::logger().new(o!("subsystem" => "kata-ctl_main"))
    };
}

fn real_main() -> Result<()> {
    let args = KataCtlCli::parse();

    let _log_level = args.log_level.unwrap_or(slog::Level::Info);

    let subsystem_level_config: HashMap<String, slog::Level> = HashMap::from([
        ("agent".to_string(), slog::Level::Info)
        ("runtimes".to_string(), slog::Level::Info)
        ("resource".to_string(), slog::Level::Info)
        ("virt-container".to_string(), slog::Level::Info)
        ("service".to_string(), slog::Level::Info)
        ("shim".to_string(), slog::Level::Info)
        ("hypervisor".to_string(), slog::Level::Info)
        ("vmm-dragonball".to_string(), slog::Level::Info)
    ]);

    let (logger, _guard) = if args.json_logging {
        logging::create_logger(crate_name!(), crate_name!(), subsystem_level_config, io::stdout())
    } else {
        logging::create_term_logger(log_level)
    };

    let _guard = slog_scope::set_global_logger(logger);

    let res = match args.command {
        Commands::Check(args) => handle_check(args),
        Commands::DirectVolume(args) => handle_direct_volume(args),
        Commands::Exec(args) => handle_exec(args),
        Commands::Env(args) => handle_env(args),
        Commands::Factory => handle_factory(),
        Commands::Iptables(args) => handle_iptables(args),
        Commands::Metrics(args) => handle_metrics(args),
        Commands::Monitor(args) => handle_monitor(args),
        Commands::Version => handle_version(),
    };

    // Log errors here, then let the logger go out of scope in main() to ensure
    // the asynchronous drain flushes all messages before exit()
    if let Err(e) = &res {
        error!(sl!(), "{:#?}", e);
    }

    res
}

fn main() {
    if let Err(_e) = real_main() {
        exit(1);
    }
}
