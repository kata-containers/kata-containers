//! AST for parsing format descriptions.

use alloc::string::String;
use alloc::vec::Vec;
use core::iter;
use core::iter::Peekable;

use super::{lexer, Error, Location, Span};

/// One part of a complete format description.
#[allow(variant_size_differences)]
pub(super) enum Item<'a> {
    /// A literal string, formatted and parsed as-is.
    Literal {
        /// The string itself.
        value: &'a [u8],
        /// Where the string originates from in the format string.
        _span: Span,
    },
    /// A sequence of brackets. The first acts as the escape character.
    EscapedBracket {
        /// The first bracket.
        _first: Location,
        /// The second bracket.
        _second: Location,
    },
    /// Part of a type, along with its modifiers.
    Component {
        /// Where the opening bracket was in the format string.
        _opening_bracket: Location,
        /// Whitespace between the opening bracket and name.
        _leading_whitespace: Option<Whitespace<'a>>,
        /// The name of the component.
        name: Name<'a>,
        /// The modifiers for the component.
        modifiers: Vec<Modifier<'a>>,
        /// Whitespace between the modifiers and closing bracket.
        _trailing_whitespace: Option<Whitespace<'a>>,
        /// Where the closing bracket was in the format string.
        _closing_bracket: Location,
    },
}

/// Whitespace within a component.
pub(super) struct Whitespace<'a> {
    /// The whitespace itself.
    pub(super) _value: &'a [u8],
    /// Where the whitespace was in the format string.
    pub(super) span: Span,
}

/// The name of a component.
pub(super) struct Name<'a> {
    /// The name itself.
    pub(super) value: &'a [u8],
    /// Where the name was in the format string.
    pub(super) span: Span,
}

/// A modifier for a component.
pub(super) struct Modifier<'a> {
    /// Whitespace preceding the modifier.
    pub(super) _leading_whitespace: Whitespace<'a>,
    /// The key of the modifier.
    pub(super) key: Key<'a>,
    /// Where the colon of the modifier was in the format string.
    pub(super) _colon: Location,
    /// The value of the modifier.
    pub(super) value: Value<'a>,
}

/// The key of a modifier.
pub(super) struct Key<'a> {
    /// The key itself.
    pub(super) value: &'a [u8],
    /// Where the key was in the format string.
    pub(super) span: Span,
}

/// The value of a modifier.
pub(super) struct Value<'a> {
    /// The value itself.
    pub(super) value: &'a [u8],
    /// Where the value was in the format string.
    pub(super) span: Span,
}

/// Parse the provided tokens into an AST.
pub(super) fn parse<'a>(
    tokens: impl Iterator<Item = lexer::Token<'a>>,
) -> impl Iterator<Item = Result<Item<'a>, Error>> {
    let mut tokens = tokens.peekable();
    iter::from_fn(move || {
        Some(match tokens.next()? {
            lexer::Token::Literal { value, span } => Ok(Item::Literal { value, _span: span }),
            lexer::Token::Bracket {
                kind: lexer::BracketKind::Opening,
                location,
            } => {
                // escaped bracket
                if let Some(&lexer::Token::Bracket {
                    kind: lexer::BracketKind::Opening,
                    location: second_location,
                }) = tokens.peek()
                {
                    tokens.next(); // consume
                    Ok(Item::EscapedBracket {
                        _first: location,
                        _second: second_location,
                    })
                }
                // component
                else {
                    parse_component(location, &mut tokens)
                }
            }
            lexer::Token::Bracket {
                kind: lexer::BracketKind::Closing,
                location: _,
            } => unreachable!(
                "internal error: closing bracket should have been consumed by `parse_component`",
            ),
            lexer::Token::ComponentPart {
                kind: _,
                value: _,
                span: _,
            } => unreachable!(
                "internal error: component part should have been consumed by `parse_component`",
            ),
        })
    })
}

/// Parse a component. This assumes that the opening bracket has already been consumed.
fn parse_component<'a>(
    opening_bracket: Location,
    tokens: &mut Peekable<impl Iterator<Item = lexer::Token<'a>>>,
) -> Result<Item<'a>, Error> {
    let leading_whitespace = if let Some(&lexer::Token::ComponentPart {
        kind: lexer::ComponentKind::Whitespace,
        value,
        span,
    }) = tokens.peek()
    {
        tokens.next(); // consume
        Some(Whitespace {
            _value: value,
            span,
        })
    } else {
        None
    };

    let name = if let Some(&lexer::Token::ComponentPart {
        kind: lexer::ComponentKind::NotWhitespace,
        value,
        span,
    }) = tokens.peek()
    {
        tokens.next(); // consume
        Name { value, span }
    } else {
        let span = leading_whitespace.map_or_else(
            || Span {
                start: opening_bracket,
                end: opening_bracket,
            },
            |whitespace| whitespace.span.shrink_to_end(),
        );
        return Err(Error {
            _inner: span.error("expected component name"),
            public: crate::error::InvalidFormatDescription::MissingComponentName {
                index: span.start_byte(),
            },
        });
    };

    let mut modifiers = Vec::new();
    let trailing_whitespace = loop {
        let whitespace = if let Some(&lexer::Token::ComponentPart {
            kind: lexer::ComponentKind::Whitespace,
            value,
            span,
        }) = tokens.peek()
        {
            tokens.next(); // consume
            Whitespace {
                _value: value,
                span,
            }
        } else {
            break None;
        };

        if let Some(&lexer::Token::ComponentPart {
            kind: lexer::ComponentKind::NotWhitespace,
            value,
            span,
        }) = tokens.peek()
        {
            tokens.next(); // consume

            let colon_index = match value.iter().position(|&b| b == b':') {
                Some(index) => index,
                None => {
                    return Err(Error {
                        _inner: span.error("modifier must be of the form `key:value`"),
                        public: crate::error::InvalidFormatDescription::InvalidModifier {
                            value: String::from_utf8_lossy(value).into_owned(),
                            index: span.start_byte(),
                        },
                    });
                }
            };
            let key = &value[..colon_index];
            let value = &value[colon_index + 1..];

            if key.is_empty() {
                return Err(Error {
                    _inner: span.shrink_to_start().error("expected modifier key"),
                    public: crate::error::InvalidFormatDescription::InvalidModifier {
                        value: String::new(),
                        index: span.start_byte(),
                    },
                });
            }
            if value.is_empty() {
                return Err(Error {
                    _inner: span.shrink_to_end().error("expected modifier value"),
                    public: crate::error::InvalidFormatDescription::InvalidModifier {
                        value: String::new(),
                        index: span.shrink_to_end().start_byte(),
                    },
                });
            }

            modifiers.push(Modifier {
                _leading_whitespace: whitespace,
                key: Key {
                    value: key,
                    span: span.subspan(..colon_index),
                },
                _colon: span.start.offset(colon_index),
                value: Value {
                    value,
                    span: span.subspan(colon_index + 1..),
                },
            });
        } else {
            break Some(whitespace);
        }
    };

    let closing_bracket = if let Some(&lexer::Token::Bracket {
        kind: lexer::BracketKind::Closing,
        location,
    }) = tokens.peek()
    {
        tokens.next(); // consume
        location
    } else {
        return Err(Error {
            _inner: opening_bracket.error("unclosed bracket"),
            public: crate::error::InvalidFormatDescription::UnclosedOpeningBracket {
                index: opening_bracket.byte,
            },
        });
    };

    Ok(Item::Component {
        _opening_bracket: opening_bracket,
        _leading_whitespace: leading_whitespace,
        name,
        modifiers,
        _trailing_whitespace: trailing_whitespace,
        _closing_bracket: closing_bracket,
    })
}
