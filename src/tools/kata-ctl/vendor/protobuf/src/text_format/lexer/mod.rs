//! Implementation of lexer for both protobuf parser and for text format parser.

pub mod float;
mod json_number_lit;
mod lexer_impl;
mod loc;
mod num_lit;
mod parser_language;
mod str_lit;
mod token;

pub use self::json_number_lit::JsonNumberLit;
pub use self::lexer_impl::Lexer;
pub use self::lexer_impl::LexerError;
pub use self::loc::Loc;
pub use self::num_lit::NumLit;
pub use self::parser_language::ParserLanguage;
pub use self::str_lit::StrLit;
pub use self::str_lit::StrLitDecodeError;
pub use self::token::Token;
pub use self::token::TokenWithLocation;
