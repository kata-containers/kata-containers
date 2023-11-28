// Copyright (c) 2023 Gabe Venberg
//
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::File,
    io::{ErrorKind, Write},
    path::Path,
};

use crate::args::LogParser;
use crate::log_parser::{log_message::AnyLogMessage, log_parser_error::LogParserError};

/// a simple dispatcher method that outputs the deserialized result of the parsed logs according to
/// the CLI arguments.
///
/// # Errors
///
/// If outputting to a file, may return a LogParserError having to do with opening and writing to
/// the output file.
pub(crate) fn output_file<O: AnyLogMessage>(
    contents: Vec<O>,
    options: &LogParser,
) -> Result<(), LogParserError> {
    let serializer = choose_formatting(options);
    if let Some(out_file) = &options.output_file {
        write_logs_to_file(out_file, contents, serializer)?;
    } else {
        print_logs(contents, serializer)?;
    };
    Ok(())
}

fn choose_formatting<O: AnyLogMessage>(
    options: &LogParser,
) -> fn(Vec<O>) -> Result<String, LogParserError> {
    match options.output_format {
        crate::args::LogOutputFormat::Csv => serialize_csv,
        crate::args::LogOutputFormat::Json => serialize_json,
        crate::args::LogOutputFormat::Ron => serialize_ron,
        crate::args::LogOutputFormat::Text => serialize_text,
        crate::args::LogOutputFormat::Toml => serialize_toml,
        crate::args::LogOutputFormat::Xml => serialize_xml,
        crate::args::LogOutputFormat::Yaml => serialize_yaml,
    }
}

fn serialize_text<O: AnyLogMessage>(input: Vec<O>) -> Result<String, LogParserError> {
    Ok(input
        .iter()
        .map(|r| Ok(format!("{:?}", r)))
        .collect::<Result<Vec<String>, LogParserError>>()?
        .join("\n"))
}

fn serialize_json<O: AnyLogMessage>(input: Vec<O>) -> Result<String, LogParserError> {
    Ok(input
        .iter()
        .map(|r| {
            serde_json::to_string(r)
                .map_err(|e| LogParserError::SerializationError(format!("{:?}", r), Box::new(e)))
        })
        .collect::<Result<Vec<String>, LogParserError>>()?
        .join("\n"))
}

fn serialize_toml<O: AnyLogMessage>(input: Vec<O>) -> Result<String, LogParserError> {
    Ok(input
        .iter()
        .map(|r| {
            toml::to_string(r)
                .map_err(|e| LogParserError::SerializationError(format!("{:?}", r), Box::new(e)))
        })
        .collect::<Result<Vec<String>, LogParserError>>()?
        .join("\n"))
}

fn serialize_yaml<O: AnyLogMessage>(input: Vec<O>) -> Result<String, LogParserError> {
    Ok(input
        .iter()
        .map(|r| {
            serde_yaml::to_string(r)
                .map_err(|e| LogParserError::SerializationError(format!("{:?}", r), Box::new(e)))
        })
        .collect::<Result<Vec<String>, LogParserError>>()?
        .join("\n"))
}

fn serialize_xml<O: AnyLogMessage>(input: Vec<O>) -> Result<String, LogParserError> {
    Ok(input
        .iter()
        .map(|r| {
            quick_xml::se::to_string(r)
                .map_err(|e| LogParserError::SerializationError(format!("{:?}", r), Box::new(e)))
        })
        .collect::<Result<Vec<String>, LogParserError>>()?
        .join("\n"))
}

fn serialize_ron<O: AnyLogMessage>(input: Vec<O>) -> Result<String, LogParserError> {
    Ok(input
        .iter()
        .map(|r| {
            ron::to_string(r)
                .map_err(|e| LogParserError::SerializationError(format!("{:?}", r), Box::new(e)))
        })
        .collect::<Result<Vec<String>, LogParserError>>()?
        .join("\n"))
}

fn serialize_csv<O: AnyLogMessage>(input: Vec<O>) -> Result<String, LogParserError> {
    let mut csv_writer = csv::Writer::from_writer(vec![]);
    for record in input {
        csv_writer.serialize(&record).map_err(|e| {
            LogParserError::SerializationError(format!("{:?}", record), Box::new(e))
        })?;
    }
    String::from_utf8(
        csv_writer
            .into_inner()
            .map_err(|e| LogParserError::Unknown(Box::new(e)))?,
    )
    .map_err(|e| LogParserError::Unknown(Box::new(e)))
}

fn write_logs_to_file<O: AnyLogMessage>(
    out_file: &Path,
    contents: Vec<O>,
    serializer: fn(Vec<O>) -> Result<String, LogParserError>,
) -> Result<(), LogParserError> {
    let mut file = File::create(out_file).map_err(|e| map_out_io_err(e, out_file))?;
    file.write_all(serializer(contents)?.as_bytes())
        .map_err(|e| LogParserError::Unknown(Box::new(e)))?;
    Ok(())
}

fn print_logs<O: AnyLogMessage>(
    contents: Vec<O>,
    serializer: fn(Vec<O>) -> Result<String, LogParserError>,
) -> Result<(), LogParserError> {
    print!("{}", serializer(contents)?);
    Ok(())
}

fn map_out_io_err(e: std::io::Error, out_file: &Path) -> LogParserError {
    match e.kind() {
        ErrorKind::PermissionDenied => LogParserError::OutputFilePermissionError(out_file.into()),
        _ => LogParserError::Unknown(Box::new(e)),
    }
}
