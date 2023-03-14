use crate::{Class, Tag};
use alloc::str;
use alloc::string;
use alloc::string::String;
use displaydoc::Display;
use nom::error::{ErrorKind, FromExternalError, ParseError};
use nom::IResult;
#[cfg(feature = "std")]
use std::io;
#[cfg(feature = "std")]
use thiserror::Error;

#[cfg(feature = "std")]
impl std::error::Error for DerConstraint {}

#[derive(Clone, Copy, Debug, Display, PartialEq)]
/// Error types for DER constraints
pub enum DerConstraint {
    /// Indefinite length not allowed
    IndefiniteLength,
    /// Object must not be constructed
    Constructed,
    /// Object must be constructed
    NotConstructed,
    /// DateTime object is missing timezone
    MissingTimeZone,
    /// DateTime object is missing seconds
    MissingSeconds,
    /// Bitstring unused bits must be set to zero
    UnusedBitsNotZero,
    /// Boolean value must be 0x00 of 0xff
    InvalidBoolean,
    /// Integer must not be empty
    IntegerEmpty,
    /// Leading zeroes in Integer encoding
    IntegerLeadingZeroes,
    /// Leading 0xff in negative Integer encoding
    IntegerLeadingFF,
}

// XXX
// thiserror does not work in no_std
// see https://github.com/dtolnay/thiserror/pull/64

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// The error type for operations of the [`FromBer`](crate::FromBer),
/// [`FromDer`](crate::FromDer), and associated traits.
#[derive(Clone, Debug, Display, PartialEq)]
// #[cfg_attr(feature = "std", derive(Error))]
pub enum Error {
    /// BER object does not have the expected type
    BerTypeError,
    /// BER object does not have the expected value
    BerValueError,
    /// Invalid Length
    InvalidLength,
    /// Invalid Value when parsing object with tag {tag:?} {msg:}
    InvalidValue { tag: Tag, msg: String },
    /// Invalid Tag
    InvalidTag,
    /// Unknown tag: {0:?}
    UnknownTag(u32),
    /// Unexpected Tag (expected: {expected:?}, actual: {actual:?})
    UnexpectedTag { expected: Option<Tag>, actual: Tag },
    /// Unexpected Class (expected: {expected:?}, actual: {actual:?})
    UnexpectedClass {
        expected: Option<Class>,
        actual: Class,
    },

    /// Indefinite length not allowed
    IndefiniteLengthUnexpected,

    /// DER object was expected to be constructed (and found to be primitive)
    ConstructExpected,
    /// DER object was expected to be primitive (and found to be constructed)
    ConstructUnexpected,

    /// Integer too large to fit requested type
    IntegerTooLarge,
    /// BER integer is negative, while an unsigned integer was requested
    IntegerNegative,
    /// BER recursive parsing reached maximum depth
    BerMaxDepth,

    /// Invalid encoding or forbidden characters in string
    StringInvalidCharset,
    /// Invalid Date or Time
    InvalidDateTime,

    /// DER Failed constraint
    DerConstraintFailed(DerConstraint),

    /// Requesting borrowed data from a temporary object
    LifetimeError,
    /// Feature is not yet implemented
    Unsupported,

    /// incomplete data, missing: {0:?}
    Incomplete(nom::Needed),

    /// nom error: {0:?}
    NomError(ErrorKind),
}

impl Error {
    /// Build an error from the provided invalid value
    #[inline]
    pub const fn invalid_value(tag: Tag, msg: String) -> Self {
        Self::InvalidValue { tag, msg }
    }

    /// Build an error from the provided unexpected class
    #[inline]
    pub const fn unexpected_class(expected: Option<Class>, actual: Class) -> Self {
        Self::UnexpectedClass { expected, actual }
    }

    /// Build an error from the provided unexpected tag
    #[inline]
    pub const fn unexpected_tag(expected: Option<Tag>, actual: Tag) -> Self {
        Self::UnexpectedTag { expected, actual }
    }
}

impl<'a> ParseError<&'a [u8]> for Error {
    fn from_error_kind(_input: &'a [u8], kind: ErrorKind) -> Self {
        Error::NomError(kind)
    }
    fn append(_input: &'a [u8], kind: ErrorKind, _other: Self) -> Self {
        Error::NomError(kind)
    }
}

impl From<Error> for nom::Err<Error> {
    fn from(e: Error) -> Self {
        nom::Err::Error(e)
    }
}

impl From<str::Utf8Error> for Error {
    fn from(_: str::Utf8Error) -> Self {
        Error::StringInvalidCharset
    }
}

impl From<string::FromUtf8Error> for Error {
    fn from(_: string::FromUtf8Error) -> Self {
        Error::StringInvalidCharset
    }
}

impl From<string::FromUtf16Error> for Error {
    fn from(_: string::FromUtf16Error) -> Self {
        Error::StringInvalidCharset
    }
}

impl From<nom::Err<Error>> for Error {
    fn from(e: nom::Err<Error>) -> Self {
        match e {
            nom::Err::Incomplete(n) => Self::Incomplete(n),
            nom::Err::Error(e) | nom::Err::Failure(e) => e,
        }
    }
}

impl<I, E> FromExternalError<I, E> for Error {
    fn from_external_error(_input: I, kind: ErrorKind, _e: E) -> Error {
        Error::NomError(kind)
    }
}

/// Holds the result of BER/DER serialization functions
pub type ParseResult<'a, T, E = Error> = IResult<&'a [u8], T, E>;

/// A specialized `Result` type for all operations from this crate.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// The error type for serialization operations of the [`ToDer`](crate::ToDer) trait.
#[cfg(feature = "std")]
#[derive(Debug, Error)]
pub enum SerializeError {
    #[error("ASN.1 error: {0:?}")]
    ASN1Error(#[from] Error),

    #[error("Invalid Class {class:}")]
    InvalidClass { class: u8 },

    #[error("Invalid Length")]
    InvalidLength,

    #[error("I/O error: {0:?}")]
    IOError(#[from] io::Error),
}

#[cfg(feature = "std")]
/// Holds the result of BER/DER encoding functions
pub type SerializeResult<T> = std::result::Result<T, SerializeError>;
