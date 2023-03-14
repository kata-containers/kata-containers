// Copyright 2017 Jonathan Creekmore
//
// Licensed under the MIT license <LICENSE.md or
// http://opensource.org/licenses/MIT>. This file may not be
// copied, modified, or distributed except according to those terms.
use std::error::Error;
use std::fmt;

/// The `pem` error type.
#[derive(Debug, Eq, PartialEq)]
#[allow(missing_docs)]
pub enum PemError {
    MismatchedTags(String, String),
    MalformedFraming,
    MissingBeginTag,
    MissingEndTag,
    MissingData,
    InvalidData(::base64::DecodeError),
    NotUtf8(::std::str::Utf8Error),
}

impl fmt::Display for PemError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PemError::MismatchedTags(b, e) => {
                write!(f, "mismatching BEGIN (\"{}\") and END (\"{}\") tags", b, e)
            }
            PemError::MalformedFraming => write!(f, "malformedframing"),
            PemError::MissingBeginTag => write!(f, "missing BEGIN tag"),
            PemError::MissingEndTag => write!(f, "missing END tag"),
            PemError::MissingData => write!(f, "missing data"),
            PemError::InvalidData(e) => write!(f, "invalid data: {}", e),
            PemError::NotUtf8(e) => write!(f, "invalid utf-8 value: {}", e),
        }
    }
}

impl Error for PemError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            // Errors originating from other libraries.
            PemError::InvalidData(e) => Some(e),
            PemError::NotUtf8(e) => Some(e),
            // Errors directly originating from `pem-rs`.
            _ => None,
        }
    }
}

/// The `pem` result type.
pub type Result<T> = ::std::result::Result<T, PemError>;
