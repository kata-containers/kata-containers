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
mod log_parser;
mod monitor;
mod ops;
mod types;
mod utils;

use crate::log_parser::log_parser;
use anyhow::Result;
use args::{Commands, KataCtlCli};
use clap::{crate_name, CommandFactory, Parser};
use kata_types::config::TomlConfig;
use std::io;
use std::process::exit;

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

    if args.show_default_config_paths {
        TomlConfig::get_default_config_file_list()
            .iter()
            .for_each(|p| println!("{}", p.display()));

        return Ok(());
    }

    let log_level = args.log_level.unwrap_or(slog::Level::Info);

    let (logger, _guard) = if args.json_logging {
        logging::create_logger(crate_name!(), crate_name!(), log_level, io::stdout())
    } else {
        logging::create_term_logger(log_level)
    };

    let _guard = slog_scope::set_global_logger(logger);

    let res = if let Some(command) = args.command {
        match command {
            Commands::Check(args) => handle_check(args),
            Commands::DirectVolume(args) => handle_direct_volume(args),
            Commands::Exec(args) => handle_exec(args),
            Commands::Env(args) => handle_env(args),
            Commands::Factory => handle_factory(),
            Commands::Iptables(args) => handle_iptables(args),
            Commands::Metrics(args) => handle_metrics(args),
            Commands::Monitor(args) => handle_monitor(args),
            Commands::Version => handle_version(),
            Commands::LogParser(args) => log_parser(args),
        }
    } else {
        // The user specified an option, but not a subcommand. We've already
        // handled show_default_config_paths, so this is an invalid CLI hence
        // display usage and exit.

        let help = KataCtlCli::command().render_help().to_string();

        eprintln!("ERROR: need command");

        eprintln!("{help}");

        // We need to exit here rather than returning an error to match clap's
        // standard behaviour.
        //
        // Note: the return value matches the clap-internal USAGE_CODE.
        exit(2);
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
