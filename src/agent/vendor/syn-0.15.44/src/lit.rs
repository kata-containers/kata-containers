use proc_macro2::{Literal, Span};
use std::str;

#[cfg(feature = "printing")]
use proc_macro2::Ident;

#[cfg(feature = "parsing")]
use proc_macro2::TokenStream;

use proc_macro2::TokenTree;

#[cfg(feature = "extra-traits")]
use std::hash::{Hash, Hasher};

#[cfg(feature = "parsing")]
use lookahead;
#[cfg(feature = "parsing")]
use parse::{Parse, Parser, Result};

ast_enum_of_structs! {
    /// A Rust literal such as a string or integer or boolean.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    ///
    /// # Syntax tree enum
    ///
    /// This type is a [syntax tree enum].
    ///
    /// [syntax tree enum]: enum.Expr.html#syntax-tree-enums
    //
    // TODO: change syntax-tree-enum link to an intra rustdoc link, currently
    // blocked on https://github.com/rust-lang/rust/issues/62833
    pub enum Lit {
        /// A UTF-8 string literal: `"foo"`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Str(LitStr #manual_extra_traits {
            token: Literal,
        }),

        /// A byte string literal: `b"foo"`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub ByteStr(LitByteStr #manual_extra_traits {
            token: Literal,
        }),

        /// A byte literal: `b'f'`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Byte(LitByte #manual_extra_traits {
            token: Literal,
        }),

        /// A character literal: `'a'`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Char(LitChar #manual_extra_traits {
            token: Literal,
        }),

        /// An integer literal: `1` or `1u16`.
        ///
        /// Holds up to 64 bits of data. Use `LitVerbatim` for any larger
        /// integer literal.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Int(LitInt #manual_extra_traits {
            token: Literal,
        }),

        /// A floating point literal: `1f64` or `1.0e10f64`.
        ///
        /// Must be finite. May not be infinte or NaN.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Float(LitFloat #manual_extra_traits {
            token: Literal,
        }),

        /// A boolean literal: `true` or `false`.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Bool(LitBool #manual_extra_traits {
            pub value: bool,
            pub span: Span,
        }),

        /// A raw token literal not interpreted by Syn, possibly because it
        /// represents an integer larger than 64 bits.
        ///
        /// *This type is available if Syn is built with the `"derive"` or
        /// `"full"` feature.*
        pub Verbatim(LitVerbatim #manual_extra_traits {
            pub token: Literal,
        }),
    }
}

impl LitStr {
    pub fn new(value: &str, span: Span) -> Self {
        let mut lit = Literal::string(value);
        lit.set_span(span);
        LitStr { token: lit }
    }

    pub fn value(&self) -> String {
        value::parse_lit_str(&self.token.to_string())
    }

    /// Parse a syntax tree node from the content of this string literal.
    ///
    /// All spans in the syntax tree will point to the span of this `LitStr`.
    ///
    /// # Example
    ///
    /// ```edition2018
    /// use proc_macro2::Span;
    /// use syn::{Attribute, Error, Ident, Lit, Meta, MetaNameValue, Path, Result};
    ///
    /// // Parses the path from an attribute that looks like:
    /// //
    /// //     #[path = "a::b::c"]
    /// //
    /// // or returns `None` if the input is some other attribute.
    /// fn get_path(attr: &Attribute) -> Result<Option<Path>> {
    ///     if !attr.path.is_ident("path") {
    ///         return Ok(None);
    ///     }
    ///
    ///     match attr.parse_meta()? {
    ///         Meta::NameValue(MetaNameValue { lit: Lit::Str(lit_str), .. }) => {
    ///             lit_str.parse().map(Some)
    ///         }
    ///         _ => {
    ///             let message = "expected #[path = \"...\"]";
    ///             Err(Error::new_spanned(attr, message))
    ///         }
    ///     }
    /// }
    /// ```
    #[cfg(feature = "parsing")]
    pub fn parse<T: Parse>(&self) -> Result<T> {
        self.parse_with(T::parse)
    }

    /// Invoke parser on the content of this string literal.
    ///
    /// All spans in the syntax tree will point to the span of this `LitStr`.
    ///
    /// # Example
    ///
    /// ```edition2018
    /// # use proc_macro2::Span;
    /// # use syn::{LitStr, Result};
    /// #
    /// # fn main() -> Result<()> {
    /// #     let lit_str = LitStr::new("a::b::c", Span::call_site());
    /// #
    /// #     const IGNORE: &str = stringify! {
    /// let lit_str: LitStr = /* ... */;
    /// #     };
    ///
    /// // Parse a string literal like "a::b::c" into a Path, not allowing
    /// // generic arguments on any of the path segments.
    /// let basic_path = lit_str.parse_with(syn::Path::parse_mod_style)?;
    /// #
    /// #     Ok(())
    /// # }
    /// ```
    #[cfg(feature = "parsing")]
    pub fn parse_with<F: Parser>(&self, parser: F) -> Result<F::Output> {
        use proc_macro2::Group;

        // Token stream with every span replaced by the given one.
        fn respan_token_stream(stream: TokenStream, span: Span) -> TokenStream {
            stream
                .into_iter()
                .map(|token| respan_token_tree(token, span))
                .collect()
        }

        // Token tree with every span replaced by the given one.
        fn respan_token_tree(mut token: TokenTree, span: Span) -> TokenTree {
            match token {
                TokenTree::Group(ref mut g) => {
                    let stream = respan_token_stream(g.stream().clone(), span);
                    *g = Group::new(g.delimiter(), stream);
                    g.set_span(span);
                }
                ref mut other => other.set_span(span),
            }
            token
        }

        // Parse string literal into a token stream with every span equal to the
        // original literal's span.
        let mut tokens = ::parse_str(&self.value())?;
        tokens = respan_token_stream(tokens, self.span());

        parser.parse2(tokens)
    }

    pub fn span(&self) -> Span {
        self.token.span()
    }

    pub fn set_span(&mut self, span: Span) {
        self.token.set_span(span)
    }
}

impl LitByteStr {
    pub fn new(value: &[u8], span: Span) -> Self {
        let mut token = Literal::byte_string(value);
        token.set_span(span);
        LitByteStr { token: token }
    }

    pub fn value(&self) -> Vec<u8> {
        value::parse_lit_byte_str(&self.token.to_string())
    }

    pub fn span(&self) -> Span {
        self.token.span()
    }

    pub fn set_span(&mut self, span: Span) {
        self.token.set_span(span)
    }
}

impl LitByte {
    pub fn new(value: u8, span: Span) -> Self {
        let mut token = Literal::u8_suffixed(value);
        token.set_span(span);
        LitByte { token: token }
    }

    pub fn value(&self) -> u8 {
        value::parse_lit_byte(&self.token.to_string())
    }

    pub fn span(&self) -> Span {
        self.token.span()
    }

    pub fn set_span(&mut self, span: Span) {
        self.token.set_span(span)
    }
}

impl LitChar {
    pub fn new(value: char, span: Span) -> Self {
        let mut token = Literal::character(value);
        token.set_span(span);
        LitChar { token: token }
    }

    pub fn value(&self) -> char {
        value::parse_lit_char(&self.token.to_string())
    }

    pub fn span(&self) -> Span {
        self.token.span()
    }

    pub fn set_span(&mut self, span: Span) {
        self.token.set_span(span)
    }
}

impl LitInt {
    pub fn new(value: u64, suffix: IntSuffix, span: Span) -> Self {
        let mut token = match suffix {
            IntSuffix::Isize => Literal::isize_suffixed(value as isize),
            IntSuffix::I8 => Literal::i8_suffixed(value as i8),
            IntSuffix::I16 => Literal::i16_suffixed(value as i16),
            IntSuffix::I32 => Literal::i32_suffixed(value as i32),
            IntSuffix::I64 => Literal::i64_suffixed(value as i64),
            IntSuffix::I128 => value::to_literal(&format!("{}i128", value)),
            IntSuffix::Usize => Literal::usize_suffixed(value as usize),
            IntSuffix::U8 => Literal::u8_suffixed(value as u8),
            IntSuffix::U16 => Literal::u16_suffixed(value as u16),
            IntSuffix::U32 => Literal::u32_suffixed(value as u32),
            IntSuffix::U64 => Literal::u64_suffixed(value),
            IntSuffix::U128 => value::to_literal(&format!("{}u128", value)),
            IntSuffix::None => Literal::u64_unsuffixed(value),
        };
        token.set_span(span);
        LitInt { token: token }
    }

    pub fn value(&self) -> u64 {
        value::parse_lit_int(&self.token.to_string()).unwrap()
    }

    pub fn suffix(&self) -> IntSuffix {
        let value = self.token.to_string();
        for (s, suffix) in vec![
            ("i8", IntSuffix::I8),
            ("i16", IntSuffix::I16),
            ("i32", IntSuffix::I32),
            ("i64", IntSuffix::I64),
            ("i128", IntSuffix::I128),
            ("isize", IntSuffix::Isize),
            ("u8", IntSuffix::U8),
            ("u16", IntSuffix::U16),
            ("u32", IntSuffix::U32),
            ("u64", IntSuffix::U64),
            ("u128", IntSuffix::U128),
            ("usize", IntSuffix::Usize),
        ] {
            if value.ends_with(s) {
                return suffix;
            }
        }
        IntSuffix::None
    }

    pub fn span(&self) -> Span {
        self.token.span()
    }

    pub fn set_span(&mut self, span: Span) {
        self.token.set_span(span)
    }
}

impl LitFloat {
    pub fn new(value: f64, suffix: FloatSuffix, span: Span) -> Self {
        let mut token = match suffix {
            FloatSuffix::F32 => Literal::f32_suffixed(value as f32),
            FloatSuffix::F64 => Literal::f64_suffixed(value),
            FloatSuffix::None => Literal::f64_unsuffixed(value),
        };
        token.set_span(span);
        LitFloat { token: token }
    }

    pub fn value(&self) -> f64 {
        value::parse_lit_float(&self.token.to_string())
    }

    pub fn suffix(&self) -> FloatSuffix {
        let value = self.token.to_string();
        for (s, suffix) in vec![("f32", FloatSuffix::F32), ("f64", FloatSuffix::F64)] {
            if value.ends_with(s) {
                return suffix;
            }
        }
        FloatSuffix::None
    }

    pub fn span(&self) -> Span {
        self.token.span()
    }

    pub fn set_span(&mut self, span: Span) {
        self.token.set_span(span)
    }
}

macro_rules! lit_extra_traits {
    ($ty:ident, $field:ident) => {
        #[cfg(feature = "extra-traits")]
        impl Eq for $ty {}

        #[cfg(feature = "extra-traits")]
        impl PartialEq for $ty {
            fn eq(&self, other: &Self) -> bool {
                self.$field.to_string() == other.$field.to_string()
            }
        }

        #[cfg(feature = "extra-traits")]
        impl Hash for $ty {
            fn hash<H>(&self, state: &mut H)
            where
                H: Hasher,
            {
                self.$field.to_string().hash(state);
            }
        }

        #[cfg(feature = "parsing")]
        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub fn $ty(marker: lookahead::TokenMarker) -> $ty {
            match marker {}
        }
    };
}

impl LitVerbatim {
    pub fn span(&self) -> Span {
        self.token.span()
    }

    pub fn set_span(&mut self, span: Span) {
        self.token.set_span(span)
    }
}

lit_extra_traits!(LitStr, token);
lit_extra_traits!(LitByteStr, token);
lit_extra_traits!(LitByte, token);
lit_extra_traits!(LitChar, token);
lit_extra_traits!(LitInt, token);
lit_extra_traits!(LitFloat, token);
lit_extra_traits!(LitBool, value);
lit_extra_traits!(LitVerbatim, token);

ast_enum! {
    /// The style of a string literal, either plain quoted or a raw string like
    /// `r##"data"##`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub enum StrStyle #no_visit {
        /// An ordinary string like `"data"`.
        Cooked,
        /// A raw string like `r##"data"##`.
        ///
        /// The unsigned integer is the number of `#` symbols used.
        Raw(usize),
    }
}

ast_enum! {
    /// The suffix on an integer literal if any, like the `u8` in `127u8`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub enum IntSuffix #no_visit {
        I8,
        I16,
        I32,
        I64,
        I128,
        Isize,
        U8,
        U16,
        U32,
        U64,
        U128,
        Usize,
        None,
    }
}

ast_enum! {
    /// The suffix on a floating point literal if any, like the `f32` in
    /// `1.0f32`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub enum FloatSuffix #no_visit {
        F32,
        F64,
        None,
    }
}

#[cfg(feature = "parsing")]
#[doc(hidden)]
#[allow(non_snake_case)]
pub fn Lit(marker: lookahead::TokenMarker) -> Lit {
    match marker {}
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;
    use parse::{Parse, ParseStream, Result};

    impl Parse for Lit {
        fn parse(input: ParseStream) -> Result<Self> {
            input.step(|cursor| {
                if let Some((lit, rest)) = cursor.literal() {
                    return Ok((Lit::new(lit), rest));
                }
                while let Some((ident, rest)) = cursor.ident() {
                    let value = if ident == "true" {
                        true
                    } else if ident == "false" {
                        false
                    } else {
                        break;
                    };
                    let lit_bool = LitBool {
                        value: value,
                        span: ident.span(),
                    };
                    return Ok((Lit::Bool(lit_bool), rest));
                }
                Err(cursor.error("expected literal"))
            })
        }
    }

    impl Parse for LitStr {
        fn parse(input: ParseStream) -> Result<Self> {
            let head = input.fork();
            match input.parse()? {
                Lit::Str(lit) => Ok(lit),
                _ => Err(head.error("expected string literal")),
            }
        }
    }

    impl Parse for LitByteStr {
        fn parse(input: ParseStream) -> Result<Self> {
            let head = input.fork();
            match input.parse()? {
                Lit::ByteStr(lit) => Ok(lit),
                _ => Err(head.error("expected byte string literal")),
            }
        }
    }

    impl Parse for LitByte {
        fn parse(input: ParseStream) -> Result<Self> {
            let head = input.fork();
            match input.parse()? {
                Lit::Byte(lit) => Ok(lit),
                _ => Err(head.error("expected byte literal")),
            }
        }
    }

    impl Parse for LitChar {
        fn parse(input: ParseStream) -> Result<Self> {
            let head = input.fork();
            match input.parse()? {
                Lit::Char(lit) => Ok(lit),
                _ => Err(head.error("expected character literal")),
            }
        }
    }

    impl Parse for LitInt {
        fn parse(input: ParseStream) -> Result<Self> {
            let head = input.fork();
            match input.parse()? {
                Lit::Int(lit) => Ok(lit),
                _ => Err(head.error("expected integer literal")),
            }
        }
    }

    impl Parse for LitFloat {
        fn parse(input: ParseStream) -> Result<Self> {
            let head = input.fork();
            match input.parse()? {
                Lit::Float(lit) => Ok(lit),
                _ => Err(head.error("expected floating point literal")),
            }
        }
    }

    impl Parse for LitBool {
        fn parse(input: ParseStream) -> Result<Self> {
            let head = input.fork();
            match input.parse()? {
                Lit::Bool(lit) => Ok(lit),
                _ => Err(head.error("expected boolean literal")),
            }
        }
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::{ToTokens, TokenStreamExt};

    impl ToTokens for LitStr {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.token.to_tokens(tokens);
        }
    }

    impl ToTokens for LitByteStr {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.token.to_tokens(tokens);
        }
    }

    impl ToTokens for LitByte {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.token.to_tokens(tokens);
        }
    }

    impl ToTokens for LitChar {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.token.to_tokens(tokens);
        }
    }

    impl ToTokens for LitInt {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.token.to_tokens(tokens);
        }
    }

    impl ToTokens for LitFloat {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.token.to_tokens(tokens);
        }
    }

    impl ToTokens for LitBool {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            let s = if self.value { "true" } else { "false" };
            tokens.append(Ident::new(s, self.span));
        }
    }

    impl ToTokens for LitVerbatim {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.token.to_tokens(tokens);
        }
    }
}

mod value {
    use super::*;
    use proc_macro2::TokenStream;
    use std::char;
    use std::ops::{Index, RangeFrom};

    impl Lit {
        /// Interpret a Syn literal from a proc-macro2 literal.
        ///
        /// Not all proc-macro2 literals are valid Syn literals. In particular,
        /// doc comments are considered by proc-macro2 to be literals but in Syn
        /// they are [`Attribute`].
        ///
        /// [`Attribute`]: crate::Attribute
        ///
        /// # Panics
        ///
        /// Panics if the input is a doc comment literal.
        pub fn new(token: Literal) -> Self {
            let value = token.to_string();

            match value::byte(&value, 0) {
                b'"' | b'r' => return Lit::Str(LitStr { token: token }),
                b'b' => match value::byte(&value, 1) {
                    b'"' | b'r' => return Lit::ByteStr(LitByteStr { token: token }),
                    b'\'' => return Lit::Byte(LitByte { token: token }),
                    _ => {}
                },
                b'\'' => return Lit::Char(LitChar { token: token }),
                b'0'...b'9' => {
                    if number_is_int(&value) {
                        return Lit::Int(LitInt { token: token });
                    } else if number_is_float(&value) {
                        return Lit::Float(LitFloat { token: token });
                    } else {
                        // number overflow
                        return Lit::Verbatim(LitVerbatim { token: token });
                    }
                }
                _ => {
                    if value == "true" || value == "false" {
                        return Lit::Bool(LitBool {
                            value: value == "true",
                            span: token.span(),
                        });
                    }
                }
            }

            panic!("Unrecognized literal: {}", value);
        }
    }

    fn number_is_int(value: &str) -> bool {
        if number_is_float(value) {
            false
        } else {
            value::parse_lit_int(value).is_some()
        }
    }

    fn number_is_float(value: &str) -> bool {
        if value.contains('.') {
            true
        } else if value.starts_with("0x") || value.ends_with("size") {
            false
        } else {
            value.contains('e') || value.contains('E')
        }
    }

    /// Get the byte at offset idx, or a default of `b'\0'` if we're looking
    /// past the end of the input buffer.
    pub fn byte<S: AsRef<[u8]> + ?Sized>(s: &S, idx: usize) -> u8 {
        let s = s.as_ref();
        if idx < s.len() {
            s[idx]
        } else {
            0
        }
    }

    fn next_chr(s: &str) -> char {
        s.chars().next().unwrap_or('\0')
    }

    pub fn parse_lit_str(s: &str) -> String {
        match byte(s, 0) {
            b'"' => parse_lit_str_cooked(s),
            b'r' => parse_lit_str_raw(s),
            _ => unreachable!(),
        }
    }

    // Clippy false positive
    // https://github.com/rust-lang-nursery/rust-clippy/issues/2329
    #[cfg_attr(feature = "cargo-clippy", allow(needless_continue))]
    fn parse_lit_str_cooked(mut s: &str) -> String {
        assert_eq!(byte(s, 0), b'"');
        s = &s[1..];

        let mut out = String::new();
        'outer: loop {
            let ch = match byte(s, 0) {
                b'"' => break,
                b'\\' => {
                    let b = byte(s, 1);
                    s = &s[2..];
                    match b {
                        b'x' => {
                            let (byte, rest) = backslash_x(s);
                            s = rest;
                            assert!(byte <= 0x80, "Invalid \\x byte in string literal");
                            char::from_u32(u32::from(byte)).unwrap()
                        }
                        b'u' => {
                            let (chr, rest) = backslash_u(s);
                            s = rest;
                            chr
                        }
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'\\' => '\\',
                        b'0' => '\0',
                        b'\'' => '\'',
                        b'"' => '"',
                        b'\r' | b'\n' => loop {
                            let ch = next_chr(s);
                            if ch.is_whitespace() {
                                s = &s[ch.len_utf8()..];
                            } else {
                                continue 'outer;
                            }
                        },
                        b => panic!("unexpected byte {:?} after \\ character in byte literal", b),
                    }
                }
                b'\r' => {
                    assert_eq!(byte(s, 1), b'\n', "Bare CR not allowed in string");
                    s = &s[2..];
                    '\n'
                }
                _ => {
                    let ch = next_chr(s);
                    s = &s[ch.len_utf8()..];
                    ch
                }
            };
            out.push(ch);
        }

        assert_eq!(s, "\"");
        out
    }

    fn parse_lit_str_raw(mut s: &str) -> String {
        assert_eq!(byte(s, 0), b'r');
        s = &s[1..];

        let mut pounds = 0;
        while byte(s, pounds) == b'#' {
            pounds += 1;
        }
        assert_eq!(byte(s, pounds), b'"');
        assert_eq!(byte(s, s.len() - pounds - 1), b'"');
        for end in s[s.len() - pounds..].bytes() {
            assert_eq!(end, b'#');
        }

        s[pounds + 1..s.len() - pounds - 1].to_owned()
    }

    pub fn parse_lit_byte_str(s: &str) -> Vec<u8> {
        assert_eq!(byte(s, 0), b'b');
        match byte(s, 1) {
            b'"' => parse_lit_byte_str_cooked(s),
            b'r' => parse_lit_byte_str_raw(s),
            _ => unreachable!(),
        }
    }

    // Clippy false positive
    // https://github.com/rust-lang-nursery/rust-clippy/issues/2329
    #[cfg_attr(feature = "cargo-clippy", allow(needless_continue))]
    fn parse_lit_byte_str_cooked(mut s: &str) -> Vec<u8> {
        assert_eq!(byte(s, 0), b'b');
        assert_eq!(byte(s, 1), b'"');
        s = &s[2..];

        // We're going to want to have slices which don't respect codepoint boundaries.
        let mut s = s.as_bytes();

        let mut out = Vec::new();
        'outer: loop {
            let byte = match byte(s, 0) {
                b'"' => break,
                b'\\' => {
                    let b = byte(s, 1);
                    s = &s[2..];
                    match b {
                        b'x' => {
                            let (b, rest) = backslash_x(s);
                            s = rest;
                            b
                        }
                        b'n' => b'\n',
                        b'r' => b'\r',
                        b't' => b'\t',
                        b'\\' => b'\\',
                        b'0' => b'\0',
                        b'\'' => b'\'',
                        b'"' => b'"',
                        b'\r' | b'\n' => loop {
                            let byte = byte(s, 0);
                            let ch = char::from_u32(u32::from(byte)).unwrap();
                            if ch.is_whitespace() {
                                s = &s[1..];
                            } else {
                                continue 'outer;
                            }
                        },
                        b => panic!("unexpected byte {:?} after \\ character in byte literal", b),
                    }
                }
                b'\r' => {
                    assert_eq!(byte(s, 1), b'\n', "Bare CR not allowed in string");
                    s = &s[2..];
                    b'\n'
                }
                b => {
                    s = &s[1..];
                    b
                }
            };
            out.push(byte);
        }

        assert_eq!(s, b"\"");
        out
    }

    fn parse_lit_byte_str_raw(s: &str) -> Vec<u8> {
        assert_eq!(byte(s, 0), b'b');
        parse_lit_str_raw(&s[1..]).into_bytes()
    }

    pub fn parse_lit_byte(s: &str) -> u8 {
        assert_eq!(byte(s, 0), b'b');
        assert_eq!(byte(s, 1), b'\'');

        // We're going to want to have slices which don't respect codepoint boundaries.
        let mut s = s[2..].as_bytes();

        let b = match byte(s, 0) {
            b'\\' => {
                let b = byte(s, 1);
                s = &s[2..];
                match b {
                    b'x' => {
                        let (b, rest) = backslash_x(s);
                        s = rest;
                        b
                    }
                    b'n' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    b'\\' => b'\\',
                    b'0' => b'\0',
                    b'\'' => b'\'',
                    b'"' => b'"',
                    b => panic!("unexpected byte {:?} after \\ character in byte literal", b),
                }
            }
            b => {
                s = &s[1..];
                b
            }
        };

        assert_eq!(byte(s, 0), b'\'');
        b
    }

    pub fn parse_lit_char(mut s: &str) -> char {
        assert_eq!(byte(s, 0), b'\'');
        s = &s[1..];

        let ch = match byte(s, 0) {
            b'\\' => {
                let b = byte(s, 1);
                s = &s[2..];
                match b {
                    b'x' => {
                        let (byte, rest) = backslash_x(s);
                        s = rest;
                        assert!(byte <= 0x80, "Invalid \\x byte in string literal");
                        char::from_u32(u32::from(byte)).unwrap()
                    }
                    b'u' => {
                        let (chr, rest) = backslash_u(s);
                        s = rest;
                        chr
                    }
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    b'\\' => '\\',
                    b'0' => '\0',
                    b'\'' => '\'',
                    b'"' => '"',
                    b => panic!("unexpected byte {:?} after \\ character in byte literal", b),
                }
            }
            _ => {
                let ch = next_chr(s);
                s = &s[ch.len_utf8()..];
                ch
            }
        };
        assert_eq!(s, "\'", "Expected end of char literal");
        ch
    }

    fn backslash_x<S>(s: &S) -> (u8, &S)
    where
        S: Index<RangeFrom<usize>, Output = S> + AsRef<[u8]> + ?Sized,
    {
        let mut ch = 0;
        let b0 = byte(s, 0);
        let b1 = byte(s, 1);
        ch += 0x10
            * match b0 {
                b'0'...b'9' => b0 - b'0',
                b'a'...b'f' => 10 + (b0 - b'a'),
                b'A'...b'F' => 10 + (b0 - b'A'),
                _ => panic!("unexpected non-hex character after \\x"),
            };
        ch += match b1 {
            b'0'...b'9' => b1 - b'0',
            b'a'...b'f' => 10 + (b1 - b'a'),
            b'A'...b'F' => 10 + (b1 - b'A'),
            _ => panic!("unexpected non-hex character after \\x"),
        };
        (ch, &s[2..])
    }

    fn backslash_u(mut s: &str) -> (char, &str) {
        if byte(s, 0) != b'{' {
            panic!("expected {{ after \\u");
        }
        s = &s[1..];

        let mut ch = 0;
        for _ in 0..6 {
            let b = byte(s, 0);
            match b {
                b'0'...b'9' => {
                    ch *= 0x10;
                    ch += u32::from(b - b'0');
                    s = &s[1..];
                }
                b'a'...b'f' => {
                    ch *= 0x10;
                    ch += u32::from(10 + b - b'a');
                    s = &s[1..];
                }
                b'A'...b'F' => {
                    ch *= 0x10;
                    ch += u32::from(10 + b - b'A');
                    s = &s[1..];
                }
                b'}' => break,
                _ => panic!("unexpected non-hex character after \\u"),
            }
        }
        assert!(byte(s, 0) == b'}');
        s = &s[1..];

        if let Some(ch) = char::from_u32(ch) {
            (ch, s)
        } else {
            panic!("character code {:x} is not a valid unicode character", ch);
        }
    }

    pub fn parse_lit_int(mut s: &str) -> Option<u64> {
        let base = match (byte(s, 0), byte(s, 1)) {
            (b'0', b'x') => {
                s = &s[2..];
                16
            }
            (b'0', b'o') => {
                s = &s[2..];
                8
            }
            (b'0', b'b') => {
                s = &s[2..];
                2
            }
            (b'0'...b'9', _) => 10,
            _ => unreachable!(),
        };

        let mut value = 0u64;
        loop {
            let b = byte(s, 0);
            let digit = match b {
                b'0'...b'9' => u64::from(b - b'0'),
                b'a'...b'f' if base > 10 => 10 + u64::from(b - b'a'),
                b'A'...b'F' if base > 10 => 10 + u64::from(b - b'A'),
                b'_' => {
                    s = &s[1..];
                    continue;
                }
                // NOTE: Looking at a floating point literal, we don't want to
                // consider these integers.
                b'.' if base == 10 => return None,
                b'e' | b'E' if base == 10 => return None,
                _ => break,
            };

            if digit >= base {
                panic!("Unexpected digit {:x} out of base range", digit);
            }

            value = match value.checked_mul(base) {
                Some(value) => value,
                None => return None,
            };
            value = match value.checked_add(digit) {
                Some(value) => value,
                None => return None,
            };
            s = &s[1..];
        }

        Some(value)
    }

    pub fn parse_lit_float(input: &str) -> f64 {
        // Rust's floating point literals are very similar to the ones parsed by
        // the standard library, except that rust's literals can contain
        // ignorable underscores. Let's remove those underscores.
        let mut bytes = input.to_owned().into_bytes();
        let mut write = 0;
        for read in 0..bytes.len() {
            if bytes[read] == b'_' {
                continue; // Don't increase write
            }
            if write != read {
                let x = bytes[read];
                bytes[write] = x;
            }
            write += 1;
        }
        bytes.truncate(write);
        let input = String::from_utf8(bytes).unwrap();
        let end = input.find('f').unwrap_or_else(|| input.len());
        input[..end].parse().unwrap()
    }

    pub fn to_literal(s: &str) -> Literal {
        let stream = s.parse::<TokenStream>().unwrap();
        match stream.into_iter().next().unwrap() {
            TokenTree::Literal(l) => l,
            _ => unreachable!(),
        }
    }
}
