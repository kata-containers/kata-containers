// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

use std::error::Error as StdError;
use std::fmt;

/// The different types of errors that can occur while manipulating control groups.
#[derive(Debug, Eq, PartialEq)]
pub enum ErrorKind {
    FsError,
    Common(String),

    /// An error occured while writing to a control group file.
    WriteFailed,

    /// An error occured while trying to read from a control group file.
    ReadFailed,

    /// An error occured while trying to parse a value from a control group file.
    ///
    /// In the future, there will be some information attached to this field.
    ParseError,

    /// You tried to do something invalid.
    ///
    /// This could be because you tried to set a value in a control group that is not a root
    /// control group. Or, when using unified hierarchy, you tried to add a task in a leaf node.
    InvalidOperation,

    /// The path of the control group was invalid.
    ///
    /// This could be caused by trying to escape the control group filesystem via a string of "..".
    /// This crate checks against this and operations will fail with this error.
    InvalidPath,

    InvalidBytesSize,

    /// An unknown error has occured.
    Other,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    cause: Option<Box<StdError + Send + Sync>>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let msg = match &self.kind {
            ErrorKind::FsError => "fs error".to_string(),
            ErrorKind::Common(s) => s.clone(),
            ErrorKind::WriteFailed => "unable to write to a control group file".to_string(),
            ErrorKind::ReadFailed => "unable to read a control group file".to_string(),
            ErrorKind::ParseError => "unable to parse control group file".to_string(),
            ErrorKind::InvalidOperation => "the requested operation is invalid".to_string(),
            ErrorKind::InvalidPath => "the given path is invalid".to_string(),
            ErrorKind::InvalidBytesSize => "invalid bytes size".to_string(),
            ErrorKind::Other => "an unknown error".to_string(),
        };

        write!(f, "{}", msg)
    }
}

impl StdError for Error {
    fn cause(&self) -> Option<&StdError> {
        match self.cause {
            Some(ref x) => Some(&**x),
            None => None,
        }
    }
}

impl Error {
    pub(crate) fn from_string(s: String) -> Self {
        Self {
            kind: ErrorKind::Common(s),
            cause: None,
        }
    }
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self { kind, cause: None }
    }

    pub(crate) fn with_cause<E>(kind: ErrorKind, cause: E) -> Self
    where
        E: 'static + Send + Sync + StdError,
    {
        Self {
            kind,
            cause: Some(Box::new(cause)),
        }
    }

    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

pub type Result<T> = ::std::result::Result<T, Error>;
