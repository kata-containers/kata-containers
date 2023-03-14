//! Protobuf error type

use std::error::Error;
use std::fmt;
use std::io;
use std::str;

use crate::wire_format::WireType;

/// `Result` alias for `ProtobufError`
pub type ProtobufResult<T> = Result<T, ProtobufError>;

/// Enum values added here for diagnostic purposes.
/// Users should not depend on specific values.
#[derive(Debug)]
pub enum WireError {
    /// Could not read complete message because stream is EOF
    UnexpectedEof,
    /// Wrong wire type for given field
    UnexpectedWireType(WireType),
    /// Incorrect tag value
    IncorrectTag(u32),
    /// Malformed map field
    IncompleteMap,
    /// Malformed varint
    IncorrectVarint,
    /// String is not valid UTD-8
    Utf8Error,
    /// Enum value is unknown
    InvalidEnumValue(i32),
    /// Message is too nested
    OverRecursionLimit,
    /// Could not read complete message because stream is EOF
    TruncatedMessage,
    /// Other error
    Other,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireError::Utf8Error => write!(f, "invalid UTF-8 sequence"),
            WireError::UnexpectedWireType(..) => write!(f, "unexpected wire type"),
            WireError::InvalidEnumValue(..) => write!(f, "invalid enum value"),
            WireError::IncorrectTag(..) => write!(f, "incorrect tag"),
            WireError::IncorrectVarint => write!(f, "incorrect varint"),
            WireError::IncompleteMap => write!(f, "incomplete map"),
            WireError::UnexpectedEof => write!(f, "unexpected EOF"),
            WireError::OverRecursionLimit => write!(f, "over recursion limit"),
            WireError::TruncatedMessage => write!(f, "truncated message"),
            WireError::Other => write!(f, "other error"),
        }
    }
}

/// Generic protobuf error
#[derive(Debug)]
pub enum ProtobufError {
    /// I/O error when reading or writing
    IoError(io::Error),
    /// Malformed input
    WireError(WireError),
    /// Protocol contains a string which is not valid UTF-8 string
    Utf8(str::Utf8Error),
    /// Not all required fields set
    MessageNotInitialized {
        /// Message name
        message: &'static str,
    },
}

impl ProtobufError {
    /// Create message not initialized error.
    #[doc(hidden)]
    pub fn message_not_initialized(message: &'static str) -> ProtobufError {
        ProtobufError::MessageNotInitialized { message: message }
    }
}

impl fmt::Display for ProtobufError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            // not sure that cause should be included in message
            &ProtobufError::IoError(ref e) => write!(f, "IO error: {}", e),
            &ProtobufError::WireError(ref e) => fmt::Display::fmt(e, f),
            &ProtobufError::Utf8(ref e) => write!(f, "{}", e),
            &ProtobufError::MessageNotInitialized { .. } => write!(f, "not all message fields set"),
        }
    }
}

impl Error for ProtobufError {
    #[allow(deprecated)] // call to `description`
    fn description(&self) -> &str {
        match self {
            // not sure that cause should be included in message
            &ProtobufError::IoError(ref e) => e.description(),
            &ProtobufError::WireError(ref e) => match *e {
                WireError::Utf8Error => "invalid UTF-8 sequence",
                WireError::UnexpectedWireType(..) => "unexpected wire type",
                WireError::InvalidEnumValue(..) => "invalid enum value",
                WireError::IncorrectTag(..) => "incorrect tag",
                WireError::IncorrectVarint => "incorrect varint",
                WireError::IncompleteMap => "incomplete map",
                WireError::UnexpectedEof => "unexpected EOF",
                WireError::OverRecursionLimit => "over recursion limit",
                WireError::TruncatedMessage => "truncated message",
                WireError::Other => "other error",
            },
            &ProtobufError::Utf8(ref e) => &e.description(),
            &ProtobufError::MessageNotInitialized { .. } => "not all message fields set",
        }
    }

    fn cause(&self) -> Option<&dyn Error> {
        match self {
            &ProtobufError::IoError(ref e) => Some(e),
            &ProtobufError::Utf8(ref e) => Some(e),
            &ProtobufError::WireError(..) => None,
            &ProtobufError::MessageNotInitialized { .. } => None,
        }
    }
}

impl From<io::Error> for ProtobufError {
    fn from(err: io::Error) -> Self {
        ProtobufError::IoError(err)
    }
}

impl From<str::Utf8Error> for ProtobufError {
    fn from(err: str::Utf8Error) -> Self {
        ProtobufError::Utf8(err)
    }
}

impl From<ProtobufError> for io::Error {
    fn from(err: ProtobufError) -> Self {
        match err {
            ProtobufError::IoError(e) => e,
            ProtobufError::WireError(e) => {
                io::Error::new(io::ErrorKind::InvalidData, ProtobufError::WireError(e))
            }
            ProtobufError::MessageNotInitialized { message: msg } => io::Error::new(
                io::ErrorKind::InvalidInput,
                ProtobufError::MessageNotInitialized { message: msg },
            ),
            e => io::Error::new(io::ErrorKind::Other, Box::new(e)),
        }
    }
}
