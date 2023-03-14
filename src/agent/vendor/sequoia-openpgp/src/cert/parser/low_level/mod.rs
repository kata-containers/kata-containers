use lalrpop_util::ParseError;

use crate::{
    Error,
    packet::Tag,
};

pub(crate) mod lexer;
lalrpop_util::lalrpop_mod!(
    #[allow(clippy::all)]
    #[allow(unused_parens)]
    grammar,
    "/cert/parser/low_level/grammar.rs"
);

pub(crate) mod grammar_util;

pub(crate) use self::lexer::Token;
pub(crate) use self::lexer::Lexer;

pub(crate) use self::grammar::CertParser;

// Converts a ParseError<usize, Token, Error> to a
// ParseError<usize, Tag, Error>.
//
// Justification: a Token is a tuple containing a Tag and a Packet.
// This function essentially drops the Packet.  Dropping the packet was
// necessary, because packets were not Sync, but Error, which we want
// to convert ParseErrors to, was.  Since we don't need the packet in
// general anyways, changing the Token to a Tag is a simple and
// sufficient fix.  Unfortunately, this conversion is a bit ugly and
// will break if lalrpop ever extends ParseError.
//
// This justification is no longer up-to-date because Packet is now
// also Sync.  However, we didn't catch this when we made Packet Sync,
// and now the justification is simply that CertParserError::Parser(_)
// expects a Tag, not a Packet.  It is not public, so we could change
// it.
pub(crate) fn parse_error_downcast(e: ParseError<usize, Token, Error>)
    -> ParseError<usize, Tag, Error>
{
    match e {
        ParseError::UnrecognizedToken {
            token: (start, t, end),
            expected,
        } => ParseError::UnrecognizedToken {
            token: (start, t.into(), end),
            expected,
        },

        ParseError::ExtraToken {
            token: (start, t, end),
        } => ParseError::ExtraToken {
            token: (start, t.into(), end),
        },

        ParseError::InvalidToken { location }
        => ParseError::InvalidToken { location },

        ParseError::User { error }
        => ParseError::User { error },

        ParseError::UnrecognizedEOF { location, expected }
        => ParseError::UnrecognizedEOF { location, expected },
    }
}

pub(crate) fn parse_error_to_openpgp_error(e: ParseError<usize, Tag, Error>)
    -> Error
{
    match e {
        ParseError::User { error } => error,
        e => Error::MalformedCert(format!("{}", e)),
    }
}

/// Errors that CertValidator::check may return.
#[derive(Debug, Clone)]
pub enum CertParserError {
    /// A parser error.
    Parser(ParseError<usize, Tag, Error>),
    /// An OpenPGP error.
    OpenPGP(Error),
}
assert_send_and_sync!(CertParserError);

impl From<CertParserError> for anyhow::Error {
    fn from(err: CertParserError) -> Self {
        match err {
            CertParserError::Parser(p) => p.into(),
            CertParserError::OpenPGP(p) => p.into(),
        }
    }
}
