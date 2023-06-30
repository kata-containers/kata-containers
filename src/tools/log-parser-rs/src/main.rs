// Copyright (c) 2023 Gabe Venberg
//
// SPDX-License-Identifier: Apache-2.0

#![warn(unused_crate_dependencies)]
#![warn(missing_debug_implementations)]

mod args;
mod log_message;
mod log_parser_error;
mod output_file;
mod parse_file;
mod process_logs;

use crate::args::Cli;
use crate::log_message::StrictLogMessage;
use crate::log_parser_error::LogParserError;
use crate::output_file::*;
use crate::parse_file::*;
use crate::process_logs::*;
use clap::Parser;
use log_message::AnyLogMessage;
use log_message::LogMessage;
use std::process::exit;

fn handle_logs<T: AnyLogMessage>(cli: Cli) -> Result<(), LogParserError> {
    let mut logs = Vec::new();

    for file in &cli.input_file {
        let in_file = open_file_into_memory(file)?;
        let file_logs = filter_errors(parse_log::<T>(in_file), &cli)?;

        if cli.error_if_file_empty && file_logs.is_empty() {
            return Err(LogParserError::FileEmpty(file.to_path_buf()));
        }

        logs.extend(file_logs)
    }

    if cli.error_if_no_records && logs.is_empty() {
        return Err(LogParserError::NoRecordsError());
    }
    if cli.check_only {
        return Ok(());
    }

    sort_logs(&mut logs);
    output_file(logs, &cli)?;
    Ok(())
}

//needed another layer of function call in order to genericize over both LogMessage and
//StrictLogMessage.
fn real_main() -> std::result::Result<(), LogParserError> {
    let cli = Cli::parse();
    if cli.ignore_missing_fields {
        handle_logs::<LogMessage>(cli)
    } else {
        handle_logs::<StrictLogMessage>(cli)
    }
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("ERROR: {:#}", e);
        exit(1);
    }
}
