use core::fmt;

/// Error struct which used with `NewVarKey`
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct InvalidKeyLength;

impl fmt::Display for InvalidKeyLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid key length")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InvalidKeyLength {}
