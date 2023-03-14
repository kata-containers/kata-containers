// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

use std::error::Error as StdError;
use std::fmt;

/// The different types of errors that can occur while manipulating control groups.
#[derive(thiserror::Error, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    #[error("fs error")]
    FsError,

    #[error("common error: {0}")]
    Common(String),

    /// An error occured while writing to a control group file.
    #[error("unable to write to a control group file {0}, value {1}")]
    WriteFailed(String, String),

    /// An error occured while trying to read from a control group file.
    #[error("unable to read a control group file {0}")]
    ReadFailed(String),

    /// An error occured while trying to remove a control group.
    #[error("unable to remove a control group")]
    RemoveFailed,

    /// An error occured while trying to parse a value from a control group file.
    ///
    /// In the future, there will be some information attached to this field.
    #[error("unable to parse control group file")]
    ParseError,

    /// You tried to do something invalid.
    ///
    /// This could be because you tried to set a value in a control group that is not a root
    /// control group. Or, when using unified hierarchy, you tried to add a task in a leaf node.
    #[error("the requested operation is invalid")]
    InvalidOperation,

    /// The path of the control group was invalid.
    ///
    /// This could be caused by trying to escape the control group filesystem via a string of "..".
    /// This crate checks against this and operations will fail with this error.
    #[error("the given path is invalid")]
    InvalidPath,

    #[error("invalid bytes size")]
    InvalidBytesSize,

    /// The specified controller is not in the list of supported controllers.
    #[error("specified controller is not in the list of supported controllers")]
    SpecifiedControllers,

    /// Using method in wrong cgroup version.
    #[error("using method in wrong cgroup version")]
    CgroupVersion,

    /// Subsystems is empty.
    #[error("subsystems is empty")]
    SubsystemsEmpty,

    /// An unknown error has occured.
    #[error("an unknown error")]
    Other,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    cause: Option<Box<dyn StdError + Send + Sync>>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(cause) = &self.cause {
            write!(f, "{} caused by: {:?}", &self.kind, cause)
        } else {
            write!(f, "{}", &self.kind)
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        #[allow(clippy::manual_map)]
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
