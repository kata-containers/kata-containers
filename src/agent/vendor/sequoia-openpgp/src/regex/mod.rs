//! OpenPGP regex parser.
//!
//! OpenPGP defines a [regular expression language].  It is used with
//! [trust signatures] to scope the trust that they extend.
//!
//!   [regular expression language]: https://tools.ietf.org/html/rfc4880#section-8
//!   [trust signatures]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
//!
//! Compared with most regular expression languages, OpenPGP's is
//! quite simple.  In particular, it only includes the following
//! features:
//!
//!   - Alternations using `|`,
//!   - Grouping using `(` and `)`,
//!   - The `*`, `+`, and `?` glob operators,
//!   - The `^`, and `$` anchors,
//!   - The '.' operator, positive *non-empty* ranges
//!     (e.g. `[a-zA-Z]`) and negative *non-empty* ranges (`[^@]`), and
//!   - The backslash operator to escape special characters (except
//!     in ranges).
//!
//! The regular expression engine defined in this module implements
//! that language with two differences.  The first difference is that
//! the compiler only works on UTF-8 strings (not bytes).  The second
//! difference is that ranges in character classes are between UTF-8
//! characters, not just ASCII characters.
//!
//! # Data Structures
//!
//! This module defines two data structures.  [`Regex`] encapsulates a
//! valid regular expression, and provides methods to check whether
//! the regular expression matches a string or a [`UserID`].
//! [`RegexSet`] is similar, but encapsulates zero or more regular
//! expressions, which may or may not be valid.  Its match methods
//! return `true` if there are no regular expressions, or, if there is
//! at least one regular expression, they return whether at least one
//! of the regular expressions matches it.  `RegexSet`'s matcher
//! handles invalid regular expressions by considering them to be
//! regular expressions that don't match anything.  These semantics
//! are consistent with a trust signature's scoping rules.  Further,
//! strings that contain control characters never match.  This
//! behavior can be overridden using [`Regex::disable_sanitizations`]
//! and [`RegexSet::disable_sanitizations`].
//!
//!   [`UserID`]: crate::packet::UserID
//!   [`Regex::disable_sanitizations`]: Regex::disable_sanitizations()
//!   [`RegexSet::disable_sanitizations`]: RegexSet::disable_sanitizations()
//!
//! # Scoped Trust Signatures
//!
//! To create a trust signature, you create a signature whose [type]
//! is either [GenericCertification], [PersonaCertification],
//! [CasualCertification], or [PositiveCertification], and add a
//! [Trust Signature] subpacket using, for instance, the
//! [`SignatureBuilder::set_trust_signature`] method.
//!
//!   [type]: https://tools.ietf.org/html/rfc4880#section-5.2.1
//!   [GenericCertification]: crate::types::SignatureType::GenericCertification
//!   [PersonaCertification]: crate::types::SignatureType::PersonaCertification
//!   [CasualCertification]: crate::types::SignatureType::CasualCertification
//!   [PositiveCertification]: crate::types::SignatureType::PositiveCertification
//!   [Trust Signature]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
//!   [`SignatureBuilder::set_trust_signature`]: crate::packet::signature::SignatureBuilder::set_trust_signature()
//!
//! To scope a trust signature, you add a [Regular Expression
//! subpacket] to it using
//! [`SignatureBuilder::set_regular_expression`] or
//! [`SignatureBuilder::add_regular_expression`].
//!
//! To extract any regular expressions, you can use
//! [`SubpacketAreas::regular_expressions`].
//!
//!   [Regular Expression subpacket]: https://tools.ietf.org/html/rfc4880#section-5.2.3.14
//!   [`SignatureBuilder::set_regular_expression`]: crate::packet::signature::SignatureBuilder::set_regular_expression()
//!   [`SignatureBuilder::add_regular_expression`]: crate::packet::signature::SignatureBuilder::add_regular_expression()
//!   [`SubpacketAreas::regular_expressions`]: crate::packet::signature::subpacket::SubpacketAreas::regular_expressions()
//!
//! # Caveat Emptor
//!
//! Note: GnuPG has [very limited regular expression support].  In
//! particular, it only recognizes regular expressions with the
//! following form:
//!
//!   [very limited regular expression support]: https://dev.gnupg.org/source/gnupg/browse/master/g10/trustdb.c;15e065dee891eef9545556f210b4199107999869$1558
//!
//! ```text
//! <[^>]+[@.]example\.com>$
//! ```
//!
//! Further, it escapes any operators between the `<[^>]+[@.]` and the
//! `>$` except `.` and `\`.  Otherwise, GnuPG treats the regular
//! expression as a literal domain (e.g., `example.com`).
//!
//! Further, until [version 2.2.22] (released in August 2020), GnuPG
//! did not support regular expressions on Windows, and other systems
//! that don't include `regcomp`.  On these systems, if a trust
//! signature included a regular expression, GnuPG conservatively
//! considered the whole trust signature to match nothing.
//!
//!   [version 2.2.22]: https://dev.gnupg.org/T5030
//!
//! # Examples
//!
//! A CA signs two certificates, one for Alice, who works at
//! `example.com`, and one for Bob, who is associated with `some.org`.
//! Carol then creates a trust signature for the CA, which she scopes
//! to `example.org` and `example.com`.  We then confirm that Carol
//! can use the CA to authenticate Alice, but not Bob.
//!
//! ```
//! use sequoia_openpgp as openpgp;
//! use openpgp::cert::prelude::*;
//! use openpgp::packet::prelude::*;
//! use openpgp::policy::StandardPolicy;
//! use openpgp::regex::RegexSet;
//! use openpgp::types::SignatureType;
//!
//! # fn main() -> openpgp::Result<()> {
//! let p = &StandardPolicy::new();
//!
//! let (ca, _)
//!     = CertBuilder::general_purpose(None, Some("OpenPGP CA <openpgp-ca@example.com>"))
//!         .generate()?;
//! let mut ca_signer = ca.primary_key().key().clone()
//!     .parts_into_secret()?.into_keypair()?;
//! let ca_userid = ca.with_policy(p, None)?
//!     .userids().nth(0).expect("Added a User ID").userid();
//!
//! // The CA certifies "Alice <alice@example.com>".
//! let (alice, _)
//!     = CertBuilder::general_purpose(None, Some("Alice <alice@example.com>"))
//!         .generate()?;
//! let alice_userid = alice.with_policy(p, None)?
//!     .userids().nth(0).expect("Added a User ID").userid();
//! let alice_certification = SignatureBuilder::new(SignatureType::GenericCertification)
//!     .sign_userid_binding(
//!         &mut ca_signer,
//!         alice.primary_key().component(),
//!         alice_userid)?;
//! let alice = alice.insert_packets(alice_certification.clone())?;
//! # assert!(alice.clone().into_packets().any(|p| {
//! #   match p {
//! #       Packet::Signature(sig) => sig == alice_certification,
//! #       _ => false,
//! #   }
//! # }));
//!
//! // The CA certifies "Bob <bob@some.org>".
//! let (bob, _)
//!     = CertBuilder::general_purpose(None, Some("Bob <bob@some.org>"))
//!         .generate()?;
//! let bob_userid = bob.with_policy(p, None)?
//!     .userids().nth(0).expect("Added a User ID").userid();
//! let bob_certification = SignatureBuilder::new(SignatureType::GenericCertification)
//!     .sign_userid_binding(
//!         &mut ca_signer,
//!         bob.primary_key().component(),
//!         bob_userid)?;
//! let bob = bob.insert_packets(bob_certification.clone())?;
//! # assert!(bob.clone().into_packets().any(|p| {
//! #   match p {
//! #       Packet::Signature(sig) => sig == bob_certification,
//! #       _ => false,
//! #   }
//! # }));
//!
//!
//! // Carol tsigns the CA's certificate.
//! let (carol, _)
//!     = CertBuilder::general_purpose(None, Some("Carol <carol@another.net>"))
//!         .generate()?;
//! let mut carol_signer = carol.primary_key().key().clone()
//!     .parts_into_secret()?.into_keypair()?;
//!
//! let ca_tsig = SignatureBuilder::new(SignatureType::GenericCertification)
//!     .set_trust_signature(2, 120)?
//!     .set_regular_expression("<[^>]+[@.]example\\.org>$")?
//!     .add_regular_expression("<[^>]+[@.]example\\.com>$")?
//!     .sign_userid_binding(
//!         &mut carol_signer,
//!         ca.primary_key().component(),
//!         ca_userid)?;
//! let ca = ca.insert_packets(ca_tsig.clone())?;
//! # assert!(ca.clone().into_packets().any(|p| {
//! #   match p {
//! #       Packet::Signature(sig) => sig == ca_tsig,
//! #       _ => false,
//! #   }
//! # }));
//!
//!
//! // Carol now tries to authenticate Alice and Bob's certificates
//! // using the CA as a trusted introducer based on `ca_tsig`.
//! let res = RegexSet::from_signature(&ca_tsig)?;
//!
//! // Should should be able to authenticate Alice.
//! let alice_ua = alice.with_policy(p, None)?
//!     .userids().nth(0).expect("Added a User ID");
//! # assert!(res.matches_userid(&alice_ua));
//! let mut authenticated = false;
//! for c in alice_ua.certifications() {
//!     if c.get_issuers().into_iter().any(|h| h.aliases(ca.key_handle())) {
//!         if c.clone().verify_userid_binding(
//!             ca.primary_key().key(),
//!             alice.primary_key().key(),
//!             alice_ua.userid()).is_ok()
//!         {
//!             authenticated |= res.matches_userid(&alice_ua);
//!         }
//!     }
//! }
//! assert!(authenticated);
//!
//! // But, although the CA has certified Bob's key, Carol doesn't rely
//! // on it, because Bob's email address ("bob@some.org") is out of
//! // scope (some.org, not example.com).
//! let bob_ua = bob.with_policy(p, None)?
//!     .userids().nth(0).expect("Added a User ID");
//! # assert!(! res.matches_userid(&bob_ua));
//! let mut have_certification = false;
//! let mut authenticated = false;
//! for c in bob_ua.certifications() {
//!     if c.get_issuers().into_iter().any(|h| h.aliases(ca.key_handle())) {
//!         if c.clone().verify_userid_binding(
//!             ca.primary_key().key(),
//!             bob.primary_key().key(),
//!             bob_ua.userid()).is_ok()
//!         {
//!             have_certification = true;
//!             authenticated |= res.matches_userid(&bob_ua);
//!         }
//!     }
//! }
//! assert!(have_certification);
//! assert!(! authenticated);
//! # Ok(()) }
//! ```

use std::borrow::Borrow;
use std::fmt;

use lalrpop_util::ParseError;
use regex_syntax::hir::{self, Hir};

use crate::Error;
use crate::Result;
use crate::packet::prelude::*;
use crate::types::SignatureType;

pub(crate) mod lexer;
lalrpop_util::lalrpop_mod!(
    #[allow(clippy::all)]
    #[allow(unused_parens)]
    grammar,
    "/regex/grammar.rs"
);

pub(crate) use self::lexer::Token;
pub(crate) use self::lexer::{Lexer, LexicalError};

const TRACE: bool = false;

// Convert tokens into strings.
//
// Unfortunately, we can't implement From, because we don't define
// ParseError in this crate.
pub(crate) fn parse_error_downcast(e: ParseError<usize, Token, LexicalError>)
    -> ParseError<usize, String, LexicalError>
{
    match e {
        ParseError::UnrecognizedToken {
            token: (start, t, end),
            expected,
        } => ParseError::UnrecognizedToken {
            token: (start, t.into(), end),
            expected,
        },

        ParseError::ExtraToken {
            token: (start, t, end),
        } => ParseError::ExtraToken {
            token: (start, t.into(), end),
        },

        ParseError::InvalidToken { location }
        => ParseError::InvalidToken { location },

        ParseError::User { error }
        => ParseError::User { error },

        ParseError::UnrecognizedEOF { location, expected }
        => ParseError::UnrecognizedEOF { location, expected },
    }
}

// Used by grammar.lalrpop to generate a regex class (e.g. '[a-ce]').
fn generate_class(caret: bool, chars: impl Iterator<Item=char>) -> Hir
{
    tracer!(TRACE, "generate_class");

    // Dealing with ranges is a bit tricky.  We need to examine three
    // tokens.  If the middle one is a dash, it's a range.

    let chars: Vec<Option<char>> = chars
        // Pad it out so what we can use windows to get three
        // characters at a time, and be sure to process all
        // characters.
        .map(Some)
        .chain(std::iter::once(None))
        .chain(std::iter::once(None))
        .collect();
    if chars.len() == 2 {
        // The grammar doesn't allow an empty class.
        unreachable!();
    } else {
        let r = chars
            .windows(3)
            .scan(0,
                  |skip: &mut usize, x: &[Option<char>]|
                      // Scan stops if the result is None.
                      // filter_map keeps only those elements that
                      // are Some.
                      -> Option<Option<hir::ClassUnicodeRange>>
                  {
                      if *skip > 0 {
                          *skip -= 1;
                          t!("Skipping: {:?} (skip now: {})", x, skip);
                          Some(None)
                      } else {
                          match (x[0], x[1], x[2]) {
                              (Some(a), Some('-'), Some(c)) => {
                                  // We've got a real range.
                                  *skip = 2;
                                  t!("range for '{}-{}'", a, c);
                                  Some(Some(hir::ClassUnicodeRange::new(a, c)))
                              }
                              (Some(a), _, _) => {
                                  t!("range for '{}'", a);
                                  Some(Some(hir::ClassUnicodeRange::new(a, a)))
                              }
                              (None, _, _) => unreachable!(),
                          }
                      }
                  })
            .flatten();
        let mut class = hir::Class::Unicode(hir::ClassUnicode::new(r));
        if caret {
            class.negate();
        }
        Hir::class(class)
    }
}

/// A compiled OpenPGP regular expression for matching UTF-8 encoded
/// strings.
///
/// A `Regex` contains a regular expression compiled according to the
/// rules defined in [Section 8 of RFC 4880] modulo two differences.
/// First, the compiler only works on UTF-8 strings (not bytes).
/// Second, ranges in character classes are between UTF-8 characters,
/// not just ASCII characters.  Further, by default, strings that
/// don't pass a sanity check (in particular, include Unicode control
/// characters) never match.  This behavior can be customized using
/// [`Regex::disable_sanitizations`].
///
///   [Section 8 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-8
///   [trust signatures]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
///   [`Regex::disable_sanitizations`]: Regex::disable_sanitizations()
///
/// Regular expressions are used to scope the trust that [trust
/// signatures] extend.
///
/// When working with trust signatures, you'll usually want to use the
/// [`RegexSet`] data structure, which already implements the correct
/// semantics.
///
///
/// See the [module-level documentation] for more details.
///
///   [module-level documentation]: self
#[derive(Clone, Debug)]
pub struct Regex {
    regex: regex::Regex,
    disable_sanitizations: bool,
}
assert_send_and_sync!(Regex);

impl Regex {
    /// Parses and compiles the regular expression.
    ///
    /// By default, strings that don't pass a sanity check (in
    /// particular, include Unicode control characters) never match.
    /// This behavior can be customized using
    /// [`Regex::disable_sanitizations`].
    ///
    ///   [`Regex::disable_sanitizations`]: Regex::disable_sanitizations()
    pub fn new(re: &str) -> Result<Self>
    {
        let lexer = Lexer::new(re);
        let hir = match grammar::RegexParser::new().parse(re, lexer) {
            Ok(hir) => hir,
            Err(err) => return Err(parse_error_downcast(err).into()),
        };

        // Converting the Hir to a string and the compiling that is
        // apparently the canonical way to convert a Hir to a Regex
        // (at least it is what rip-grep does, which the author of
        // regex also wrote.  See
        // ripgrep/crates/regex/src/config.rs:ConfiguredHir::regex.
        let regex = regex::RegexBuilder::new(&hir.to_string())
            .build()?;

        Ok(Self {
            regex,
            disable_sanitizations: false,
        })
    }

    /// Parses and compiles the regular expression.
    ///
    /// Returns an error if `re` is not a valid UTF-8 string.
    ///
    /// By default, strings that don't pass a sanity check (in
    /// particular, include Unicode control characters) never match.
    /// This behavior can be customized using
    /// [`Regex::disable_sanitizations`].
    ///
    ///   [`Regex::disable_sanitizations`]: Regex::disable_sanitizations()
    pub fn from_bytes(re: &[u8]) -> Result<Self> {
        Self::new(std::str::from_utf8(re)?)
    }

    /// Controls whether matched strings must pass a sanity check.
    ///
    /// If `false` (the default), i.e., sanity checks are enabled, and
    /// the string doesn't pass the sanity check (in particular, it
    /// contains a Unicode control character according to
    /// [`char::is_control`], including newlines and an embedded `NUL`
    /// byte), this returns `false`.
    ///
    ///   [`char::is_control`]: https://doc.rust-lang.org/std/primitive.char.html#method.is_control
    pub fn disable_sanitizations(&mut self, disabled: bool) {
        self.disable_sanitizations = disabled;
    }

    /// Returns whether the regular expression matches the string.
    ///
    /// If sanity checks are enabled (the default) and the string
    /// doesn't pass the sanity check (in particular, it contains a
    /// Unicode control character according to [`char::is_control`],
    /// including newlines and an embedded `NUL` byte), this returns
    /// `false`.
    ///
    ///   [`char::is_control`]: https://doc.rust-lang.org/std/primitive.char.html#method.is_control
    pub fn is_match(&self, s: &str) -> bool {
        if ! self.disable_sanitizations && s.chars().any(char::is_control) {
            return false;
        }

        self.is_match_clean(s)
    }

    // is_match, but without the sanity check.
    fn is_match_clean(&self, s: &str) -> bool {
        self.regex.is_match(s)
    }

    /// Returns whether the regular expression matches the User ID.
    ///
    /// If the User ID is not a valid UTF-8 string, this returns
    /// `false`.
    ///
    /// If sanity checks are enabled (the default) and the string
    /// doesn't pass the sanity check (in particular, it contains a
    /// Unicode control character according to [`char::is_control`],
    /// including newlines and an embedded `NUL` byte), this returns
    /// `false`.
    ///
    ///   [`char::is_control`]: https://doc.rust-lang.org/std/primitive.char.html#method.is_control
    pub fn matches_userid(&self, u: &UserID) -> bool {
        if let Ok(u) = std::str::from_utf8(u.value()) {
            self.is_match(u)
        } else {
            false
        }
    }
}

#[derive(Clone, Debug)]
enum RegexSet_ {
    Regex(Regex),
    Invalid,
    Everything,
}
assert_send_and_sync!(RegexSet_);

/// A set of regular expressions.
///
/// A `RegexSet` encapsulates a set of regular expressions.  The
/// regular expressions are compiled according to the rules defined in
/// [Section 8 of RFC 4880] modulo two differences.  First, the
/// compiler only works on UTF-8 strings (not bytes).  Second, ranges
/// in character classes are between UTF-8 characters, not just ASCII
/// characters.  Further, by default, strings that don't pass a sanity
/// check (in particular, include Unicode control characters) never
/// match.  This behavior can be customized using
/// [`RegexSet::disable_sanitizations`].
///
///   [Section 8 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-8
///   [`RegexSet::disable_sanitizations`]: RegexSet::disable_sanitizations()
///
/// `RegexSet` implements the semantics of [regular expression]s used
/// in [Trust Signatures].  In particular, a `RegexSet` makes it
/// easier to deal with trust signatures that:
///
///   - Contain multiple Regular Expression subpackts,
///   - Have no Regular Expression subpackets, and/or
///   - Include one or more Regular Expression subpackets that are invalid.
///
///   [regular expressions]: https://tools.ietf.org/html/rfc4880#section-5.2.3.14
///   [Trust Signatures]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
///
/// `RegexSet` compiles each regular expression individually.  If
/// there are no regular expressions, the `RegexSet` matches
/// everything.  If a regular expression is invalid, `RegexSet` treats
/// it as if it doesn't match anything.  Thus, if all regular
/// expressions are invalid, the `RegexSet` matches nothing (not
/// everything!).
///
/// See the [module-level documentation] for more details.
///
///   [module-level documentation]: self
#[derive(Clone)]
pub struct RegexSet {
    re_set: RegexSet_,
    disable_sanitizations: bool,
}
assert_send_and_sync!(RegexSet);

impl fmt::Debug for RegexSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("RegexSet");
        match self.re_set {
            RegexSet_::Everything => {
                d.field("regex", &"<Everything>")
            }
            RegexSet_::Invalid => {
                d.field("regex", &"<Invalid>")
            }
            RegexSet_::Regex(ref r) => {
                d.field("regex", &r.regex)
            }
        }
        .field("sanitizations", &!self.disable_sanitizations)
            .finish()
    }
}

impl RegexSet {
    /// Parses and compiles the regular expressions.
    ///
    /// Invalid regular expressions do not cause this to fail.  See
    /// [`RegexSet`]'s top-level documentation for details.
    ///
    ///
    /// By default, strings that don't pass a sanity check (in
    /// particular, include Unicode control characters) never match.
    /// This behavior can be customized using
    /// [`RegexSet::disable_sanitizations`].
    ///
    ///   [`RegexSet::disable_sanitizations`]: RegexSet::disable_sanitizations()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::regex::RegexSet;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// // Extract the regex and compile it.
    /// let res = &[
    ///     "<[^>]+[@.]example\\.org>$",
    ///     // Invalid.
    ///     "[..",
    /// ];
    ///
    /// let res = RegexSet::new(res)?;
    ///
    /// assert!(res.is_match("Alice <alice@example.org>"));
    /// assert!(! res.is_match("Bob <bob@example.com>"));
    /// # Ok(()) }
    /// ```
    pub fn new<'a, RE, I>(res: I) -> Result<Self>
    where RE: Borrow<&'a str>,
          I: IntoIterator<Item=RE>,
    {
        tracer!(TRACE, "RegexSet::new");

        let mut regexes = Vec::with_capacity(2);
        let mut had_good = false;
        let mut had_bad = false;

        for re in res {
            let re = re.borrow();
            let lexer = Lexer::new(re);
            match grammar::RegexParser::new().parse(re, lexer) {
                Ok(hir) => {
                    had_good = true;
                    let hir = hir::Hir::group(hir::Group {
                        kind: hir::GroupKind::NonCapturing,
                        hir: Box::new(hir),
                    });
                    regexes.push(hir);
                }
                Err(err) => {
                    had_bad = true;
                    t!("Compiling {:?}: {}", re, err);
                }
            }
        }

        if had_bad && ! had_good {
            t!("All regular expressions were invalid.");
            Ok(RegexSet {
                re_set: RegexSet_::Invalid,
                disable_sanitizations: false,
            })
        } else if ! had_bad && ! had_good {
            // Match everything.
            t!("No regular expressions provided.");
            Ok(RegexSet {
                re_set: RegexSet_::Everything,
                disable_sanitizations: false,
            })
        } else {
            // Match any of the regular expressions.
            Ok(RegexSet {
                re_set: RegexSet_::Regex(
                    Regex {
                        regex: regex::RegexBuilder::new(
                            &Hir::alternation(regexes).to_string())
                            .build()?,
                        disable_sanitizations: false,
                    }),
                disable_sanitizations: false,
            })
        }
    }

    /// Parses and compiles the regular expressions.
    ///
    /// The regular expressions are first converted to UTF-8 strings.
    /// Byte sequences that are not valid UTF-8 strings are considered
    /// to be invalid regular expressions.  Invalid regular
    /// expressions do not cause this to fail.  See [`RegexSet`]'s
    /// top-level documentation for details.
    ///
    ///
    /// By default, strings that don't pass a sanity check (in
    /// particular, include Unicode control characters) never match.
    /// This behavior can be customized using
    /// [`RegexSet::disable_sanitizations`].
    ///
    ///   [`RegexSet::disable_sanitizations`]: RegexSet::disable_sanitizations()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::regex::RegexSet;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// // A valid and an invalid UTF-8 byte sequence.  The invalid
    /// // sequence doesn't match anything.  But, that doesn't impact
    /// // the other regular expressions.
    /// let res: &[ &[u8] ] = &[
    ///     &b"<[^>]+[@.]example\\.org>$"[..],
    ///     // Invalid UTF-8.
    ///     &b"\xC3\x28"[..],
    /// ];
    /// assert!(std::str::from_utf8(res[0]).is_ok());
    /// assert!(std::str::from_utf8(res[1]).is_err());
    ///
    /// let re_set = RegexSet::from_bytes(res.into_iter())?;
    ///
    /// assert!(re_set.is_match("Alice <alice@example.org>"));
    /// assert!(! re_set.is_match("Bob <bob@example.com>"));
    ///
    /// // If we only have invalid UTF-8 strings, then nothing
    /// // matches.
    /// let res: &[ &[u8] ] = &[
    ///     // Invalid UTF-8.
    ///     &b"\xC3\x28"[..],
    /// ];
    /// assert!(std::str::from_utf8(res[0]).is_err());
    ///
    /// let re_set = RegexSet::from_bytes(res.into_iter())?;
    ///
    /// assert!(! re_set.is_match("Alice <alice@example.org>"));
    /// assert!(! re_set.is_match("Bob <bob@example.com>"));
    ///
    ///
    /// // But, if we have no regular expressions, everything matches.
    /// let res: &[ &[u8] ] = &[];
    /// let re_set = RegexSet::from_bytes(res.into_iter())?;
    ///
    /// assert!(re_set.is_match("Alice <alice@example.org>"));
    /// assert!(re_set.is_match("Bob <bob@example.com>"));
    /// # Ok(()) }
    /// ```
    pub fn from_bytes<'a, I, RE>(res: I) -> Result<Self>
    where I: IntoIterator<Item=RE>,
          RE: Borrow<&'a [u8]>,
    {
        let mut have_valid_utf8 = false;
        let mut have_invalid_utf8 = false;
        let re_set = Self::new(
            res
                .into_iter()
                .scan((&mut have_valid_utf8, &mut have_invalid_utf8),
                      |(valid, invalid), re|
                      {
                          if let Ok(re) = std::str::from_utf8(re.borrow()) {
                              **valid = true;
                              Some(Some(re))
                          } else {
                              **invalid = true;
                              Some(None)
                          }
                      })
                .flatten());

        if !have_valid_utf8 && have_invalid_utf8 {
            // None of the strings were valid UTF-8.  Reject
            // everything.
            Ok(RegexSet {
                re_set: RegexSet_::Invalid,
                disable_sanitizations: false,
            })
        } else {
            // We had nothing or at least one string was valid UTF-8.
            // RegexSet::new did the right thing.
            re_set
        }
    }

    /// Creates a `RegexSet` from the regular expressions stored in a
    /// trust signature.
    ///
    /// This method is a convenience function, which extracts any
    /// regular expressions from a [Trust Signature] and wraps them in a
    /// `RegexSet`.
    ///
    ///   [Trust Signature]: https://tools.ietf.org/html/rfc4880#section-5.2.3.13
    ///
    /// If the signature is not a valid trust signature (its [type] is
    /// [GenericCertification], [PersonaCertification],
    /// [CasualCertification], or [PositiveCertification], and the
    /// [Trust Signature] subpacket is present), this returns an
    /// error.
    ///
    ///   [type]: https://tools.ietf.org/html/rfc4880#section-5.2.1
    ///   [GenericCertification]: crate::types::SignatureType::GenericCertification
    ///   [PersonaCertification]: crate::types::SignatureType::PersonaCertification
    ///   [CasualCertification]: crate::types::SignatureType::CasualCertification
    ///   [PositiveCertification]: crate::types::SignatureType::PositiveCertification
    ///
    /// By default, strings that don't pass a sanity check (in
    /// particular, include Unicode control characters) never match.
    /// This behavior can be customized using
    /// [`RegexSet::disable_sanitizations`].
    ///
    ///   [`RegexSet::disable_sanitizations`]: RegexSet::disable_sanitizations()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::cert::prelude::*;
    /// # use openpgp::packet::prelude::*;
    /// # use openpgp::policy::StandardPolicy;
    /// use openpgp::regex::RegexSet;
    /// # use openpgp::types::SignatureType;
    /// #
    /// # fn main() -> openpgp::Result<()> {
    /// # let p = &StandardPolicy::new();
    /// #
    /// # let (alice, _)
    /// #     = CertBuilder::general_purpose(None, Some("Alice <alice@example.org>"))
    /// #         .generate()?;
    /// # let mut alices_signer = alice.primary_key().key().clone()
    /// #     .parts_into_secret()?.into_keypair()?;
    /// #
    /// # let (example_com, _)
    /// #     = CertBuilder::general_purpose(None, Some("OpenPGP CA <openpgp-ca@example.com>"))
    /// #         .generate()?;
    /// # let example_com_userid = example_com.with_policy(p, None)?
    /// #     .userids().nth(0).expect("Added a User ID").userid();
    /// #
    /// # let certification = SignatureBuilder::new(SignatureType::GenericCertification)
    /// #     .set_trust_signature(1, 120)?
    /// #     .set_regular_expression("<[^>]+[@.]example\\.org>$")?
    /// #     .add_regular_expression("<[^>]+[@.]example\\.com>$")?
    /// #     .sign_userid_binding(
    /// #         &mut alices_signer,
    /// #         example_com.primary_key().component(),
    /// #         example_com_userid)?;
    ///
    /// // certification is a trust signature, which contains two regular
    /// // expressions: one that matches all mail addresses for 'example.org'
    /// // and another that matches all mail addresses for 'example.com'.
    /// let certification: &Signature = // ...;
    /// # &certification;
    ///
    /// // Extract the regex and compile it.
    /// let res = RegexSet::from_signature(certification)?;
    ///
    /// // Some positive examples.
    /// assert!(res.is_match("Alice <alice@example.org>"));
    /// assert!(res.is_match("Bob <bob@example.com>"));
    ///
    /// // Wrong domain.
    /// assert!(! res.is_match("Carol <carol@acme.com>"));
    ///
    /// // The standard regex, "<[^>]+[@.]example\\.org>$" only matches
    /// // email addresses wrapped in <>.
    /// assert!(! res.is_match("dave@example.com"));
    ///
    /// // And, it is case sensitive.
    /// assert!(res.is_match("Ellen <ellen@example.com>"));
    /// assert!(! res.is_match("Ellen <ellen@EXAMPLE.COM>"));
    /// # Ok(()) }
    /// ```
    pub fn from_signature(sig: &Signature) -> Result<Self>
    {
        use SignatureType::*;
        match sig.typ() {
            GenericCertification => (),
            PersonaCertification => (),
            CasualCertification => (),
            PositiveCertification => (),
            t => return Err(
                Error::InvalidArgument(
                    format!(
                        "Expected a certification signature, found a {}",
                        t))
                    .into()),
        }

        if sig.trust_signature().is_none() {
            return Err(
                Error::InvalidArgument(
                    "Expected a trust signature, \
                     but the signature does not include \
                     a valid Trust Signature subpacket".into())
                    .into());
        }

        Self::from_bytes(sig.regular_expressions())
    }

    /// Returns a `RegexSet` that matches everything.
    ///
    /// Note: sanitizations are still enabled.  So, to really match
    /// everything, you still need to call
    /// [`RegexSet::disable_sanitizations`].
    ///
    ///   [`RegexSet::disable_sanitizations`]: RegexSet::disable_sanitizations()
    ///
    /// This can be used to optimize the evaluation of scoping rules
    /// along a path: if a `RegexSet` matches everything, then it
    /// doesn't further constrain the path.
    pub fn everything() -> Result<Self>
    {
        Ok(Self {
            re_set: RegexSet_::Everything,
            disable_sanitizations: false,
        })
    }

    /// Returns whether a `RegexSet` matches everything.
    ///
    /// Normally, this only returns true if the `RegexSet` was created
    /// using [`RegexSet::everything`].  [`RegexSet::new`],
    /// [`RegexSet::from_bytes`], [`RegexSet::from_signature`] do
    /// detect some regular expressions that match everything (e.g.,
    /// if no regular expressions are supplied).  But, they do not
    /// guarantee that a `RegexSet` containing a regular expression
    /// like `.?`, which does in fact match everything, is detected as
    /// matching everything.
    ///
    ///   [`RegexSet::everything`]: RegexSet::everything()
    ///   [`RegexSet::new`]: RegexSet::everything()
    ///   [`RegexSet::from_bytes`]: RegexSet::from_bytes()
    ///   [`RegexSet::from_signature`]: RegexSet::from_signature()
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::regex::RegexSet;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// assert!(RegexSet::everything()?.matches_everything());
    /// let empty: &[ &str ] = &[];
    /// assert!(RegexSet::new(empty)?.matches_everything());
    ///
    /// // A regular expression that matches everything.  But
    /// // `RegexSet` returns false, because it can't detect it.
    /// let res: &[ &str ] = &[
    ///     &".?"[..],
    /// ];
    /// let re_set = RegexSet::new(res.into_iter())?;
    /// assert!(! re_set.matches_everything());
    /// # Ok(()) }
    /// ```
    pub fn matches_everything(&self) -> bool {
        matches!(self.re_set, RegexSet_::Everything)
    }

    /// Controls whether strings with control characters are allowed.
    ///
    /// If `false` (the default), i.e., sanity checks are enabled, and
    /// the string doesn't pass the sanity check (in particular, it
    /// contains a Unicode control character according to
    /// [`char::is_control`], including newlines and an embedded `NUL`
    /// byte), this returns `false`.
    ///
    ///   [`char::is_control`]: https://doc.rust-lang.org/std/primitive.char.html#method.is_control
    pub fn disable_sanitizations(&mut self, allowed: bool) {
        self.disable_sanitizations = allowed;
        if let RegexSet_::Regex(ref mut re) = self.re_set {
            re.disable_sanitizations(allowed);
        }
    }

    /// Returns whether the regular expression set matches the string.
    ///
    /// If sanity checks are enabled (the default) and the string
    /// doesn't pass the sanity check (in particular, it contains a
    /// Unicode control character according to [`char::is_control`],
    /// including newlines and an embedded `NUL` byte), this returns
    /// `false`.
    ///
    ///   [`char::is_control`]: https://doc.rust-lang.org/std/primitive.char.html#method.is_control
    ///
    /// If the `RegexSet` contains one or more regular expressions,
    /// this method returns whether at least one of the regular
    /// expressions matches.  Invalid regular expressions never match.
    ///
    /// If the `RegexSet` does not contain any regular expressions
    /// (valid or otherwise), this method returns `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::regex::RegexSet;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// // A regular expression that matches anything.  (Note: this is
    /// // equivalent to providing no regular expressions.)
    /// let res: &[ &str ] = &[
    ///     &""[..],
    /// ];
    /// let re_set = RegexSet::new(res.into_iter())?;
    ///
    /// assert!(re_set.is_match("Alice Lovelace <alice@example.org>"));
    ///
    /// // If a User ID has an embedded control character, it doesn't
    /// // match.
    /// assert!(! re_set.is_match("Alice <alice@example.org>\0"));
    /// # Ok(()) }
    /// ```
    pub fn is_match(&self, s: &str) -> bool {
        if ! self.disable_sanitizations && s.chars().any(char::is_control) {
            return false;
        }

        match self.re_set {
            RegexSet_::Regex(ref re) =>
                re.is_match_clean(s),
            RegexSet_::Invalid =>
                false,
            RegexSet_::Everything =>
                true,
        }
    }

    /// Returns whether the regular expression matches the User ID.
    ///
    /// If the User ID is not a valid UTF-8 string, this returns `false`.
    ///
    /// If sanity checks are enabled (the default) and the string
    /// doesn't pass the sanity check (in particular, it contains a
    /// Unicode control character according to [`char::is_control`],
    /// including newlines and an embedded `NUL` byte), this returns
    /// `false`.
    ///
    ///   [`char::is_control`]: https://doc.rust-lang.org/std/primitive.char.html#method.is_control
    ///
    /// If the `RegexSet` contains one or more regular expressions,
    /// this method returns whether at least one of the regular
    /// expressions matches.  Invalid regular expressions never match.
    ///
    /// If the `RegexSet` does not contain any regular expressions
    /// (valid or otherwise), this method returns `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::packet::UserID;
    /// use openpgp::regex::RegexSet;
    ///
    /// # fn main() -> openpgp::Result<()> {
    /// // A regular expression that matches anything.  (Note: this is
    /// // equivalent to providing no regular expressions.)
    /// let res: &[ &str ] = &[
    ///     "",
    /// ];
    /// let re_set = RegexSet::new(res.into_iter())?;
    ///
    /// assert!(re_set.matches_userid(
    ///     &UserID::from(&b"Alice Lovelace <alice@example.org>"[..])));
    ///
    /// // If a User ID is not valid UTF-8, it never matches.
    /// assert!(! re_set.matches_userid(
    ///     &UserID::from(&b"Alice \xC3\x28 Lovelace <alice@example.org>"[..])));
    ///
    /// // If a User ID has an embedded control character, it doesn't
    /// // match.
    /// assert!(! re_set.matches_userid(
    ///     &UserID::from(&b"Alice <alice@example.org>\0"[..])));
    /// # Ok(()) }
    /// ```
    pub fn matches_userid(&self, u: &UserID) -> bool
    {
        let u = u.borrow();
        if let Ok(u) = std::str::from_utf8(u.value()) {
            self.is_match(u)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex() -> Result<()> {
        fn a(regex: &str, matches: &[(bool, &str)]) {
            eprint!("{} -> ", regex);
            let mut compiled = Regex::new(regex).unwrap();
            compiled.disable_sanitizations(true);
            eprintln!("{:?}", compiled);
            for &(matches, text) in matches {
                assert_eq!(matches, compiled.is_match(text),
                           "regex: {}\n text: {:?} should{} match",
                           regex, text, if matches { "" } else { " not" });
            }
        }
        fn f(regex: &str) {
            eprint!("{} -> ", regex);
            let compiled = Regex::new(regex);
            assert!(compiled.is_err());
            eprintln!("failed (expected)");
        }

        // Test an important corner case: the + should only apply to
        // the b!  See: https://github.com/rust-lang/regex/issues/731
        a("xab+y", &[
            (true, "xaby"),
            (true, "xabby"),
            (false, "xababy"),
        ]);
        a("x(ab+)y", &[
            (false, "xy"),
            (false, "xay"),
            (true, "xaby"),
            (true, "xabby"),
            (true, "xabbby"),
            (false, "xababy"),
        ]);
        // But here the + matches "ab", not just the "b".
        a("x(ab)+y", &[
            (false, "xy"),
            (true, "xaby"),
            (false, "xabby"),
            (true, "xababy"),
            (true, "xabababy"),
            (false, "x(ab)y"),
        ]);



        a("", &[
            (true, "s"),
            (true, "ss"),
        ]);
        a("s", &[
            (true, "s"),
            (true, "ss"),
            (false, "a"),
            (true, "hello, my prettiessss"),
            (false, "S"),
        ]);
        a("ss", &[
            (false, "s"),
            (true, "ss"),
            (true, "sss"),
            (false, "this has lots of ses, but not two ses together"),
            (true, "halloss"),
        ]);

        a("a|b", &[
            (true, "a"),
            (true, "b"),
            (false, "c"),
            (true, "xxxaxxxbxxx"),
        ]);
        a("a|b|c", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (false, "d"),
            (true, "xxxaxxxbxxx"),
        ]);
        // This should match anything.
        a("|a", &[
            (true, "a"),
            (true, "b"),
        ]);
        a("a|", &[
            (true, "a"),
            (true, "b"),
        ]);
        a("|a|b", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
        ]);
        a("|a|b|c|d", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "d"),
            (true, "eeee"),
        ]);
        a("a|b|", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
        ]);
        a("a|b|c|", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "d"),
            (true, "eeee"),
        ]);
        a("|", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "d"),
            (true, "eeee"),
        ]);
        a("|a|", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "d"),
            (true, "eeee"),
        ]);
        a("|a|b|", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "d"),
            (true, "eeee"),
        ]);
        // A nested empty.
        a("(a|)|b", &[
            (true, "a"),
            (true, "b"),
        ]);
        // empty+
        a("(a|b|()+)", &[
            (true, "a"),
            (true, "b"),
        ]);
        // (empty)+
        a("(a|b|(())+)", &[
            (true, "a"),
            (true, "b"),
        ]);
        // Multiple empty branches.
        a("(a|b|(()())())", &[
            (true, "a"),
            (true, "b"),
        ]);
        a("(a|b|(()())())|", &[
            (true, "a"),
            (true, "b"),
        ]);

        // This is: "ab" or "cd", not a followed by b or c followed by d:
        //
        //   A regular expression is zero or more branches, separated by '|'.
        //   ...
        //   A branch is zero or more pieces, concatenated.
        //   ...
        //   A piece is an atom
        //   ...
        //   An atom is... a single character.
        a("ab|cd", &[
            (true, "abd"),
            (true, "acd"),
            (true, "abcd"),
            (false, "ad"),
            (false, "b"),
            (false, "c"),
            (false, "bb"),
        ]);

        a("a*", &[
            (true, ""),
            (true, "a"),
            (true, "aa"),
            (true, "b"),
        ]);
        a("xa*y", &[
            (true, "xy"),
            (true, "xay"),
            (true, "xaay"),
            (false, "y"),
            (false, "ay"),
            (false, "aay"),
            (false, "x y"),
            (false, "x ay"),
            (false, "x aay"),
        ]);
        f("*");

        a("a+", &[
            (false, ""),
            (true, "a"),
            (true, "aa"),
            (false, "b"),
            (true, "baab"),
            (true, "by ab"),
            (true, "baa b"),
        ]);
        a("ab+", &[
            (false, ""),
            (false, "a"),
            (false, "b"),
            (true, "ab"),
            (false, "bb"),
            (true, "baab"),
            (true, "by ab"),
            (false, "baa b"),
        ]);
        f("+");

        a("a?", &[
            (true, ""),
            (true, "a"),
            (true, "aa"),
            (true, "aaa"),
            (true, "b"),
            (true, "baab"),
            (true, "by ab"),
            (true, "baa b"),
        ]);
        a("xa?y", &[
            (false, ""),
            (true, "xy"),
            (false, "a"),
            (true, "xay"),
            (false, "aa"),
            (false, "xaay"),
            (false, "b"),
            (false, "bxaayb"),
            (true, "by xayb"),
            (true, "baxay b"),
        ]);
        f("?");

        f("a*?");
        a("a*b?c+", &[
            (false, ""),
            (true, "c"),
            (true, "abc"),
            (true, "aabbcc"),
            (false, "aab"),
            (true, "aaaaaabcccccccc"),
        ]);
        f("a?*+");

        a("a?|b+", &[
            (true, ""),
            (true, "aaa"),
            (true, "bbb"),
            (true, "abaa"),
        ]);
        a("a+|b+", &[
            (false, ""),
            (true, "a"),
            (true, "aaa"),
            (true, "b"),
            (true, "bbb"),
            (true, "abaa"),
        ]);
        a("a+|b+|c+", &[
            (false, ""),
            (true, "a"),
            (true, "aaa"),
            (true, "b"),
            (true, "bbb"),
            (true, "abaa"),
            (true, "c"),
            (true, "ccc"),
            (true, "abaaccc"),
        ]);
        a("xa+|b+|c+y", &[
            (false, ""),
            (true, "xa"),
            (true, "xaa"),
            (true, "b"),
            (true, "bb"),
            (true, "cy"),
            (true, "ccy"),

            (false, "a"),
            (false, "aaa"),
            (false, "c"),
            (false, "ccc"),
        ]);
        a("xa+y|sb+u", &[
            (false, ""),
            (true, "xay"),
            (true, "xaay"),
            (true, "sbu"),
            (true, "sbbu"),
            (true, "xysbu"),

            (false, "a"),
            (false, "aaa"),
            (false, "xyu"),
            (false, "ccc"),
        ]);
        a("a*|a+|ab+cd+|", &[
            (true, ""),
        ]);

        a("()", &[
            (true, ""),
            (true, "xyzzy"),
        ]);
        a("(())", &[
            (true, ""),
            (true, "xyzzy"),
        ]);
        a("((()))", &[
            (true, ""),
            (true, "xyzzy"),
        ]);
        f("((())");
        f("((())))");
        a("(a)", &[
            (true, "a"),
            (true, "(a)"),
            (false, "b"),
        ]);
        a("x(a)y", &[
            (false, "xy"),
            (true, "xay"),
            (false, "x(a)y"),
            (true, "(xay)"),
            (false, "a"),
            (false, "yax"),
        ]);
        a("x(ab)y", &[
            (false, "xy"),
            (false, "xay"),
            (false, "xby"),
            (true, "xaby"),
            (false, "x(ab)y"),
            (true, "(xaby)"),
        ]);
        a("x(ab)(cd)y", &[
            (true, "xabcdy"),
            (true, "zxabcdyz"),
        ]);
        a("a(bc)d(ef)g", &[
            (true, "abcdefg"),
            (true, "xabcdefgy"),
            (false, "xa(bc)d(ef)gy"),
        ]);
        a("a((bc))d((ef))g", &[
            (true, "abcdefg"),
            (true, "xabcdefgy"),
            (false, "xa(bc)d(ef)gy"),
        ]);
        a("a(b(c)d)e", &[
            (true, "abcde"),
            (true, "xabcdey"),
            (false, "xa(b(c)d)ey"),
        ]);
        a("x(a+|b+)y", &[
            (false, "xy"),
            (true, "xay"),
            (true, "xby"),
            (true, "xaay"),
            (true, "xbby"),
            (false, "xaby"),
            (false, "xaaby"),
            (false, "xabby"),
            (false, "xaabby"),
            (false, "xcy"),
        ]);

        a(".", &[
            (false, ""),
            (true, "a"),
            (true, "ab"),
            (true, "ab\nc"),
            (true, "ab.c"),
        ]);
        a("x.y", &[
            (false, ""),
            (false, "xy"),
            (true, "xay"),
            (true, "x\ny"),
            (true, "x.y"),
            (false, "x..y"),
        ]);

        a("^", &[
            (true, ""),
            (true, "xx"),
        ]);
        a("^abc", &[
            (false, ""),
            (true, "abcdef"),
            (false, "xabcdef"),
            (false, "\nabcdef"),
        ]);
        a("(^abc|^def)", &[
            (false, ""),
            (true, "abcd"),
            (true, "defg"),
            (false, "xabcd"),
            (false, "xdefg"),
            (false, "^abc"),
            (false, "^(abc|def)"),
            (false, "\nabcdef"),
        ]);
        a("(^abc|def)", &[
            (false, ""),
            (true, "abcd"),
            (true, "defg"),
            (false, "xabcd"),
            (true, "xdefg"),
            (false, "^abc"),
            (true, "^(abc|def)"),
            (false, "\nabcde"),
        ]);
        a("^^", &[
            (true, ""),
            (true, "abcdef"),
        ]);
        a("^abc^", &[
            (false, ""),
            (false, "abcdef"),
            (false, "xabcdef"),
            (false, "abc\n"),
            (false, "\nabc\n"),
            (false, "^abc^"),
        ]);

        a("$", &[
            (true, ""),
            (true, "abc"),
        ]);
        a("abc$", &[
            (false, ""),
            (true, "abc"),
            (false, "abcx"),
            (false, "abc\n"),
            (false, "abc$"),
        ]);
        a("abc$$", &[
            (false, ""),
            (true, "abc"),
            (false, "abcx"),
            (false, "abc\n"),
            (false, "abc$"),
        ]);
        a("(abc$)x", &[
            (false, ""),
            (false, "abc"),
            (false, "abcx"),
            (false, "abc\nx"),
            (false, "abc$x"),
        ]);
        a("abc$|def$", &[
            (false, ""),
            (true, "abc"),
            (false, "abcx"),
            (false, "abc\n"),
            (false, "abc$"),
            (true, "def"),
            (false, "defx"),
            (false, "def\n"),
            (false, "def$"),
            (true, "abcdef"),
        ]);

        a("\\|", &[
            (true, "|"),
            (false, ""),
            (false, "a"),
        ]);
        a("\\*", &[
            (true, "*"),
            (false, ""),
            (false, "a"),
        ]);
        a("\\+", &[
            (true, "+"),
            (false, ""),
            (false, "a"),
        ]);
        a("\\?", &[
            (true, "?"),
            (false, ""),
            (false, "a"),
        ]);
        a("\\.", &[
            (true, "."),
            (false, ""),
            (false, "a"),
        ]);
        a("\\^", &[
            (true, "^"),
            (false, ""),
            (false, "a"),
        ]);
        a("\\$", &[
            (true, "$"),
            (false, ""),
            (false, "a"),
        ]);
        a("\\\\", &[
            (true, "\\"),
            (false, ""),
            (false, "a"),
        ]);
        a("\\[", &[
            (true, "["),
            (false, ""),
            (false, "a"),
        ]);
        a("\\]", &[
            (true, "]"),
            (false, ""),
            (false, "a"),
        ]);
        a("\\-", &[
            (true, "-"),
            (false, ""),
            (false, "a"),
        ]);
        f("\\");

        a("[a]", &[
            (true, "a"),
            (false, "b"),
        ]);
        a("[abc]", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (false, "d"),
        ]);
        a("[a-c]", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (false, "d"),
        ]);
        a("[xa-c]", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "x"),
            (false, "d"),
        ]);
        a("[a-cxyz]", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "x"),
            (false, "d"),
        ]);
        a("[a-c]x", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (true, "ax"),
            (true, "bx"),
            (true, "cx"),
            (false, "d"),
            (false, "dx"),
        ]);
        a("[a-cxy]", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "x"),
            (true, "y"),
            (false, "d"),
        ]);
        a("[a-c]xy", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "ax"),
            (false, "bx"),
            (false, "cx"),
            (true, "axy"),
            (true, "bxy"),
            (true, "cxy"),
            (false, "d"),
        ]);
        a("[a-cxyz]", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "x"),
            (true, "y"),
            (true, "z"),
            (false, "d"),
        ]);
        a("[a-c]xyz", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "ax"),
            (false, "bx"),
            (false, "cx"),
            (false, "axy"),
            (false, "bxy"),
            (false, "cxy"),
            (true, "axyz"),
            (true, "bxyz"),
            (true, "cxyz"),
            (false, "d"),
        ]);
        a("xyz[a-c]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "xa"),
            (false, "xb"),
            (false, "xc"),
            (false, "xya"),
            (false, "xyb"),
            (false, "xyc"),
            (true, "xyza"),
            (true, "xyzb"),
            (true, "xyzc"),
            (false, "d"),
        ]);
        a("[xyza-c]", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "x"),
            (true, "y"),
            (true, "z"),
            (false, "d"),
        ]);
        a("[xya-cyz]", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "x"),
            (true, "y"),
            (true, "z"),
            (false, "d"),
        ]);
        a("[x-za-c]", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "x"),
            (true, "y"),
            (true, "z"),
            (false, "d"),
        ]);
        a("[x-zmna-c]", &[
            (true, "a"),
            (true, "b"),
            (true, "c"),
            (true, "x"),
            (true, "y"),
            (true, "z"),
            (true, "m"),
            (true, "n"),
            (false, "d"),
        ]);
        a("[-]", &[
            (true, "-"),
            (false, "d"),
        ]);
        a("[a-]", &[
            (true, "-"),
            (true, "a"),
            (false, "d"),
        ]);
        a("[-b]", &[
            (true, "-"),
            (true, "b"),
            (false, "d"),
        ]);
        a("[-bd-g]", &[
            (false, "a"),
            (true, "-"),
            (true, "b"),
            (true, "d"),
            (true, "f"),
        ]);
        a("[bd-g-]", &[
            (false, "a"),
            (true, "-"),
            (true, "b"),
            (true, "d"),
            (true, "f"),
        ]);
        // Backwards ranges.
        a("[9-0]", &[
            (false, "a"),
            (false, "-"),
            (true, "9"),
            (true, "0"),
            (true, "5"),
        ]);

        a("[^a]", &[
            (false, "a"),
            (true, "b"),
        ]);
        a("[^abc]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (true, "d"),
        ]);
        a("[^a-c]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (true, "d"),
        ]);
        a("[^xa-c]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (true, "d"),
        ]);
        a("[^a-cxyz]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (true, "d"),
        ]);
        a("[^a-c]x", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "ax"),
            (false, "bx"),
            (false, "cx"),
            (false, "d"),
            (true, "dx"),
        ]);
        a("[^a-cxy]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "y"),
            (true, "d"),
        ]);
        a("[^a-c]xy", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "ax"),
            (false, "bx"),
            (false, "cx"),
            (false, "axy"),
            (false, "bxy"),
            (false, "cxy"),
            (true, "dxy"),
            (false, "d"),
        ]);
        a("[^a-cxyz]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "y"),
            (false, "z"),
            (true, "d"),
        ]);
        a("[^a-c]xyz", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "ax"),
            (false, "bx"),
            (false, "cx"),
            (false, "axy"),
            (false, "bxy"),
            (false, "cxy"),
            (false, "axyz"),
            (false, "bxyz"),
            (false, "cxyz"),
            (true, "dxyz"),
            (false, "d"),
        ]);
        a("xyz[^a-c]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "xa"),
            (false, "xb"),
            (false, "xc"),
            (false, "xya"),
            (false, "xyb"),
            (false, "xyc"),
            (false, "xyza"),
            (false, "xyzb"),
            (false, "xyzc"),
            (true, "xyzd"),
            (false, "d"),
        ]);
        a("[^xyza-c]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "y"),
            (false, "z"),
            (true, "d"),
        ]);
        a("[^xya-cyz]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "y"),
            (false, "z"),
            (true, "d"),
        ]);
        a("[^x-za-c]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "y"),
            (false, "z"),
            (true, "d"),
        ]);
        a("[^x-zmna-c]", &[
            (false, "a"),
            (false, "b"),
            (false, "c"),
            (false, "x"),
            (false, "y"),
            (false, "z"),
            (false, "m"),
            (false, "n"),
            (true, "d"),
        ]);
        a("[^-]", &[
            (false, "-"),
            (true, "d"),
        ]);
        a("[^a-]", &[
            (false, "-"),
            (false, "a"),
            (true, "d"),
        ]);
        a("[^-b]", &[
            (false, "-"),
            (false, "b"),
            (true, "d"),
        ]);
        a("[^-bd-g]", &[
            (true, "a"),
            (false, "-"),
            (false, "b"),
            (false, "d"),
            (false, "f"),
        ]);
        a("[^bd-g-]", &[
            (true, "a"),
            (false, "-"),
            (false, "b"),
            (false, "d"),
            (false, "f"),
        ]);

        a("[a|b]", &[
            (true, "a"),
            (true, "|"),
            (false, "c"),
        ]);
        a("[a\\|b]", &[
            (true, "a"),
            (true, "|"),
            (true, "\\"),
            (false, "c"),
        ]);
        a("[a(b]", &[
            (true, "a"),
            (true, "("),
            (false, "c"),
        ]);
        a("[a)b]", &[
            (true, "a"),
            (true, ")"),
            (false, "c"),
        ]);
        a("[a^b]", &[
            (true, "a"),
            (true, "^"),
            (false, "c"),
        ]);

        f("[]");
        f("[^]");
        a("[^]]", &[
            (true, "a"),
            (false, "]"),
            (true, "^"),
        ]);
        a("[]]", &[
            (false, "a"),
            (true, "]"),
        ]);
        // Matches [ or ].
        a("[][]", &[
            (false, "a"),
            (true, "["),
            (true, "]"),
        ]);
        // Matches anything but [ or ].
        a("[^][]", &[
            (true, "a"),
            (false, "["),
            (false, "]"),
        ]);
        // Anything but ^.
        a("[^^]", &[
            (true, "a"),
            (false, "^"),
            (true, "c"),
        ]);

        // Make sure - is recognized as an atom when it is not part of
        // a range.  That is: a-z matches a or - or z, but it doesn't
        // match b (it's not a range).
        a("a-z", &[
            (true, "a-z"),
            (false, "a"),
            (false, "-"),
            (false, "z"),
            (false, "c"),
        ]);

        a("a|-|z", &[
            (true, "a"),
            (true, "-"),
            (true, "z"),
            (false, "c"),
        ]);

        Ok(())
    }

    #[test]
    fn regex_set() -> Result<()> {
        let re = RegexSet::new(&[ "ab", "cd" ])?;
        assert!(re.is_match("ab"));
        assert!(re.is_match("cdef"));
        assert!(!re.is_match("xxx"));

        // Try to make sure one re does not leak into another.
        let re = RegexSet::new(&[ "cd$", "^ab" ])?;
        assert!(re.is_match("abxx"));
        assert!(re.is_match("xxcd"));

        // Invalid regular expressions should be ignored.
        let re = RegexSet::new(&[ "[ab", "cd]", "x" ])?;
        assert!(!re.is_match("a"));
        assert!(!re.is_match("ab"));
        assert!(!re.is_match("[ab"));
        assert!(!re.is_match("c"));
        assert!(!re.is_match("cd"));
        assert!(!re.is_match("cd]"));
        assert!(re.is_match("x"));

        // If all regular expressions are invalid, nothing should
        // match.
        let re = RegexSet::new(&[ "[ab", "cd]" ])?;
        assert!(!re.is_match("a"));
        assert!(!re.is_match("ab"));
        assert!(!re.is_match("[ab"));
        assert!(!re.is_match("c"));
        assert!(!re.is_match("cd"));
        assert!(!re.is_match("cd]"));
        assert!(!re.is_match("x"));

        // If there are no regular expressions, everything should
        // match.
        let s: [&str; 0] = [];
        let re = RegexSet::new(&s)?;
        assert!(re.is_match("a"));
        assert!(re.is_match("ab"));
        assert!(re.is_match("[ab"));
        assert!(re.is_match("c"));
        assert!(re.is_match("cd"));
        assert!(re.is_match("cd]"));
        assert!(re.is_match("x"));

        Ok(())
    }

    #[test]
    fn regex_set_sequoia() -> Result<()> {
        let re = RegexSet::new(&["<[^>]+[@.]sequoia-pgp\\.org>$"])?;
        dbg!(&re);
        assert!(re.is_match("<justus@sequoia-pgp.org>"));
        assert!(!re.is_match("<justus@gnupg.org>"));
        Ok(())
    }

    #[test]
    fn regex_set_sequoia_nodash() -> Result<()> {
        let re = RegexSet::new(&["<[^>]+[@.]sequoiapgp\\.org>$"])?;
        dbg!(&re);
        assert!(re.is_match("<justus@sequoiapgp.org>"));
        assert!(!re.is_match("<justus@gnupg.org>"));
        Ok(())
    }
}
