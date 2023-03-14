#[cfg(feature = "std")]
use std::error::Error;

use alloc::fmt::{self, Display, Formatter};
use alloc::string::String;

#[derive(Debug, Clone, PartialEq, Eq)]
/// Different error types for `Byte` and `ByteUnit`.
pub enum ByteError {
    ValueIncorrect(String),
    UnitIncorrect(String),
}

impl Display for ByteError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str(self.as_ref())
    }
}

#[cfg(feature = "std")]
impl Error for ByteError {}

impl AsRef<str> for ByteError {
    #[inline]
    fn as_ref(&self) -> &str {
        match &self {
            ByteError::ValueIncorrect(s) => s,
            ByteError::UnitIncorrect(s) => s,
        }
    }
}

impl Into<String> for ByteError {
    #[inline]
    fn into(self) -> String {
        match self {
            ByteError::ValueIncorrect(s) => s,
            ByteError::UnitIncorrect(s) => s,
        }
    }
}
