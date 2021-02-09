//! Protobuf encoding and decoding errors.

use std::borrow::Cow;
use std::error;
use std::fmt;
use std::io;

/// A Protobuf message decoding error.
///
/// `DecodeError` indicates that the input buffer does not caontain a valid
/// Protobuf message. The error details should be considered 'best effort': in
/// general it is not possible to exactly pinpoint why data is malformed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError {
    /// A 'best effort' root cause description.
    description: Cow<'static, str>,
    /// A stack of (message, field) name pairs, which identify the specific
    /// message type and field where decoding failed. The stack contains an
    /// entry per level of nesting.
    stack: Vec<(&'static str, &'static str)>,
}

impl DecodeError {
    /// Creates a new `DecodeError` with a 'best effort' root cause description.
    ///
    /// Meant to be used only by `Message` implementations.
    #[doc(hidden)]
    pub fn new<S>(description: S) -> DecodeError
    where
        S: Into<Cow<'static, str>>,
    {
        DecodeError {
            description: description.into(),
            stack: Vec::new(),
        }
    }

    /// Pushes a (message, field) name location pair on to the location stack.
    ///
    /// Meant to be used only by `Message` implementations.
    #[doc(hidden)]
    pub fn push(&mut self, message: &'static str, field: &'static str) {
        self.stack.push((message, field));
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("failed to decode Protobuf message: ")?;
        for &(message, field) in &self.stack {
            write!(f, "{}.{}: ", message, field)?;
        }
        f.write_str(&self.description)
    }
}

impl error::Error for DecodeError {
    fn description(&self) -> &str {
        &self.description
    }
}

impl From<DecodeError> for io::Error {
    fn from(error: DecodeError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}

/// A Protobuf message encoding error.
///
/// `EncodeError` always indicates that a message failed to encode because the
/// provided buffer had insufficient capacity. Message encoding is otherwise
/// infallible.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct EncodeError {
    required: usize,
    remaining: usize,
}

impl EncodeError {
    /// Creates a new `EncodeError`.
    pub(crate) fn new(required: usize, remaining: usize) -> EncodeError {
        EncodeError {
            required,
            remaining,
        }
    }

    /// Returns the required buffer capacity to encode the message.
    pub fn required_capacity(&self) -> usize {
        self.required
    }

    /// Returns the remaining length in the provided buffer at the time of encoding.
    pub fn remaining(&self) -> usize {
        self.remaining
    }
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(error::Error::description(self))?;
        write!(
            f,
            " (required: {}, remaining: {})",
            self.required, self.remaining
        )
    }
}

impl error::Error for EncodeError {
    fn description(&self) -> &str {
        "failed to encode Protobuf message: insufficient buffer capacity"
    }
}

impl From<EncodeError> for io::Error {
    fn from(error: EncodeError) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, error)
    }
}
