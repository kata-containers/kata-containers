// Copyright (c) 2023 Gabe Venberg
//
// SPDX-License-Identifier: Apache-2.0

use crate::args::LogParser;
use crate::log_parser::{log_message::AnyLogMessage, log_parser_error::LogParserError};

/// Calls functions to either check for errors, or to filter them out, discarding them or printing
/// them to stderr.
///
/// # Errors
///
///  If cli.strict is true, will return the first error it finds in input (if any)
pub(crate) fn filter_errors<O: AnyLogMessage>(
    input: Vec<Result<O, LogParserError>>,
    cli: &LogParser,
) -> Result<Vec<O>, LogParserError> {
    if cli.strict {
        find_errors(input)
    } else if cli.quiet {
        Ok(filter_errors_quiet(input))
    } else {
        Ok(filter_errors_to_stderr(input))
    }
}

/// checks if any errors are in the passed log vector. If there are none, returns a vec of O
/// (stripping the containing Result). If there are some, returns the first found error.
fn find_errors<O: AnyLogMessage>(
    input: Vec<Result<O, LogParserError>>,
) -> Result<Vec<O>, LogParserError> {
    // yes, this is only the one line, but the behavior of collect() here is non-obvious. In short,
    // as Result implements IntoIter, you can go from a `Vec<Result<T, E>` to a `Result<Vec<T>, E>`
    // with just collect()
    input.into_iter().collect()
}

/// removes all LogParserErrors, returning a vec of just O
fn filter_errors_quiet<O: AnyLogMessage>(input: Vec<Result<O, LogParserError>>) -> Vec<O> {
    input.into_iter().filter_map(|l| l.ok()).collect()
}

/// removes all LogParserErrors, returning a vec of just O, and prints any errors to stderr.
fn filter_errors_to_stderr<O: AnyLogMessage>(input: Vec<Result<O, LogParserError>>) -> Vec<O> {
    input
        .into_iter()
        .filter_map(|l| match l {
            Ok(log) => Some(log),
            Err(e) => {
                eprintln!("{}", e);
                None
            }
        })
        .collect()
}

/// does what it says on the tin. Sorts a vec of logs in place by their timestamp.
pub(crate) fn sort_logs<O: AnyLogMessage>(input: &mut [O]) {
    input.sort_by_key(|l| l.get_timestamp());
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::log_parser::log_message::LogMessage;
    #[test]
    fn error_filter() {
        let unclean_logs = vec![
            Ok(LogMessage::default()),
            Err(LogParserError::SerializationError(
                "test error".to_string(),
                Box::new(slog::Error::Fmt(std::fmt::Error)),
            )),
        ];
        let logs = vec![LogMessage::default()];
        assert_eq!(filter_errors_quiet(unclean_logs), logs)
    }

    #[test]
    fn error_filter_to_stderr() {
        let unclean_logs = vec![
            Ok(LogMessage::default()),
            Err(LogParserError::SerializationError(
                "test error".to_string(),
                Box::new(slog::Error::Fmt(std::fmt::Error)),
            )),
        ];
        let logs = vec![LogMessage::default()];
        assert_eq!(filter_errors_to_stderr(unclean_logs), logs)
    }
}
