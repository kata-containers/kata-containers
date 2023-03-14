//! Lexer for parsing format descriptions.

use core::iter;

use super::{Location, Span};

/// A token emitted by the lexer. There is no semantic meaning at this stage.
pub(super) enum Token<'a> {
    /// A literal string, formatted and parsed as-is.
    Literal {
        /// The string itself.
        value: &'a [u8],
        /// Where the string was in the format string.
        span: Span,
    },
    /// An opening or closing bracket. May or may not be the start or end of a component.
    Bracket {
        /// Whether the bracket is opening or closing.
        kind: BracketKind,
        /// Where the bracket was in the format string.
        location: Location,
    },
    /// One part of a component. This could be its name, a modifier, or whitespace.
    ComponentPart {
        /// Whether the part is whitespace or not.
        kind: ComponentKind,
        /// The part itself.
        value: &'a [u8],
        /// Where the part was in the format string.
        span: Span,
    },
}

/// What type of bracket is present.
pub(super) enum BracketKind {
    /// An opening bracket: `[`
    Opening,
    /// A closing bracket: `]`
    Closing,
}

/// Indicates whether the component is whitespace or not.
pub(super) enum ComponentKind {
    #[allow(clippy::missing_docs_in_private_items)]
    Whitespace,
    #[allow(clippy::missing_docs_in_private_items)]
    NotWhitespace,
}

/// Attach [`Location`] information to each byte in the iterator.
fn attach_location(iter: impl Iterator<Item = u8>) -> impl Iterator<Item = (u8, Location)> {
    let mut line = 1;
    let mut column = 1;
    let mut byte_pos = 0;

    iter.map(move |byte| {
        let location = Location {
            line,
            column,
            byte: byte_pos,
        };
        column += 1;
        byte_pos += 1;

        if byte == b'\n' {
            line += 1;
            column = 1;
        }

        (byte, location)
    })
}

/// Parse the string into a series of [`Token`]s.
pub(super) fn lex(mut input: &[u8]) -> impl Iterator<Item = Token<'_>> {
    let mut depth: u8 = 0;
    let mut iter = attach_location(input.iter().copied()).peekable();
    let mut second_bracket_location = None;

    iter::from_fn(move || {
        // There is a flag set to emit the second half of an escaped bracket pair.
        if let Some(location) = second_bracket_location.take() {
            return Some(Token::Bracket {
                kind: BracketKind::Opening,
                location,
            });
        }

        Some(match iter.next()? {
            (b'[', location) => {
                if let Some((_, second_location)) = iter.next_if(|&(byte, _)| byte == b'[') {
                    // escaped bracket
                    second_bracket_location = Some(second_location);
                    input = &input[2..];
                } else {
                    // opening bracket
                    depth += 1;
                    input = &input[1..];
                }

                Token::Bracket {
                    kind: BracketKind::Opening,
                    location,
                }
            }
            // closing bracket
            (b']', location) if depth > 0 => {
                depth -= 1;
                input = &input[1..];
                Token::Bracket {
                    kind: BracketKind::Closing,
                    location,
                }
            }
            // literal
            (_, start_location) if depth == 0 => {
                let mut bytes = 1;
                let mut end_location = start_location;

                while let Some((_, location)) = iter.next_if(|&(byte, _)| byte != b'[') {
                    end_location = location;
                    bytes += 1;
                }

                let value = &input[..bytes];
                input = &input[bytes..];
                Token::Literal {
                    value,
                    span: Span::start_end(start_location, end_location),
                }
            }
            // component part
            (byte, start_location) => {
                let mut bytes = 1;
                let mut end_location = start_location;
                let is_whitespace = byte.is_ascii_whitespace();

                while let Some((_, location)) = iter.next_if(|&(byte, _)| {
                    byte != b'[' && byte != b']' && is_whitespace == byte.is_ascii_whitespace()
                }) {
                    end_location = location;
                    bytes += 1;
                }

                let value = &input[..bytes];
                input = &input[bytes..];
                Token::ComponentPart {
                    kind: if is_whitespace {
                        ComponentKind::Whitespace
                    } else {
                        ComponentKind::NotWhitespace
                    },
                    value,
                    span: Span::start_end(start_location, end_location),
                }
            }
        })
    })
}
