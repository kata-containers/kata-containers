#[cfg(feature = "std")]
use std::error::Error;

use core::fmt::{self, Display, Formatter};

#[derive(Debug, Clone)]
/// Error types for parsing values.
pub enum ValueIncorrectError {
    Negative(f64),
    NotNumber(char),
    NoValue,
}

impl Display for ValueIncorrectError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            ValueIncorrectError::Negative(value) => {
                f.write_fmt(format_args!("The value `{}` is negative.", value))
            }
            ValueIncorrectError::NotNumber(c) => {
                f.write_fmt(format_args!("The character {:?} is not a number.", c))
            }
            ValueIncorrectError::NoValue => f.write_str("No value."),
        }
    }
}

#[cfg(feature = "std")]
impl Error for ValueIncorrectError {}

#[derive(Debug, Clone)]
/// Errors for `ByteUnit`.
pub struct UnitIncorrectError {
    pub character: char,
    pub expected_characters: &'static [char],
    pub also_expect_no_character: bool,
}

impl Display for UnitIncorrectError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        let expected_characters_length = self.expected_characters.len();

        if expected_characters_length == 0 {
            f.write_fmt(format_args!(
                "The character {:?} is incorrect. No character is expected.",
                self.character
            ))
        } else {
            f.write_fmt(format_args!("The character {:?} is incorrect.", self.character))?;

            f.write_fmt(format_args!(" {:?}", self.expected_characters[0]))?;

            if expected_characters_length > 1 {
                for c in self.expected_characters[1..].iter().take(expected_characters_length - 2) {
                    f.write_fmt(format_args!(", {:?}", c))?;
                }
            }

            if self.also_expect_no_character {
                f.write_fmt(format_args!(
                    ", {:?} or no character is expected.",
                    self.expected_characters[expected_characters_length - 1]
                ))
            } else {
                f.write_fmt(format_args!(
                    " or {:?} is expected.",
                    self.expected_characters[expected_characters_length - 1]
                ))
            }
        }
    }
}

#[cfg(feature = "std")]
impl Error for UnitIncorrectError {}

#[derive(Debug, Clone)]
/// Error types for `Byte` and `ByteUnit`.
pub enum ByteError {
    ValueIncorrect(ValueIncorrectError),
    UnitIncorrect(UnitIncorrectError),
}

impl From<ValueIncorrectError> for ByteError {
    #[inline]
    fn from(error: ValueIncorrectError) -> Self {
        ByteError::ValueIncorrect(error)
    }
}

impl From<UnitIncorrectError> for ByteError {
    #[inline]
    fn from(error: UnitIncorrectError) -> Self {
        ByteError::UnitIncorrect(error)
    }
}

impl Display for ByteError {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            ByteError::ValueIncorrect(error) => Display::fmt(error, f),
            ByteError::UnitIncorrect(error) => Display::fmt(error, f),
        }
    }
}

#[cfg(feature = "std")]
impl Error for ByteError {}
