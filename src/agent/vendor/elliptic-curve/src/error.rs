//! Error type.

use core::fmt::{self, Display};

/// Result type.
pub type Result<T> = core::result::Result<T, Error>;

/// Elliptic curve errors.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Error;

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("crypto error")
    }
}

#[cfg(feature = "pkcs8")]
impl From<pkcs8::Error> for Error {
    fn from(_: pkcs8::Error) -> Error {
        Error
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
