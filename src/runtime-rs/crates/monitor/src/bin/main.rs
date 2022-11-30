// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{bail, Context, Result};
use monitor::http_server::http_server;
use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

pub enum Action {
    Run(Args),
    Help,
}

pub struct Args {
    pub socket_addr: String,
    pub loglevel: slog::Level,
}

const BIN_NAME: &str = "kata_monitor";

const DEFAULT_SOCKET_ADDR: &str = "127.0.0.1:8090";
const DEFAULT_LOG_LEVEL: &str = "info";

fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();

    let action = parse_args(&args).context("parse args")?;

    match action {
        // TODO: log
        Action::Run(args) => http_server(&args.socket_addr),
        Action::Help => show_help(&args[0]),
    }
}

fn parse_args(args: &[OsString]) -> Result<Action> {
    let mut socket_addr = DEFAULT_SOCKET_ADDR.to_string();
    let mut log_level = DEFAULT_LOG_LEVEL.to_string();
    let mut help = false;

    go_flag::parse_args_with_warnings::<String, _, _>(&args[1..], None, |flags| {
        flags.add_flag("address", &mut socket_addr);
        flags.add_flag("loglevel", &mut log_level);
        flags.add_flag("help", &mut help);
    })?;

    if help {
        Ok(Action::Help)
    } else {
        Ok(Action::Run(Args {
            socket_addr,
            loglevel: parse_loglevel(log_level.as_str())?,
        }))
    }
}

fn show_help(cmd: &OsStr) -> Result<()> {
    let path = PathBuf::from(cmd);
    let name = match path.file_name() {
        Some(v) => v.to_str(),
        None => None,
    };

    let name = name.unwrap_or(BIN_NAME);

    println!(
        r#"Usage of {}:
  -listen-address string
        The address to listen on for HTTP requests. (default "127.0.0.1:8090")
  -log-level string
        Log level of logrus(trace/debug/info/warn/error/fatal/panic). (default "info")
"#,
        name
    );

    Ok(())
}

fn parse_loglevel(loglevel_str: &str) -> Result<slog::Level> {
    let level = match loglevel_str {
        "fatal" | "panic" => slog::Level::Critical,
        "critical" => slog::Level::Critical,
        "error" => slog::Level::Error,
        "warn" | "warning" => slog::Level::Warning,
        "info" => slog::Level::Info,
        "debug" => slog::Level::Debug,
        "trace" => slog::Level::Trace,
        _ => bail!("invalid log level"),
    };

    Ok(level)
}
