//! Error types of the crate.

use std::{borrow::Cow, io};
use thiserror::Error;

/// Spezialized result type for oci spec operations. It is
/// used for any operation that might produce an error. This
/// typedef is generally used to avoid writing out
/// [OciSpecError] directly and is otherwise a direct mapping
/// to [Result](std::result::Result).
pub type Result<T> = std::result::Result<T, OciSpecError>;

/// Error type for oci spec errors.
#[derive(Error, Debug)]
pub enum OciSpecError {
    /// Will be returned if an error occurs that cannot
    /// be mapped to a more specialized error variant.
    #[error("{0}")]
    Other(String),

    /// Will be returned when an error happens during
    /// io operations.
    #[error("io operation failed")]
    Io(#[from] io::Error),

    /// Will be returned when an error happens during
    /// serialization or deserialization.
    #[error("serde failed")]
    SerDe(#[from] serde_json::Error),

    /// Builder specific errors.
    #[error("uninitialized field")]
    Builder(#[from] derive_builder::UninitializedFieldError),
}

pub(crate) fn oci_error<'a, M>(message: M) -> OciSpecError
where
    M: Into<Cow<'a, str>>,
{
    let message = message.into();
    match message {
        Cow::Borrowed(s) => OciSpecError::Other(s.to_owned()),
        Cow::Owned(s) => OciSpecError::Other(s),
    }
}
