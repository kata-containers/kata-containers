//! Error types

use core::fmt;

/// Result type.
pub type Result<T> = core::result::Result<T, Error>;

/// Error type.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// Base64-related errors.
    Base64,

    /// Character encoding-related errors.
    CharacterEncoding,

    /// Errors in the encapsulated text (which aren't specifically Base64-related).
    EncapsulatedText,

    /// Header detected in the encapsulated text.
    HeaderDisallowed,

    /// Invalid label.
    Label,

    /// Invalid length.
    Length,

    /// "Preamble" (text before pre-encapsulation boundary) contains invalid data.
    Preamble,

    /// Errors in the pre-encapsulation boundary.
    PreEncapsulationBoundary,

    /// Errors in the post-encapsulation boundary.
    PostEncapsulationBoundary,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Error::Base64 => "PEM Base64 error",
            Error::CharacterEncoding => "PEM character encoding error",
            Error::EncapsulatedText => "PEM error in encapsulated text",
            Error::HeaderDisallowed => "PEM headers disallowed by RFC7468",
            Error::Label => "PEM type label invalid",
            Error::Length => "PEM length invalid",
            Error::Preamble => "PEM preamble contains invalid data (NUL byte)",
            Error::PreEncapsulationBoundary => "PEM error in pre-encapsulation boundary",
            Error::PostEncapsulationBoundary => "PEM error in post-encapsulation boundary",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

impl From<base64ct::Error> for Error {
    fn from(_: base64ct::Error) -> Error {
        Error::Base64
    }
}

impl From<base64ct::InvalidLengthError> for Error {
    fn from(_: base64ct::InvalidLengthError) -> Error {
        Error::Length
    }
}
