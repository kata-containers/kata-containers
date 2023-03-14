use core::fmt;

/// The error type returned when stream cipher has reached the end of a keystream.
#[derive(Copy, Clone, Debug)]
pub struct LoopError;

impl fmt::Display for LoopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Loop Error")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for LoopError {}

/// The error type returned when key and/or nonce used in stream cipher
/// initialization had an invalid length.
#[derive(Copy, Clone, Debug)]
pub struct InvalidKeyNonceLength;

impl fmt::Display for InvalidKeyNonceLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Loop Error")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InvalidKeyNonceLength {}

/// The error type returned when a cipher position can not be represented
/// by the requested type.
#[derive(Copy, Clone, Debug)]
pub struct OverflowError;

impl fmt::Display for OverflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Overflow Error")
    }
}

impl From<OverflowError> for LoopError {
    fn from(_: OverflowError) -> LoopError {
        LoopError
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OverflowError {}
