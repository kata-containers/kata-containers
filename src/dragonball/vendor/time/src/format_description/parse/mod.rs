//! Parser for format descriptions.

use alloc::vec::Vec;
use core::ops::{RangeFrom, RangeTo};

mod ast;
mod format_item;
mod lexer;

/// Parse a sequence of items from the format description.
///
/// The syntax for the format description can be found in [the
/// book](https://time-rs.github.io/book/api/format-description.html).
pub fn parse(
    s: &str,
) -> Result<Vec<crate::format_description::FormatItem<'_>>, crate::error::InvalidFormatDescription>
{
    let lexed = lexer::lex(s.as_bytes());
    let ast = ast::parse(lexed);
    let format_items = format_item::parse(ast);
    Ok(format_items
        .map(|res| res.map(Into::into))
        .collect::<Result<Vec<_>, _>>()?)
}

/// Parse a sequence of items from the format description.
///
/// The syntax for the format description can be found in [the
/// book](https://time-rs.github.io/book/api/format-description.html).
///
/// Unlike [`parse`], this function returns [`OwnedFormatItem`], which owns its contents. This means
/// that there is no lifetime that needs to be handled.
///
/// [`OwnedFormatItem`]: crate::format_description::OwnedFormatItem
pub fn parse_owned(
    s: &str,
) -> Result<crate::format_description::OwnedFormatItem, crate::error::InvalidFormatDescription> {
    let lexed = lexer::lex(s.as_bytes());
    let ast = ast::parse(lexed);
    let format_items = format_item::parse(ast);
    let items = format_items
        .map(|res| res.map(Into::into))
        .collect::<Result<Vec<_>, _>>()?
        .into_boxed_slice();
    Ok(crate::format_description::OwnedFormatItem::Compound(items))
}

/// A location within a string.
#[derive(Clone, Copy)]
struct Location {
    /// The one-indexed line of the string.
    line: usize,
    /// The one-indexed column of the string.
    column: usize,
    /// The zero-indexed byte of the string.
    byte: usize,
}

impl Location {
    /// Offset the location by the provided amount.
    ///
    /// Note that this assumes the resulting location is on the same line as the original location.
    #[must_use = "this does not modify the original value"]
    const fn offset(&self, offset: usize) -> Self {
        Self {
            line: self.line,
            column: self.column + offset,
            byte: self.byte + offset,
        }
    }

    /// Create an error with the provided message at this location.
    const fn error(self, message: &'static str) -> ErrorInner {
        ErrorInner {
            _message: message,
            _span: Span {
                start: self,
                end: self,
            },
        }
    }
}

/// A start and end point within a string.
#[derive(Clone, Copy)]
struct Span {
    #[allow(clippy::missing_docs_in_private_items)]
    start: Location,
    #[allow(clippy::missing_docs_in_private_items)]
    end: Location,
}

impl Span {
    /// Create a new `Span` from the provided start and end locations.
    const fn start_end(start: Location, end: Location) -> Self {
        Self { start, end }
    }

    /// Reduce this span to the provided range.
    #[must_use = "this does not modify the original value"]
    fn subspan(&self, range: impl Subspan) -> Self {
        range.subspan(self)
    }

    /// Obtain a `Span` pointing at the start of the pre-existing span.
    #[must_use = "this does not modify the original value"]
    const fn shrink_to_start(&self) -> Self {
        Self {
            start: self.start,
            end: self.start,
        }
    }

    /// Obtain a `Span` pointing at the end of the pre-existing span.
    #[must_use = "this does not modify the original value"]
    const fn shrink_to_end(&self) -> Self {
        Self {
            start: self.end,
            end: self.end,
        }
    }

    /// Create an error with the provided message at this span.
    const fn error(self, message: &'static str) -> ErrorInner {
        ErrorInner {
            _message: message,
            _span: self,
        }
    }

    /// Get the byte index that the span starts at.
    const fn start_byte(&self) -> usize {
        self.start.byte
    }
}

/// A trait for types that can be used to reduce a `Span`.
trait Subspan {
    /// Reduce the provided `Span` to a new `Span`.
    fn subspan(self, span: &Span) -> Span;
}

impl Subspan for RangeFrom<usize> {
    fn subspan(self, span: &Span) -> Span {
        assert_eq!(span.start.line, span.end.line);

        Span {
            start: Location {
                line: span.start.line,
                column: span.start.column + self.start,
                byte: span.start.byte + self.start,
            },
            end: span.end,
        }
    }
}

impl Subspan for RangeTo<usize> {
    fn subspan(self, span: &Span) -> Span {
        assert_eq!(span.start.line, span.end.line);

        Span {
            start: span.start,
            end: Location {
                line: span.start.line,
                column: span.start.column + self.end - 1,
                byte: span.start.byte + self.end - 1,
            },
        }
    }
}

/// The internal error type.
struct ErrorInner {
    /// The message displayed to the user.
    _message: &'static str,
    /// Where the error originated.
    _span: Span,
}

/// A complete error description.
struct Error {
    /// The internal error.
    _inner: ErrorInner,
    /// The error needed for interoperability with the rest of `time`.
    public: crate::error::InvalidFormatDescription,
}

impl From<Error> for crate::error::InvalidFormatDescription {
    fn from(error: Error) -> Self {
        error.public
    }
}
