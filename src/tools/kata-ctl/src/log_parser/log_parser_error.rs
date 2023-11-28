// Copyright (c) 2023 Gabe Venberg
//
// SPDX-License-Identifier: Apache-2.0

use std::{error::Error, path::PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LogParserError {
    #[error("Unknown Error")]
    Unknown(Box<dyn Error + Send + Sync>),

    #[error("Input file '{0}' cannot be found")]
    InputFileNotFound(PathBuf),

    #[error("Input file '{0}' does not contain any valid logs")]
    FileEmpty(PathBuf),

    #[error("No permission to open '{0}'")]
    InputFilePermissionError(PathBuf),

    #[error("No permission to write to '{0}'")]
    OutputFilePermissionError(PathBuf),

    #[error("Log parsing error: {0} with string {1}")]
    ParsingError(serde_json::Error, String),

    #[error("Error serializing {0}: {1}")]
    SerializationError(String, Box<dyn Error + Send + Sync>),

    #[error("No logs in any file")]
    NoRecordsError(),
}

impl PartialEq for LogParserError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unknown(l0), Self::Unknown(r0)) => l0.to_string() == r0.to_string(),
            (Self::InputFileNotFound(l0), Self::InputFileNotFound(r0)) => l0 == r0,
            (Self::FileEmpty(l0), Self::FileEmpty(r0)) => l0 == r0,
            (Self::InputFilePermissionError(l0), Self::InputFilePermissionError(r0)) => l0 == r0,
            (Self::OutputFilePermissionError(l0), Self::OutputFilePermissionError(r0)) => l0 == r0,
            //serde_json::Error does not impl partialeq, but for testing cases a quick and dirty
            //string comparison works well enough.
            (Self::ParsingError(l0, l1), Self::ParsingError(r0, r1)) => {
                l0.to_string() == r0.to_string() && l1 == r1
            }
            //this catch all returns whether the two enums are the same variant type. eg,
            //core::mem:discriminant(LogParserError::Unkown)==core:mem:discriminant(LogParserError::Unkown)
            //is true, but it would not be true, for say, Unkown and InputFilePermissionError.
            //Note that it only compares the variants, not the contents of the variants, hence the
            //need for the above branches.
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl Eq for LogParserError {}
