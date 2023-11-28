// Copyright (c) 2023 Gabe Venberg
//
// SPDX-License-Identifier: Apache-2.0

use crate::log_parser::{log_message::AnyLogMessage, log_parser_error::LogParserError};
use std::{fs, io::ErrorKind, path::Path};

/// Reads the entire file into memory as a string.
///
/// # Errors
///
/// This function will return an error if opening the file returns an error.
/// FileNotFound and FilePermissionError *should* be the only error types that can happen, and they
/// are wrapped in the appropriate LogParserError types. Any other error will panic.
///
/// # Panic
///
/// Will panic if File::open returns an error other than NotFound or PermissionDenied.
/// This *should* not happen.
///
/// # Gochas
///
/// Memory use can be unexpectedly high, as entire file is read into memory.
pub(crate) fn open_file_into_memory(inputfile: &Path) -> Result<String, LogParserError> {
    match fs::read_to_string(inputfile) {
        Ok(s) => Ok(s),
        Err(e) => match e.kind() {
            ErrorKind::NotFound => Err(LogParserError::InputFileNotFound(inputfile.to_path_buf())),
            ErrorKind::PermissionDenied => Err(LogParserError::InputFilePermissionError(
                inputfile.to_path_buf(),
            )),
            _ => Err(LogParserError::Unknown(Box::new(e))),
        },
    }
}

/// Parses a series of logs from a string, returning an array of log messages and parsing errors.
///
/// # Errors
///
/// Will pass any errors from serde in to the output array
pub(crate) fn parse_log<O>(in_file: String) -> Vec<Result<O, LogParserError>>
where
    O: AnyLogMessage,
{
    let mut output = Vec::new();
    for line in in_file.lines() {
        let logentry = serde_json::from_str::<O>(line)
            .map_err(|e| LogParserError::ParsingError(e, line.to_string()));
        output.push(logentry);
    }
    output
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::log_parser::log_message::LogMessage;

    #[test]
    fn parse_strings() {
        let log = r#"{"msg":"vmm-master thread is uninitialized or has exited.","level":"DEBG","ts":"2023-03-15T14:17:02.526992506Z","pid":"3327263","version":"0.1.0","name":"kata-runtime","subsystem":"hypervisor","source":"foo"}
{"msg":"resource clean up","level":"INFO","ts":"2023-03-15T14:17:02.527047136Z","subsystem":"virt-container","name":"kata-runtime","pid":"3327263","version":"0.1.0","source":"foo"}"#;
        let result = vec![
            serde_json::from_str(r#"{"msg":"vmm-master thread is uninitialized or has exited.","level":"DEBG","ts":"2023-03-15T14:17:02.526992506Z","pid":"3327263","version":"0.1.0","name":"kata-runtime","subsystem":"hypervisor","source":"foo"}"#).map_err(|e| LogParserError::ParsingError(e, r#"Will not happen"#.to_string())),
            serde_json::from_str(r#"{"msg":"resource clean up","level":"INFO","ts":"2023-03-15T14:17:02.527047136Z","subsystem":"virt-container","name":"kata-runtime","pid":"3327263","version":"0.1.0","source":"foo"}"#).map_err(|e| LogParserError::ParsingError(e, r#"Will not happen"#.to_string())),
            ];
        assert_eq!(parse_log::<LogMessage>(log.into()), result)
    }
    #[test]
    fn parse_mixed() {
        let log = r#"{"msg":"vmm-master thread is uninitialized or has exited.","level":"DEBG","ts":"2023-03-15T14:17:02.526992506Z","pid":"3327263","version":"0.1.0","name":"kata-runtime","subsystem":"hypervisor","source":"foo"}
Random Kernel Message"#;
        let result = vec![
            serde_json::from_str(r#"{"msg":"vmm-master thread is uninitialized or has exited.","level":"DEBG","ts":"2023-03-15T14:17:02.526992506Z","pid":"3327263","version":"0.1.0","name":"kata-runtime","subsystem":"hypervisor","source":"foo"}"#).map_err(|e| LogParserError::ParsingError(e, r#"Will not happen"#.to_string())),

                Err(LogParserError::ParsingError(
                    serde_json::from_str::<LogMessage>("Random Kernel Message")
                        .err()
                        .unwrap()
                , "Random Kernel Message".to_string())),
            ];
        assert_eq!(parse_log::<LogMessage>(log.into()), result)
    }
}
