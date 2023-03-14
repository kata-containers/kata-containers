//! Message support.
//!
//! An OpenPGP message is a sequence of OpenPGP packets that
//! corresponds to an optionally signed, optionally encrypted,
//! optionally compressed literal data packet.  The exact format of an
//! OpenPGP message is described in [Section 11.3 of RFC 4880].
//!
//! This module provides support for validating and working with
//! OpenPGP messages.
//!
//! Example of ASCII-armored OpenPGP message:
//!
//! ```txt
//! -----BEGIN PGP MESSAGE-----
//!
//! yDgBO22WxBHv7O8X7O/jygAEzol56iUKiXmV+XmpCtmpqQUKiQrFqclFqUDBovzS
//! vBSFjNSiVHsuAA==
//! =njUN
//! -----END PGP MESSAGE-----
//! ```
//!
//! [Section 11.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-11.3

// XXX: For Token.  Drop this once Token is private.
#![allow(deprecated)]

use std::convert::TryFrom;
use std::fmt;
use std::io;
use std::path::Path;

use crate::Result;
use crate::Error;
use crate::Packet;
use crate::PacketPile;
use crate::packet::Literal;
use crate::packet::Tag;
use crate::parse::Parse;

mod lexer;
lalrpop_util::lalrpop_mod!(#[allow(clippy::all, deprecated)] grammar, "/message/grammar.rs");

use self::lexer::{Lexer, LexicalError};
pub use self::lexer::Token;

use lalrpop_util::ParseError;

use self::grammar::MessageParser;

/// Errors that MessageValidator::check may return.
///
/// Note: This enum cannot be exhaustively matched to allow future
/// extensions.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum MessageParserError {
    /// A parser error.
    Parser(ParseError<usize, Token, LexicalError>),
    /// An OpenPGP error.
    OpenPGP(Error),
}
assert_send_and_sync!(MessageParserError);

impl From<MessageParserError> for anyhow::Error {
    fn from(err: MessageParserError) -> Self {
        match err {
            MessageParserError::Parser(p) => p.into(),
            MessageParserError::OpenPGP(p) => p.into(),
        }
    }
}


/// Represents the status of a parsed message.
#[derive(Debug)]
pub(crate) enum MessageValidity {
    /// The packet sequence is a valid OpenPGP message.
    Message,
    /// The packet sequence appears to be a valid OpenPGP message that
    /// has been truncated, i.e., the packet sequence is a valid
    /// prefix of an OpenPGP message.
    MessagePrefix,
    /// The message is definitely not valid.
    Error(anyhow::Error),
}

#[allow(unused)]
impl MessageValidity {
    /// Returns whether the packet sequence is a valid message.
    ///
    /// Note: a `MessageValidator` will only return this after
    /// `MessageValidator::finish` has been called.
    pub fn is_message(&self) -> bool {
        matches!(self, MessageValidity::Message)
    }

    /// Returns whether the packet sequence forms a valid message
    /// prefix.
    ///
    /// Note: a `MessageValidator` will only return this before
    /// `MessageValidator::finish` has been called.
    pub fn is_message_prefix(&self) -> bool {
        matches!(self, MessageValidity::MessagePrefix)
    }

    /// Returns whether the packet sequence is definitely not a valid
    /// OpenPGP Message.
    pub fn is_err(&self) -> bool {
        matches!(self, MessageValidity::Error(_))
    }
}

/// Used to help validate a packet sequence is a valid OpenPGP message.
#[derive(Debug)]
pub(crate) struct MessageValidator {
    tokens: Vec<Token>,
    finished: bool,
    // Once a raw token is pushed, this is set to None and pushing
    // packet Tags is no longer supported.
    depth: Option<isize>,

    // If we know that the packet sequence is invalid.
    error: Option<MessageParserError>,
}

impl Default for MessageValidator {
    fn default() -> Self {
        MessageValidator::new()
    }
}

impl MessageValidator {
    /// Instantiates a new `MessageValidator`.
    pub fn new() -> Self {
        MessageValidator {
            tokens: vec![],
            finished: false,
            depth: Some(0),
            error: None,
        }
    }

    /// Adds a token to the token stream.
    #[cfg(test)]
    pub(crate) fn push_raw(&mut self, token: Token) {
        assert!(!self.finished);

        if self.error.is_some() {
            return;
        }

        self.depth = None;
        self.tokens.push(token);
    }

    /// Add the token `token` at position `path` to the token stream.
    ///
    /// Note: top-level packets are at `[n]`, their immediate
    /// children are at `[n, m]`, etc.
    ///
    /// This function pushes any required `Token::Pop` tokens based on
    /// changes in the `path`.
    ///
    /// Note: the token *must* correspond to a packet; this function
    /// will panic if `token` is `Token::Pop`.
    pub fn push_token(&mut self, token: Token, path: &[usize]) {
        assert!(!self.finished);
        assert!(self.depth.is_some());
        assert!(token != Token::Pop);
        assert!(!path.is_empty());

        if self.error.is_some() {
            return;
        }

        // We popped one or more containers.
        let depth = path.len() as isize - 1;
        if self.depth.unwrap() > depth {
            for _ in 1..self.depth.unwrap() - depth + 1 {
                self.tokens.push(Token::Pop);
            }
        }
        self.depth = Some(depth);

        self.tokens.push(token);
    }

    /// Add a packet of type `tag` at position `path` to the token
    /// stream.
    ///
    /// Note: top-level packets are at `[n]`, their immediate
    /// children are at `[n, m]`, etc.
    ///
    /// Unlike `push_token`, this function does not automatically
    /// account for changes in the depth.  If you use this function
    /// directly, you must push any required `Token::Pop` tokens.
    pub fn push(&mut self, tag: Tag, version: Option<u8>, path: &[usize]) {
        if self.error.is_some() {
            return;
        }

        let token = match tag {
            Tag::Literal => Token::Literal,
            Tag::CompressedData => Token::CompressedData,
            Tag::SKESK => Token::SKESK,
            Tag::PKESK => Token::PKESK,
            Tag::SEIP if version == Some(1) => Token::SEIPv1,
            Tag::MDC => Token::MDC,
            Tag::AED => Token::AED,
            Tag::OnePassSig => Token::OPS,
            Tag::Signature => Token::SIG,
            Tag::Marker => {
                // "[Marker packets] MUST be ignored when received.",
                // section 5.8 of RFC4880.
                return;
            },
            _ => {
                // Unknown token.
                self.error = Some(MessageParserError::OpenPGP(
                    Error::MalformedMessage(
                        format!("Invalid OpenPGP message: \
                                 {:?} packet (at {:?}) not expected",
                                tag, path))));
                self.tokens.clear();
                return;
            }
        };

        self.push_token(token, path)
    }

    /// Note that the entire message has been seen.
    pub fn finish(&mut self) {
        assert!(!self.finished);

        if let Some(depth) = self.depth {
            // Pop any containers.
            for _ in 0..depth {
                self.tokens.push(Token::Pop);
            }
        }

        self.finished = true;
    }

    /// Returns whether the token stream corresponds to a valid
    /// OpenPGP message.
    ///
    /// This returns a tri-state: if the message is valid, it returns
    /// MessageValidity::Message, if the message is invalid, then it
    /// returns MessageValidity::Error.  If the message could be
    /// valid, then it returns MessageValidity::MessagePrefix.
    ///
    /// Note: if MessageValidator::finish() *hasn't* been called, then
    /// this function will only ever return either
    /// MessageValidity::MessagePrefix or MessageValidity::Error.  Once
    /// MessageValidity::finish() has been called, then only
    /// MessageValidity::Message or MessageValidity::Error will be returned.
    pub fn check(&self) -> MessageValidity {
        if let Some(ref err) = self.error {
            return MessageValidity::Error((*err).clone().into());
        }

        let r = MessageParser::new().parse(
            Lexer::from_tokens(&self.tokens[..]));

        if self.finished {
            match r {
                Ok(_) => MessageValidity::Message,
                Err(ref err) =>
                    MessageValidity::Error(
                        MessageParserError::Parser((*err).clone()).into()),
            }
        } else {
            match r {
                Ok(_) => MessageValidity::MessagePrefix,
                Err(ParseError::UnrecognizedEOF { .. }) =>
                    MessageValidity::MessagePrefix,
                Err(ref err) =>
                    MessageValidity::Error(
                        MessageParserError::Parser((*err).clone()).into()),
            }
        }
    }
}

/// A message.
///
/// An OpenPGP message is a structured sequence of OpenPGP packets.
/// Basically, it's an optionally encrypted, optionally signed literal
/// data packet.  The exact structure is defined in [Section 11.3 of RFC
/// 4880].
///
///   [Section 11.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-11.3
///
/// [ASCII Armored Messages] are wrapped in `-----BEGIN PGP MESSAGE-----` header
/// and `-----END PGP MESSAGE-----` tail lines:
///
/// ```txt
/// -----BEGIN PGP MESSAGE-----
///
/// xA0DAAoW5saJekzviSQByxBiAAAAAADYtdiv2KfZgtipwnUEABYKACcFglwJHYoW
/// IQRnpIdTo4Cms7fffcXmxol6TO+JJAkQ5saJekzviSQAAIJ6APwK6FxtHXn8txDl
/// tBFsIXlOSLOs4BvArlZzZSMomIyFLAEAwCLJUChMICDxWXRlHxORqU5x6hlO3DdW
/// sl/1DAbnRgI=
/// =AqoO
/// -----END PGP MESSAGE-----
/// ```
///
/// [ASCII Armored Messages]: https://tools.ietf.org/html/rfc4880#section-6.6
///
/// # Examples
///
/// Creating a Message encrypted with a password.
///
/// ```
/// # use sequoia_openpgp as openpgp;
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use std::io::Write;
/// use openpgp::serialize::stream::{Message, Encryptor, LiteralWriter};
///
/// let mut sink = vec![];
/// let message = Encryptor::with_passwords(
///     Message::new(&mut sink), Some("ściśle tajne")).build()?;
/// let mut w = LiteralWriter::new(message).build()?;
/// w.write_all(b"Hello world.")?;
/// w.finalize()?;
/// # Ok(()) }
/// ```
#[derive(PartialEq)]
pub struct Message {
    /// A message is just a validated packet pile.
    pile: PacketPile,
}
assert_send_and_sync!(Message);

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Message")
            .field("pile", &self.pile)
            .finish()
    }
}

impl<'a> Parse<'a, Message> for Message {
    /// Reads a `Message` from the specified reader.
    ///
    /// See [`Message::try_from`] for more details.
    ///
    ///   [`Message::try_from`]: Message::try_from()
    fn from_reader<R: 'a + io::Read + Send + Sync>(reader: R) -> Result<Self> {
        Self::try_from(PacketPile::from_reader(reader)?)
    }

    /// Reads a `Message` from the specified file.
    ///
    /// See [`Message::try_from`] for more details.
    ///
    ///   [`Message::try_from`]: Message::try_from()
    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::try_from(PacketPile::from_file(path)?)
    }

    /// Reads a `Message` from `buf`.
    ///
    /// See [`Message::try_from`] for more details.
    ///
    ///   [`Message::try_from`]: Message::try_from()
    fn from_bytes<D: AsRef<[u8]> + ?Sized + Send + Sync>(data: &'a D) -> Result<Self> {
        Self::try_from(PacketPile::from_bytes(data)?)
    }
}

impl std::str::FromStr for Message {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::from_bytes(s.as_bytes())
    }
}

impl Message {
    /// Returns the body of the message.
    ///
    /// Returns `None` if no literal data packet is found.  This
    /// happens if a SEIP container has not been decrypted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use sequoia_openpgp as openpgp;
    /// # fn main() -> openpgp::Result<()> {
    /// use std::io;
    /// use std::io::Read;
    /// use openpgp::Message;
    /// use openpgp::armor::{Reader, ReaderMode};
    /// use openpgp::parse::Parse;
    ///
    /// let data = "yxJiAAAAAABIZWxsbyB3b3JsZCE="; // base64 over literal data packet
    ///
    /// let mut cursor = io::Cursor::new(&data);
    /// let mut reader = Reader::from_reader(&mut cursor, ReaderMode::VeryTolerant);
    ///
    /// let mut buf = Vec::new();
    /// reader.read_to_end(&mut buf)?;
    ///
    /// let message = Message::from_bytes(&buf)?;
    /// assert_eq!(message.body().unwrap().body(), b"Hello world!");
    /// # Ok(())
    /// # }
    /// ```
    pub fn body(&self) -> Option<&Literal> {
        for packet in self.pile.descendants() {
            if let Packet::Literal(ref l) = packet {
                return Some(l);
            }
        }

        // No literal data packet found.
        None
    }
}

impl TryFrom<PacketPile> for Message {
    type Error = anyhow::Error;

    /// Converts the `PacketPile` to a `Message`.
    ///
    /// Converting a `PacketPile` to a `Message` doesn't change the
    /// packets; it asserts that the packet sequence is an optionally
    /// encrypted, optionally signed, optionally compressed literal
    /// data packet.  The exact grammar is defined in [Section 11.3 of
    /// RFC 4880].
    ///
    /// Caveats: this function assumes that any still encrypted parts
    /// or still compressed parts are valid messages.
    ///
    ///   [Section 11.3 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-11.3
    fn try_from(pile: PacketPile) -> Result<Self> {
        let mut v = MessageValidator::new();
        for (mut path, packet) in pile.descendants().paths() {
            match packet {
                Packet::Unknown(ref u) =>
                    return Err(MessageParserError::OpenPGP(
                        Error::MalformedMessage(
                            format!("Invalid OpenPGP message: \
                                     {:?} packet (at {:?}) not expected: {}",
                                    u.tag(), path, u.error())))
                               .into()),
                _ => v.push(packet.tag(), packet.version(), &path),
            }

            match packet {
                Packet::CompressedData(_) | Packet::SEIP(_) | Packet::AED(_) =>
                {
                    // If a container's content is not unpacked, then
                    // we treat the content as an opaque message.

                    path.push(0);
                    if packet.children().is_none() {
                        v.push_token(Token::OpaqueContent, &path);
                    }
                }
                _ => {}
            }
        }
        v.finish();

        match v.check() {
            MessageValidity::Message => Ok(Message { pile }),
            MessageValidity::MessagePrefix => unreachable!(),
            // We really want to squash the lexer's error: it is an
            // internal detail that may change, and meaningless even
            // to an immediate user of this crate.
            MessageValidity::Error(e) => Err(e),
        }
    }
}

impl TryFrom<Vec<Packet>> for Message {
    type Error = anyhow::Error;

    /// Converts the vector of `Packets` to a `Message`.
    ///
    /// See [`Message::try_from`] for more details.
    ///
    ///   [`Message::try_from`]: Message::try_from()
    fn try_from(packets: Vec<Packet>) -> Result<Self> {
        Self::try_from(PacketPile::from(packets))
    }
}

impl From<Message> for PacketPile {
    fn from(m: Message) -> Self {
        m.pile
    }
}

impl ::std::ops::Deref for Message {
    type Target = PacketPile;

    fn deref(&self) -> &Self::Target {
        &self.pile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::types::DataFormat::Text;
    use crate::HashAlgorithm;
    use crate::types::CompressionAlgorithm;
    use crate::SymmetricAlgorithm;
    use crate::PublicKeyAlgorithm;
    use crate::SignatureType;
    use crate::crypto::S2K;
    use crate::crypto::mpi::{Ciphertext, MPI};
    use crate::packet::prelude::*;

    #[test]
    fn tokens() {
        use self::lexer::{Token, Lexer};
        use self::lexer::Token::*;
        use self::grammar::MessageParser;

        struct TestVector<'a> {
            s: &'a [Token],
            result: bool,
        }

        let test_vectors = [
            TestVector {
                s: &[Literal][..],
                result: true,
            },
            TestVector {
                s: &[CompressedData, Literal, Pop],
                result: true,
            },
            TestVector {
                s: &[CompressedData, CompressedData, Literal,
                     Pop, Pop],
                result: true,
            },
            TestVector {
                s: &[SEIPv1, Literal, MDC, Pop],
                result: true,
            },
            TestVector {
                s: &[CompressedData, SEIPv1, Literal, MDC, Pop, Pop],
                result: true,
            },
            TestVector {
                s: &[CompressedData, SEIPv1, CompressedData, Literal,
                     Pop, MDC, Pop, Pop],
                result: true,
            },
            TestVector {
                s: &[SEIPv1, MDC, Pop],
                result: false,
            },
            TestVector {
                s: &[SKESK, SEIPv1, Literal, MDC, Pop],
                result: true,
            },
            TestVector {
                s: &[PKESK, SEIPv1, Literal, MDC, Pop],
                result: true,
            },
            TestVector {
                s: &[SKESK, SKESK, SEIPv1, Literal, MDC, Pop],
                result: true,
            },

            TestVector {
                s: &[AED, Literal, Pop],
                result: true,
            },
            TestVector {
                s: &[CompressedData, AED, Literal, Pop, Pop],
                result: true,
            },
            TestVector {
                s: &[CompressedData, AED, CompressedData, Literal,
                     Pop, Pop, Pop],
                result: true,
            },
            TestVector {
                s: &[AED, Pop],
                result: false,
            },
            TestVector {
                s: &[SKESK, AED, Literal, Pop],
                result: true,
            },
            TestVector {
                s: &[PKESK, AED, Literal, Pop],
                result: true,
            },
            TestVector {
                s: &[SKESK, SKESK, AED, Literal, Pop],
                result: true,
            },

            TestVector {
                s: &[OPS, Literal, SIG],
                result: true,
            },
            TestVector {
                s: &[OPS, OPS, Literal, SIG, SIG],
                result: true,
            },
            TestVector {
                s: &[OPS, OPS, Literal, SIG],
                result: false,
            },
            TestVector {
                s: &[OPS, OPS, SEIPv1, OPS, SEIPv1, Literal, MDC, Pop,
                     SIG, MDC, Pop, SIG, SIG],
                result: true,
            },

            TestVector {
                s: &[CompressedData, OpaqueContent],
                result: false,
            },
            TestVector {
                s: &[CompressedData, OpaqueContent, Pop],
                result: true,
            },
            TestVector {
                s: &[CompressedData, CompressedData, OpaqueContent, Pop, Pop],
                result: true,
            },
            TestVector {
                s: &[SEIPv1, CompressedData, OpaqueContent, Pop, MDC, Pop],
                result: true,
            },
            TestVector {
                s: &[SEIPv1, OpaqueContent, Pop],
                result: true,
            },
        ];

        for v in &test_vectors {
            if v.result {
                let mut l = MessageValidator::new();
                for token in v.s.iter() {
                    l.push_raw(*token);
                    assert_match!(MessageValidity::MessagePrefix = l.check());
                }

                l.finish();
                assert_match!(MessageValidity::Message = l.check());
            }

            match MessageParser::new().parse(Lexer::from_tokens(v.s)) {
                Ok(r) => assert!(v.result, "Parsing: {:?} => {:?}", v.s, r),
                Err(e) => assert!(! v.result, "Parsing: {:?} => {:?}", v.s, e),
            }
        }
    }

    #[test]
    fn tags() {
        use crate::packet::Tag::*;

        struct TestVector<'a> {
            s: &'a [(Tag, Option<u8>, isize)],
            result: bool,
        }

        let test_vectors = [
            TestVector {
                s: &[(Literal, None, 0)][..],
                result: true,
            },
            TestVector {
                s: &[(CompressedData, None, 0), (Literal, None, 1)],
                result: true,
            },
            TestVector {
                s: &[(CompressedData, None, 0), (CompressedData, None, 1),
                     (Literal, None, 2)],
                result: true,
            },
            TestVector {
                s: &[(SEIP, Some(1), 0), (Literal, None, 1), (MDC, None, 1)],
                result: true,
            },
            TestVector {
                s: &[(CompressedData, None, 0), (SEIP, Some(1), 1),
                     (Literal, None, 2), (MDC, None, 2)],
                result: true,
            },
            TestVector {
                s: &[(CompressedData, None, 0), (SEIP, Some(1), 1),
                     (CompressedData, None, 2), (Literal, None, 3),
                     (MDC, None, 2)],
                result: true,
            },
            TestVector {
                s: &[(CompressedData, None, 0), (SEIP, Some(1), 1),
                     (CompressedData, None, 2), (Literal, None, 3),
                     (MDC, None, 3)],
                result: false,
            },
            TestVector {
                s: &[(SEIP, Some(1), 0), (MDC, None, 0)],
                result: false,
            },
            TestVector {
                s: &[(SKESK, None, 0), (SEIP, Some(1), 0), (Literal, None, 1),
                     (MDC, None, 1)],
                result: true,
            },
            TestVector {
                s: &[(PKESK, None, 0), (SEIP, Some(1), 0), (Literal, None, 1),
                     (MDC, None, 1)],
                result: true,
            },
            TestVector {
                s: &[(PKESK, None, 0), (SEIP, Some(1), 0),
                     (CompressedData, None, 1), (Literal, None, 2),
                     (MDC, None, 1)],
                result: true,
            },
            TestVector {
                s: &[(SKESK, None, 0), (SKESK, None, 0), (SEIP, Some(1), 0),
                     (Literal, None, 1), (MDC, None, 1)],
                result: true,
            },

            TestVector {
                s: &[(OnePassSig, None, 0), (Literal, None, 0),
                     (Signature, None, 0)],
                result: true,
            },
            TestVector {
                s: &[(OnePassSig, None, 0), (CompressedData, None, 0),
                     (Literal, None, 1),
                     (Signature, None, 0)],
                result: true,
            },
            TestVector {
                s: &[(OnePassSig, None, 0), (OnePassSig, None, 0),
                     (Literal, None, 0),
                     (Signature, None, 0), (Signature, None, 0)],
                result: true,
            },
            TestVector {
                s: &[(OnePassSig, None, 0), (OnePassSig, None, 0),
                     (Literal, None, 0),
                     (Signature, None, 0)],
                result: false,
            },
            TestVector {
                s: &[(OnePassSig, None, 0), (OnePassSig, None, 0),
                     (SEIP, Some(1), 0),
                     (OnePassSig, None, 1), (SEIP, Some(1), 1),
                     (Literal, None, 2), (MDC, None, 2),
                     (Signature, None, 1), (MDC, None, 1), (Signature, None, 0),
                     (Signature, None, 0)],
                result: true,
            },

            // "[A Marker packet] MUST be ignored when received.  It
            // may be placed at the beginning of a message that uses
            // features not available in PGP 2.6.x in order to cause
            // that version to report that newer software is necessary
            // to process the message.", section 5.8 of RFC4880.
            TestVector {
                s: &[(Marker, None, 0),
                     (OnePassSig, None, 0), (Literal, None, 0),
                     (Signature, None, 0)],
                result: true,
            },
            TestVector {
                s: &[(OnePassSig, None, 0), (Literal, None, 0),
                     (Signature, None, 0),
                     (Marker, None, 0)],
                result: true,
            },
        ];

        for v in &test_vectors {
            let mut l = MessageValidator::new();
            for (token, version, depth) in v.s.iter() {
                l.push(*token,
                       *version,
                       &(0..1 + *depth)
                           .map(|x| x as usize)
                           .collect::<Vec<_>>()[..]);
                if v.result {
                    assert_match!(MessageValidity::MessagePrefix = l.check());
                }
            }

            l.finish();

            if v.result {
                assert_match!(MessageValidity::Message = l.check());
            } else {
                assert_match!(MessageValidity::Error(_) = l.check());
            }
        }
    }

    #[test]
    fn basic() {
        // Empty.
        // => bad.
        let message = Message::try_from(vec![]);
        assert!(message.is_err(), "{:?}", message);

        // 0: Literal
        // => good.
        let mut packets = Vec::new();
        let mut lit = Literal::new(Text);
        lit.set_body(b"data".to_vec());
        packets.push(lit.into());

        let message = Message::try_from(packets);
        assert!(message.is_ok(), "{:?}", message);
    }

    #[allow(clippy::vec_init_then_push)]
    #[test]
    fn compressed_part() {
        let mut lit = Literal::new(Text);
        lit.set_body(b"data".to_vec());

        // 0: CompressedData
        //  0: Literal
        // => good.
        let mut packets = Vec::new();
        packets.push(
            CompressedData::new(CompressionAlgorithm::Uncompressed)
                .push(lit.clone().into())
                .into());

        let message = Message::try_from(packets);
        assert!(message.is_ok(), "{:?}", message);

        // 0: CompressedData
        //  0: Literal
        //  1: Literal
        // => bad.
        let mut packets = Vec::new();
        packets.push(
            CompressedData::new(CompressionAlgorithm::Uncompressed)
                .push(lit.clone().into())
                .push(lit.clone().into())
                .into());

        let message = Message::try_from(packets);
        assert!(message.is_err(), "{:?}", message);

        // 0: CompressedData
        //  0: Literal
        // 1: Literal
        // => bad.
        let mut packets = Vec::new();
        packets.push(
            CompressedData::new(CompressionAlgorithm::Uncompressed)
                .push(lit.clone().into())
                .into());
        packets.push(lit.clone().into());

        let message = Message::try_from(packets);
        assert!(message.is_err(), "{:?}", message);

        // 0: CompressedData
        //  0: CompressedData
        //   0: Literal
        // => good.
        let mut packets = Vec::new();
        packets.push(
            CompressedData::new(CompressionAlgorithm::Uncompressed)
                .push(CompressedData::new(CompressionAlgorithm::Uncompressed)
                      .push(lit.clone()
                            .into())
                      .into())
                .into());

        let message = Message::try_from(packets);
        assert!(message.is_ok(), "{:?}", message);
    }

    #[allow(clippy::vec_init_then_push)]
    #[test]
    fn one_pass_sig_part() {
        let mut lit = Literal::new(Text);
        lit.set_body(b"data".to_vec());

        let hash = crate::types::HashAlgorithm::SHA512;
        let key: key::SecretKey =
            crate::packet::key::Key4::generate_ecc(true, crate::types::Curve::Ed25519)
            .unwrap().into();
        let mut pair = key.clone().into_keypair().unwrap();
        let sig = crate::packet::signature::SignatureBuilder::new(SignatureType::Binary)
            .sign_hash(&mut pair, hash.context().unwrap()).unwrap();

        // 0: OnePassSig
        // => bad.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(OnePassSig3::new(SignatureType::Binary).into());

        let message = Message::try_from(packets);
        assert!(message.is_err(), "{:?}", message);

        // 0: OnePassSig
        // 1: Literal
        // => bad.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(OnePassSig3::new(SignatureType::Binary).into());
        packets.push(lit.clone().into());

        let message = Message::try_from(packets);
        assert!(message.is_err(), "{:?}", message);

        // 0: OnePassSig
        // 1: Literal
        // 2: Signature
        // => good.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(OnePassSig3::new(SignatureType::Binary).into());
        packets.push(lit.clone().into());
        packets.push(sig.clone().into());

        let message = Message::try_from(packets);
        assert!(message.is_ok(), "{:?}", message);

        // 0: OnePassSig
        // 1: Literal
        // 2: Signature
        // 3: Signature
        // => bad.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(OnePassSig3::new(SignatureType::Binary).into());
        packets.push(lit.clone().into());
        packets.push(sig.clone().into());
        packets.push(sig.clone().into());

        let message = Message::try_from(packets);
        assert!(message.is_err(), "{:?}", message);

        // 0: OnePassSig
        // 1: OnePassSig
        // 2: Literal
        // 3: Signature
        // 4: Signature
        // => good.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(OnePassSig3::new(SignatureType::Binary).into());
        packets.push(OnePassSig3::new(SignatureType::Binary).into());
        packets.push(lit.clone().into());
        packets.push(sig.clone().into());
        packets.push(sig.clone().into());

        let message = Message::try_from(packets);
        assert!(message.is_ok(), "{:?}", message);

        // 0: OnePassSig
        // 1: OnePassSig
        // 2: Literal
        // 3: Literal
        // 4: Signature
        // 5: Signature
        // => bad.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(OnePassSig3::new(SignatureType::Binary).into());
        packets.push(OnePassSig3::new(SignatureType::Binary).into());
        packets.push(lit.clone().into());
        packets.push(lit.clone().into());
        packets.push(sig.clone().into());
        packets.push(sig.clone().into());

        let message = Message::try_from(packets);
        assert!(message.is_err(), "{:?}", message);

        // 0: OnePassSig
        // 1: OnePassSig
        // 2: CompressedData
        //  0: Literal
        // 3: Signature
        // 4: Signature
        // => good.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(OnePassSig3::new(SignatureType::Binary).into());
        packets.push(OnePassSig3::new(SignatureType::Binary).into());
        packets.push(
            CompressedData::new(CompressionAlgorithm::Uncompressed)
                .push(lit.clone().into())
                .into());
        packets.push(sig.clone().into());
        packets.push(sig.clone().into());

        let message = Message::try_from(packets);
        assert!(message.is_ok(), "{:?}", message);
    }

    #[allow(clippy::vec_init_then_push)]
    #[test]
    fn signature_part() {
        let mut lit = Literal::new(Text);
        lit.set_body(b"data".to_vec());

        let hash = crate::types::HashAlgorithm::SHA512;
        let key: key::SecretKey =
            crate::packet::key::Key4::generate_ecc(true, crate::types::Curve::Ed25519)
            .unwrap().into();
        let mut pair = key.clone().into_keypair().unwrap();
        let sig = crate::packet::signature::SignatureBuilder::new(SignatureType::Binary)
            .sign_hash(&mut pair, hash.context().unwrap()).unwrap();

        // 0: Signature
        // => bad.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(sig.clone().into());

        let message = Message::try_from(packets);
        assert!(message.is_err(), "{:?}", message);

        // 0: Signature
        // 1: Literal
        // => good.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(sig.clone().into());
        packets.push(lit.clone().into());

        let message = Message::try_from(packets);
        assert!(message.is_ok(), "{:?}", message);

        // 0: Signature
        // 1: Signature
        // 2: Literal
        // => good.
        let mut packets : Vec<Packet> = Vec::new();
        packets.push(sig.clone().into());
        packets.push(sig.clone().into());
        packets.push(lit.clone().into());

        let message = Message::try_from(packets);
        assert!(message.is_ok(), "{:?}", message);
    }

    #[test]
    fn encrypted_part() {
        // There are no simple constructors for SEIP packets: they are
        // interleaved with SK-ESK and PK-ESK packets.  And, the
        // session key needs to be managed.  Instead, we use some
        // internal interfaces to progressively build up more
        // complicated messages.

        let mut lit = Literal::new(Text);
        lit.set_body(b"data".to_vec());

        // 0: SK-ESK
        // => bad.
        let mut packets : Vec<Packet> = Vec::new();
        let cipher = SymmetricAlgorithm::AES256;
        let sk = crate::crypto::SessionKey::new(cipher.key_size().unwrap());
        #[allow(deprecated)]
        packets.push(SKESK4::with_password(
            cipher,
            cipher,
            S2K::Simple { hash: HashAlgorithm::SHA256 },
            &sk,
            &"12345678".into()).unwrap().into());
        let message = Message::try_from(packets.clone());
        assert!(message.is_err(), "{:?}", message);

        // 0: SK-ESK
        // 1: Literal
        // => bad.
        packets.push(lit.clone().into());

        assert!(packets.iter().map(|p| p.tag()).collect::<Vec<Tag>>()
                == [ Tag::SKESK, Tag::Literal ]);

        let message = Message::try_from(packets.clone());
        assert!(message.is_err(), "{:?}", message);

        // 0: SK-ESK
        // 1: SEIP
        //  0: Literal
        //  1: MDC
        // => good.
        let mut seip = SEIP1::new();
        seip.children_mut().unwrap().push(
            lit.clone().into());
        seip.children_mut().unwrap().push(
            MDC::from([0u8; 20]).into());
        packets[1] = seip.into();

        assert!(packets.iter().map(|p| p.tag()).collect::<Vec<Tag>>()
                == [ Tag::SKESK, Tag::SEIP ]);

        let message = Message::try_from(packets.clone());
        assert!(message.is_ok(), "{:#?}", message);

        // 0: SK-ESK
        // 1: SEIP
        //  0: Literal
        //  1: MDC
        // 2: SK-ESK
        // => bad.
        let skesk = packets[0].clone();
        packets.push(skesk);

        assert!(packets.iter().map(|p| p.tag()).collect::<Vec<Tag>>()
                == [ Tag::SKESK, Tag::SEIP, Tag::SKESK ]);

        let message = Message::try_from(packets.clone());
        assert!(message.is_err(), "{:#?}", message);

        // 0: SK-ESK
        // 1: SK-ESK
        // 2: SEIP
        //  0: Literal
        //  1: MDC
        // => good.
        packets.swap(1, 2);

        assert!(packets.iter().map(|p| p.tag()).collect::<Vec<Tag>>()
                == [ Tag::SKESK, Tag::SKESK, Tag::SEIP ]);

        let message = Message::try_from(packets.clone());
        assert!(message.is_ok(), "{:#?}", message);

        // 0: SK-ESK
        // 1: SK-ESK
        // 2: SEIP
        //  0: Literal
        //  1: MDC
        // 3: SEIP
        //  0: Literal
        //  1: MDC
        // => bad.
        let seip = packets[2].clone();
        packets.push(seip);

        assert!(packets.iter().map(|p| p.tag()).collect::<Vec<Tag>>()
                == [ Tag::SKESK, Tag::SKESK, Tag::SEIP, Tag::SEIP ]);

        let message = Message::try_from(packets.clone());
        assert!(message.is_err(), "{:#?}", message);

        // 0: SK-ESK
        // 1: SK-ESK
        // 2: SEIP
        //  0: Literal
        //  1: MDC
        // 3: Literal
        // => bad.
        packets[3] = lit.clone().into();

        assert!(packets.iter().map(|p| p.tag()).collect::<Vec<Tag>>()
                == [ Tag::SKESK, Tag::SKESK, Tag::SEIP, Tag::Literal ]);

        let message = Message::try_from(packets.clone());
        assert!(message.is_err(), "{:#?}", message);

        // 0: SK-ESK
        // 1: SK-ESK
        // 2: SEIP
        //  0: Literal
        //  1: MDC
        //  2: Literal
        // => bad.
        packets.remove(3);
        packets[2].container_mut().unwrap()
            .children_mut().unwrap().push(lit.clone().into());

        assert!(packets.iter().map(|p| p.tag()).collect::<Vec<Tag>>()
                == [ Tag::SKESK, Tag::SKESK, Tag::SEIP ]);

        let message = Message::try_from(packets.clone());
        assert!(message.is_err(), "{:#?}", message);

        // 0: SK-ESK
        // 2: PK-ESK
        // 1: SK-ESK
        // 2: SEIP
        //  0: Literal
        // => good.
        packets[2].container_mut().unwrap()
            .children_mut().unwrap().pop().unwrap();

        #[allow(deprecated)]
        packets.insert(
            1,
            PKESK3::new(
                "0000111122223333".parse().unwrap(),
                PublicKeyAlgorithm::RSAEncrypt,
                Ciphertext::RSA { c: MPI::new(&[]) }).unwrap().into());

        assert!(packets.iter().map(|p| p.tag()).collect::<Vec<Tag>>()
                == [ Tag::SKESK, Tag::PKESK, Tag::SKESK, Tag::SEIP ]);

        let message = Message::try_from(packets.clone());
        assert!(message.is_ok(), "{:#?}", message);
    }

    #[test]
    fn basic_message_validator() {
        use crate::message::{MessageValidator, MessageValidity, Token};

        let mut l = MessageValidator::new();
        l.push_token(Token::Literal, &[0]);
        l.finish();
        assert!(matches!(l.check(), MessageValidity::Message));
    }

    #[test]
    fn message_validator_push_token() {
        use crate::message::{MessageValidator, MessageValidity, Token};

        let mut l = MessageValidator::new();
        l.push_token(Token::Literal, &[0]);
        l.finish();

        assert!(matches!(l.check(), MessageValidity::Message));
    }

    #[test]
    fn message_validator_push() {
        use crate::message::{MessageValidator, MessageValidity};
        use crate::packet::Tag;

        let mut l = MessageValidator::new();
        l.push(Tag::Literal, None, &[0]);
        l.finish();

        assert!(matches!(l.check(), MessageValidity::Message));
    }

    #[test]
    fn message_validator_finish() {
        use crate::message::{MessageValidator, MessageValidity};

        let mut l = MessageValidator::new();
        l.finish();

        assert!(matches!(l.check(), MessageValidity::Error(_)));
    }

    #[test]
    fn message_validator_check() {
        use crate::message::{MessageValidator, MessageValidity};
        use crate::packet::Tag;

        // No packets will return an error.
        let mut l = MessageValidator::new();
        assert!(matches!(l.check(), MessageValidity::MessagePrefix));
        l.finish();

        assert!(matches!(l.check(), MessageValidity::Error(_)));

        // Simple one-literal message.
        let mut l = MessageValidator::new();
        l.push(Tag::Literal, None, &[0]);
        assert!(matches!(l.check(), MessageValidity::MessagePrefix));
        l.finish();

        assert!(matches!(l.check(), MessageValidity::Message));
    }
}
