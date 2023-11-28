// Copyright (c) 2023 Gabe Venberg
//
// SPDX-License-Identifier: Apache-2.0

#![warn(missing_debug_implementations)]

mod log_message;
mod log_parser_error;
mod output_file;
mod parse_file;
mod process_logs;

use crate::args::LogParser;
use anyhow::Context;
use log_message::AnyLogMessage;
use log_message::LogMessage;
use log_message::StrictLogMessage;
use log_parser_error::LogParserError;
use output_file::*;
use parse_file::*;
use process_logs::*;

fn handle_logs<T: AnyLogMessage>(cli: LogParser) -> Result<(), LogParserError> {
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
pub fn log_parser(args: LogParser) -> anyhow::Result<()> {
    if args.ignore_missing_fields {
        handle_logs::<LogMessage>(args)
    } else {
        handle_logs::<StrictLogMessage>(args)
    }
    .context("Could not parse logs")
}
