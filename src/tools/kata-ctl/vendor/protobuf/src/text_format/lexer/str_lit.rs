use std::fmt;
use std::string::FromUtf8Error;

use super::lexer_impl::Lexer;
use super::lexer_impl::LexerError;
use crate::text_format::lexer::ParserLanguage;

#[derive(Debug)]
pub enum StrLitDecodeError {
    FromUtf8Error(FromUtf8Error),
    // TODO: be more specific
    OtherError,
}

impl fmt::Display for StrLitDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StrLitDecodeError::FromUtf8Error(e) => write!(f, "{}", e),
            StrLitDecodeError::OtherError => write!(f, "String literal decode error"),
        }
    }
}

impl std::error::Error for StrLitDecodeError {}

impl From<LexerError> for StrLitDecodeError {
    fn from(_: LexerError) -> Self {
        StrLitDecodeError::OtherError
    }
}

impl From<FromUtf8Error> for StrLitDecodeError {
    fn from(e: FromUtf8Error) -> Self {
        StrLitDecodeError::FromUtf8Error(e)
    }
}

pub type StrLitDecodeResult<T> = Result<T, StrLitDecodeError>;

/// String literal, both `string` and `bytes`.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct StrLit {
    pub escaped: String,
}

impl StrLit {
    /// May fail if not valid UTF8
    pub fn decode_utf8(&self) -> StrLitDecodeResult<String> {
        let mut lexer = Lexer::new(&self.escaped, ParserLanguage::Json);
        let mut r = Vec::new();
        while !lexer.eof() {
            r.push(lexer.next_byte_value()?);
        }
        Ok(String::from_utf8(r)?)
    }

    pub fn decode_bytes(&self) -> StrLitDecodeResult<Vec<u8>> {
        let mut lexer = Lexer::new(&self.escaped, ParserLanguage::Json);
        let mut r = Vec::new();
        while !lexer.eof() {
            r.push(lexer.next_byte_value()?);
        }
        Ok(r)
    }

    pub fn quoted(&self) -> String {
        format!("\"{}\"", self.escaped)
    }
}

#[cfg(test)]
mod test {
    use crate::text_format::lexer::StrLit;

    #[test]
    fn decode_utf8() {
        assert_eq!(
            "\u{1234}".to_owned(),
            StrLit {
                escaped: "\\341\\210\\264".to_owned()
            }
            .decode_utf8()
            .unwrap()
        )
    }
}
