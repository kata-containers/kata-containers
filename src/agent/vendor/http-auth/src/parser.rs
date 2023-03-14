// Copyright (C) 2021 Scott Lamb <slamb@slamb.org>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Parses as in [RFC 7235](https://datatracker.ietf.org/doc/html/rfc7235).
//!
//! Most callers don't need to directly parse; see [`crate::PasswordClient`] instead.

// State machine implementation of challenge parsing with a state machine.
// Nice qualities: predictable performance (no backtracking), low dependencies.
//
// The implementation is *not* a straightforward translation of the ABNF
// grammar, so we verify correctness via a fuzz tester that compares with a
// nom-based parser. See `fuzz/fuzz_targets/parse_challenges.rs`.

use std::{fmt::Display, ops::Range};

use crate::{ChallengeRef, ParamValue};

use crate::{char_classes, C_ESCAPABLE, C_OWS, C_QDTEXT, C_TCHAR};

/// Calls `log::trace!` only if the `trace` cargo feature is enabled.
macro_rules! trace {
    ($($arg:tt)+) => (#[cfg(feature = "trace")] log::trace!($($arg)+))
}

/// Parses a list of challenges as in [RFC
/// 7235](https://datatracker.ietf.org/doc/html/rfc7235) `Proxy-Authenticate`
/// or `WWW-Authenticate` header values.
///
/// Most callers don't need to directly parse; see [`crate::PasswordClient`] instead.
///
/// This is an iterator that parses lazily, returning each challenge as soon as
/// its end has been found. (Due to the grammar's ambiguous use of commas to
/// separate both challenges and parameters, a challenge's end is found after
/// parsing the *following* challenge's scheme name.) On encountering a syntax
/// error, it yields `Some(Err(_))` and fuses: all subsequent calls to
/// [`Iterator::next`] will return `None`.
///
/// See also the [`crate::parse_challenges`] convenience wrapper.
///
/// ## Example
///
/// ```rust
/// use http_auth::{parser::ChallengeParser, ChallengeRef, ParamValue};
/// let challenges = "UnsupportedSchemeA, Basic realm=\"foo\", error error";
/// let mut parser = ChallengeParser::new(challenges);
/// let c = parser.next().unwrap().unwrap();
/// assert_eq!(c, ChallengeRef {
///     scheme: "UnsupportedSchemeA",
///     params: vec![],
/// });
/// let c = parser.next().unwrap().unwrap();
/// assert_eq!(c, ChallengeRef {
///     scheme: "Basic",
///     params: vec![("realm", ParamValue::try_from_escaped("foo").unwrap())],
/// });
/// let c = parser.next().unwrap().unwrap_err();
/// ```
///
/// ## Implementation notes
///
/// This rigorously matches the official ABNF grammar except as follows:
///
/// *   Doesn't allow non-ASCII characters. [RFC 7235 Appendix
///     B](https://datatracker.ietf.org/doc/html/rfc7235#appendix-B) references
///     the `quoted-string` rule from [RFC 7230 section
///     3.2.6](https://datatracker.ietf.org/doc/html/rfc7230#section-3.2.6),
///     which allows these via `obs-text`, but the meaning is ill-defined in
///     the context of RFC 7235.
/// *   Doesn't allow `token68`, which as far as I know has never been and will
///     never be used in a `challenge`:
///     *   [RFC 2617](https://datatracker.ietf.org/doc/html/rfc2617) never
///         allowed `token68` for challenges.
///     *   [RFC 7235 Appendix
///         A](https://datatracker.ietf.org/doc/html/rfc7235#appendix-A) says
///         `token68` "was added for consistency with legacy authentication
///         schemes such as `Basic`", but `Basic` only uses `token68` in
///         `credential`, not `challenge`.
///     *   [RFC 7235 section
///         5.1.2](https://datatracker.ietf.org/doc/html/rfc7235#section-5.1.2)
///         says "new schemes ought to use the `auth-param` syntax instead
///         [of `token68`], because otherwise future extensions will be
///         impossible."
///     *   No scheme in the [registry](https://www.iana.org/assignments/http-authschemes/http-authschemes.xhtml)
///         uses `token68` challenges as of 2021-10-19.
pub struct ChallengeParser<'i> {
    input: &'i str,
    pos: usize,
    state: State<'i>,
}

impl<'i> ChallengeParser<'i> {
    pub fn new(input: &'i str) -> Self {
        ChallengeParser {
            input,
            pos: 0,
            state: State::PreToken {
                challenge: None,
                next: Possibilities(P_SCHEME),
            },
        }
    }
}

/// Describes a parse error and where in the input it occurs.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Error<'i> {
    input: &'i str,
    pos: usize,
    error: &'static str,
}

impl<'i> Error<'i> {
    fn invalid_byte(input: &'i str, pos: usize) -> Self {
        Self {
            input,
            pos,
            error: "invalid byte",
        }
    }
}

impl<'i> Display for Error<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at byte {}: {:?}",
            self.error,
            self.pos,
            format!(
                "{}(HERE-->){}",
                &self.input[..self.pos],
                &self.input[self.pos..]
            ),
        )
    }
}

impl<'i> std::error::Error for Error<'i> {}

/// A set of zero or more `P_*` values indicating possibilities for the current
/// and/or upcoming tokens.
#[derive(Copy, Clone, PartialEq, Eq)]
struct Possibilities(u8);

const P_SCHEME: u8 = 1;
const P_PARAM_KEY: u8 = 2;
const P_EOF: u8 = 4;
const P_WHITESPACE: u8 = 8;
const P_COMMA_PARAM_KEY: u8 = 16; // a comma, then a param_key.
const P_COMMA_EOF: u8 = 32; // a comma, then eof.

impl std::fmt::Debug for Possibilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut l = f.debug_set();
        if (self.0 & P_SCHEME) != 0 {
            l.entry(&"scheme");
        }
        if (self.0 & P_PARAM_KEY) != 0 {
            l.entry(&"param_key");
        }
        if (self.0 & P_EOF) != 0 {
            l.entry(&"eof");
        }
        if (self.0 & P_WHITESPACE) != 0 {
            l.entry(&"whitespace");
        }
        if (self.0 & P_COMMA_PARAM_KEY) != 0 {
            l.entry(&"comma_param_key");
        }
        if (self.0 & P_COMMA_EOF) != 0 {
            l.entry(&"comma_eof");
        }
        l.finish()
    }
}

enum State<'i> {
    Done,

    /// Consuming OWS and commas, then advancing to `Token`.
    PreToken {
        challenge: Option<ChallengeRef<'i>>,
        next: Possibilities,
    },

    /// Parsing a scheme/parameter key, or the whitespace immediately following it.
    Token {
        /// Current `challenge`, if any. If none, this token must be a scheme.
        challenge: Option<ChallengeRef<'i>>,
        token_pos: Range<usize>,
        cur: Possibilities, // subset of P_SCHEME|P_PARAM_KEY
    },

    /// Transitioned from `Token` or `PostToken` on first `=` after parameter key.
    /// Kept there for BWS in param case.
    PostEquals {
        challenge: ChallengeRef<'i>,
        key_pos: Range<usize>,
    },

    /// Transitioned from `Equals` on initial `C_TCHAR`.
    ParamUnquotedValue {
        challenge: ChallengeRef<'i>,
        key_pos: Range<usize>,
        value_start: usize,
    },

    /// Transitioned from `Equals` on initial `"`.
    ParamQuotedValue {
        challenge: ChallengeRef<'i>,
        key_pos: Range<usize>,
        value_start: usize,
        escapes: usize,
        in_backslash: bool,
    },
}

impl<'i> Iterator for ChallengeParser<'i> {
    type Item = Result<ChallengeRef<'i>, Error<'i>>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            let classes = char_classes(b);
            match std::mem::replace(&mut self.state, State::Done) {
                State::Done => return None,
                State::PreToken { challenge, next } => {
                    trace!(
                        "PreToken({:?}) pos={} b={:?}",
                        next,
                        self.pos,
                        char::from(b)
                    );
                    if (classes & C_OWS) != 0 && (next.0 & P_WHITESPACE) != 0 {
                        self.state = State::PreToken {
                            challenge,
                            next: Possibilities(next.0 & !P_EOF),
                        }
                    } else if b == b',' {
                        let next = Possibilities(
                            next.0
                                | P_WHITESPACE
                                | P_SCHEME
                                | if (next.0 & P_COMMA_PARAM_KEY) != 0 {
                                    P_PARAM_KEY
                                } else {
                                    0
                                }
                                | if (next.0 & P_COMMA_EOF) != 0 {
                                    P_EOF
                                } else {
                                    0
                                },
                        );
                        self.state = State::PreToken { challenge, next }
                    } else if (classes & C_TCHAR) != 0 {
                        self.state = State::Token {
                            challenge,
                            token_pos: self.pos..self.pos + 1,
                            cur: Possibilities(next.0 & (P_SCHEME | P_PARAM_KEY)),
                        }
                    } else {
                        return Some(Err(Error::invalid_byte(self.input, self.pos)));
                    }
                }
                State::Token {
                    challenge,
                    token_pos,
                    cur,
                } => {
                    trace!(
                        "Token({:?}, {:?}) pos={} b={:?}, cur challenge = {:#?}",
                        token_pos,
                        cur,
                        self.pos,
                        char::from(b),
                        challenge
                    );
                    if (classes & C_TCHAR) != 0 {
                        if token_pos.end == self.pos {
                            self.state = State::Token {
                                challenge,
                                token_pos: token_pos.start..self.pos + 1,
                                cur,
                            };
                        } else {
                            // Ending a scheme, starting a parameter key without an intermediate comma.
                            // The whitespace between must be exactly one space.
                            if (cur.0 & P_SCHEME) == 0
                                || &self.input[token_pos.end..self.pos] != " "
                            {
                                return Some(Err(Error::invalid_byte(self.input, self.pos)));
                            }
                            self.state = State::Token {
                                challenge: Some(ChallengeRef::new(&self.input[token_pos])),
                                token_pos: self.pos..self.pos + 1,
                                cur: Possibilities(P_PARAM_KEY),
                            };
                            if let Some(c) = challenge {
                                self.pos += 1;
                                return Some(Ok(c));
                            }
                        }
                    } else {
                        match b {
                            b',' if (cur.0 & P_SCHEME) != 0 => {
                                self.state = State::PreToken {
                                    challenge: Some(ChallengeRef::new(&self.input[token_pos])),
                                    next: Possibilities(
                                        P_SCHEME | P_WHITESPACE | P_EOF | P_COMMA_EOF,
                                    ),
                                };
                                if let Some(c) = challenge {
                                    self.pos += 1;
                                    return Some(Ok(c));
                                }
                            }
                            b'=' if (cur.0 & P_PARAM_KEY) != 0 => match challenge {
                                Some(challenge) => {
                                    self.state = State::PostEquals {
                                        challenge,
                                        key_pos: token_pos,
                                    }
                                }
                                None => {
                                    return Some(Err(Error {
                                        input: self.input,
                                        pos: self.pos,
                                        error: "= without existing challenge",
                                    }));
                                }
                            },

                            b' ' | b'\t' => {
                                self.state = State::Token {
                                    challenge,
                                    token_pos,
                                    cur,
                                }
                            }

                            _ => return Some(Err(Error::invalid_byte(self.input, self.pos))),
                        }
                    }
                }
                State::PostEquals { challenge, key_pos } => {
                    trace!("PostEquals pos={} b={:?}", self.pos, char::from(b));
                    if (classes & C_OWS) != 0 {
                        // Note this doesn't advance key_pos.end, so in the token68 case, another
                        // `=` will not be allowed.
                        self.state = State::PostEquals { challenge, key_pos };
                    } else if b == b'"' {
                        self.state = State::ParamQuotedValue {
                            challenge,
                            key_pos,
                            value_start: self.pos + 1,
                            escapes: 0,
                            in_backslash: false,
                        };
                    } else if (classes & C_TCHAR) != 0 {
                        self.state = State::ParamUnquotedValue {
                            challenge,
                            key_pos,
                            value_start: self.pos,
                        };
                    } else {
                        return Some(Err(Error::invalid_byte(self.input, self.pos)));
                    }
                }
                State::ParamUnquotedValue {
                    mut challenge,
                    key_pos,
                    value_start,
                } => {
                    trace!("ParamUnquotedValue pos={} b={:?}", self.pos, char::from(b));
                    if (classes & C_TCHAR) != 0 {
                        self.state = State::ParamUnquotedValue {
                            challenge,
                            key_pos,
                            value_start,
                        };
                    } else if (classes & C_OWS) != 0 {
                        challenge.params.push((
                            &self.input[key_pos],
                            ParamValue {
                                escapes: 0,
                                escaped: &self.input[value_start..self.pos],
                            },
                        ));
                        self.state = State::PreToken {
                            challenge: Some(challenge),
                            next: Possibilities(P_WHITESPACE | P_COMMA_PARAM_KEY | P_COMMA_EOF),
                        };
                    } else if b == b',' {
                        challenge.params.push((
                            &self.input[key_pos],
                            ParamValue {
                                escapes: 0,
                                escaped: &self.input[value_start..self.pos],
                            },
                        ));
                        self.state = State::PreToken {
                            challenge: Some(challenge),
                            next: Possibilities(
                                P_WHITESPACE
                                    | P_PARAM_KEY
                                    | P_SCHEME
                                    | P_EOF
                                    | P_COMMA_PARAM_KEY
                                    | P_COMMA_EOF,
                            ),
                        };
                    } else {
                        return Some(Err(Error::invalid_byte(self.input, self.pos)));
                    }
                }
                State::ParamQuotedValue {
                    mut challenge,
                    key_pos,
                    value_start,
                    escapes,
                    in_backslash,
                } => {
                    trace!("ParamQuotedValue pos={} b={:?}", self.pos, char::from(b));
                    if in_backslash {
                        if (classes & C_ESCAPABLE) == 0 {
                            return Some(Err(Error::invalid_byte(self.input, self.pos)));
                        }
                        self.state = State::ParamQuotedValue {
                            challenge,
                            key_pos,
                            value_start,
                            escapes: escapes + 1,
                            in_backslash: false,
                        };
                    } else if b == b'\\' {
                        self.state = State::ParamQuotedValue {
                            challenge,
                            key_pos,
                            value_start,
                            escapes,
                            in_backslash: true,
                        };
                    } else if b == b'"' {
                        challenge.params.push((
                            &self.input[key_pos],
                            ParamValue {
                                escapes,
                                escaped: &self.input[value_start..self.pos],
                            },
                        ));
                        self.state = State::PreToken {
                            challenge: Some(challenge),
                            next: Possibilities(
                                P_WHITESPACE | P_EOF | P_COMMA_PARAM_KEY | P_COMMA_EOF,
                            ),
                        };
                    } else if (classes & C_QDTEXT) != 0 {
                        self.state = State::ParamQuotedValue {
                            challenge,
                            key_pos,
                            value_start,
                            escapes,
                            in_backslash,
                        };
                    } else {
                        return Some(Err(Error::invalid_byte(self.input, self.pos)));
                    }
                }
            };
            self.pos += 1;
        }
        match std::mem::replace(&mut self.state, State::Done) {
            State::Done => {}
            State::PreToken {
                challenge, next, ..
            } => {
                trace!("eof, PreToken({:?})", next);
                if (next.0 & P_EOF) == 0 {
                    return Some(Err(Error {
                        input: self.input,
                        pos: self.input.len(),
                        error: "unexpected EOF",
                    }));
                }
                if let Some(challenge) = challenge {
                    return Some(Ok(challenge));
                }
            }
            State::Token {
                challenge,
                token_pos,
                cur,
            } => {
                trace!("eof, Token({:?})", cur);
                if (cur.0 & P_SCHEME) == 0 {
                    return Some(Err(Error {
                        input: self.input,
                        pos: self.input.len(),
                        error: "unexpected EOF expecting =",
                    }));
                }
                if token_pos.end != self.input.len() && &self.input[token_pos.end..] != " " {
                    return Some(Err(Error {
                        input: self.input,
                        pos: self.input.len(),
                        error: "EOF after whitespace",
                    }));
                }
                if let Some(challenge) = challenge {
                    self.state = State::Token {
                        challenge: None,
                        token_pos,
                        cur,
                    };
                    return Some(Ok(challenge));
                }
                return Some(Ok(ChallengeRef::new(&self.input[token_pos])));
            }
            State::PostEquals { .. } => {
                trace!("eof, PostEquals");
                return Some(Err(Error {
                    input: self.input,
                    pos: self.input.len(),
                    error: "unexpected EOF expecting param value",
                }));
            }
            State::ParamUnquotedValue {
                mut challenge,
                key_pos,
                value_start,
            } => {
                trace!("eof, ParamUnquotedValue");
                challenge.params.push((
                    &self.input[key_pos],
                    ParamValue {
                        escapes: 0,
                        escaped: &self.input[value_start..],
                    },
                ));
                return Some(Ok(challenge));
            }
            State::ParamQuotedValue { .. } => {
                trace!("eof, ParamQuotedValue");
                return Some(Err(Error {
                    input: self.input,
                    pos: self.input.len(),
                    error: "unexpected EOF in quoted param value",
                }));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::{ChallengeRef, ParamValue};

    // A couple basic tests. The fuzz testing is far more comprehensive.

    #[test]
    fn multi_challenge() {
        // https://datatracker.ietf.org/doc/html/rfc7235#section-4.1
        let input =
            r#"Newauth realm="apps", type=1, title="Login to \"apps\"", Basic realm="simple""#;
        let challenges = crate::parse_challenges(input).unwrap();
        assert_eq!(
            &challenges[..],
            &[
                ChallengeRef {
                    scheme: "Newauth",
                    params: vec![
                        ("realm", ParamValue::new(0, "apps")),
                        ("type", ParamValue::new(0, "1")),
                        ("title", ParamValue::new(2, r#"Login to \"apps\""#)),
                    ],
                },
                ChallengeRef {
                    scheme: "Basic",
                    params: vec![("realm", ParamValue::new(0, "simple")),],
                },
            ]
        );
    }

    #[test]
    fn empty() {
        crate::parse_challenges("").unwrap_err();
        crate::parse_challenges(",").unwrap_err();
    }
}
