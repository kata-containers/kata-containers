use thiserror::Error;

#[derive(Error, Debug)]
pub enum DerError {
    #[error("Unexpected end of input.")]
    UnexpectedEndOfInput,

    #[error("Invalid tag: {0}")]
    InvalidTag(String),

    #[error("Invalid length: {0}")]
    InvalidLength(String),

    #[error("Invalid contents: {0}")]
    InvalidContents(String),

    #[error("Overflow length.")]
    Overflow,

    #[error("Failed to read: {0}")]
    ReadFailure(#[source] std::io::Error),
}
