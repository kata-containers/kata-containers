use crate::parser::{Lexer, Loc, ParserError};

#[derive(Debug)]
pub enum StrLitDecodeError {
    Error,
}

impl From<ParserError> for StrLitDecodeError {
    fn from(_: ParserError) -> Self {
        StrLitDecodeError::Error
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
        let mut lexer = Lexer {
            input: &self.escaped,
            pos: 0,
            loc: Loc::start(),
        };
        let mut r = String::new();
        while !lexer.eof() {
            r.push(lexer.next_char_value()?);
        }
        Ok(r)
    }

    pub fn quoted(&self) -> String {
        format!("\"{}\"", self.escaped)
    }
}
