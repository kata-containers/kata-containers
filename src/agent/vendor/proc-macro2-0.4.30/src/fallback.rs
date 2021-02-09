#[cfg(span_locations)]
use std::cell::RefCell;
#[cfg(procmacro2_semver_exempt)]
use std::cmp;
use std::fmt;
use std::iter;
#[cfg(procmacro2_semver_exempt)]
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::vec;

use strnom::{block_comment, skip_whitespace, whitespace, word_break, Cursor, PResult};
use unicode_xid::UnicodeXID;

use {Delimiter, Punct, Spacing, TokenTree};

#[derive(Clone)]
pub struct TokenStream {
    inner: Vec<TokenTree>,
}

#[derive(Debug)]
pub struct LexError;

impl TokenStream {
    pub fn new() -> TokenStream {
        TokenStream { inner: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }
}

#[cfg(span_locations)]
fn get_cursor(src: &str) -> Cursor {
    // Create a dummy file & add it to the source map
    SOURCE_MAP.with(|cm| {
        let mut cm = cm.borrow_mut();
        let name = format!("<parsed string {}>", cm.files.len());
        let span = cm.add_file(&name, src);
        Cursor {
            rest: src,
            off: span.lo,
        }
    })
}

#[cfg(not(span_locations))]
fn get_cursor(src: &str) -> Cursor {
    Cursor { rest: src }
}

impl FromStr for TokenStream {
    type Err = LexError;

    fn from_str(src: &str) -> Result<TokenStream, LexError> {
        // Create a dummy file & add it to the source map
        let cursor = get_cursor(src);

        match token_stream(cursor) {
            Ok((input, output)) => {
                if skip_whitespace(input).len() != 0 {
                    Err(LexError)
                } else {
                    Ok(output)
                }
            }
            Err(LexError) => Err(LexError),
        }
    }
}

impl fmt::Display for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut joint = false;
        for (i, tt) in self.inner.iter().enumerate() {
            if i != 0 && !joint {
                write!(f, " ")?;
            }
            joint = false;
            match *tt {
                TokenTree::Group(ref tt) => {
                    let (start, end) = match tt.delimiter() {
                        Delimiter::Parenthesis => ("(", ")"),
                        Delimiter::Brace => ("{", "}"),
                        Delimiter::Bracket => ("[", "]"),
                        Delimiter::None => ("", ""),
                    };
                    if tt.stream().into_iter().next().is_none() {
                        write!(f, "{} {}", start, end)?
                    } else {
                        write!(f, "{} {} {}", start, tt.stream(), end)?
                    }
                }
                TokenTree::Ident(ref tt) => write!(f, "{}", tt)?,
                TokenTree::Punct(ref tt) => {
                    write!(f, "{}", tt.as_char())?;
                    match tt.spacing() {
                        Spacing::Alone => {}
                        Spacing::Joint => joint = true,
                    }
                }
                TokenTree::Literal(ref tt) => write!(f, "{}", tt)?,
            }
        }

        Ok(())
    }
}

impl fmt::Debug for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("TokenStream ")?;
        f.debug_list().entries(self.clone()).finish()
    }
}

#[cfg(use_proc_macro)]
impl From<::proc_macro::TokenStream> for TokenStream {
    fn from(inner: ::proc_macro::TokenStream) -> TokenStream {
        inner
            .to_string()
            .parse()
            .expect("compiler token stream parse failed")
    }
}

#[cfg(use_proc_macro)]
impl From<TokenStream> for ::proc_macro::TokenStream {
    fn from(inner: TokenStream) -> ::proc_macro::TokenStream {
        inner
            .to_string()
            .parse()
            .expect("failed to parse to compiler tokens")
    }
}

impl From<TokenTree> for TokenStream {
    fn from(tree: TokenTree) -> TokenStream {
        TokenStream { inner: vec![tree] }
    }
}

impl iter::FromIterator<TokenTree> for TokenStream {
    fn from_iter<I: IntoIterator<Item = TokenTree>>(streams: I) -> Self {
        let mut v = Vec::new();

        for token in streams.into_iter() {
            v.push(token);
        }

        TokenStream { inner: v }
    }
}

impl iter::FromIterator<TokenStream> for TokenStream {
    fn from_iter<I: IntoIterator<Item = TokenStream>>(streams: I) -> Self {
        let mut v = Vec::new();

        for stream in streams.into_iter() {
            v.extend(stream.inner);
        }

        TokenStream { inner: v }
    }
}

impl Extend<TokenTree> for TokenStream {
    fn extend<I: IntoIterator<Item = TokenTree>>(&mut self, streams: I) {
        self.inner.extend(streams);
    }
}

impl Extend<TokenStream> for TokenStream {
    fn extend<I: IntoIterator<Item = TokenStream>>(&mut self, streams: I) {
        self.inner
            .extend(streams.into_iter().flat_map(|stream| stream));
    }
}

pub type TokenTreeIter = vec::IntoIter<TokenTree>;

impl IntoIterator for TokenStream {
    type Item = TokenTree;
    type IntoIter = TokenTreeIter;

    fn into_iter(self) -> TokenTreeIter {
        self.inner.into_iter()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SourceFile {
    path: PathBuf,
}

impl SourceFile {
    /// Get the path to this source file as a string.
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn is_real(&self) -> bool {
        // XXX(nika): Support real files in the future?
        false
    }
}

impl fmt::Debug for SourceFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("SourceFile")
            .field("path", &self.path())
            .field("is_real", &self.is_real())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

#[cfg(span_locations)]
thread_local! {
    static SOURCE_MAP: RefCell<SourceMap> = RefCell::new(SourceMap {
        // NOTE: We start with a single dummy file which all call_site() and
        // def_site() spans reference.
        files: vec![{
            #[cfg(procmacro2_semver_exempt)]
            {
                FileInfo {
                    name: "<unspecified>".to_owned(),
                    span: Span { lo: 0, hi: 0 },
                    lines: vec![0],
                }
            }

            #[cfg(not(procmacro2_semver_exempt))]
            {
                FileInfo {
                    span: Span { lo: 0, hi: 0 },
                    lines: vec![0],
                }
            }
        }],
    });
}

#[cfg(span_locations)]
struct FileInfo {
    #[cfg(procmacro2_semver_exempt)]
    name: String,
    span: Span,
    lines: Vec<usize>,
}

#[cfg(span_locations)]
impl FileInfo {
    fn offset_line_column(&self, offset: usize) -> LineColumn {
        assert!(self.span_within(Span {
            lo: offset as u32,
            hi: offset as u32
        }));
        let offset = offset - self.span.lo as usize;
        match self.lines.binary_search(&offset) {
            Ok(found) => LineColumn {
                line: found + 1,
                column: 0,
            },
            Err(idx) => LineColumn {
                line: idx,
                column: offset - self.lines[idx - 1],
            },
        }
    }

    fn span_within(&self, span: Span) -> bool {
        span.lo >= self.span.lo && span.hi <= self.span.hi
    }
}

/// Computesthe offsets of each line in the given source string.
#[cfg(span_locations)]
fn lines_offsets(s: &str) -> Vec<usize> {
    let mut lines = vec![0];
    let mut prev = 0;
    while let Some(len) = s[prev..].find('\n') {
        prev += len + 1;
        lines.push(prev);
    }
    lines
}

#[cfg(span_locations)]
struct SourceMap {
    files: Vec<FileInfo>,
}

#[cfg(span_locations)]
impl SourceMap {
    fn next_start_pos(&self) -> u32 {
        // Add 1 so there's always space between files.
        //
        // We'll always have at least 1 file, as we initialize our files list
        // with a dummy file.
        self.files.last().unwrap().span.hi + 1
    }

    fn add_file(&mut self, name: &str, src: &str) -> Span {
        let lines = lines_offsets(src);
        let lo = self.next_start_pos();
        // XXX(nika): Shouild we bother doing a checked cast or checked add here?
        let span = Span {
            lo: lo,
            hi: lo + (src.len() as u32),
        };

        #[cfg(procmacro2_semver_exempt)]
        self.files.push(FileInfo {
            name: name.to_owned(),
            span: span,
            lines: lines,
        });

        #[cfg(not(procmacro2_semver_exempt))]
        self.files.push(FileInfo {
            span: span,
            lines: lines,
        });
        let _ = name;

        span
    }

    fn fileinfo(&self, span: Span) -> &FileInfo {
        for file in &self.files {
            if file.span_within(span) {
                return file;
            }
        }
        panic!("Invalid span with no related FileInfo!");
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Span {
    #[cfg(span_locations)]
    lo: u32,
    #[cfg(span_locations)]
    hi: u32,
}

impl Span {
    #[cfg(not(span_locations))]
    pub fn call_site() -> Span {
        Span {}
    }

    #[cfg(span_locations)]
    pub fn call_site() -> Span {
        Span { lo: 0, hi: 0 }
    }

    #[cfg(procmacro2_semver_exempt)]
    pub fn def_site() -> Span {
        Span::call_site()
    }

    #[cfg(procmacro2_semver_exempt)]
    pub fn resolved_at(&self, _other: Span) -> Span {
        // Stable spans consist only of line/column information, so
        // `resolved_at` and `located_at` only select which span the
        // caller wants line/column information from.
        *self
    }

    #[cfg(procmacro2_semver_exempt)]
    pub fn located_at(&self, other: Span) -> Span {
        other
    }

    #[cfg(procmacro2_semver_exempt)]
    pub fn source_file(&self) -> SourceFile {
        SOURCE_MAP.with(|cm| {
            let cm = cm.borrow();
            let fi = cm.fileinfo(*self);
            SourceFile {
                path: Path::new(&fi.name).to_owned(),
            }
        })
    }

    #[cfg(span_locations)]
    pub fn start(&self) -> LineColumn {
        SOURCE_MAP.with(|cm| {
            let cm = cm.borrow();
            let fi = cm.fileinfo(*self);
            fi.offset_line_column(self.lo as usize)
        })
    }

    #[cfg(span_locations)]
    pub fn end(&self) -> LineColumn {
        SOURCE_MAP.with(|cm| {
            let cm = cm.borrow();
            let fi = cm.fileinfo(*self);
            fi.offset_line_column(self.hi as usize)
        })
    }

    #[cfg(procmacro2_semver_exempt)]
    pub fn join(&self, other: Span) -> Option<Span> {
        SOURCE_MAP.with(|cm| {
            let cm = cm.borrow();
            // If `other` is not within the same FileInfo as us, return None.
            if !cm.fileinfo(*self).span_within(other) {
                return None;
            }
            Some(Span {
                lo: cmp::min(self.lo, other.lo),
                hi: cmp::max(self.hi, other.hi),
            })
        })
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        #[cfg(procmacro2_semver_exempt)]
        return write!(f, "bytes({}..{})", self.lo, self.hi);

        #[cfg(not(procmacro2_semver_exempt))]
        write!(f, "Span")
    }
}

pub fn debug_span_field_if_nontrivial(debug: &mut fmt::DebugStruct, span: Span) {
    if cfg!(procmacro2_semver_exempt) {
        debug.field("span", &span);
    }
}

#[derive(Clone)]
pub struct Group {
    delimiter: Delimiter,
    stream: TokenStream,
    span: Span,
}

impl Group {
    pub fn new(delimiter: Delimiter, stream: TokenStream) -> Group {
        Group {
            delimiter: delimiter,
            stream: stream,
            span: Span::call_site(),
        }
    }

    pub fn delimiter(&self) -> Delimiter {
        self.delimiter
    }

    pub fn stream(&self) -> TokenStream {
        self.stream.clone()
    }

    pub fn span(&self) -> Span {
        self.span
    }

    #[cfg(procmacro2_semver_exempt)]
    pub fn span_open(&self) -> Span {
        self.span
    }

    #[cfg(procmacro2_semver_exempt)]
    pub fn span_close(&self) -> Span {
        self.span
    }

    pub fn set_span(&mut self, span: Span) {
        self.span = span;
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (left, right) = match self.delimiter {
            Delimiter::Parenthesis => ("(", ")"),
            Delimiter::Brace => ("{", "}"),
            Delimiter::Bracket => ("[", "]"),
            Delimiter::None => ("", ""),
        };

        f.write_str(left)?;
        self.stream.fmt(f)?;
        f.write_str(right)?;

        Ok(())
    }
}

impl fmt::Debug for Group {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = fmt.debug_struct("Group");
        debug.field("delimiter", &self.delimiter);
        debug.field("stream", &self.stream);
        #[cfg(procmacro2_semver_exempt)]
        debug.field("span", &self.span);
        debug.finish()
    }
}

#[derive(Clone)]
pub struct Ident {
    sym: String,
    span: Span,
    raw: bool,
}

impl Ident {
    fn _new(string: &str, raw: bool, span: Span) -> Ident {
        validate_ident(string);

        Ident {
            sym: string.to_owned(),
            span: span,
            raw: raw,
        }
    }

    pub fn new(string: &str, span: Span) -> Ident {
        Ident::_new(string, false, span)
    }

    pub fn new_raw(string: &str, span: Span) -> Ident {
        Ident::_new(string, true, span)
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn set_span(&mut self, span: Span) {
        self.span = span;
    }
}

#[inline]
fn is_ident_start(c: char) -> bool {
    ('a' <= c && c <= 'z')
        || ('A' <= c && c <= 'Z')
        || c == '_'
        || (c > '\x7f' && UnicodeXID::is_xid_start(c))
}

#[inline]
fn is_ident_continue(c: char) -> bool {
    ('a' <= c && c <= 'z')
        || ('A' <= c && c <= 'Z')
        || c == '_'
        || ('0' <= c && c <= '9')
        || (c > '\x7f' && UnicodeXID::is_xid_continue(c))
}

fn validate_ident(string: &str) {
    let validate = string;
    if validate.is_empty() {
        panic!("Ident is not allowed to be empty; use Option<Ident>");
    }

    if validate.bytes().all(|digit| digit >= b'0' && digit <= b'9') {
        panic!("Ident cannot be a number; use Literal instead");
    }

    fn ident_ok(string: &str) -> bool {
        let mut chars = string.chars();
        let first = chars.next().unwrap();
        if !is_ident_start(first) {
            return false;
        }
        for ch in chars {
            if !is_ident_continue(ch) {
                return false;
            }
        }
        true
    }

    if !ident_ok(validate) {
        panic!("{:?} is not a valid Ident", string);
    }
}

impl PartialEq for Ident {
    fn eq(&self, other: &Ident) -> bool {
        self.sym == other.sym && self.raw == other.raw
    }
}

impl<T> PartialEq<T> for Ident
where
    T: ?Sized + AsRef<str>,
{
    fn eq(&self, other: &T) -> bool {
        let other = other.as_ref();
        if self.raw {
            other.starts_with("r#") && self.sym == other[2..]
        } else {
            self.sym == other
        }
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.raw {
            "r#".fmt(f)?;
        }
        self.sym.fmt(f)
    }
}

impl fmt::Debug for Ident {
    // Ident(proc_macro), Ident(r#union)
    #[cfg(not(procmacro2_semver_exempt))]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = f.debug_tuple("Ident");
        debug.field(&format_args!("{}", self));
        debug.finish()
    }

    // Ident {
    //     sym: proc_macro,
    //     span: bytes(128..138)
    // }
    #[cfg(procmacro2_semver_exempt)]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = f.debug_struct("Ident");
        debug.field("sym", &format_args!("{}", self));
        debug.field("span", &self.span);
        debug.finish()
    }
}

#[derive(Clone)]
pub struct Literal {
    text: String,
    span: Span,
}

macro_rules! suffixed_numbers {
    ($($name:ident => $kind:ident,)*) => ($(
        pub fn $name(n: $kind) -> Literal {
            Literal::_new(format!(concat!("{}", stringify!($kind)), n))
        }
    )*)
}

macro_rules! unsuffixed_numbers {
    ($($name:ident => $kind:ident,)*) => ($(
        pub fn $name(n: $kind) -> Literal {
            Literal::_new(n.to_string())
        }
    )*)
}

impl Literal {
    fn _new(text: String) -> Literal {
        Literal {
            text: text,
            span: Span::call_site(),
        }
    }

    suffixed_numbers! {
        u8_suffixed => u8,
        u16_suffixed => u16,
        u32_suffixed => u32,
        u64_suffixed => u64,
        usize_suffixed => usize,
        i8_suffixed => i8,
        i16_suffixed => i16,
        i32_suffixed => i32,
        i64_suffixed => i64,
        isize_suffixed => isize,

        f32_suffixed => f32,
        f64_suffixed => f64,
    }

    #[cfg(u128)]
    suffixed_numbers! {
        u128_suffixed => u128,
        i128_suffixed => i128,
    }

    unsuffixed_numbers! {
        u8_unsuffixed => u8,
        u16_unsuffixed => u16,
        u32_unsuffixed => u32,
        u64_unsuffixed => u64,
        usize_unsuffixed => usize,
        i8_unsuffixed => i8,
        i16_unsuffixed => i16,
        i32_unsuffixed => i32,
        i64_unsuffixed => i64,
        isize_unsuffixed => isize,
    }

    #[cfg(u128)]
    unsuffixed_numbers! {
        u128_unsuffixed => u128,
        i128_unsuffixed => i128,
    }

    pub fn f32_unsuffixed(f: f32) -> Literal {
        let mut s = f.to_string();
        if !s.contains(".") {
            s.push_str(".0");
        }
        Literal::_new(s)
    }

    pub fn f64_unsuffixed(f: f64) -> Literal {
        let mut s = f.to_string();
        if !s.contains(".") {
            s.push_str(".0");
        }
        Literal::_new(s)
    }

    pub fn string(t: &str) -> Literal {
        let mut text = String::with_capacity(t.len() + 2);
        text.push('"');
        for c in t.chars() {
            if c == '\'' {
                // escape_default turns this into "\'" which is unnecessary.
                text.push(c);
            } else {
                text.extend(c.escape_default());
            }
        }
        text.push('"');
        Literal::_new(text)
    }

    pub fn character(t: char) -> Literal {
        let mut text = String::new();
        text.push('\'');
        if t == '"' {
            // escape_default turns this into '\"' which is unnecessary.
            text.push(t);
        } else {
            text.extend(t.escape_default());
        }
        text.push('\'');
        Literal::_new(text)
    }

    pub fn byte_string(bytes: &[u8]) -> Literal {
        let mut escaped = "b\"".to_string();
        for b in bytes {
            match *b {
                b'\0' => escaped.push_str(r"\0"),
                b'\t' => escaped.push_str(r"\t"),
                b'\n' => escaped.push_str(r"\n"),
                b'\r' => escaped.push_str(r"\r"),
                b'"' => escaped.push_str("\\\""),
                b'\\' => escaped.push_str("\\\\"),
                b'\x20'...b'\x7E' => escaped.push(*b as char),
                _ => escaped.push_str(&format!("\\x{:02X}", b)),
            }
        }
        escaped.push('"');
        Literal::_new(escaped)
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn set_span(&mut self, span: Span) {
        self.span = span;
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.text.fmt(f)
    }
}

impl fmt::Debug for Literal {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = fmt.debug_struct("Literal");
        debug.field("lit", &format_args!("{}", self.text));
        #[cfg(procmacro2_semver_exempt)]
        debug.field("span", &self.span);
        debug.finish()
    }
}

fn token_stream(mut input: Cursor) -> PResult<TokenStream> {
    let mut trees = Vec::new();
    loop {
        let input_no_ws = skip_whitespace(input);
        if input_no_ws.rest.len() == 0 {
            break;
        }
        if let Ok((a, tokens)) = doc_comment(input_no_ws) {
            input = a;
            trees.extend(tokens);
            continue;
        }

        let (a, tt) = match token_tree(input_no_ws) {
            Ok(p) => p,
            Err(_) => break,
        };
        trees.push(tt);
        input = a;
    }
    Ok((input, TokenStream { inner: trees }))
}

#[cfg(not(span_locations))]
fn spanned<'a, T>(
    input: Cursor<'a>,
    f: fn(Cursor<'a>) -> PResult<'a, T>,
) -> PResult<'a, (T, ::Span)> {
    let (a, b) = f(skip_whitespace(input))?;
    Ok((a, ((b, ::Span::_new_stable(Span::call_site())))))
}

#[cfg(span_locations)]
fn spanned<'a, T>(
    input: Cursor<'a>,
    f: fn(Cursor<'a>) -> PResult<'a, T>,
) -> PResult<'a, (T, ::Span)> {
    let input = skip_whitespace(input);
    let lo = input.off;
    let (a, b) = f(input)?;
    let hi = a.off;
    let span = ::Span::_new_stable(Span { lo: lo, hi: hi });
    Ok((a, (b, span)))
}

fn token_tree(input: Cursor) -> PResult<TokenTree> {
    let (rest, (mut tt, span)) = spanned(input, token_kind)?;
    tt.set_span(span);
    Ok((rest, tt))
}

named!(token_kind -> TokenTree, alt!(
    map!(group, |g| TokenTree::Group(::Group::_new_stable(g)))
    |
    map!(literal, |l| TokenTree::Literal(::Literal::_new_stable(l))) // must be before symbol
    |
    map!(op, TokenTree::Punct)
    |
    symbol_leading_ws
));

named!(group -> Group, alt!(
    delimited!(
        punct!("("),
        token_stream,
        punct!(")")
    ) => { |ts| Group::new(Delimiter::Parenthesis, ts) }
    |
    delimited!(
        punct!("["),
        token_stream,
        punct!("]")
    ) => { |ts| Group::new(Delimiter::Bracket, ts) }
    |
    delimited!(
        punct!("{"),
        token_stream,
        punct!("}")
    ) => { |ts| Group::new(Delimiter::Brace, ts) }
));

fn symbol_leading_ws(input: Cursor) -> PResult<TokenTree> {
    symbol(skip_whitespace(input))
}

fn symbol(input: Cursor) -> PResult<TokenTree> {
    let mut chars = input.char_indices();

    let raw = input.starts_with("r#");
    if raw {
        chars.next();
        chars.next();
    }

    match chars.next() {
        Some((_, ch)) if is_ident_start(ch) => {}
        _ => return Err(LexError),
    }

    let mut end = input.len();
    for (i, ch) in chars {
        if !is_ident_continue(ch) {
            end = i;
            break;
        }
    }

    let a = &input.rest[..end];
    if a == "r#_" {
        Err(LexError)
    } else {
        let ident = if raw {
            ::Ident::_new_raw(&a[2..], ::Span::call_site())
        } else {
            ::Ident::new(a, ::Span::call_site())
        };
        Ok((input.advance(end), ident.into()))
    }
}

fn literal(input: Cursor) -> PResult<Literal> {
    let input_no_ws = skip_whitespace(input);

    match literal_nocapture(input_no_ws) {
        Ok((a, ())) => {
            let start = input.len() - input_no_ws.len();
            let len = input_no_ws.len() - a.len();
            let end = start + len;
            Ok((a, Literal::_new(input.rest[start..end].to_string())))
        }
        Err(LexError) => Err(LexError),
    }
}

named!(literal_nocapture -> (), alt!(
    string
    |
    byte_string
    |
    byte
    |
    character
    |
    float
    |
    int
));

named!(string -> (), alt!(
    quoted_string
    |
    preceded!(
        punct!("r"),
        raw_string
    ) => { |_| () }
));

named!(quoted_string -> (), delimited!(
    punct!("\""),
    cooked_string,
    tag!("\"")
));

fn cooked_string(input: Cursor) -> PResult<()> {
    let mut chars = input.char_indices().peekable();
    while let Some((byte_offset, ch)) = chars.next() {
        match ch {
            '"' => {
                return Ok((input.advance(byte_offset), ()));
            }
            '\r' => {
                if let Some((_, '\n')) = chars.next() {
                    // ...
                } else {
                    break;
                }
            }
            '\\' => match chars.next() {
                Some((_, 'x')) => {
                    if !backslash_x_char(&mut chars) {
                        break;
                    }
                }
                Some((_, 'n')) | Some((_, 'r')) | Some((_, 't')) | Some((_, '\\'))
                | Some((_, '\'')) | Some((_, '"')) | Some((_, '0')) => {}
                Some((_, 'u')) => {
                    if !backslash_u(&mut chars) {
                        break;
                    }
                }
                Some((_, '\n')) | Some((_, '\r')) => {
                    while let Some(&(_, ch)) = chars.peek() {
                        if ch.is_whitespace() {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                _ => break,
            },
            _ch => {}
        }
    }
    Err(LexError)
}

named!(byte_string -> (), alt!(
    delimited!(
        punct!("b\""),
        cooked_byte_string,
        tag!("\"")
    ) => { |_| () }
    |
    preceded!(
        punct!("br"),
        raw_string
    ) => { |_| () }
));

fn cooked_byte_string(mut input: Cursor) -> PResult<()> {
    let mut bytes = input.bytes().enumerate();
    'outer: while let Some((offset, b)) = bytes.next() {
        match b {
            b'"' => {
                return Ok((input.advance(offset), ()));
            }
            b'\r' => {
                if let Some((_, b'\n')) = bytes.next() {
                    // ...
                } else {
                    break;
                }
            }
            b'\\' => match bytes.next() {
                Some((_, b'x')) => {
                    if !backslash_x_byte(&mut bytes) {
                        break;
                    }
                }
                Some((_, b'n')) | Some((_, b'r')) | Some((_, b't')) | Some((_, b'\\'))
                | Some((_, b'0')) | Some((_, b'\'')) | Some((_, b'"')) => {}
                Some((newline, b'\n')) | Some((newline, b'\r')) => {
                    let rest = input.advance(newline + 1);
                    for (offset, ch) in rest.char_indices() {
                        if !ch.is_whitespace() {
                            input = rest.advance(offset);
                            bytes = input.bytes().enumerate();
                            continue 'outer;
                        }
                    }
                    break;
                }
                _ => break,
            },
            b if b < 0x80 => {}
            _ => break,
        }
    }
    Err(LexError)
}

fn raw_string(input: Cursor) -> PResult<()> {
    let mut chars = input.char_indices();
    let mut n = 0;
    while let Some((byte_offset, ch)) = chars.next() {
        match ch {
            '"' => {
                n = byte_offset;
                break;
            }
            '#' => {}
            _ => return Err(LexError),
        }
    }
    for (byte_offset, ch) in chars {
        match ch {
            '"' if input.advance(byte_offset + 1).starts_with(&input.rest[..n]) => {
                let rest = input.advance(byte_offset + 1 + n);
                return Ok((rest, ()));
            }
            '\r' => {}
            _ => {}
        }
    }
    Err(LexError)
}

named!(byte -> (), do_parse!(
    punct!("b") >>
    tag!("'") >>
    cooked_byte >>
    tag!("'") >>
    (())
));

fn cooked_byte(input: Cursor) -> PResult<()> {
    let mut bytes = input.bytes().enumerate();
    let ok = match bytes.next().map(|(_, b)| b) {
        Some(b'\\') => match bytes.next().map(|(_, b)| b) {
            Some(b'x') => backslash_x_byte(&mut bytes),
            Some(b'n') | Some(b'r') | Some(b't') | Some(b'\\') | Some(b'0') | Some(b'\'')
            | Some(b'"') => true,
            _ => false,
        },
        b => b.is_some(),
    };
    if ok {
        match bytes.next() {
            Some((offset, _)) => {
                if input.chars().as_str().is_char_boundary(offset) {
                    Ok((input.advance(offset), ()))
                } else {
                    Err(LexError)
                }
            }
            None => Ok((input.advance(input.len()), ())),
        }
    } else {
        Err(LexError)
    }
}

named!(character -> (), do_parse!(
    punct!("'") >>
    cooked_char >>
    tag!("'") >>
    (())
));

fn cooked_char(input: Cursor) -> PResult<()> {
    let mut chars = input.char_indices();
    let ok = match chars.next().map(|(_, ch)| ch) {
        Some('\\') => match chars.next().map(|(_, ch)| ch) {
            Some('x') => backslash_x_char(&mut chars),
            Some('u') => backslash_u(&mut chars),
            Some('n') | Some('r') | Some('t') | Some('\\') | Some('0') | Some('\'') | Some('"') => {
                true
            }
            _ => false,
        },
        ch => ch.is_some(),
    };
    if ok {
        match chars.next() {
            Some((idx, _)) => Ok((input.advance(idx), ())),
            None => Ok((input.advance(input.len()), ())),
        }
    } else {
        Err(LexError)
    }
}

macro_rules! next_ch {
    ($chars:ident @ $pat:pat $(| $rest:pat)*) => {
        match $chars.next() {
            Some((_, ch)) => match ch {
                $pat $(| $rest)*  => ch,
                _ => return false,
            },
            None => return false
        }
    };
}

fn backslash_x_char<I>(chars: &mut I) -> bool
where
    I: Iterator<Item = (usize, char)>,
{
    next_ch!(chars @ '0'...'7');
    next_ch!(chars @ '0'...'9' | 'a'...'f' | 'A'...'F');
    true
}

fn backslash_x_byte<I>(chars: &mut I) -> bool
where
    I: Iterator<Item = (usize, u8)>,
{
    next_ch!(chars @ b'0'...b'9' | b'a'...b'f' | b'A'...b'F');
    next_ch!(chars @ b'0'...b'9' | b'a'...b'f' | b'A'...b'F');
    true
}

fn backslash_u<I>(chars: &mut I) -> bool
where
    I: Iterator<Item = (usize, char)>,
{
    next_ch!(chars @ '{');
    next_ch!(chars @ '0'...'9' | 'a'...'f' | 'A'...'F');
    loop {
        let c = next_ch!(chars @ '0'...'9' | 'a'...'f' | 'A'...'F' | '_' | '}');
        if c == '}' {
            return true;
        }
    }
}

fn float(input: Cursor) -> PResult<()> {
    let (rest, ()) = float_digits(input)?;
    for suffix in &["f32", "f64"] {
        if rest.starts_with(suffix) {
            return word_break(rest.advance(suffix.len()));
        }
    }
    word_break(rest)
}

fn float_digits(input: Cursor) -> PResult<()> {
    let mut chars = input.chars().peekable();
    match chars.next() {
        Some(ch) if ch >= '0' && ch <= '9' => {}
        _ => return Err(LexError),
    }

    let mut len = 1;
    let mut has_dot = false;
    let mut has_exp = false;
    while let Some(&ch) = chars.peek() {
        match ch {
            '0'...'9' | '_' => {
                chars.next();
                len += 1;
            }
            '.' => {
                if has_dot {
                    break;
                }
                chars.next();
                if chars
                    .peek()
                    .map(|&ch| ch == '.' || UnicodeXID::is_xid_start(ch))
                    .unwrap_or(false)
                {
                    return Err(LexError);
                }
                len += 1;
                has_dot = true;
            }
            'e' | 'E' => {
                chars.next();
                len += 1;
                has_exp = true;
                break;
            }
            _ => break,
        }
    }

    let rest = input.advance(len);
    if !(has_dot || has_exp || rest.starts_with("f32") || rest.starts_with("f64")) {
        return Err(LexError);
    }

    if has_exp {
        let mut has_exp_value = false;
        while let Some(&ch) = chars.peek() {
            match ch {
                '+' | '-' => {
                    if has_exp_value {
                        break;
                    }
                    chars.next();
                    len += 1;
                }
                '0'...'9' => {
                    chars.next();
                    len += 1;
                    has_exp_value = true;
                }
                '_' => {
                    chars.next();
                    len += 1;
                }
                _ => break,
            }
        }
        if !has_exp_value {
            return Err(LexError);
        }
    }

    Ok((input.advance(len), ()))
}

fn int(input: Cursor) -> PResult<()> {
    let (rest, ()) = digits(input)?;
    for suffix in &[
        "isize", "i8", "i16", "i32", "i64", "i128", "usize", "u8", "u16", "u32", "u64", "u128",
    ] {
        if rest.starts_with(suffix) {
            return word_break(rest.advance(suffix.len()));
        }
    }
    word_break(rest)
}

fn digits(mut input: Cursor) -> PResult<()> {
    let base = if input.starts_with("0x") {
        input = input.advance(2);
        16
    } else if input.starts_with("0o") {
        input = input.advance(2);
        8
    } else if input.starts_with("0b") {
        input = input.advance(2);
        2
    } else {
        10
    };

    let mut len = 0;
    let mut empty = true;
    for b in input.bytes() {
        let digit = match b {
            b'0'...b'9' => (b - b'0') as u64,
            b'a'...b'f' => 10 + (b - b'a') as u64,
            b'A'...b'F' => 10 + (b - b'A') as u64,
            b'_' => {
                if empty && base == 10 {
                    return Err(LexError);
                }
                len += 1;
                continue;
            }
            _ => break,
        };
        if digit >= base {
            return Err(LexError);
        }
        len += 1;
        empty = false;
    }
    if empty {
        Err(LexError)
    } else {
        Ok((input.advance(len), ()))
    }
}

fn op(input: Cursor) -> PResult<Punct> {
    let input = skip_whitespace(input);
    match op_char(input) {
        Ok((rest, '\'')) => {
            symbol(rest)?;
            Ok((rest, Punct::new('\'', Spacing::Joint)))
        }
        Ok((rest, ch)) => {
            let kind = match op_char(rest) {
                Ok(_) => Spacing::Joint,
                Err(LexError) => Spacing::Alone,
            };
            Ok((rest, Punct::new(ch, kind)))
        }
        Err(LexError) => Err(LexError),
    }
}

fn op_char(input: Cursor) -> PResult<char> {
    if input.starts_with("//") || input.starts_with("/*") {
        // Do not accept `/` of a comment as an op.
        return Err(LexError);
    }

    let mut chars = input.chars();
    let first = match chars.next() {
        Some(ch) => ch,
        None => {
            return Err(LexError);
        }
    };
    let recognized = "~!@#$%^&*-=+|;:,<.>/?'";
    if recognized.contains(first) {
        Ok((input.advance(first.len_utf8()), first))
    } else {
        Err(LexError)
    }
}

fn doc_comment(input: Cursor) -> PResult<Vec<TokenTree>> {
    let mut trees = Vec::new();
    let (rest, ((comment, inner), span)) = spanned(input, doc_comment_contents)?;
    trees.push(TokenTree::Punct(Punct::new('#', Spacing::Alone)));
    if inner {
        trees.push(Punct::new('!', Spacing::Alone).into());
    }
    let mut stream = vec![
        TokenTree::Ident(::Ident::new("doc", span)),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Literal(::Literal::string(comment)),
    ];
    for tt in stream.iter_mut() {
        tt.set_span(span);
    }
    let group = Group::new(Delimiter::Bracket, stream.into_iter().collect());
    trees.push(::Group::_new_stable(group).into());
    for tt in trees.iter_mut() {
        tt.set_span(span);
    }
    Ok((rest, trees))
}

named!(doc_comment_contents -> (&str, bool), alt!(
    do_parse!(
        punct!("//!") >>
        s: take_until_newline_or_eof!() >>
        ((s, true))
    )
    |
    do_parse!(
        option!(whitespace) >>
        peek!(tag!("/*!")) >>
        s: block_comment >>
        ((s, true))
    )
    |
    do_parse!(
        punct!("///") >>
        not!(tag!("/")) >>
        s: take_until_newline_or_eof!() >>
        ((s, false))
    )
    |
    do_parse!(
        option!(whitespace) >>
        peek!(tuple!(tag!("/**"), not!(tag!("*")))) >>
        s: block_comment >>
        ((s, false))
    )
));
