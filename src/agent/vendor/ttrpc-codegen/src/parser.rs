use std::f64;
use std::fmt;
use std::num::ParseIntError;
use std::str;

use crate::model::*;
use crate::str_lit::*;
use protobuf_codegen::float;

const FIRST_LINE: u32 = 1;
const FIRST_COL: u32 = 1;

/// Location in file
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Loc {
    /// 1-based
    pub line: u32,
    /// 1-based
    pub col: u32,
}

impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

impl Loc {
    pub fn start() -> Loc {
        Loc {
            line: FIRST_LINE,
            col: FIRST_COL,
        }
    }
}

/// Basic information about parsing error.
#[derive(Debug)]
pub enum ParserError {
    IncorrectInput,
    IncorrectFloatLit,
    NotUtf8,
    ExpectChar(char),
    ExpectConstant,
    ExpectIdent,
    ExpectHexDigit,
    ExpectOctDigit,
    ExpectDecDigit,
    UnknownSyntax,
    UnexpectedEof,
    ParseIntError,
    IntegerOverflow,
    LabelNotAllowed,
    LabelRequired,
    InternalError,
    StrLitDecodeError(StrLitDecodeError),
    GroupNameShouldStartWithUpperCase,
    MapFieldNotAllowed,
    ExpectNamedIdent(String),
}

#[derive(Debug)]
pub struct ParserErrorWithLocation {
    pub error: ParserError,
    /// 1-based
    pub line: u32,
    /// 1-based
    pub col: u32,
}

impl From<StrLitDecodeError> for ParserError {
    fn from(e: StrLitDecodeError) -> Self {
        ParserError::StrLitDecodeError(e)
    }
}

impl From<ParseIntError> for ParserError {
    fn from(_: ParseIntError) -> Self {
        ParserError::ParseIntError
    }
}

impl From<float::ProtobufFloatParseError> for ParserError {
    fn from(_: float::ProtobufFloatParseError) -> Self {
        ParserError::IncorrectFloatLit
    }
}

pub type ParserResult<T> = Result<T, ParserError>;

trait ToU8 {
    fn to_u8(&self) -> ParserResult<u8>;
}

trait ToI32 {
    fn to_i32(&self) -> ParserResult<i32>;
}

trait ToI64 {
    fn to_i64(&self) -> ParserResult<i64>;
}

trait ToChar {
    fn to_char(&self) -> ParserResult<char>;
}

impl ToI32 for u64 {
    fn to_i32(&self) -> ParserResult<i32> {
        if *self <= i32::max_value() as u64 {
            Ok(*self as i32)
        } else {
            Err(ParserError::IntegerOverflow)
        }
    }
}

impl ToI32 for i64 {
    fn to_i32(&self) -> ParserResult<i32> {
        if *self <= i32::max_value() as i64 && *self >= i32::min_value() as i64 {
            Ok(*self as i32)
        } else {
            Err(ParserError::IntegerOverflow)
        }
    }
}

impl ToI64 for u64 {
    fn to_i64(&self) -> Result<i64, ParserError> {
        if *self <= i64::max_value() as u64 {
            Ok(*self as i64)
        } else {
            Err(ParserError::IntegerOverflow)
        }
    }
}

impl ToChar for u8 {
    fn to_char(&self) -> Result<char, ParserError> {
        if *self <= 0x7f {
            Ok(*self as char)
        } else {
            Err(ParserError::NotUtf8)
        }
    }
}

impl ToU8 for u32 {
    fn to_u8(&self) -> Result<u8, ParserError> {
        if *self as u8 as u32 == *self {
            Ok(*self as u8)
        } else {
            Err(ParserError::IntegerOverflow)
        }
    }
}

trait U64Extensions {
    fn neg(&self) -> ParserResult<i64>;
}

impl U64Extensions for u64 {
    fn neg(&self) -> ParserResult<i64> {
        if *self <= 0x7fff_ffff_ffff_ffff {
            Ok(-(*self as i64))
        } else if *self == 0x8000_0000_0000_0000 {
            Ok(-0x8000_0000_0000_0000)
        } else {
            Err(ParserError::IntegerOverflow)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    Symbol(char),
    IntLit(u64),
    // including quotes
    StrLit(StrLit),
    FloatLit(f64),
}

impl Token {
    /// Back to original
    fn format(&self) -> String {
        match self {
            &Token::Ident(ref s) => s.clone(),
            &Token::Symbol(c) => c.to_string(),
            &Token::IntLit(ref i) => i.to_string(),
            &Token::StrLit(ref s) => s.quoted(),
            &Token::FloatLit(ref f) => f.to_string(),
        }
    }

    fn to_num_lit(&self) -> ParserResult<NumLit> {
        match self {
            &Token::IntLit(i) => Ok(NumLit::U64(i)),
            &Token::FloatLit(f) => Ok(NumLit::F64(f)),
            _ => Err(ParserError::IncorrectInput),
        }
    }
}

#[derive(Clone)]
struct TokenWithLocation {
    token: Token,
    loc: Loc,
}

#[derive(Copy, Clone)]
pub struct Lexer<'a> {
    pub input: &'a str,
    pub pos: usize,
    pub loc: Loc,
}

fn is_letter(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

impl<'a> Lexer<'a> {
    /// No more chars
    pub fn eof(&self) -> bool {
        self.pos == self.input.len()
    }

    /// Remaining chars
    fn rem_chars(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn lookahead_char_is_in(&self, alphabet: &str) -> bool {
        self.lookahead_char()
            .map_or(false, |c| alphabet.contains(c))
    }

    fn next_char_opt(&mut self) -> Option<char> {
        let rem = self.rem_chars();
        if rem.is_empty() {
            None
        } else {
            let mut char_indices = rem.char_indices();
            let (_, c) = char_indices.next().unwrap();
            let c_len = char_indices.next().map(|(len, _)| len).unwrap_or(rem.len());
            self.pos += c_len;
            if c == '\n' {
                self.loc.line += 1;
                self.loc.col = FIRST_COL;
            } else {
                self.loc.col += 1;
            }
            Some(c)
        }
    }

    fn next_char(&mut self) -> ParserResult<char> {
        self.next_char_opt().ok_or(ParserError::UnexpectedEof)
    }

    /// Skip whitespaces
    fn skip_whitespaces(&mut self) {
        self.take_while(|c| c.is_whitespace());
    }

    fn skip_comment(&mut self) -> ParserResult<()> {
        if self.skip_if_lookahead_is_str("/*") {
            let end = "*/";
            match self.rem_chars().find(end) {
                None => Err(ParserError::UnexpectedEof),
                Some(len) => {
                    let new_pos = self.pos + len + end.len();
                    self.skip_to_pos(new_pos);
                    Ok(())
                }
            }
        } else {
            Ok(())
        }
    }

    fn skip_block_comment(&mut self) {
        if self.skip_if_lookahead_is_str("//") {
            loop {
                match self.next_char_opt() {
                    Some('\n') | None => break,
                    _ => {}
                }
            }
        }
    }

    fn skip_ws(&mut self) -> ParserResult<()> {
        loop {
            let pos = self.pos;
            self.skip_whitespaces();
            self.skip_comment()?;
            self.skip_block_comment();
            if pos == self.pos {
                // Did not advance
                return Ok(());
            }
        }
    }

    fn take_while<F>(&mut self, f: F) -> &'a str
    where
        F: Fn(char) -> bool,
    {
        let start = self.pos;
        while self.lookahead_char().map(&f) == Some(true) {
            self.next_char_opt().unwrap();
        }
        let end = self.pos;
        &self.input[start..end]
    }

    fn lookahead_char(&self) -> Option<char> {
        self.clone().next_char_opt()
    }

    fn lookahead_is_str(&self, s: &str) -> bool {
        self.rem_chars().starts_with(s)
    }

    fn skip_if_lookahead_is_str(&mut self, s: &str) -> bool {
        if self.lookahead_is_str(s) {
            let new_pos = self.pos + s.len();
            self.skip_to_pos(new_pos);
            true
        } else {
            false
        }
    }

    fn next_char_if<P>(&mut self, p: P) -> Option<char>
    where
        P: FnOnce(char) -> bool,
    {
        let mut clone = self.clone();
        match clone.next_char_opt() {
            Some(c) if p(c) => {
                *self = clone;
                Some(c)
            }
            _ => None,
        }
    }

    fn next_char_if_eq(&mut self, expect: char) -> bool {
        self.next_char_if(|c| c == expect) != None
    }

    fn next_char_if_in(&mut self, alphabet: &str) -> Option<char> {
        for c in alphabet.chars() {
            if self.next_char_if_eq(c) {
                return Some(c);
            }
        }
        None
    }

    fn next_char_expect_eq(&mut self, expect: char) -> ParserResult<()> {
        if self.next_char_if_eq(expect) {
            Ok(())
        } else {
            Err(ParserError::ExpectChar(expect))
        }
    }

    // str functions

    /// properly update line and column
    fn skip_to_pos(&mut self, new_pos: usize) -> &'a str {
        assert!(new_pos >= self.pos);
        assert!(new_pos <= self.input.len());
        let pos = self.pos;
        while self.pos != new_pos {
            self.next_char_opt().unwrap();
        }
        &self.input[pos..new_pos]
    }

    // Protobuf grammar

    // char functions

    // letter = "A" … "Z" | "a" … "z"
    // https://github.com/google/protobuf/issues/4565
    fn next_letter_opt(&mut self) -> Option<char> {
        self.next_char_if(is_letter)
    }

    // capitalLetter =  "A" … "Z"
    fn _next_capital_letter_opt(&mut self) -> Option<char> {
        self.next_char_if(|c| c >= 'A' && c <= 'Z')
    }

    fn is_ascii_alphanumeric(c: char) -> bool {
        (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9')
    }

    fn next_ident_part(&mut self) -> Option<char> {
        self.next_char_if(|c| Lexer::is_ascii_alphanumeric(c) || c == '_')
    }

    // Identifiers

    // ident = letter { letter | decimalDigit | "_" }
    fn next_ident_opt(&mut self) -> ParserResult<Option<String>> {
        if let Some(c) = self.next_letter_opt() {
            let mut ident = String::new();
            ident.push(c);
            while let Some(c) = self.next_ident_part() {
                ident.push(c);
            }
            Ok(Some(ident))
        } else {
            Ok(None)
        }
    }

    // Integer literals

    fn is_ascii_hexdigit(c: char) -> bool {
        (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
    }

    // hexLit     = "0" ( "x" | "X" ) hexDigit { hexDigit }
    fn next_hex_lit(&mut self) -> ParserResult<Option<u64>> {
        Ok(
            if self.skip_if_lookahead_is_str("0x") || self.skip_if_lookahead_is_str("0X") {
                let s = self.take_while(Lexer::is_ascii_hexdigit);
                Some(u64::from_str_radix(s, 16)? as u64)
            } else {
                None
            },
        )
    }

    fn is_ascii_digit(c: char) -> bool {
        c >= '0' && c <= '9'
    }

    // decimalLit = ( "1" … "9" ) { decimalDigit }
    // octalLit   = "0" { octalDigit }
    fn next_decimal_octal_lit(&mut self) -> ParserResult<Option<u64>> {
        // do not advance on number parse error
        let mut clone = self.clone();

        let pos = clone.pos;

        Ok(if clone.next_char_if(Lexer::is_ascii_digit) != None {
            clone.take_while(Lexer::is_ascii_digit);
            let value = clone.input[pos..clone.pos].parse()?;
            *self = clone;
            Some(value)
        } else {
            None
        })
    }

    // hexDigit     = "0" … "9" | "A" … "F" | "a" … "f"
    fn next_hex_digit(&mut self) -> ParserResult<u32> {
        let mut clone = self.clone();
        let r = match clone.next_char()? {
            c if c >= '0' && c <= '9' => c as u32 - b'0' as u32,
            c if c >= 'A' && c <= 'F' => c as u32 - b'A' as u32 + 10,
            c if c >= 'a' && c <= 'f' => c as u32 - b'a' as u32 + 10,
            _ => return Err(ParserError::ExpectHexDigit),
        };
        *self = clone;
        Ok(r)
    }

    // octalDigit   = "0" … "7"
    fn next_octal_digit(&mut self) -> ParserResult<u32> {
        let mut clone = self.clone();
        let r = match clone.next_char()? {
            c if c >= '0' && c <= '7' => c as u32 - b'0' as u32,
            _ => return Err(ParserError::ExpectOctDigit),
        };
        *self = clone;
        Ok(r)
    }

    // decimalDigit = "0" … "9"
    fn next_decimal_digit(&mut self) -> ParserResult<u32> {
        let mut clone = self.clone();
        let r = match clone.next_char()? {
            c if c >= '0' && c <= '9' => c as u32 - '0' as u32,
            _ => return Err(ParserError::ExpectDecDigit),
        };
        *self = clone;
        Ok(r)
    }

    // decimals  = decimalDigit { decimalDigit }
    fn next_decimal_digits(&mut self) -> ParserResult<()> {
        self.next_decimal_digit()?;
        self.take_while(|c| c >= '0' && c <= '9');
        Ok(())
    }

    // intLit     = decimalLit | octalLit | hexLit
    fn next_int_lit_opt(&mut self) -> ParserResult<Option<u64>> {
        self.skip_ws()?;
        if let Some(i) = self.next_hex_lit()? {
            return Ok(Some(i));
        }
        if let Some(i) = self.next_decimal_octal_lit()? {
            return Ok(Some(i));
        }
        Ok(None)
    }

    // Floating-point literals

    // exponent  = ( "e" | "E" ) [ "+" | "-" ] decimals
    fn next_exponent_opt(&mut self) -> ParserResult<Option<()>> {
        if self.next_char_if_in("eE") != None {
            self.next_char_if_in("+-");
            self.next_decimal_digits()?;
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }

    // floatLit = ( decimals "." [ decimals ] [ exponent ] | decimals exponent | "."decimals [ exponent ] ) | "inf" | "nan"
    fn next_float_lit(&mut self) -> ParserResult<()> {
        // "inf" and "nan" are handled as part of ident
        if self.next_char_if_eq('.') {
            self.next_decimal_digits()?;
            self.next_exponent_opt()?;
        } else {
            self.next_decimal_digits()?;
            if self.next_char_if_eq('.') {
                self.next_decimal_digits()?;
                self.next_exponent_opt()?;
            } else {
                if self.next_exponent_opt()? == None {
                    return Err(ParserError::IncorrectFloatLit);
                }
            }
        }
        Ok(())
    }

    // String literals

    // charValue = hexEscape | octEscape | charEscape | /[^\0\n\\]/
    // hexEscape = '\' ( "x" | "X" ) hexDigit hexDigit
    // https://github.com/google/protobuf/issues/4560
    // octEscape = '\' octalDigit octalDigit octalDigit
    // charEscape = '\' ( "a" | "b" | "f" | "n" | "r" | "t" | "v" | '\' | "'" | '"' )
    // quote = "'" | '"'
    pub fn next_char_value(&mut self) -> ParserResult<char> {
        match self.next_char()? {
            '\\' => {
                match self.next_char()? {
                    '\'' => Ok('\''),
                    '"' => Ok('"'),
                    '\\' => Ok('\\'),
                    'a' => Ok('\x07'),
                    'b' => Ok('\x08'),
                    'f' => Ok('\x0c'),
                    'n' => Ok('\n'),
                    'r' => Ok('\r'),
                    't' => Ok('\t'),
                    'v' => Ok('\x0b'),
                    'x' => {
                        let d1 = self.next_hex_digit()? as u8;
                        let d2 = self.next_hex_digit()? as u8;
                        // TODO: do not decode as char if > 0x80
                        Ok(((d1 << 4) | d2) as char)
                    }
                    d if d >= '0' && d <= '7' => {
                        let mut r = d as u8 - b'0';
                        for _ in 0..2 {
                            match self.next_octal_digit() {
                                Err(_) => break,
                                Ok(d) => r = (r << 3) + d as u8,
                            }
                        }
                        // TODO: do not decode as char if > 0x80
                        Ok(r as char)
                    }
                    // https://github.com/google/protobuf/issues/4562
                    c => Ok(c),
                }
            }
            '\n' | '\0' => Err(ParserError::IncorrectInput),
            c => Ok(c),
        }
    }

    // https://github.com/google/protobuf/issues/4564
    // strLit = ( "'" { charValue } "'" ) | ( '"' { charValue } '"' )
    fn next_str_lit_raw(&mut self) -> ParserResult<String> {
        let mut raw = String::new();

        let mut first = true;
        loop {
            if !first {
                self.skip_ws()?;
            }

            let start = self.pos;

            let q = match self.next_char_if_in("'\"") {
                Some(q) => q,
                None if !first => break,
                None => return Err(ParserError::IncorrectInput),
            };
            first = false;
            while self.lookahead_char() != Some(q) {
                self.next_char_value()?;
            }
            self.next_char_expect_eq(q)?;

            raw.push_str(&self.input[start + 1..self.pos - 1]);
        }
        Ok(raw)
    }

    fn next_str_lit_raw_opt(&mut self) -> ParserResult<Option<String>> {
        if self.lookahead_char_is_in("'\"") {
            Ok(Some(self.next_str_lit_raw()?))
        } else {
            Ok(None)
        }
    }

    fn is_ascii_punctuation(c: char) -> bool {
        match c {
            '.' | ',' | ':' | ';' | '/' | '\\' | '=' | '%' | '+' | '-' | '*' | '<' | '>' | '('
            | ')' | '{' | '}' | '[' | ']' => true,
            _ => false,
        }
    }

    fn next_token_inner(&mut self) -> ParserResult<Token> {
        if let Some(ident) = self.next_ident_opt()? {
            let token = if ident == float::PROTOBUF_NAN {
                Token::FloatLit(f64::NAN)
            } else if ident == float::PROTOBUF_INF {
                Token::FloatLit(f64::INFINITY)
            } else {
                Token::Ident(ident.to_owned())
            };
            return Ok(token);
        }

        let mut clone = self.clone();
        let pos = clone.pos;
        if let Ok(_) = clone.next_float_lit() {
            let f = float::parse_protobuf_float(&self.input[pos..clone.pos])?;
            *self = clone;
            return Ok(Token::FloatLit(f));
        }

        if let Some(lit) = self.next_int_lit_opt()? {
            return Ok(Token::IntLit(lit));
        }

        if let Some(escaped) = self.next_str_lit_raw_opt()? {
            return Ok(Token::StrLit(StrLit { escaped }));
        }

        // This branch must be after str lit
        if let Some(c) = self.next_char_if(Lexer::is_ascii_punctuation) {
            return Ok(Token::Symbol(c));
        }

        if let Some(ident) = self.next_ident_opt()? {
            return Ok(Token::Ident(ident));
        }

        Err(ParserError::IncorrectInput)
    }

    fn next_token(&mut self) -> ParserResult<Option<TokenWithLocation>> {
        self.skip_ws()?;
        let loc = self.loc;

        Ok(if self.eof() {
            None
        } else {
            let token = self.next_token_inner()?;
            // Skip whitespace here to update location
            // to the beginning of the next token
            self.skip_ws()?;
            Some(TokenWithLocation { token, loc })
        })
    }
}

#[derive(Clone)]
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    syntax: Syntax,
    next_token: Option<TokenWithLocation>,
}

#[derive(Copy, Clone)]
enum MessageBodyParseMode {
    MessageProto2,
    MessageProto3,
    Oneof,
    ExtendProto2,
    ExtendProto3,
}

impl MessageBodyParseMode {
    fn label_allowed(&self, label: Rule) -> bool {
        match label {
            Rule::Repeated => match *self {
                MessageBodyParseMode::MessageProto2
                | MessageBodyParseMode::MessageProto3
                | MessageBodyParseMode::ExtendProto2
                | MessageBodyParseMode::ExtendProto3 => true,
                MessageBodyParseMode::Oneof => false,
            },
            Rule::Optional | Rule::Required => match *self {
                MessageBodyParseMode::MessageProto2 | MessageBodyParseMode::ExtendProto2 => true,
                MessageBodyParseMode::MessageProto3
                | MessageBodyParseMode::ExtendProto3
                | MessageBodyParseMode::Oneof => false,
            },
        }
    }

    fn some_label_required(&self) -> bool {
        match *self {
            MessageBodyParseMode::MessageProto2 | MessageBodyParseMode::ExtendProto2 => true,
            MessageBodyParseMode::MessageProto3
            | MessageBodyParseMode::ExtendProto3
            | MessageBodyParseMode::Oneof => false,
        }
    }

    fn map_allowed(&self) -> bool {
        match *self {
            MessageBodyParseMode::MessageProto2
            | MessageBodyParseMode::MessageProto3
            | MessageBodyParseMode::ExtendProto2
            | MessageBodyParseMode::ExtendProto3 => true,
            MessageBodyParseMode::Oneof => false,
        }
    }

    fn is_most_non_fields_allowed(&self) -> bool {
        match *self {
            MessageBodyParseMode::MessageProto2 | MessageBodyParseMode::MessageProto3 => true,
            MessageBodyParseMode::ExtendProto2
            | MessageBodyParseMode::ExtendProto3
            | MessageBodyParseMode::Oneof => false,
        }
    }

    fn is_option_allowed(&self) -> bool {
        match *self {
            MessageBodyParseMode::MessageProto2
            | MessageBodyParseMode::MessageProto3
            | MessageBodyParseMode::Oneof => true,
            MessageBodyParseMode::ExtendProto2 | MessageBodyParseMode::ExtendProto3 => false,
        }
    }
}

#[derive(Default)]
pub struct MessageBody {
    pub fields: Vec<Field>,
    pub oneofs: Vec<OneOf>,
    pub reserved_nums: Vec<FieldNumberRange>,
    pub reserved_names: Vec<String>,
    pub messages: Vec<Message>,
    pub enums: Vec<Enumeration>,
    pub options: Vec<ProtobufOption>,
}

#[derive(Copy, Clone)]
enum NumLit {
    U64(u64),
    F64(f64),
}

impl NumLit {
    fn to_option_value(&self, sign_is_plus: bool) -> ParserResult<ProtobufConstant> {
        Ok(match (*self, sign_is_plus) {
            (NumLit::U64(u), true) => ProtobufConstant::U64(u),
            (NumLit::F64(f), true) => ProtobufConstant::F64(f),
            (NumLit::U64(u), false) => ProtobufConstant::I64(u.neg()?),
            (NumLit::F64(f), false) => ProtobufConstant::F64(-f),
        })
    }
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Parser<'a> {
        Parser {
            lexer: Lexer {
                input,
                pos: 0,
                loc: Loc::start(),
            },
            syntax: Syntax::Proto2,
            next_token: None,
        }
    }

    pub fn loc(&self) -> Loc {
        self.next_token.clone().map_or(self.lexer.loc, |n| n.loc)
    }

    fn lookahead(&mut self) -> ParserResult<Option<&Token>> {
        Ok(match self.next_token {
            Some(ref token) => Some(&token.token),
            None => {
                self.next_token = self.lexer.next_token()?;
                match self.next_token {
                    Some(ref token) => Some(&token.token),
                    None => None,
                }
            }
        })
    }

    fn lookahead_some(&mut self) -> ParserResult<&Token> {
        match self.lookahead()? {
            Some(token) => Ok(token),
            None => Err(ParserError::UnexpectedEof),
        }
    }

    fn next(&mut self) -> ParserResult<Option<Token>> {
        self.lookahead()?;
        Ok(self
            .next_token
            .take()
            .map(|TokenWithLocation { token, .. }| token))
    }

    fn next_some(&mut self) -> ParserResult<Token> {
        match self.next()? {
            Some(token) => Ok(token),
            None => Err(ParserError::UnexpectedEof),
        }
    }

    /// Can be called only after lookahead, otherwise it's error
    fn advance(&mut self) -> ParserResult<Token> {
        self.next_token
            .take()
            .map(|TokenWithLocation { token, .. }| token)
            .ok_or(ParserError::InternalError)
    }

    /// No more tokens
    fn syntax_eof(&mut self) -> ParserResult<bool> {
        Ok(self.lookahead()?.is_none())
    }

    fn next_token_if_map<P, R>(&mut self, p: P) -> ParserResult<Option<R>>
    where
        P: FnOnce(&Token) -> Option<R>,
    {
        self.lookahead()?;
        let v = match self.next_token {
            Some(ref token) => match p(&token.token) {
                Some(v) => v,
                None => return Ok(None),
            },
            _ => return Ok(None),
        };
        self.next_token = None;
        Ok(Some(v))
    }

    fn next_token_check_map<P, R>(&mut self, p: P) -> ParserResult<R>
    where
        P: FnOnce(&Token) -> ParserResult<R>,
    {
        self.lookahead()?;
        let r = match self.next_token {
            Some(ref token) => p(&token.token)?,
            None => return Err(ParserError::UnexpectedEof),
        };
        self.next_token = None;
        Ok(r)
    }

    fn next_token_if<P>(&mut self, p: P) -> ParserResult<Option<Token>>
    where
        P: FnOnce(&Token) -> bool,
    {
        self.next_token_if_map(|token| if p(token) { Some(token.clone()) } else { None })
    }

    fn next_ident_if_in(&mut self, idents: &[&str]) -> ParserResult<Option<String>> {
        let v = match self.lookahead()? {
            Some(&Token::Ident(ref next)) => {
                if idents.into_iter().find(|&i| i == next).is_some() {
                    next.clone()
                } else {
                    return Ok(None);
                }
            }
            _ => return Ok(None),
        };
        self.advance()?;
        Ok(Some(v))
    }

    fn next_ident_if_eq(&mut self, word: &str) -> ParserResult<bool> {
        Ok(self.next_ident_if_in(&[word])? != None)
    }

    pub fn next_ident_expect_eq(&mut self, word: &str) -> ParserResult<()> {
        if self.next_ident_if_eq(word)? {
            Ok(())
        } else {
            Err(ParserError::ExpectNamedIdent(word.to_owned()))
        }
    }

    fn next_ident_if_eq_error(&mut self, word: &str) -> ParserResult<()> {
        if self.clone().next_ident_if_eq(word)? {
            return Err(ParserError::IncorrectInput);
        }
        Ok(())
    }

    fn next_symbol_if_eq(&mut self, symbol: char) -> ParserResult<bool> {
        Ok(self.next_token_if(|token| match token {
            &Token::Symbol(c) if c == symbol => true,
            _ => false,
        })? != None)
    }

    fn next_symbol_expect_eq(&mut self, symbol: char) -> ParserResult<()> {
        if self.lookahead_is_symbol(symbol)? {
            self.advance()?;
            Ok(())
        } else {
            Err(ParserError::ExpectChar(symbol))
        }
    }

    fn lookahead_if_symbol(&mut self) -> ParserResult<Option<char>> {
        Ok(match self.lookahead()? {
            Some(&Token::Symbol(c)) => Some(c),
            _ => None,
        })
    }

    fn lookahead_is_symbol(&mut self, symbol: char) -> ParserResult<bool> {
        Ok(self.lookahead_if_symbol()? == Some(symbol))
    }

    // Protobuf grammar

    fn next_ident(&mut self) -> ParserResult<String> {
        self.next_token_check_map(|token| match token {
            &Token::Ident(ref ident) => Ok(ident.clone()),
            _ => Err(ParserError::ExpectIdent),
        })
    }

    fn next_str_lit(&mut self) -> ParserResult<StrLit> {
        self.next_token_check_map(|token| match token {
            &Token::StrLit(ref str_lit) => Ok(str_lit.clone()),
            _ => Err(ParserError::IncorrectInput),
        })
    }

    // fullIdent = ident { "." ident }
    fn next_full_ident(&mut self) -> ParserResult<String> {
        let mut full_ident = String::new();
        // https://github.com/google/protobuf/issues/4563
        if self.next_symbol_if_eq('.')? {
            full_ident.push('.');
        }
        full_ident.push_str(&self.next_ident()?);
        while self.next_symbol_if_eq('.')? {
            full_ident.push('.');
            full_ident.push_str(&self.next_ident()?);
        }
        Ok(full_ident)
    }

    // emptyStatement = ";"
    fn next_empty_statement_opt(&mut self) -> ParserResult<Option<()>> {
        if self.next_symbol_if_eq(';')? {
            Ok(Some(()))
        } else {
            Ok(None)
        }
    }

    // messageName = ident
    // enumName = ident
    // messageType = [ "." ] { ident "." } messageName
    // enumType = [ "." ] { ident "." } enumName
    fn next_message_or_enum_type(&mut self) -> ParserResult<String> {
        let mut full_name = String::new();
        if self.next_symbol_if_eq('.')? {
            full_name.push('.');
        }
        full_name.push_str(&self.next_ident()?);
        while self.next_symbol_if_eq('.')? {
            full_name.push('.');
            full_name.push_str(&self.next_ident()?);
        }
        Ok(full_name)
    }

    fn is_ascii_uppercase(c: char) -> bool {
        c >= 'A' && c <= 'Z'
    }

    // groupName = capitalLetter { letter | decimalDigit | "_" }
    fn next_group_name(&mut self) -> ParserResult<String> {
        // lexer cannot distinguish between group name and other ident
        let mut clone = self.clone();
        let ident = clone.next_ident()?;
        if !Parser::is_ascii_uppercase(ident.chars().next().unwrap()) {
            return Err(ParserError::GroupNameShouldStartWithUpperCase);
        }
        *self = clone;
        Ok(ident)
    }

    // Boolean

    // boolLit = "true" | "false"
    fn next_bool_lit_opt(&mut self) -> ParserResult<Option<bool>> {
        Ok(if self.next_ident_if_eq("true")? {
            Some(true)
        } else if self.next_ident_if_eq("false")? {
            Some(false)
        } else {
            None
        })
    }

    // Constant

    fn next_num_lit(&mut self) -> ParserResult<NumLit> {
        self.next_token_check_map(|token| token.to_num_lit())
    }

    // constant = fullIdent | ( [ "-" | "+" ] intLit ) | ( [ "-" | "+" ] floatLit ) |
    //            strLit | boolLit
    fn next_constant(&mut self) -> ParserResult<ProtobufConstant> {
        // https://github.com/google/protobuf/blob/a21f225824e994ebd35e8447382ea4e0cd165b3c/src/google/protobuf/unittest_custom_options.proto#L350
        if self.lookahead_is_symbol('{')? {
            return Ok(ProtobufConstant::BracedExpr(self.next_braces()?));
        }

        if let Some(b) = self.next_bool_lit_opt()? {
            return Ok(ProtobufConstant::Bool(b));
        }

        if let &Token::Symbol(c) = self.lookahead_some()? {
            if c == '+' || c == '-' {
                self.advance()?;
                let sign = c == '+';
                return Ok(self.next_num_lit()?.to_option_value(sign)?);
            }
        }

        if let Some(r) = self.next_token_if_map(|token| match token {
            &Token::StrLit(ref s) => Some(ProtobufConstant::String(s.clone())),
            _ => None,
        })? {
            return Ok(r);
        }

        match self.lookahead_some()? {
            &Token::IntLit(..) | &Token::FloatLit(..) => {
                return self.next_num_lit()?.to_option_value(true);
            }
            &Token::Ident(..) => {
                return Ok(ProtobufConstant::Ident(self.next_full_ident()?));
            }
            _ => {}
        }

        Err(ParserError::ExpectConstant)
    }

    fn next_int_lit(&mut self) -> ParserResult<u64> {
        self.next_token_check_map(|token| match token {
            &Token::IntLit(i) => Ok(i),
            _ => Err(ParserError::IncorrectInput),
        })
    }

    // Syntax

    // syntax = "syntax" "=" quote "proto2" quote ";"
    // syntax = "syntax" "=" quote "proto3" quote ";"
    fn next_syntax(&mut self) -> ParserResult<Option<Syntax>> {
        if self.next_ident_if_eq("syntax")? {
            self.next_symbol_expect_eq('=')?;
            let syntax_str = self.next_str_lit()?.decode_utf8()?;
            let syntax = if syntax_str == "proto2" {
                Syntax::Proto2
            } else if syntax_str == "proto3" {
                Syntax::Proto3
            } else {
                return Err(ParserError::UnknownSyntax);
            };
            self.next_symbol_expect_eq(';')?;
            Ok(Some(syntax))
        } else {
            Ok(None)
        }
    }

    // Import Statement

    // import = "import" [ "weak" | "public" ] strLit ";"
    fn next_import_opt(&mut self) -> ParserResult<Option<String>> {
        if self.next_ident_if_eq("import")? {
            self.next_ident_if_in(&["weak", "public"])?;
            let import_path = self.next_str_lit()?.decode_utf8()?;
            self.next_symbol_expect_eq(';')?;
            Ok(Some(import_path))
        } else {
            Ok(None)
        }
    }

    // Package

    // package = "package" fullIdent ";"
    fn next_package_opt(&mut self) -> ParserResult<Option<String>> {
        if self.next_ident_if_eq("package")? {
            let package = self.next_full_ident()?;
            self.next_symbol_expect_eq(';')?;
            Ok(Some(package))
        } else {
            Ok(None)
        }
    }

    // Option

    fn next_ident_or_braced(&mut self) -> ParserResult<String> {
        let mut ident_or_braced = String::new();
        if self.next_symbol_if_eq('(')? {
            ident_or_braced.push('(');
            ident_or_braced.push_str(&self.next_full_ident()?);
            self.next_symbol_expect_eq(')')?;
            ident_or_braced.push(')');
        } else {
            ident_or_braced.push_str(&self.next_ident()?);
        }
        Ok(ident_or_braced)
    }

    // https://github.com/google/protobuf/issues/4563
    // optionName = ( ident | "(" fullIdent ")" ) { "." ident }
    fn next_option_name(&mut self) -> ParserResult<String> {
        let mut option_name = String::new();
        option_name.push_str(&self.next_ident_or_braced()?);
        while self.next_symbol_if_eq('.')? {
            option_name.push('.');
            option_name.push_str(&self.next_ident_or_braced()?);
        }
        Ok(option_name)
    }

    // option = "option" optionName  "=" constant ";"
    fn next_option_opt(&mut self) -> ParserResult<Option<ProtobufOption>> {
        if self.next_ident_if_eq("option")? {
            let name = self.next_option_name()?;
            self.next_symbol_expect_eq('=')?;
            let value = self.next_constant()?;
            self.next_symbol_expect_eq(';')?;
            Ok(Some(ProtobufOption { name, value }))
        } else {
            Ok(None)
        }
    }

    // Fields

    // label = "required" | "optional" | "repeated"
    fn next_label(&mut self, mode: MessageBodyParseMode) -> ParserResult<Rule> {
        let map = &[
            ("optional", Rule::Optional),
            ("required", Rule::Required),
            ("repeated", Rule::Repeated),
        ];
        for &(name, value) in map {
            let mut clone = self.clone();
            if clone.next_ident_if_eq(name)? {
                if !mode.label_allowed(value) {
                    return Err(ParserError::LabelNotAllowed);
                }

                *self = clone;
                return Ok(value);
            }
        }

        if mode.some_label_required() {
            Err(ParserError::LabelRequired)
        } else {
            Ok(Rule::Optional)
        }
    }

    fn next_field_type(&mut self) -> ParserResult<FieldType> {
        let simple = &[
            ("int32", FieldType::Int32),
            ("int64", FieldType::Int64),
            ("uint32", FieldType::Uint32),
            ("uint64", FieldType::Uint64),
            ("sint32", FieldType::Sint32),
            ("sint64", FieldType::Sint64),
            ("fixed32", FieldType::Fixed32),
            ("sfixed32", FieldType::Sfixed32),
            ("fixed64", FieldType::Fixed64),
            ("sfixed64", FieldType::Sfixed64),
            ("bool", FieldType::Bool),
            ("string", FieldType::String),
            ("bytes", FieldType::Bytes),
            ("float", FieldType::Float),
            ("double", FieldType::Double),
        ];
        for &(ref n, ref t) in simple {
            if self.next_ident_if_eq(n)? {
                return Ok(t.clone());
            }
        }

        if let Some(t) = self.next_map_field_type_opt()? {
            return Ok(t);
        }

        let message_or_enum = self.next_message_or_enum_type()?;
        Ok(FieldType::MessageOrEnum(message_or_enum))
    }

    fn next_field_number(&mut self) -> ParserResult<i32> {
        self.next_token_check_map(|token| match token {
            &Token::IntLit(i) => i.to_i32(),
            _ => Err(ParserError::IncorrectInput),
        })
    }

    // fieldOption = optionName "=" constant
    fn next_field_option(&mut self) -> ParserResult<ProtobufOption> {
        let name = self.next_option_name()?;
        self.next_symbol_expect_eq('=')?;
        let value = self.next_constant()?;
        Ok(ProtobufOption { name, value })
    }

    // fieldOptions = fieldOption { ","  fieldOption }
    fn next_field_options(&mut self) -> ParserResult<Vec<ProtobufOption>> {
        let mut options = Vec::new();

        options.push(self.next_field_option()?);

        while self.next_symbol_if_eq(',')? {
            options.push(self.next_field_option()?);
        }

        Ok(options)
    }

    // field = label type fieldName "=" fieldNumber [ "[" fieldOptions "]" ] ";"
    // group = label "group" groupName "=" fieldNumber messageBody
    fn next_field(&mut self, mode: MessageBodyParseMode) -> ParserResult<Field> {
        let rule = if self.clone().next_ident_if_eq("map")? {
            if !mode.map_allowed() {
                return Err(ParserError::MapFieldNotAllowed);
            }
            Rule::Optional
        } else {
            self.next_label(mode)?
        };
        if self.next_ident_if_eq("group")? {
            let name = self.next_group_name()?.to_owned();
            self.next_symbol_expect_eq('=')?;
            let number = self.next_field_number()?;

            let mode = match self.syntax {
                Syntax::Proto2 => MessageBodyParseMode::MessageProto2,
                Syntax::Proto3 => MessageBodyParseMode::MessageProto3,
            };

            let MessageBody { fields, .. } = self.next_message_body(mode)?;

            Ok(Field {
                name,
                rule,
                typ: FieldType::Group(fields),
                number,
                options: Vec::new(),
            })
        } else {
            let typ = self.next_field_type()?;
            let name = self.next_ident()?.to_owned();
            self.next_symbol_expect_eq('=')?;
            let number = self.next_field_number()?;

            let mut options = Vec::new();

            if self.next_symbol_if_eq('[')? {
                for o in self.next_field_options()? {
                    options.push(o);
                }
                self.next_symbol_expect_eq(']')?;
            }
            self.next_symbol_expect_eq(';')?;
            Ok(Field {
                name,
                rule,
                typ,
                number,
                options,
            })
        }
    }

    // oneof = "oneof" oneofName "{" { oneofField | emptyStatement } "}"
    // oneofField = type fieldName "=" fieldNumber [ "[" fieldOptions "]" ] ";"
    fn next_oneof_opt(&mut self) -> ParserResult<Option<OneOf>> {
        if self.next_ident_if_eq("oneof")? {
            let name = self.next_ident()?.to_owned();
            let MessageBody { fields, .. } = self.next_message_body(MessageBodyParseMode::Oneof)?;
            Ok(Some(OneOf { name, fields }))
        } else {
            Ok(None)
        }
    }

    // mapField = "map" "<" keyType "," type ">" mapName "=" fieldNumber [ "[" fieldOptions "]" ] ";"
    // keyType = "int32" | "int64" | "uint32" | "uint64" | "sint32" | "sint64" |
    //           "fixed32" | "fixed64" | "sfixed32" | "sfixed64" | "bool" | "string"
    fn next_map_field_type_opt(&mut self) -> ParserResult<Option<FieldType>> {
        if self.next_ident_if_eq("map")? {
            self.next_symbol_expect_eq('<')?;
            // TODO: restrict key types
            let key = self.next_field_type()?;
            self.next_symbol_expect_eq(',')?;
            let value = self.next_field_type()?;
            self.next_symbol_expect_eq('>')?;
            Ok(Some(FieldType::Map(Box::new((key, value)))))
        } else {
            Ok(None)
        }
    }

    // Extensions and Reserved

    // Extensions

    // range =  intLit [ "to" ( intLit | "max" ) ]
    fn next_range(&mut self) -> ParserResult<FieldNumberRange> {
        let from = self.next_field_number()?;
        let to = if self.next_ident_if_eq("to")? {
            if self.next_ident_if_eq("max")? {
                i32::max_value()
            } else {
                self.next_field_number()?
            }
        } else {
            from
        };
        Ok(FieldNumberRange { from, to })
    }

    // ranges = range { "," range }
    fn next_ranges(&mut self) -> ParserResult<Vec<FieldNumberRange>> {
        let mut ranges = Vec::new();
        ranges.push(self.next_range()?);
        while self.next_symbol_if_eq(',')? {
            ranges.push(self.next_range()?);
        }
        Ok(ranges)
    }

    // extensions = "extensions" ranges ";"
    fn next_extensions_opt(&mut self) -> ParserResult<Option<Vec<FieldNumberRange>>> {
        if self.next_ident_if_eq("extensions")? {
            Ok(Some(self.next_ranges()?))
        } else {
            Ok(None)
        }
    }

    // Reserved

    // Grammar is incorrect: https://github.com/google/protobuf/issues/4558
    // reserved = "reserved" ( ranges | fieldNames ) ";"
    // fieldNames = fieldName { "," fieldName }
    fn next_reserved_opt(&mut self) -> ParserResult<Option<(Vec<FieldNumberRange>, Vec<String>)>> {
        if self.next_ident_if_eq("reserved")? {
            let (ranges, names) = if let &Token::StrLit(..) = self.lookahead_some()? {
                let mut names = Vec::new();
                names.push(self.next_str_lit()?.decode_utf8()?);
                while self.next_symbol_if_eq(',')? {
                    names.push(self.next_str_lit()?.decode_utf8()?);
                }
                (Vec::new(), names)
            } else {
                (self.next_ranges()?, Vec::new())
            };

            self.next_symbol_expect_eq(';')?;

            Ok(Some((ranges, names)))
        } else {
            Ok(None)
        }
    }

    // Top Level definitions

    // Enum definition

    // enumValueOption = optionName "=" constant
    fn next_enum_value_option(&mut self) -> ParserResult<()> {
        self.next_option_name()?;
        self.next_symbol_expect_eq('=')?;
        self.next_constant()?;
        Ok(())
    }

    // https://github.com/google/protobuf/issues/4561
    fn next_enum_value(&mut self) -> ParserResult<i32> {
        let minus = self.next_symbol_if_eq('-')?;
        let lit = self.next_int_lit()?;
        Ok(if minus {
            let unsigned = lit.to_i64()?;
            match unsigned.checked_neg() {
                Some(neg) => neg.to_i32()?,
                None => return Err(ParserError::IntegerOverflow),
            }
        } else {
            lit.to_i32()?
        })
    }

    // enumField = ident "=" intLit [ "[" enumValueOption { ","  enumValueOption } "]" ]";"
    fn next_enum_field(&mut self) -> ParserResult<EnumValue> {
        let name = self.next_ident()?.to_owned();
        self.next_symbol_expect_eq('=')?;
        let number = self.next_enum_value()?;
        if self.next_symbol_if_eq('[')? {
            self.next_enum_value_option()?;
            while self.next_symbol_if_eq(',')? {
                self.next_enum_value_option()?;
            }
            self.next_symbol_expect_eq(']')?;
        }

        Ok(EnumValue { name, number })
    }

    // enum = "enum" enumName enumBody
    // enumBody = "{" { option | enumField | emptyStatement } "}"
    fn next_enum_opt(&mut self) -> ParserResult<Option<Enumeration>> {
        if self.next_ident_if_eq("enum")? {
            let name = self.next_ident()?.to_owned();

            let mut values = Vec::new();
            let mut options = Vec::new();

            self.next_symbol_expect_eq('{')?;
            while self.lookahead_if_symbol()? != Some('}') {
                // emptyStatement
                if self.next_symbol_if_eq(';')? {
                    continue;
                }

                if let Some(o) = self.next_option_opt()? {
                    options.push(o);
                    continue;
                }

                values.push(self.next_enum_field()?);
            }
            self.next_symbol_expect_eq('}')?;
            Ok(Some(Enumeration {
                name,
                values,
                options,
            }))
        } else {
            Ok(None)
        }
    }

    // Message definition

    // messageBody = "{" { field | enum | message | extend | extensions | group |
    //               option | oneof | mapField | reserved | emptyStatement } "}"
    fn next_message_body(&mut self, mode: MessageBodyParseMode) -> ParserResult<MessageBody> {
        self.next_symbol_expect_eq('{')?;

        let mut r = MessageBody::default();

        while self.lookahead_if_symbol()? != Some('}') {
            // emptyStatement
            if self.next_symbol_if_eq(';')? {
                continue;
            }

            if mode.is_most_non_fields_allowed() {
                if let Some((field_nums, field_names)) = self.next_reserved_opt()? {
                    r.reserved_nums.extend(field_nums);
                    r.reserved_names.extend(field_names);
                    continue;
                }

                if let Some(oneof) = self.next_oneof_opt()? {
                    r.oneofs.push(oneof);
                    continue;
                }

                if let Some(_extensions) = self.next_extensions_opt()? {
                    continue;
                }

                if let Some(_extend) = self.next_extend_opt()? {
                    continue;
                }

                if let Some(nested_message) = self.next_message_opt()? {
                    r.messages.push(nested_message);
                    continue;
                }

                if let Some(nested_enum) = self.next_enum_opt()? {
                    r.enums.push(nested_enum);
                    continue;
                }
            } else {
                self.next_ident_if_eq_error("reserved")?;
                self.next_ident_if_eq_error("oneof")?;
                self.next_ident_if_eq_error("extensions")?;
                self.next_ident_if_eq_error("extend")?;
                self.next_ident_if_eq_error("message")?;
                self.next_ident_if_eq_error("enum")?;
            }

            if mode.is_option_allowed() {
                if let Some(option) = self.next_option_opt()? {
                    r.options.push(option);
                    continue;
                }
            } else {
                self.next_ident_if_eq_error("option")?;
            }

            r.fields.push(self.next_field(mode)?);
        }

        self.next_symbol_expect_eq('}')?;

        Ok(r)
    }

    // message = "message" messageName messageBody
    fn next_message_opt(&mut self) -> ParserResult<Option<Message>> {
        if self.next_ident_if_eq("message")? {
            let name = self.next_ident()?.to_owned();

            let mode = match self.syntax {
                Syntax::Proto2 => MessageBodyParseMode::MessageProto2,
                Syntax::Proto3 => MessageBodyParseMode::MessageProto3,
            };

            let MessageBody {
                fields,
                oneofs,
                reserved_nums,
                reserved_names,
                messages,
                enums,
                options,
            } = self.next_message_body(mode)?;

            Ok(Some(Message {
                name,
                fields,
                oneofs,
                reserved_nums,
                reserved_names,
                messages,
                enums,
                options,
            }))
        } else {
            Ok(None)
        }
    }

    // Extend

    // extend = "extend" messageType "{" {field | group | emptyStatement} "}"
    fn next_extend_opt(&mut self) -> ParserResult<Option<Vec<Extension>>> {
        let mut clone = self.clone();
        if clone.next_ident_if_eq("extend")? {
            // According to spec `extend` is only for `proto2`, but it is used in `proto3`
            // https://github.com/google/protobuf/issues/4610

            *self = clone;

            let extendee = self.next_message_or_enum_type()?;

            let mode = match self.syntax {
                Syntax::Proto2 => MessageBodyParseMode::ExtendProto2,
                Syntax::Proto3 => MessageBodyParseMode::ExtendProto3,
            };

            let MessageBody { fields, .. } = self.next_message_body(mode)?;

            let extensions = fields
                .into_iter()
                .map(|field| {
                    let extendee = extendee.clone();
                    Extension { extendee, field }
                })
                .collect();

            Ok(Some(extensions))
        } else {
            Ok(None)
        }
    }

    // Service definition

    fn next_braces(&mut self) -> ParserResult<String> {
        let mut r = String::new();
        self.next_symbol_expect_eq('{')?;
        r.push('{');
        loop {
            if self.lookahead_if_symbol()? == Some('{') {
                r.push_str(&self.next_braces()?);
                continue;
            }
            let next = self.next_some()?;
            r.push_str(&next.format());
            if let Token::Symbol('}') = next {
                break;
            }
        }
        Ok(r)
    }

    fn next_options_or_colon(&mut self) -> ParserResult<Vec<ProtobufOption>> {
        let mut options = Vec::new();
        if self.next_symbol_if_eq('{')? {
            while self.lookahead_if_symbol()? != Some('}') {
                if let Some(option) = self.next_option_opt()? {
                    options.push(option);
                    continue;
                }

                if let Some(()) = self.next_empty_statement_opt()? {
                    continue;
                }

                return Err(ParserError::IncorrectInput);
            }
            self.next_symbol_expect_eq('}')?;
        } else {
            self.next_symbol_expect_eq(';')?;
        }

        Ok(options)
    }

    // stream = "stream" streamName "(" messageType "," messageType ")"
    //        (( "{" { option | emptyStatement } "}") | ";" )
    fn next_stream_opt(&mut self) -> ParserResult<Option<Method>> {
        assert_eq!(Syntax::Proto2, self.syntax);
        if self.next_ident_if_eq("stream")? {
            let name = self.next_ident()?;
            self.next_symbol_expect_eq('(')?;
            let input_type = self.next_ident()?;
            self.next_symbol_expect_eq(',')?;
            let output_type = self.next_ident()?;
            self.next_symbol_expect_eq(')')?;
            let options = self.next_options_or_colon()?;
            Ok(Some(Method {
                name,
                input_type,
                output_type,
                client_streaming: true,
                server_streaming: true,
                options,
            }))
        } else {
            Ok(None)
        }
    }

    // rpc = "rpc" rpcName "(" [ "stream" ] messageType ")"
    //     "returns" "(" [ "stream" ] messageType ")"
    //     (( "{" { option | emptyStatement } "}" ) | ";" )
    fn next_rpc_opt(&mut self) -> ParserResult<Option<Method>> {
        if self.next_ident_if_eq("rpc")? {
            let name = self.next_ident()?;
            self.next_symbol_expect_eq('(')?;
            let client_streaming = self.next_ident_if_eq("stream")?;
            let input_type = self.next_message_or_enum_type()?;
            self.next_symbol_expect_eq(')')?;
            self.next_ident_expect_eq("returns")?;
            self.next_symbol_expect_eq('(')?;
            let server_streaming = self.next_ident_if_eq("stream")?;
            let output_type = self.next_message_or_enum_type()?;
            self.next_symbol_expect_eq(')')?;
            let options = self.next_options_or_colon()?;
            Ok(Some(Method {
                name,
                input_type,
                output_type,
                client_streaming,
                server_streaming,
                options,
            }))
        } else {
            Ok(None)
        }
    }

    // proto2:
    // service = "service" serviceName "{" { option | rpc | stream | emptyStatement } "}"
    //
    // proto3:
    // service = "service" serviceName "{" { option | rpc | emptyStatement } "}"
    fn next_service_opt(&mut self) -> ParserResult<Option<Service>> {
        if self.next_ident_if_eq("service")? {
            let name = self.next_ident()?;
            let mut methods = Vec::new();
            let mut options = Vec::new();
            self.next_symbol_expect_eq('{')?;
            while self.lookahead_if_symbol()? != Some('}') {
                if let Some(method) = self.next_rpc_opt()? {
                    methods.push(method);
                    continue;
                }

                if self.syntax == Syntax::Proto2 {
                    if let Some(method) = self.next_stream_opt()? {
                        methods.push(method);
                        continue;
                    }
                }

                if let Some(o) = self.next_option_opt()? {
                    options.push(o);
                    continue;
                }

                if let Some(()) = self.next_empty_statement_opt()? {
                    continue;
                }

                return Err(ParserError::IncorrectInput);
            }
            self.next_symbol_expect_eq('}')?;
            Ok(Some(Service {
                name,
                methods,
                options,
            }))
        } else {
            Ok(None)
        }
    }

    // Proto file

    // proto = syntax { import | package | option | topLevelDef | emptyStatement }
    // topLevelDef = message | enum | extend | service
    pub fn next_proto(&mut self) -> ParserResult<FileDescriptor> {
        let syntax = self.next_syntax()?.unwrap_or(Syntax::Proto2);
        self.syntax = syntax;

        let mut import_paths = Vec::new();
        let mut package = String::new();
        let mut messages = Vec::new();
        let mut enums = Vec::new();
        let mut extensions = Vec::new();
        let mut options = Vec::new();
        let mut services = Vec::new();

        while !self.syntax_eof()? {
            if let Some(import_path) = self.next_import_opt()? {
                import_paths.push(import_path);
                continue;
            }

            if let Some(next_package) = self.next_package_opt()? {
                package = next_package.to_owned();
                continue;
            }

            if let Some(option) = self.next_option_opt()? {
                options.push(option);
                continue;
            }

            if let Some(message) = self.next_message_opt()? {
                messages.push(message);
                continue;
            }

            if let Some(enumeration) = self.next_enum_opt()? {
                enums.push(enumeration);
                continue;
            }

            if let Some(more_extensions) = self.next_extend_opt()? {
                extensions.extend(more_extensions);
                continue;
            }

            if let Some(service) = self.next_service_opt()? {
                services.push(service);
                continue;
            }

            if self.next_symbol_if_eq(';')? {
                continue;
            }

            return Err(ParserError::IncorrectInput);
        }

        Ok(FileDescriptor {
            import_paths,
            package,
            syntax,
            messages,
            enums,
            extensions,
            services,
            options,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn lex<P, R>(input: &str, parse_what: P) -> R
    where
        P: FnOnce(&mut Lexer) -> ParserResult<R>,
    {
        let mut lexer = Lexer {
            input,
            pos: 0,
            loc: Loc::start(),
        };
        let r = parse_what(&mut lexer).expect(&format!("lexer failed at {}", lexer.loc));
        assert!(lexer.eof(), "check eof failed at {}", lexer.loc);
        r
    }

    fn lex_opt<P, R>(input: &str, parse_what: P) -> R
    where
        P: FnOnce(&mut Lexer) -> ParserResult<Option<R>>,
    {
        let mut lexer = Lexer {
            input,
            pos: 0,
            loc: Loc::start(),
        };
        let o = parse_what(&mut lexer).expect(&format!("lexer failed at {}", lexer.loc));
        let r = o.expect(&format!("lexer returned none at {}", lexer.loc));
        assert!(lexer.eof(), "check eof failed at {}", lexer.loc);
        r
    }

    fn parse<P, R>(input: &str, parse_what: P) -> R
    where
        P: FnOnce(&mut Parser) -> ParserResult<R>,
    {
        let mut parser = Parser::new(input);
        let r = parse_what(&mut parser).expect(&format!("parse failed at {}", parser.loc()));
        let eof = parser
            .syntax_eof()
            .expect(&format!("check eof failed at {}", parser.loc()));
        assert!(eof, "{}", parser.loc());
        r
    }

    fn parse_opt<P, R>(input: &str, parse_what: P) -> R
    where
        P: FnOnce(&mut Parser) -> ParserResult<Option<R>>,
    {
        let mut parser = Parser::new(input);
        let o = parse_what(&mut parser).expect(&format!("parse failed at {}", parser.loc()));
        let r = o.expect(&format!("parser returned none at {}", parser.loc()));
        assert!(parser.syntax_eof().unwrap());
        r
    }

    #[test]
    fn test_lexer_int_lit() {
        let msg = r#"10"#;
        let mess = lex_opt(msg, |p| p.next_int_lit_opt());
        assert_eq!(10, mess);
    }

    #[test]
    fn test_lexer_float_lit() {
        let msg = r#"12.3"#;
        let mess = lex(msg, |p| p.next_token_inner());
        assert_eq!(Token::FloatLit(12.3), mess);
    }

    #[test]
    fn test_ident() {
        let msg = r#"  aabb_c  "#;
        let mess = parse(msg, |p| p.next_ident().map(|s| s.to_owned()));
        assert_eq!("aabb_c", mess);
    }

    #[test]
    fn test_str_lit() {
        let msg = r#"  "a\nb"  "#;
        let mess = parse(msg, |p| p.next_str_lit());
        assert_eq!(
            StrLit {
                escaped: r#"a\nb"#.to_owned()
            },
            mess
        );
    }

    #[test]
    fn test_syntax() {
        let msg = r#"  syntax = "proto3";  "#;
        let mess = parse_opt(msg, |p| p.next_syntax());
        assert_eq!(Syntax::Proto3, mess);
    }

    #[test]
    fn test_field_default_value_int() {
        let msg = r#"  optional int64 f = 4 [default = 12];  "#;
        let mess = parse(msg, |p| p.next_field(MessageBodyParseMode::MessageProto2));
        assert_eq!("f", mess.name);
        assert_eq!("default", mess.options[0].name);
        assert_eq!("12", mess.options[0].value.format());
    }

    #[test]
    fn test_field_default_value_float() {
        let msg = r#"  optional float f = 2 [default = 10.0];  "#;
        let mess = parse(msg, |p| p.next_field(MessageBodyParseMode::MessageProto2));
        assert_eq!("f", mess.name);
        assert_eq!("default", mess.options[0].name);
        assert_eq!("10.0", mess.options[0].value.format());
    }

    #[test]
    fn test_message() {
        let msg = r#"message ReferenceData
    {
        repeated ScenarioInfo  scenarioSet = 1;
        repeated CalculatedObjectInfo calculatedObjectSet = 2;
        repeated RiskFactorList riskFactorListSet = 3;
        repeated RiskMaturityInfo riskMaturitySet = 4;
        repeated IndicatorInfo indicatorSet = 5;
        repeated RiskStrikeInfo riskStrikeSet = 6;
        repeated FreeProjectionList freeProjectionListSet = 7;
        repeated ValidationProperty ValidationSet = 8;
        repeated CalcProperties calcPropertiesSet = 9;
        repeated MaturityInfo maturitySet = 10;
    }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!(10, mess.fields.len());
    }

    #[test]
    fn test_enum() {
        let msg = r#"enum PairingStatus {
                DEALPAIRED        = 0;
                INVENTORYORPHAN   = 1;
                CALCULATEDORPHAN  = 2;
                CANCELED          = 3;
    }"#;

        let enumeration = parse_opt(msg, |p| p.next_enum_opt());
        assert_eq!(4, enumeration.values.len());
    }

    #[test]
    fn test_ignore() {
        let msg = r#"option optimize_for = SPEED;"#;

        parse_opt(msg, |p| p.next_option_opt());
    }

    #[test]
    fn test_import() {
        let msg = r#"syntax = "proto3";
    import "test_import_nested_imported_pb.proto";
    message ContainsImportedNested {
        ContainerForNested.NestedMessage m = 1;
        ContainerForNested.NestedEnum e = 2;
    }
    "#;
        let desc = parse(msg, |p| p.next_proto());

        assert_eq!(
            vec!["test_import_nested_imported_pb.proto"],
            desc.import_paths
        );
    }

    #[test]
    fn test_package() {
        let msg = r#"
        package foo.bar;
    message ContainsImportedNested {
        optional ContainerForNested.NestedMessage m = 1;
        optional ContainerForNested.NestedEnum e = 2;
    }
    "#;
        let desc = parse(msg, |p| p.next_proto());
        assert_eq!("foo.bar".to_string(), desc.package);
    }

    #[test]
    fn test_nested_message() {
        let msg = r#"message A
    {
        message B {
            repeated int32 a = 1;
            optional string b = 2;
        }
        optional string b = 1;
    }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!(1, mess.messages.len());
    }

    #[test]
    fn test_map() {
        let msg = r#"message A
    {
        optional map<string, int32> b = 1;
    }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!(1, mess.fields.len());
        match mess.fields[0].typ {
            FieldType::Map(ref f) => match &**f {
                &(FieldType::String, FieldType::Int32) => (),
                ref f => panic!("Expecting Map<String, Int32> found {:?}", f),
            },
            ref f => panic!("Expecting map, got {:?}", f),
        }
    }

    #[test]
    fn test_oneof() {
        let msg = r#"message A
    {
        optional int32 a1 = 1;
        oneof a_oneof {
            string a2 = 2;
            int32 a3 = 3;
            bytes a4 = 4;
        }
        repeated bool a5 = 5;
    }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!(1, mess.oneofs.len());
        assert_eq!(3, mess.oneofs[0].fields.len());
    }

    #[test]
    fn test_reserved() {
        let msg = r#"message Sample {
       reserved 4, 15, 17 to 20, 30;
       reserved "foo", "bar";
       optional uint64 age =1;
       required bytes name =2;
    }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!(
            vec![
                FieldNumberRange { from: 4, to: 4 },
                FieldNumberRange { from: 15, to: 15 },
                FieldNumberRange { from: 17, to: 20 },
                FieldNumberRange { from: 30, to: 30 }
            ],
            mess.reserved_nums
        );
        assert_eq!(
            vec!["foo".to_string(), "bar".to_string()],
            mess.reserved_names
        );
        assert_eq!(2, mess.fields.len());
    }

    #[test]
    fn test_default_value_int() {
        let msg = r#"message Sample {
            optional int32 x = 1 [default = 17];
        }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!("default", mess.fields[0].options[0].name);
        assert_eq!("17", mess.fields[0].options[0].value.format());
    }

    #[test]
    fn test_default_value_string() {
        let msg = r#"message Sample {
            optional string x = 1 [default = "ab\nc d\"g\'h\0\"z"];
        }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!(
            r#""ab\nc d\"g\'h\0\"z""#,
            mess.fields[0].options[0].value.format()
        );
    }

    #[test]
    fn test_default_value_bytes() {
        let msg = r#"message Sample {
            optional bytes x = 1 [default = "ab\nc d\xfeE\"g\'h\0\"z"];
        }"#;

        let mess = parse_opt(msg, |p| p.next_message_opt());
        assert_eq!(
            r#""ab\nc d\xfeE\"g\'h\0\"z""#,
            mess.fields[0].options[0].value.format()
        );
    }

    #[test]
    fn test_group() {
        let msg = r#"message MessageWithGroup {
            optional string aaa = 1;
            repeated group Identifier = 18 {
                optional int32 iii = 19;
                optional string sss = 20;
            }
            required int bbb = 3;
        }"#;
        let mess = parse_opt(msg, |p| p.next_message_opt());

        assert_eq!("Identifier", mess.fields[1].name);
        if let FieldType::Group(ref group_fields) = mess.fields[1].typ {
            assert_eq!(2, group_fields.len());
        } else {
            panic!("expecting group");
        }

        assert_eq!("bbb", mess.fields[2].name);
    }

    #[test]
    fn test_incorrect_file_descriptor() {
        let msg = r#"
            message Foo {}
            dfgdg
        "#;

        let err = FileDescriptor::parse(msg).err().expect("err");
        assert_eq!(4, err.line);
    }

    #[test]
    fn test_extend() {
        let proto = r#"
            syntax = "proto2";
            extend google.protobuf.FileOptions {
                optional bool foo = 17001;
                optional string bar = 17002;
            }
            extend google.protobuf.MessageOptions {
                optional bool baz = 17003;
            }
        "#;

        let fd = FileDescriptor::parse(proto).expect("fd");
        assert_eq!(3, fd.extensions.len());
        assert_eq!("google.protobuf.FileOptions", fd.extensions[0].extendee);
        assert_eq!("google.protobuf.FileOptions", fd.extensions[1].extendee);
        assert_eq!("google.protobuf.MessageOptions", fd.extensions[2].extendee);
        assert_eq!(17003, fd.extensions[2].field.number);
    }
}
