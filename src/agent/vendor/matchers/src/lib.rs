//! Regex matchers on character and byte streams.
//!
//! ## Overview
//!
//! The [`regex`] crate implements regular expression matching on strings and byte
//! arrays. However, in order to match the output of implementations of `fmt::Debug`
//! and `fmt::Display`, or by any code which writes to an instance of `fmt::Write`
//! or `io::Write`, it is necessary to first allocate a buffer, write to that
//! buffer, and then match the buffer against a regex.
//!
//! In cases where it is not necessary to extract substrings, but only to test whether
//! or not output matches a regex, it is not strictly necessary to allocate and
//! write this output to a buffer. This crate provides a simple interface on top of
//! the lower-level [`regex-automata`] library that implements `fmt::Write` and
//! `io::Write` for regex patterns. This may be used to test whether streaming
//! output matches a pattern without buffering that output.
//!
//! Users who need to extract substrings based on a pattern or who already have
//! buffered data should probably use the [`regex`] crate instead.
//!
//! ## Syntax
//!
//! This crate uses the same [regex syntax][syntax] of the `regex-automata` crate.
//!
//! [`regex`]: https://crates.io/crates/regex
//! [`regex-automata`]: https://crates.io/crates/regex-automata
//! [syntax]: https://docs.rs/regex-automata/0.1.7/regex_automata/#syntax

use regex_automata::{DenseDFA, SparseDFA, StateID, DFA};
use std::{fmt, io, marker::PhantomData, str::FromStr};

pub use regex_automata::Error;

/// A compiled match pattern that can match multipe inputs, or return a
/// [`Matcher`] that matches a single input.
///
/// [`Matcher`]: ../struct.Matcher.html
#[derive(Debug, Clone)]
pub struct Pattern<S = usize, A = DenseDFA<Vec<S>, S>>
where
    S: StateID,
    A: DFA<ID = S>,
{
    automaton: A,
}

/// A reference to a [`Pattern`] that matches a single input.
///
/// [`Pattern`]: ../struct.Pattern.html
#[derive(Debug, Clone)]
pub struct Matcher<'a, S = usize, A = DenseDFA<&'a [S], S>>
where
    S: StateID,
    A: DFA<ID = S>,
{
    automaton: A,
    state: S,
    _lt: PhantomData<&'a ()>,
}

// === impl Pattern ===

impl Pattern {
    /// Returns a new `Pattern` for the given regex, or an error if the regex
    /// was invalid.
    pub fn new(pattern: &str) -> Result<Self, Error> {
        let automaton = DenseDFA::new(pattern)?;
        Ok(Pattern { automaton })
    }
}

impl FromStr for Pattern {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl<S, A> Pattern<S, A>
where
    S: StateID,
    A: DFA<ID = S>,
    Self: for<'a> ToMatcher<'a, S>,
{
    /// Returns `true` if this pattern matches the given string.
    #[inline]
    pub fn matches(&self, s: &impl AsRef<str>) -> bool {
        self.matcher().matches(s)
    }

    /// Returns `true` if this pattern matches the formatted output of the given
    /// type implementing `fmt::Debug`.
    ///
    /// For example:
    /// ```rust
    /// use matchers::Pattern;
    ///
    /// #[derive(Debug)]
    /// pub struct Hello {
    ///     to: &'static str,
    /// }
    ///
    /// let pattern = Pattern::new(r#"Hello \{ to: "W[^"]*" \}"#).unwrap();
    ///
    /// let hello_world = Hello { to: "World" };
    /// assert!(pattern.debug_matches(&hello_world));
    ///
    /// let hello_sf = Hello { to: "San Francisco" };
    /// assert_eq!(pattern.debug_matches(&hello_sf), false);
    ///
    /// let hello_washington = Hello { to: "Washington" };
    /// assert!(pattern.debug_matches(&hello_washington));
    /// ```
    #[inline]
    pub fn debug_matches(&self, d: &impl fmt::Debug) -> bool {
        self.matcher().debug_matches(d)
    }

    /// Returns `true` if this pattern matches the formatted output of the given
    /// type implementing `fmt::Display`.
    ///
    /// For example:
    /// ```rust
    /// # use std::fmt;
    /// use matchers::Pattern;
    ///
    /// #[derive(Debug)]
    /// pub struct Hello {
    ///     to: &'static str,
    /// }
    ///
    /// impl fmt::Display for Hello {
    ///     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    ///         write!(f, "Hello {}", self.to)
    ///     }
    /// }
    ///
    /// let pattern = Pattern::new("Hello [Ww].+").unwrap();
    ///
    /// let hello_world = Hello { to: "world" };
    /// assert!(pattern.display_matches(&hello_world));
    /// assert_eq!(pattern.debug_matches(&hello_world), false);
    ///
    /// let hello_sf = Hello { to: "San Francisco" };
    /// assert_eq!(pattern.display_matches(&hello_sf), false);
    ///
    /// let hello_washington = Hello { to: "Washington" };
    /// assert!(pattern.display_matches(&hello_washington));
    /// ```
    #[inline]
    pub fn display_matches(&self, d: &impl fmt::Display) -> bool {
        self.matcher().display_matches(d)
    }

    /// Returns either a `bool` indicating whether or not this pattern matches the
    /// data read from the provided `io::Read` stream, or an `io::Error` if an
    /// error occurred reading from the stream.
    #[inline]
    pub fn read_matches(&self, io: impl io::Read) -> io::Result<bool> {
        self.matcher().read_matches(io)
    }
}

// === impl Matcher ===

impl<'a, S, A> Matcher<'a, S, A>
where
    S: StateID,
    A: DFA<ID = S>,
{
    fn new(automaton: A) -> Self {
        let state = automaton.start_state();
        Self {
            automaton,
            state,
            _lt: PhantomData,
        }
    }

    #[inline]
    fn advance(&mut self, input: u8) {
        self.state = unsafe {
            // It's safe to call `next_state_unchecked` since the matcher may
            // only be constructed by a `Pattern`, which, in turn,can only be
            // constructed with a valid DFA.
            self.automaton.next_state_unchecked(self.state, input)
        };
    }

    /// Returns `true` if this `Matcher` has matched any input that has been
    /// provided.
    #[inline]
    pub fn is_matched(&self) -> bool {
        self.automaton.is_match_state(self.state)
    }

    /// Returns `true` if this pattern matches the formatted output of the given
    /// type implementing `fmt::Debug`.
    pub fn matches(mut self, s: &impl AsRef<str>) -> bool {
        for &byte in s.as_ref().as_bytes() {
            self.advance(byte);
            if self.automaton.is_dead_state(self.state) {
                return false;
            }
        }
        self.is_matched()
    }

    /// Returns `true` if this pattern matches the formatted output of the given
    /// type implementing `fmt::Debug`.
    pub fn debug_matches(mut self, d: &impl fmt::Debug) -> bool {
        use std::fmt::Write;
        write!(&mut self, "{:?}", d).expect("matcher write impl should not fail");
        self.is_matched()
    }

    /// Returns `true` if this pattern matches the formatted output of the given
    /// type implementing `fmt::Display`.
    pub fn display_matches(mut self, d: &impl fmt::Display) -> bool {
        use std::fmt::Write;
        write!(&mut self, "{}", d).expect("matcher write impl should not fail");
        self.is_matched()
    }

    /// Returns either a `bool` indicating whether or not this pattern matches the
    /// data read from the provided `io::Read` stream, or an `io::Error` if an
    /// error occurred reading from the stream.
    pub fn read_matches(mut self, io: impl io::Read + Sized) -> io::Result<bool> {
        for r in io.bytes() {
            self.advance(r?);
            if self.automaton.is_dead_state(self.state) {
                return Ok(false);
            }
        }
        Ok(self.is_matched())
    }
}

impl<'a, S, A> fmt::Write for Matcher<'a, S, A>
where
    S: StateID,
    A: DFA<ID = S>,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &byte in s.as_bytes() {
            self.advance(byte);
            if self.automaton.is_dead_state(self.state) {
                break;
            }
        }
        Ok(())
    }
}

impl<'a, S, A> io::Write for Matcher<'a, S, A>
where
    S: StateID,
    A: DFA<ID = S>,
{
    fn write(&mut self, bytes: &[u8]) -> Result<usize, io::Error> {
        let mut i = 0;
        for &byte in bytes {
            self.advance(byte);
            i += 1;
            if self.automaton.is_dead_state(self.state) {
                break;
            }
        }
        Ok(i)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
        Ok(())
    }
}

pub trait ToMatcher<'a, S>
where
    Self: crate::sealed::Sealed,
    S: StateID + 'a,
{
    type Automaton: DFA<ID = S>;
    fn matcher(&'a self) -> Matcher<'a, S, Self::Automaton>;
}

impl<S> crate::sealed::Sealed for Pattern<S, DenseDFA<Vec<S>, S>> where S: StateID {}

impl<'a, S> ToMatcher<'a, S> for Pattern<S, DenseDFA<Vec<S>, S>>
where
    S: StateID + 'a,
{
    type Automaton = DenseDFA<&'a [S], S>;
    fn matcher(&'a self) -> Matcher<'a, S, Self::Automaton> {
        Matcher::new(self.automaton.as_ref())
    }
}

impl<'a, S> ToMatcher<'a, S> for Pattern<S, SparseDFA<Vec<u8>, S>>
where
    S: StateID + 'a,
{
    type Automaton = SparseDFA<&'a [u8], S>;
    fn matcher(&'a self) -> Matcher<'a, S, Self::Automaton> {
        Matcher::new(self.automaton.as_ref())
    }
}

impl<S> crate::sealed::Sealed for Pattern<S, SparseDFA<Vec<u8>, S>> where S: StateID {}

mod sealed {
    pub trait Sealed {}
}

#[cfg(test)]
mod test {
    use super::*;

    struct Str<'a>(&'a str);
    struct ReadStr<'a>(io::Cursor<&'a [u8]>);

    impl<'a> fmt::Debug for Str<'a> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl<'a> fmt::Display for Str<'a> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl<'a> io::Read for ReadStr<'a> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
    }

    impl Str<'static> {
        fn hello_world() -> Self {
            Self::new("hello world")
        }
    }

    impl<'a> Str<'a> {
        fn new(s: &'a str) -> Self {
            Str(s)
        }

        fn to_reader(self) -> ReadStr<'a> {
            ReadStr(io::Cursor::new(self.0.as_bytes()))
        }
    }

    #[test]
    fn debug_matches() {
        let pat = Pattern::new("hello world").unwrap();
        assert!(pat.debug_matches(&Str::hello_world()));

        let pat = Pattern::new("hel+o w[orl]{3}d").unwrap();
        assert!(pat.debug_matches(&Str::hello_world()));

        let pat = Pattern::new("goodbye world").unwrap();
        assert_eq!(pat.debug_matches(&Str::hello_world()), false);
    }

    #[test]
    fn display_matches() {
        let pat = Pattern::new("hello world").unwrap();
        assert!(pat.display_matches(&Str::hello_world()));

        let pat = Pattern::new("hel+o w[orl]{3}d").unwrap();
        assert!(pat.display_matches(&Str::hello_world()));

        let pat = Pattern::new("goodbye world").unwrap();
        assert_eq!(pat.display_matches(&Str::hello_world()), false);
    }

    #[test]
    fn reader_matches() {
        let pat = Pattern::new("hello world").unwrap();
        assert!(pat
            .read_matches(Str::hello_world().to_reader())
            .expect("no io error should occur"));

        let pat = Pattern::new("hel+o w[orl]{3}d").unwrap();
        assert!(pat
            .read_matches(Str::hello_world().to_reader())
            .expect("no io error should occur"));

        let pat = Pattern::new("goodbye world").unwrap();
        assert_eq!(
            pat.read_matches(Str::hello_world().to_reader())
                .expect("no io error should occur"),
            false
        );
    }

    #[test]
    fn debug_rep_pattern() {
        let pat = Pattern::new("a+b").unwrap();
        assert!(pat.debug_matches(&Str::new("ab")));
        assert!(pat.debug_matches(&Str::new("aaaab")));
        assert!(pat.debug_matches(&Str::new("aaaaaaaaaab")));
        assert_eq!(pat.debug_matches(&Str::new("b")), false);
        assert_eq!(pat.debug_matches(&Str::new("abb")), false);
        assert_eq!(pat.debug_matches(&Str::new("aaaaabb")), false);
    }
}
