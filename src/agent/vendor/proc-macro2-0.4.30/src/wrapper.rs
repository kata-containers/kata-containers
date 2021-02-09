use std::fmt;
use std::iter;
use std::panic::{self, PanicInfo};
#[cfg(super_unstable)]
use std::path::PathBuf;
use std::str::FromStr;

use fallback;
use proc_macro;

use {Delimiter, Punct, Spacing, TokenTree};

#[derive(Clone)]
pub enum TokenStream {
    Compiler(proc_macro::TokenStream),
    Fallback(fallback::TokenStream),
}

pub enum LexError {
    Compiler(proc_macro::LexError),
    Fallback(fallback::LexError),
}

fn nightly_works() -> bool {
    use std::sync::atomic::*;
    use std::sync::Once;

    #[allow(deprecated)]
    static WORKS: AtomicUsize = ATOMIC_USIZE_INIT;
    static INIT: Once = Once::new();

    match WORKS.load(Ordering::SeqCst) {
        1 => return false,
        2 => return true,
        _ => {}
    }

    // Swap in a null panic hook to avoid printing "thread panicked" to stderr,
    // then use catch_unwind to determine whether the compiler's proc_macro is
    // working. When proc-macro2 is used from outside of a procedural macro all
    // of the proc_macro crate's APIs currently panic.
    //
    // The Once is to prevent the possibility of this ordering:
    //
    //     thread 1 calls take_hook, gets the user's original hook
    //     thread 1 calls set_hook with the null hook
    //     thread 2 calls take_hook, thinks null hook is the original hook
    //     thread 2 calls set_hook with the null hook
    //     thread 1 calls set_hook with the actual original hook
    //     thread 2 calls set_hook with what it thinks is the original hook
    //
    // in which the user's hook has been lost.
    //
    // There is still a race condition where a panic in a different thread can
    // happen during the interval that the user's original panic hook is
    // unregistered such that their hook is incorrectly not called. This is
    // sufficiently unlikely and less bad than printing panic messages to stderr
    // on correct use of this crate. Maybe there is a libstd feature request
    // here. For now, if a user needs to guarantee that this failure mode does
    // not occur, they need to call e.g. `proc_macro2::Span::call_site()` from
    // the main thread before launching any other threads.
    INIT.call_once(|| {
        type PanicHook = Fn(&PanicInfo) + Sync + Send + 'static;

        let null_hook: Box<PanicHook> = Box::new(|_panic_info| { /* ignore */ });
        let sanity_check = &*null_hook as *const PanicHook;
        let original_hook = panic::take_hook();
        panic::set_hook(null_hook);

        let works = panic::catch_unwind(|| proc_macro::Span::call_site()).is_ok();
        WORKS.store(works as usize + 1, Ordering::SeqCst);

        let hopefully_null_hook = panic::take_hook();
        panic::set_hook(original_hook);
        if sanity_check != &*hopefully_null_hook {
            panic!("observed race condition in proc_macro2::nightly_works");
        }
    });
    nightly_works()
}

fn mismatch() -> ! {
    panic!("stable/nightly mismatch")
}

impl TokenStream {
    pub fn new() -> TokenStream {
        if nightly_works() {
            TokenStream::Compiler(proc_macro::TokenStream::new())
        } else {
            TokenStream::Fallback(fallback::TokenStream::new())
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            TokenStream::Compiler(tts) => tts.is_empty(),
            TokenStream::Fallback(tts) => tts.is_empty(),
        }
    }

    fn unwrap_nightly(self) -> proc_macro::TokenStream {
        match self {
            TokenStream::Compiler(s) => s,
            TokenStream::Fallback(_) => mismatch(),
        }
    }

    fn unwrap_stable(self) -> fallback::TokenStream {
        match self {
            TokenStream::Compiler(_) => mismatch(),
            TokenStream::Fallback(s) => s,
        }
    }
}

impl FromStr for TokenStream {
    type Err = LexError;

    fn from_str(src: &str) -> Result<TokenStream, LexError> {
        if nightly_works() {
            Ok(TokenStream::Compiler(src.parse()?))
        } else {
            Ok(TokenStream::Fallback(src.parse()?))
        }
    }
}

impl fmt::Display for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TokenStream::Compiler(tts) => tts.fmt(f),
            TokenStream::Fallback(tts) => tts.fmt(f),
        }
    }
}

impl From<proc_macro::TokenStream> for TokenStream {
    fn from(inner: proc_macro::TokenStream) -> TokenStream {
        TokenStream::Compiler(inner)
    }
}

impl From<TokenStream> for proc_macro::TokenStream {
    fn from(inner: TokenStream) -> proc_macro::TokenStream {
        match inner {
            TokenStream::Compiler(inner) => inner,
            TokenStream::Fallback(inner) => inner.to_string().parse().unwrap(),
        }
    }
}

impl From<fallback::TokenStream> for TokenStream {
    fn from(inner: fallback::TokenStream) -> TokenStream {
        TokenStream::Fallback(inner)
    }
}

impl From<TokenTree> for TokenStream {
    fn from(token: TokenTree) -> TokenStream {
        if !nightly_works() {
            return TokenStream::Fallback(token.into());
        }
        let tt: proc_macro::TokenTree = match token {
            TokenTree::Group(tt) => tt.inner.unwrap_nightly().into(),
            TokenTree::Punct(tt) => {
                let spacing = match tt.spacing() {
                    Spacing::Joint => proc_macro::Spacing::Joint,
                    Spacing::Alone => proc_macro::Spacing::Alone,
                };
                let mut op = proc_macro::Punct::new(tt.as_char(), spacing);
                op.set_span(tt.span().inner.unwrap_nightly());
                op.into()
            }
            TokenTree::Ident(tt) => tt.inner.unwrap_nightly().into(),
            TokenTree::Literal(tt) => tt.inner.unwrap_nightly().into(),
        };
        TokenStream::Compiler(tt.into())
    }
}

impl iter::FromIterator<TokenTree> for TokenStream {
    fn from_iter<I: IntoIterator<Item = TokenTree>>(trees: I) -> Self {
        if nightly_works() {
            let trees = trees
                .into_iter()
                .map(TokenStream::from)
                .flat_map(|t| match t {
                    TokenStream::Compiler(s) => s,
                    TokenStream::Fallback(_) => mismatch(),
                });
            TokenStream::Compiler(trees.collect())
        } else {
            TokenStream::Fallback(trees.into_iter().collect())
        }
    }
}

impl iter::FromIterator<TokenStream> for TokenStream {
    fn from_iter<I: IntoIterator<Item = TokenStream>>(streams: I) -> Self {
        let mut streams = streams.into_iter();
        match streams.next() {
            #[cfg(slow_extend)]
            Some(TokenStream::Compiler(first)) => {
                let stream = iter::once(first)
                    .chain(streams.map(|s| match s {
                        TokenStream::Compiler(s) => s,
                        TokenStream::Fallback(_) => mismatch(),
                    }))
                    .collect();
                TokenStream::Compiler(stream)
            }
            #[cfg(not(slow_extend))]
            Some(TokenStream::Compiler(mut first)) => {
                first.extend(streams.map(|s| match s {
                    TokenStream::Compiler(s) => s,
                    TokenStream::Fallback(_) => mismatch(),
                }));
                TokenStream::Compiler(first)
            }
            Some(TokenStream::Fallback(mut first)) => {
                first.extend(streams.map(|s| match s {
                    TokenStream::Fallback(s) => s,
                    TokenStream::Compiler(_) => mismatch(),
                }));
                TokenStream::Fallback(first)
            }
            None => TokenStream::new(),
        }
    }
}

impl Extend<TokenTree> for TokenStream {
    fn extend<I: IntoIterator<Item = TokenTree>>(&mut self, streams: I) {
        match self {
            TokenStream::Compiler(tts) => {
                #[cfg(not(slow_extend))]
                {
                    tts.extend(
                        streams
                            .into_iter()
                            .map(|t| TokenStream::from(t).unwrap_nightly()),
                    );
                }
                #[cfg(slow_extend)]
                {
                    *tts =
                        tts.clone()
                            .into_iter()
                            .chain(streams.into_iter().map(TokenStream::from).flat_map(
                                |t| match t {
                                    TokenStream::Compiler(tts) => tts.into_iter(),
                                    _ => mismatch(),
                                },
                            ))
                            .collect();
                }
            }
            TokenStream::Fallback(tts) => tts.extend(streams),
        }
    }
}

impl Extend<TokenStream> for TokenStream {
    fn extend<I: IntoIterator<Item = TokenStream>>(&mut self, streams: I) {
        match self {
            TokenStream::Compiler(tts) => {
                #[cfg(not(slow_extend))]
                {
                    tts.extend(streams.into_iter().map(|stream| stream.unwrap_nightly()));
                }
                #[cfg(slow_extend)]
                {
                    *tts = tts
                        .clone()
                        .into_iter()
                        .chain(streams.into_iter().flat_map(|t| match t {
                            TokenStream::Compiler(tts) => tts.into_iter(),
                            _ => mismatch(),
                        }))
                        .collect();
                }
            }
            TokenStream::Fallback(tts) => {
                tts.extend(streams.into_iter().map(|stream| stream.unwrap_stable()))
            }
        }
    }
}

impl fmt::Debug for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TokenStream::Compiler(tts) => tts.fmt(f),
            TokenStream::Fallback(tts) => tts.fmt(f),
        }
    }
}

impl From<proc_macro::LexError> for LexError {
    fn from(e: proc_macro::LexError) -> LexError {
        LexError::Compiler(e)
    }
}

impl From<fallback::LexError> for LexError {
    fn from(e: fallback::LexError) -> LexError {
        LexError::Fallback(e)
    }
}

impl fmt::Debug for LexError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LexError::Compiler(e) => e.fmt(f),
            LexError::Fallback(e) => e.fmt(f),
        }
    }
}

#[derive(Clone)]
pub enum TokenTreeIter {
    Compiler(proc_macro::token_stream::IntoIter),
    Fallback(fallback::TokenTreeIter),
}

impl IntoIterator for TokenStream {
    type Item = TokenTree;
    type IntoIter = TokenTreeIter;

    fn into_iter(self) -> TokenTreeIter {
        match self {
            TokenStream::Compiler(tts) => TokenTreeIter::Compiler(tts.into_iter()),
            TokenStream::Fallback(tts) => TokenTreeIter::Fallback(tts.into_iter()),
        }
    }
}

impl Iterator for TokenTreeIter {
    type Item = TokenTree;

    fn next(&mut self) -> Option<TokenTree> {
        let token = match self {
            TokenTreeIter::Compiler(iter) => iter.next()?,
            TokenTreeIter::Fallback(iter) => return iter.next(),
        };
        Some(match token {
            proc_macro::TokenTree::Group(tt) => ::Group::_new(Group::Compiler(tt)).into(),
            proc_macro::TokenTree::Punct(tt) => {
                let spacing = match tt.spacing() {
                    proc_macro::Spacing::Joint => Spacing::Joint,
                    proc_macro::Spacing::Alone => Spacing::Alone,
                };
                let mut o = Punct::new(tt.as_char(), spacing);
                o.set_span(::Span::_new(Span::Compiler(tt.span())));
                o.into()
            }
            proc_macro::TokenTree::Ident(s) => ::Ident::_new(Ident::Compiler(s)).into(),
            proc_macro::TokenTree::Literal(l) => ::Literal::_new(Literal::Compiler(l)).into(),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            TokenTreeIter::Compiler(tts) => tts.size_hint(),
            TokenTreeIter::Fallback(tts) => tts.size_hint(),
        }
    }
}

impl fmt::Debug for TokenTreeIter {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TokenTreeIter").finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
#[cfg(super_unstable)]
pub enum SourceFile {
    Compiler(proc_macro::SourceFile),
    Fallback(fallback::SourceFile),
}

#[cfg(super_unstable)]
impl SourceFile {
    fn nightly(sf: proc_macro::SourceFile) -> Self {
        SourceFile::Compiler(sf)
    }

    /// Get the path to this source file as a string.
    pub fn path(&self) -> PathBuf {
        match self {
            SourceFile::Compiler(a) => a.path(),
            SourceFile::Fallback(a) => a.path(),
        }
    }

    pub fn is_real(&self) -> bool {
        match self {
            SourceFile::Compiler(a) => a.is_real(),
            SourceFile::Fallback(a) => a.is_real(),
        }
    }
}

#[cfg(super_unstable)]
impl fmt::Debug for SourceFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SourceFile::Compiler(a) => a.fmt(f),
            SourceFile::Fallback(a) => a.fmt(f),
        }
    }
}

#[cfg(any(super_unstable, feature = "span-locations"))]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

#[derive(Copy, Clone)]
pub enum Span {
    Compiler(proc_macro::Span),
    Fallback(fallback::Span),
}

impl Span {
    pub fn call_site() -> Span {
        if nightly_works() {
            Span::Compiler(proc_macro::Span::call_site())
        } else {
            Span::Fallback(fallback::Span::call_site())
        }
    }

    #[cfg(super_unstable)]
    pub fn def_site() -> Span {
        if nightly_works() {
            Span::Compiler(proc_macro::Span::def_site())
        } else {
            Span::Fallback(fallback::Span::def_site())
        }
    }

    #[cfg(super_unstable)]
    pub fn resolved_at(&self, other: Span) -> Span {
        match (self, other) {
            (Span::Compiler(a), Span::Compiler(b)) => Span::Compiler(a.resolved_at(b)),
            (Span::Fallback(a), Span::Fallback(b)) => Span::Fallback(a.resolved_at(b)),
            _ => mismatch(),
        }
    }

    #[cfg(super_unstable)]
    pub fn located_at(&self, other: Span) -> Span {
        match (self, other) {
            (Span::Compiler(a), Span::Compiler(b)) => Span::Compiler(a.located_at(b)),
            (Span::Fallback(a), Span::Fallback(b)) => Span::Fallback(a.located_at(b)),
            _ => mismatch(),
        }
    }

    pub fn unwrap(self) -> proc_macro::Span {
        match self {
            Span::Compiler(s) => s,
            Span::Fallback(_) => panic!("proc_macro::Span is only available in procedural macros"),
        }
    }

    #[cfg(super_unstable)]
    pub fn source_file(&self) -> SourceFile {
        match self {
            Span::Compiler(s) => SourceFile::nightly(s.source_file()),
            Span::Fallback(s) => SourceFile::Fallback(s.source_file()),
        }
    }

    #[cfg(any(super_unstable, feature = "span-locations"))]
    pub fn start(&self) -> LineColumn {
        match self {
            #[cfg(proc_macro_span)]
            Span::Compiler(s) => {
                let proc_macro::LineColumn { line, column } = s.start();
                LineColumn { line, column }
            }
            #[cfg(not(proc_macro_span))]
            Span::Compiler(_) => LineColumn { line: 0, column: 0 },
            Span::Fallback(s) => {
                let fallback::LineColumn { line, column } = s.start();
                LineColumn { line, column }
            }
        }
    }

    #[cfg(any(super_unstable, feature = "span-locations"))]
    pub fn end(&self) -> LineColumn {
        match self {
            #[cfg(proc_macro_span)]
            Span::Compiler(s) => {
                let proc_macro::LineColumn { line, column } = s.end();
                LineColumn { line, column }
            }
            #[cfg(not(proc_macro_span))]
            Span::Compiler(_) => LineColumn { line: 0, column: 0 },
            Span::Fallback(s) => {
                let fallback::LineColumn { line, column } = s.end();
                LineColumn { line, column }
            }
        }
    }

    #[cfg(super_unstable)]
    pub fn join(&self, other: Span) -> Option<Span> {
        let ret = match (self, other) {
            (Span::Compiler(a), Span::Compiler(b)) => Span::Compiler(a.join(b)?),
            (Span::Fallback(a), Span::Fallback(b)) => Span::Fallback(a.join(b)?),
            _ => return None,
        };
        Some(ret)
    }

    #[cfg(super_unstable)]
    pub fn eq(&self, other: &Span) -> bool {
        match (self, other) {
            (Span::Compiler(a), Span::Compiler(b)) => a.eq(b),
            (Span::Fallback(a), Span::Fallback(b)) => a.eq(b),
            _ => false,
        }
    }

    fn unwrap_nightly(self) -> proc_macro::Span {
        match self {
            Span::Compiler(s) => s,
            Span::Fallback(_) => mismatch(),
        }
    }
}

impl From<proc_macro::Span> for ::Span {
    fn from(proc_span: proc_macro::Span) -> ::Span {
        ::Span::_new(Span::Compiler(proc_span))
    }
}

impl From<fallback::Span> for Span {
    fn from(inner: fallback::Span) -> Span {
        Span::Fallback(inner)
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Span::Compiler(s) => s.fmt(f),
            Span::Fallback(s) => s.fmt(f),
        }
    }
}

pub fn debug_span_field_if_nontrivial(debug: &mut fmt::DebugStruct, span: Span) {
    match span {
        Span::Compiler(s) => {
            debug.field("span", &s);
        }
        Span::Fallback(s) => fallback::debug_span_field_if_nontrivial(debug, s),
    }
}

#[derive(Clone)]
pub enum Group {
    Compiler(proc_macro::Group),
    Fallback(fallback::Group),
}

impl Group {
    pub fn new(delimiter: Delimiter, stream: TokenStream) -> Group {
        match stream {
            TokenStream::Compiler(stream) => {
                let delimiter = match delimiter {
                    Delimiter::Parenthesis => proc_macro::Delimiter::Parenthesis,
                    Delimiter::Bracket => proc_macro::Delimiter::Bracket,
                    Delimiter::Brace => proc_macro::Delimiter::Brace,
                    Delimiter::None => proc_macro::Delimiter::None,
                };
                Group::Compiler(proc_macro::Group::new(delimiter, stream))
            }
            TokenStream::Fallback(stream) => {
                Group::Fallback(fallback::Group::new(delimiter, stream))
            }
        }
    }

    pub fn delimiter(&self) -> Delimiter {
        match self {
            Group::Compiler(g) => match g.delimiter() {
                proc_macro::Delimiter::Parenthesis => Delimiter::Parenthesis,
                proc_macro::Delimiter::Bracket => Delimiter::Bracket,
                proc_macro::Delimiter::Brace => Delimiter::Brace,
                proc_macro::Delimiter::None => Delimiter::None,
            },
            Group::Fallback(g) => g.delimiter(),
        }
    }

    pub fn stream(&self) -> TokenStream {
        match self {
            Group::Compiler(g) => TokenStream::Compiler(g.stream()),
            Group::Fallback(g) => TokenStream::Fallback(g.stream()),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Group::Compiler(g) => Span::Compiler(g.span()),
            Group::Fallback(g) => Span::Fallback(g.span()),
        }
    }

    #[cfg(super_unstable)]
    pub fn span_open(&self) -> Span {
        match self {
            Group::Compiler(g) => Span::Compiler(g.span_open()),
            Group::Fallback(g) => Span::Fallback(g.span_open()),
        }
    }

    #[cfg(super_unstable)]
    pub fn span_close(&self) -> Span {
        match self {
            Group::Compiler(g) => Span::Compiler(g.span_close()),
            Group::Fallback(g) => Span::Fallback(g.span_close()),
        }
    }

    pub fn set_span(&mut self, span: Span) {
        match (self, span) {
            (Group::Compiler(g), Span::Compiler(s)) => g.set_span(s),
            (Group::Fallback(g), Span::Fallback(s)) => g.set_span(s),
            _ => mismatch(),
        }
    }

    fn unwrap_nightly(self) -> proc_macro::Group {
        match self {
            Group::Compiler(g) => g,
            Group::Fallback(_) => mismatch(),
        }
    }
}

impl From<fallback::Group> for Group {
    fn from(g: fallback::Group) -> Self {
        Group::Fallback(g)
    }
}

impl fmt::Display for Group {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Group::Compiler(group) => group.fmt(formatter),
            Group::Fallback(group) => group.fmt(formatter),
        }
    }
}

impl fmt::Debug for Group {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Group::Compiler(group) => group.fmt(formatter),
            Group::Fallback(group) => group.fmt(formatter),
        }
    }
}

#[derive(Clone)]
pub enum Ident {
    Compiler(proc_macro::Ident),
    Fallback(fallback::Ident),
}

impl Ident {
    pub fn new(string: &str, span: Span) -> Ident {
        match span {
            Span::Compiler(s) => Ident::Compiler(proc_macro::Ident::new(string, s)),
            Span::Fallback(s) => Ident::Fallback(fallback::Ident::new(string, s)),
        }
    }

    pub fn new_raw(string: &str, span: Span) -> Ident {
        match span {
            Span::Compiler(s) => {
                let p: proc_macro::TokenStream = string.parse().unwrap();
                let ident = match p.into_iter().next() {
                    Some(proc_macro::TokenTree::Ident(mut i)) => {
                        i.set_span(s);
                        i
                    }
                    _ => panic!(),
                };
                Ident::Compiler(ident)
            }
            Span::Fallback(s) => Ident::Fallback(fallback::Ident::new_raw(string, s)),
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Ident::Compiler(t) => Span::Compiler(t.span()),
            Ident::Fallback(t) => Span::Fallback(t.span()),
        }
    }

    pub fn set_span(&mut self, span: Span) {
        match (self, span) {
            (Ident::Compiler(t), Span::Compiler(s)) => t.set_span(s),
            (Ident::Fallback(t), Span::Fallback(s)) => t.set_span(s),
            _ => mismatch(),
        }
    }

    fn unwrap_nightly(self) -> proc_macro::Ident {
        match self {
            Ident::Compiler(s) => s,
            Ident::Fallback(_) => mismatch(),
        }
    }
}

impl PartialEq for Ident {
    fn eq(&self, other: &Ident) -> bool {
        match (self, other) {
            (Ident::Compiler(t), Ident::Compiler(o)) => t.to_string() == o.to_string(),
            (Ident::Fallback(t), Ident::Fallback(o)) => t == o,
            _ => mismatch(),
        }
    }
}

impl<T> PartialEq<T> for Ident
where
    T: ?Sized + AsRef<str>,
{
    fn eq(&self, other: &T) -> bool {
        let other = other.as_ref();
        match self {
            Ident::Compiler(t) => t.to_string() == other,
            Ident::Fallback(t) => t == other,
        }
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Ident::Compiler(t) => t.fmt(f),
            Ident::Fallback(t) => t.fmt(f),
        }
    }
}

impl fmt::Debug for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Ident::Compiler(t) => t.fmt(f),
            Ident::Fallback(t) => t.fmt(f),
        }
    }
}

#[derive(Clone)]
pub enum Literal {
    Compiler(proc_macro::Literal),
    Fallback(fallback::Literal),
}

macro_rules! suffixed_numbers {
    ($($name:ident => $kind:ident,)*) => ($(
        pub fn $name(n: $kind) -> Literal {
            if nightly_works() {
                Literal::Compiler(proc_macro::Literal::$name(n))
            } else {
                Literal::Fallback(fallback::Literal::$name(n))
            }
        }
    )*)
}

macro_rules! unsuffixed_integers {
    ($($name:ident => $kind:ident,)*) => ($(
        pub fn $name(n: $kind) -> Literal {
            if nightly_works() {
                Literal::Compiler(proc_macro::Literal::$name(n))
            } else {
                Literal::Fallback(fallback::Literal::$name(n))
            }
        }
    )*)
}

impl Literal {
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
        i128_suffixed => i128,
        u128_suffixed => u128,
    }

    unsuffixed_integers! {
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
    unsuffixed_integers! {
        i128_unsuffixed => i128,
        u128_unsuffixed => u128,
    }

    pub fn f32_unsuffixed(f: f32) -> Literal {
        if nightly_works() {
            Literal::Compiler(proc_macro::Literal::f32_unsuffixed(f))
        } else {
            Literal::Fallback(fallback::Literal::f32_unsuffixed(f))
        }
    }

    pub fn f64_unsuffixed(f: f64) -> Literal {
        if nightly_works() {
            Literal::Compiler(proc_macro::Literal::f64_unsuffixed(f))
        } else {
            Literal::Fallback(fallback::Literal::f64_unsuffixed(f))
        }
    }

    pub fn string(t: &str) -> Literal {
        if nightly_works() {
            Literal::Compiler(proc_macro::Literal::string(t))
        } else {
            Literal::Fallback(fallback::Literal::string(t))
        }
    }

    pub fn character(t: char) -> Literal {
        if nightly_works() {
            Literal::Compiler(proc_macro::Literal::character(t))
        } else {
            Literal::Fallback(fallback::Literal::character(t))
        }
    }

    pub fn byte_string(bytes: &[u8]) -> Literal {
        if nightly_works() {
            Literal::Compiler(proc_macro::Literal::byte_string(bytes))
        } else {
            Literal::Fallback(fallback::Literal::byte_string(bytes))
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Literal::Compiler(lit) => Span::Compiler(lit.span()),
            Literal::Fallback(lit) => Span::Fallback(lit.span()),
        }
    }

    pub fn set_span(&mut self, span: Span) {
        match (self, span) {
            (Literal::Compiler(lit), Span::Compiler(s)) => lit.set_span(s),
            (Literal::Fallback(lit), Span::Fallback(s)) => lit.set_span(s),
            _ => mismatch(),
        }
    }

    fn unwrap_nightly(self) -> proc_macro::Literal {
        match self {
            Literal::Compiler(s) => s,
            Literal::Fallback(_) => mismatch(),
        }
    }
}

impl From<fallback::Literal> for Literal {
    fn from(s: fallback::Literal) -> Literal {
        Literal::Fallback(s)
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Literal::Compiler(t) => t.fmt(f),
            Literal::Fallback(t) => t.fmt(f),
        }
    }
}

impl fmt::Debug for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Literal::Compiler(t) => t.fmt(f),
            Literal::Fallback(t) => t.fmt(f),
        }
    }
}
