//! Error handling.

use thiserror::Error;

/// Library errors.
#[derive(Error, Debug)]
#[error("caps error: {0}")]
pub struct CapsError(pub(crate) String);

impl From<&str> for CapsError {
    fn from(arg: &str) -> Self {
        Self(arg.to_string())
    }
}

impl From<String> for CapsError {
    fn from(arg: String) -> Self {
        Self(arg)
    }
}
