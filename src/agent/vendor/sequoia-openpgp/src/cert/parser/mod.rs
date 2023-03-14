use std::io;
use std::mem;
use std::vec;
use std::path::Path;

use lalrpop_util::ParseError;

use crate::{
    Error,
    KeyHandle,
    packet::Tag,
    Packet,
    parse::{
        Parse,
        PacketParserResult,
        PacketParser
    },
    Result,
    cert::bundle::ComponentBundle,
    Cert,
};

mod low_level;
use low_level::{
    Lexer,
    CertParser as CertLowLevelParser,
    CertParserError,
    Token,
    parse_error_downcast,
};

const TRACE : bool = false;

/// Whether a packet sequence is a valid keyring.
///
/// This is used
#[derive(Debug)]
pub(crate) enum KeyringValidity {
    /// The packet sequence is a valid keyring.
    Keyring,
    /// The packet sequence is a valid keyring prefix.
    KeyringPrefix,
    /// The packet sequence is definitely not a keyring.
    Error(anyhow::Error),
}

#[allow(unused)]
impl KeyringValidity {
    /// Returns whether the packet sequence is a valid keyring.
    ///
    /// Note: a `KeyringValidator` will only return this after
    /// `KeyringValidator::finish` has been called.
    pub fn is_keyring(&self) -> bool {
        matches!(self, KeyringValidity::Keyring)
    }

    /// Returns whether the packet sequence is a valid Keyring prefix.
    ///
    /// Note: a `KeyringValidator` will only return this before
    /// `KeyringValidator::finish` has been called.
    pub fn is_keyring_prefix(&self) -> bool {
        matches!(self, KeyringValidity::KeyringPrefix)
    }

    /// Returns whether the packet sequence is definitely not a valid
    /// keyring.
    pub fn is_err(&self) -> bool {
        matches!(self, KeyringValidity::Error(_))
    }
}

/// Used to help validate that a packet sequence is a valid keyring.
#[derive(Debug)]
pub(crate) struct KeyringValidator {
    tokens: Vec<Token>,
    n_keys: usize,
    n_packets: usize,
    finished: bool,

    // If we know that the packet sequence is invalid.
    error: Option<CertParserError>,
}

impl Default for KeyringValidator {
    fn default() -> Self {
        KeyringValidator::new()
    }
}

#[allow(unused)]
impl KeyringValidator {
    /// Instantiates a new `KeyringValidator`.
    pub fn new() -> Self {
        KeyringValidator {
            tokens: vec![],
            n_keys: 0,
            n_packets: 0,
            finished: false,
            error: None,
        }
    }

    /// Returns whether the packet sequence is a valid keyring.
    ///
    /// Note: a `KeyringValidator` will only return this after
    /// `KeyringValidator::finish` has been called.
    pub fn is_keyring(&self) -> bool {
        self.check().is_keyring()
    }

    /// Returns whether the packet sequence forms a valid keyring
    /// prefix.
    ///
    /// Note: a `KeyringValidator` will only return this before
    /// `KeyringValidator::finish` has been called.
    pub fn is_keyring_prefix(&self) -> bool {
        self.check().is_keyring_prefix()
    }

    /// Returns whether the packet sequence is definitely not a valid
    /// keyring.
    pub fn is_err(&self) -> bool {
        self.check().is_err()
    }

    /// Add the token `token` to the token stream.
    pub fn push_token(&mut self, token: Token) {
        assert!(!self.finished);

        if self.error.is_some() {
            return;
        }

        if let Token::PublicKey(_) | Token::SecretKey(_) = token {
            self.tokens.clear();
            self.n_keys += 1;
        }

        self.n_packets += 1;
        match (&token, self.tokens.last()) {
            (Token::Signature(None), Some(Token::Signature(None))) => {
                // Compress multiple signatures in a row.  This is
                // essential for dealing with flooded keys
            },
            _ => self.tokens.push(token),
        }
    }

    /// Add a packet of type `tag` to the token stream.
    pub fn push(&mut self, tag: Tag) {
        let token = match tag {
            Tag::PublicKey => Token::PublicKey(None),
            Tag::SecretKey => Token::SecretKey(None),
            Tag::PublicSubkey => Token::PublicSubkey(None),
            Tag::SecretSubkey => Token::SecretSubkey(None),
            Tag::UserID => Token::UserID(None),
            Tag::UserAttribute => Token::UserAttribute(None),
            Tag::Signature => Token::Signature(None),
            Tag::Trust => Token::Trust(None),
            Tag::Marker => {
                // Ignore Marker Packet.  RFC4880, section 5.8:
                //
                //   Such a packet MUST be ignored when received.
                return;
            },
            _ => {
                // Unknown token.
                self.error = Some(CertParserError::OpenPGP(
                    Error::MalformedMessage(
                        format!("Invalid Cert: {:?} packet (at {}) not expected",
                                tag, self.n_packets))));
                self.tokens.clear();
                return;
            }
        };

        self.push_token(token)
    }

    /// Notes that the entire message has been seen.
    ///
    /// This function may only be called once.
    ///
    /// Once called, this function will no longer return
    /// `KeyringValidity::KeyringPrefix`.
    pub fn finish(&mut self) {
        assert!(!self.finished);
        self.finished = true;
    }

    /// Returns whether the token stream corresponds to a valid
    /// keyring.
    ///
    /// This returns a tri-state: if the packet sequence is a valid
    /// Keyring, it returns `KeyringValidity::Keyring`, if the packet
    /// sequence is invalid, then it returns `KeyringValidity::Error`.
    /// If the packet sequence that has been processed so far is a
    /// valid prefix, then it returns
    /// `KeyringValidity::KeyringPrefix`.
    ///
    /// Note: if `KeyringValidator::finish()` *hasn't* been called,
    /// then this function will only ever return either
    /// `KeyringValidity::KeyringPrefix` or `KeyringValidity::Error`.
    /// Once `KeyringValidity::finish()` has been called, then it will
    /// only return either `KeyringValidity::Keyring` or
    /// `KeyringValidity::Error`.
    pub fn check(&self) -> KeyringValidity {
        if let Some(ref err) = self.error {
            return KeyringValidity::Error((*err).clone().into());
        }

        let r = CertLowLevelParser::new().parse(
            Lexer::from_tokens(&self.tokens));

        if self.finished {
            match r {
                Ok(_) => KeyringValidity::Keyring,
                Err(err) =>
                    KeyringValidity::Error(
                        CertParserError::Parser(parse_error_downcast(err)).into()),
            }
        } else {
            match r {
                Ok(_) => KeyringValidity::KeyringPrefix,
                Err(ParseError::UnrecognizedEOF { .. }) =>
                    KeyringValidity::KeyringPrefix,
                Err(err) =>
                    KeyringValidity::Error(
                        CertParserError::Parser(parse_error_downcast(err)).into()),
            }
        }
    }
}

/// Whether a packet sequence is a valid Cert.
#[derive(Debug)]
#[allow(unused)]
pub(crate) enum CertValidity {
    /// The packet sequence is a valid Cert.
    Cert,
    /// The packet sequence is a valid Cert prefix.
    CertPrefix,
    /// The packet sequence is definitely not a Cert.
    Error(anyhow::Error),
}

#[allow(unused)]
impl CertValidity {
    /// Returns whether the packet sequence is a valid Cert.
    ///
    /// Note: a `CertValidator` will only return this after
    /// `CertValidator::finish` has been called.
    pub fn is_cert(&self) -> bool {
        matches!(self, CertValidity::Cert)
    }

    /// Returns whether the packet sequence is a valid Cert prefix.
    ///
    /// Note: a `CertValidator` will only return this before
    /// `CertValidator::finish` has been called.
    pub fn is_cert_prefix(&self) -> bool {
        matches!(self, CertValidity::CertPrefix)
    }

    /// Returns whether the packet sequence is definitely not a valid
    /// Cert.
    pub fn is_err(&self) -> bool {
        matches!(self, CertValidity::Error(_))
    }
}

/// Used to help validate that a packet sequence is a valid Cert.
#[derive(Debug)]
pub(crate) struct CertValidator(KeyringValidator);

impl Default for CertValidator {
    fn default() -> Self {
        CertValidator::new()
    }
}

impl CertValidator {
    /// Instantiates a new `CertValidator`.
    pub fn new() -> Self {
        CertValidator(Default::default())
    }

    /// Add the token `token` to the token stream.
    #[cfg(test)]
    pub fn push_token(&mut self, token: Token) {
        self.0.push_token(token)
    }

    /// Add a packet of type `tag` to the token stream.
    pub fn push(&mut self, tag: Tag) {
        self.0.push(tag)
    }

    /// Note that the entire message has been seen.
    ///
    /// This function may only be called once.
    ///
    /// Once called, this function will no longer return
    /// `CertValidity::CertPrefix`.
    pub fn finish(&mut self) {
        self.0.finish()
    }

    /// Returns whether the token stream corresponds to a valid
    /// Cert.
    ///
    /// This returns a tri-state: if the packet sequence is a valid
    /// Cert, it returns `CertValidity::Cert`, if the packet sequence
    /// is invalid, then it returns `CertValidity::Error`.  If the
    /// packet sequence that has been processed so far is a valid
    /// prefix, then it returns `CertValidity::CertPrefix`.
    ///
    /// Note: if `CertValidator::finish()` *hasn't* been called, then
    /// this function will only ever return either
    /// `CertValidity::CertPrefix` or `CertValidity::Error`.  Once
    /// `CertValidity::finish()` has been called, then it will only
    /// return either `CertValidity::Cert` or `CertValidity::Error`.
    pub fn check(&self) -> CertValidity {
        if self.0.n_keys > 1 {
            return CertValidity::Error(Error::MalformedMessage(
                    "More than one key found, this is a keyring".into()).into());
        }

        match self.0.check() {
            KeyringValidity::Keyring => CertValidity::Cert,
            KeyringValidity::KeyringPrefix => CertValidity::CertPrefix,
            KeyringValidity::Error(e) => CertValidity::Error(e),
        }
    }
}

/// An iterator over a sequence of certificates, i.e., an OpenPGP keyring.
///
/// The source of packets is a fallible iterator over [`Packet`]s.  In
/// this way, it is possible to propagate parse errors.
///
/// A `CertParser` returns each [`TPK`] or [`TSK`] that it encounters.
/// Its behavior can be modeled using a simple state machine.
///
/// In the first and initial state, it looks for the start of a
/// certificate, a [`Public Key`] packet or a [`Secret Key`] packet.
/// When it encounters such a packet it buffers it, and transitions to
/// the second state.  Any other packet or an error causes it to emit
/// an error and stay in the same state.  When the source of packets
/// is exhausted, it enters the `End` state.
///
/// In the second state, it looks for packets that belong to a
/// certificate's body.  If it encounters a valid body packet, then it
/// buffers it and stays in the same state.  If it encounters the
/// start of a certificate, then it emits the buffered certificate,
/// buffers the packet, and stays in the same state.  If it encounters
/// an invalid packet (e.g., a [`Literal Data`] packet), it emits two
/// items, the buffered certificate, and an error, and then it
/// transitions back to the initial state.  When the source of packets
/// is exhausted, it emits the buffered certificate and enters the end
/// state.
///
/// In the end state, it emits `None`.
///
/// ```text
///                       Invalid Packet / Error
///                     ,------------------------.
///                     v                        |
///    Not a      +---------+                +---------+
///    Start  .-> | Looking | -------------> | Looking | <-. Cert
///  of Cert  |   |   for   |     Start      |   for   |   | Body
///   Packet  |   |  Start  |    of Cert     |  Cert   |   | Packet
///  / Error  `-- | of Cert |     Packet     |  Body   | --'
///               +---------+            .-> +---------+
///                    |                 |      |  |
///                    |                 `------'  |
///                    |    Start of Cert Packet   |
///                    |                           |
///                EOF |         +-----+           | EOF
///                     `------> | End | <---------'
///                              +-----+
///                               |  ^
///                               `--'
/// ```
///
/// The parser does not recurse into containers, thus when it
/// encounters a container like a [`Compressed Data`] Packet, it will
/// return an error even if the container contains a valid
/// certificate.
///
/// The parser considers unknown packets to be valid body packets.
/// (In a [`Cert`], these show up as [`Unknown`] components.)  The
/// goal is to provide some future compatibility.
///
/// [`Packet`]: crate::packet::Packet
/// [`TPK`]: https://tools.ietf.org/html/rfc4880#section-11.1
/// [`TSK`]: https://tools.ietf.org/html/rfc4880#section-11.2
/// [`Public Key`]: super::Packet::PublicKey
/// [`Secret Key`]: super::Packet::SecretKey
/// [`Literal Data`]: super::Packet::Literal
/// [`Compressed Data`]: super::Packet::CompressedData
/// [`Cert`]: super::Cert
/// [`Unknown`]: super::Packet::Unknown
///
/// # Examples
///
/// Print information about all certificates in a keyring:
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// use openpgp::parse::Parse;
/// use openpgp::parse::PacketParser;
/// # use openpgp::serialize::Serialize;
/// use openpgp::cert::prelude::*;
///
/// # fn main() -> Result<()> {
/// # let (alice, _) =
/// #       CertBuilder::general_purpose(None, Some("alice@example.org"))
/// #       .generate()?;
/// # let (bob, _) =
/// #       CertBuilder::general_purpose(None, Some("bob@example.org"))
/// #       .generate()?;
/// #
/// # let mut keyring = Vec::new();
/// # alice.serialize(&mut keyring)?;
/// # bob.serialize(&mut keyring)?;
/// #
/// # let mut count = 0;
/// let ppr = PacketParser::from_bytes(&keyring)?;
/// for certo in CertParser::from(ppr) {
///     match certo {
///         Ok(cert) => {
///             println!("Key: {}", cert.fingerprint());
///             for ua in cert.userids() {
///                 println!("  User ID: {}", ua.userid());
///             }
/// #           count += 1;
///         }
///         Err(err) => {
///             eprintln!("Error reading keyring: {}", err);
/// #           unreachable!();
///         }
///     }
/// }
/// # assert_eq!(count, 2);
/// #     Ok(())
/// # }
/// ```
///
/// When an invalid packet is encountered, an error is returned and
/// parsing continues:
///
/// ```rust
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// # use openpgp::serialize::Serialize;
/// use openpgp::cert::prelude::*;
/// use openpgp::packet::prelude::*;
/// use openpgp::types::DataFormat;
///
/// # fn main() -> Result<()> {
/// let mut lit = Literal::new(DataFormat::Text);
/// lit.set_body(b"test".to_vec());
///
/// let (alice, _) =
///       CertBuilder::general_purpose(None, Some("alice@example.org"))
///       .generate()?;
/// let (bob, _) =
///       CertBuilder::general_purpose(None, Some("bob@example.org"))
///       .generate()?;
///
/// let mut packets : Vec<Packet> = Vec::new();
/// packets.extend(alice.clone());
/// packets.push(lit.clone().into());
/// packets.push(lit.clone().into());
/// packets.extend(bob.clone());
///
/// let r : Vec<Result<Cert>> = CertParser::from(packets).collect();
/// assert_eq!(r.len(), 4);
/// assert_eq!(r[0].as_ref().unwrap().fingerprint(), alice.fingerprint());
/// assert!(r[1].is_err());
/// assert!(r[2].is_err());
/// assert_eq!(r[3].as_ref().unwrap().fingerprint(), bob.fingerprint());
/// #     Ok(())
/// # }
/// ```

#[derive(Default)]
pub struct CertParser<'a> {
    source: Option<Box<dyn Iterator<Item=Result<Packet>> + 'a + Send + Sync>>,
    packets: Vec<Packet>,
    queued_error: Option<anyhow::Error>,
    saw_error: bool,
    filter: Vec<Box<dyn Send + Sync + Fn(&Cert, bool) -> bool + 'a>>,
}
assert_send_and_sync!(CertParser<'_>);

// When using a `PacketParser`, we never use the `Iter` variant.
// Nevertheless, we need to provide a concrete type.
// vec::IntoIter<Packet> is about as good as any other.
impl<'a> From<PacketParserResult<'a>> for CertParser<'a>
{
    /// Initializes a `CertParser` from a `PacketParser`.
    fn from(ppr: PacketParserResult<'a>) -> Self {
        use std::io::ErrorKind::UnexpectedEof;
        let mut parser : Self = Default::default();
        if let PacketParserResult::Some(pp) = ppr {
            let mut ppp : Box<Option<PacketParser>> = Box::new(Some(pp));
            let mut retry_with_reader = Box::new(None);
            parser.source = Some(
                Box::new(std::iter::from_fn(move || {
                    if let Some(reader) = retry_with_reader.take() {
                        // Try to find the next (armored) blob.
                        match PacketParser::from_buffered_reader(reader) {
                            Ok(PacketParserResult::Some(pp)) => {
                                // We read at least one packet.  Try
                                // to parse the next cert.
                                *ppp = Some(pp);
                            },
                            Ok(PacketParserResult::EOF(_)) =>
                                (), // No dice.
                            Err(err) => {
                                // See if we just reached the end.
                                if let Some(e) = err.downcast_ref::<io::Error>()
                                {
                                    if e.kind() == UnexpectedEof {
                                        return None;
                                    }
                                }
                                return Some(Err(err));
                            },
                        }
                    }

                    if let Some(mut pp) = ppp.take() {
                        if let Packet::Unknown(_) = pp.packet {
                            // Buffer unknown packets.  This may be a
                            // signature that we don't understand, and
                            // keeping it intact is important.
                            if let Err(e) = pp.buffer_unread_content() {
                                return Some(Err(e));
                            }
                        }

                        match pp.next() {
                            Ok((packet, ppr)) => {
                                match ppr {
                                    PacketParserResult::Some(pp) =>
                                        *ppp = Some(pp),
                                    PacketParserResult::EOF(eof) =>
                                        *retry_with_reader =
                                            Some(eof.into_reader()),
                                }
                                Some(Ok(packet))
                            },
                            Err(err) => {
                                Some(Err(err))
                            }
                        }
                    } else {
                        None
                    }
                })));
        }
        parser
    }
}

impl<'a> From<Vec<Result<Packet>>> for CertParser<'a> {
    fn from(p: Vec<Result<Packet>>) -> CertParser<'a> {
        CertParser::from_iter(p)
    }
}

impl<'a> From<Vec<Packet>> for CertParser<'a> {
    fn from(p: Vec<Packet>) -> CertParser<'a> {
        CertParser::from_iter(p)
    }
}

impl<'a> Parse<'a, CertParser<'a>> for CertParser<'a>
{
    /// Initializes a `CertParser` from a `Read`er.
    fn from_reader<R: 'a + io::Read + Send + Sync>(reader: R) -> Result<Self> {
        Ok(Self::from(PacketParser::from_reader(reader)?))
    }

    /// Initializes a `CertParser` from a `File`.
    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Ok(Self::from(PacketParser::from_file(path)?))
    }

    /// Initializes a `CertParser` from a byte string.
    fn from_bytes<D: AsRef<[u8]> + ?Sized + Send + Sync>(data: &'a D) -> Result<Self> {
        Ok(Self::from(PacketParser::from_bytes(data)?))
    }
}

#[allow(clippy::should_implement_trait)]
impl<'a> CertParser<'a> {
    /// Creates a `CertParser` from a `Result<Packet>` iterator.
    ///
    /// Note: because we implement `From<Packet>` for
    /// `Result<Packet>`, it is possible to pass in an iterator over
    /// `Packet`s.
    ///
    /// # Examples
    ///
    /// From a `Vec<Packet>`:
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::PacketPile;
    /// # use openpgp::parse::Parse;
    /// # use openpgp::serialize::Serialize;
    /// use openpgp::cert::prelude::*;
    /// use openpgp::packet::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// # let (alice, _) =
    /// #       CertBuilder::general_purpose(None, Some("alice@example.org"))
    /// #       .generate()?;
    /// # let (bob, _) =
    /// #       CertBuilder::general_purpose(None, Some("bob@example.org"))
    /// #       .generate()?;
    /// #
    /// # let mut keyring = Vec::new();
    /// # alice.serialize(&mut keyring)?;
    /// # bob.serialize(&mut keyring)?;
    /// #
    /// # let mut count = 0;
    /// # let pp = PacketPile::from_bytes(&keyring)?;
    /// # let packets : Vec<Packet> = pp.into();
    /// for certo in CertParser::from_iter(packets) {
    ///     match certo {
    ///         Ok(cert) => {
    ///             println!("Key: {}", cert.fingerprint());
    ///             for ua in cert.userids() {
    ///                 println!("  User ID: {}", ua.userid());
    ///             }
    /// #           count += 1;
    ///         }
    ///         Err(err) => {
    ///             eprintln!("Error reading keyring: {}", err);
    /// #           unreachable!();
    ///         }
    ///     }
    /// }
    /// # assert_eq!(count, 2);
    /// #     Ok(())
    /// # }
    /// ```
    pub fn from_iter<I, J>(iter: I) -> Self
        where I: 'a + IntoIterator<Item=J>,
              J: 'a + Into<Result<Packet>>,
              <I as IntoIterator>::IntoIter: Send + Sync,
    {
        Self {
            source: Some(Box::new(iter.into_iter().map(Into::into))),
            ..Default::default()
        }
    }

    /// Filters the Certs prior to validation.
    ///
    /// By default, the `CertParser` only returns valdiated [`Cert`]s.
    /// Checking that a certificate's self-signatures are valid,
    /// however, is computationally expensive, and not always
    /// necessary.  For example, when looking for a small number of
    /// certificates in a large keyring, most certificates can be
    /// immediately discarded.  That is, it is more efficient to
    /// filter, validate, and double check, than to validate and
    /// filter.  (It is necessary to double check, because the check
    /// might have been on an invalid part.  For example, if searching
    /// for a key with a particular Key ID, a matching key might not
    /// have any self signatures.)
    ///
    /// If the `CertParser` gave out unvalidated `Cert`s, and provided
    /// an interface to validate them, then the caller could implement
    /// this check-validate-double-check pattern.  Giving out
    /// unvalidated `Cert`s, however, is dangerous: inevitably, a
    /// `Cert` will be used without having been validated in a context
    /// where it should have been.
    ///
    /// This function avoids this class of bugs while still providing
    /// a mechanism to filter `Cert`s prior to validation: the caller
    /// provides a callback that is invoked on the *unvalidated*
    /// `Cert`.  If the callback returns `true`, then the parser
    /// validates the `Cert`, and invokes the callback *a second time*
    /// to make sure the `Cert` is really wanted.  If the callback
    /// returns false, then the `Cert` is skipped.
    ///
    /// Note: calling this function multiple times on a single
    /// `CertParser` will not replace the existing filter, but install
    /// multiple filters.
    ///
    /// [`Cert`]: super::Cert
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// # use openpgp::Result;
    /// # use openpgp::parse::{Parse, PacketParser};
    /// use openpgp::cert::prelude::*;
    ///
    /// # fn main() -> Result<()> {
    /// #     let ppr = PacketParser::from_bytes(b"")?;
    /// #     let some_keyid = "C2B819056C652598".parse()?;
    /// for certr in CertParser::from(ppr)
    ///     .unvalidated_cert_filter(|cert, _| {
    ///         for component in cert.keys() {
    ///             if component.key().keyid() == some_keyid {
    ///                 return true;
    ///             }
    ///         }
    ///         false
    ///     })
    /// {
    ///     match certr {
    ///         Ok(cert) => {
    ///             // The Cert contains the subkey.
    ///         }
    ///         Err(err) => {
    ///             eprintln!("Error reading keyring: {}", err);
    ///         }
    ///     }
    /// }
    /// #     Ok(())
    /// # }
    /// ```
    pub fn unvalidated_cert_filter<F: 'a>(mut self, filter: F) -> Self
        where F: Send + Sync + Fn(&Cert, bool) -> bool
    {
        self.filter.push(Box::new(filter));
        self
    }

    // Parses the next packet in the packet stream.
    //
    // If we complete parsing a Cert, returns the Cert.  Otherwise,
    // returns None.
    fn parse(&mut self, p: Packet) -> Result<Option<Cert>> {
        tracer!(TRACE, "CertParser::parse", 0);
        if let Packet::Marker(_) = p {
            // Ignore Marker Packet.  RFC4880, section 5.8:
            //
            //   Such a packet MUST be ignored when received.
            return Ok(None);
        }

        if !self.packets.is_empty() {
            if self.packets.len() == 1 {
                if let Err(err) = Cert::valid_start(&self.packets[0]) {
                    t!("{}", err);
                    return self.cert(Some(p));
                }
            }

            if Cert::valid_start(&p).is_ok() {
                t!("Encountered the start of a new certificate ({}), \
                    finishing buffered certificate", p.tag());
                return self.cert(Some(p));
            } else if let Err(err) = Cert::valid_packet(&p) {
                t!("Encountered an invalid packet ({}), \
                    finishing buffered certificate: {}",
                   p.tag(), err);
                return self.cert(Some(p));
            }
        }

        self.packets.push(p);
        Ok(None)
    }

    // Resets the parser so that it starts parsing a new cert.
    //
    // Returns the old state.  Note: the packet iterator is preserved.
    fn reset(&mut self) -> Self {
        // We need to preserve `source` and `filter`.
        let mut orig = mem::take(self);
        self.source = orig.source.take();
        mem::swap(&mut self.filter, &mut orig.filter);
        orig
    }

    // Finalizes the current Cert and returns it.  Sets the parser up to
    // begin parsing the next Cert.
    //
    // `pk` is buffered for the next certificate.
    fn cert(&mut self, pk: Option<Packet>) -> Result<Option<Cert>> {
        tracer!(TRACE, "CertParser::cert", 0);
        let orig = self.reset();

        if let Some(pk) = pk {
            self.packets.push(pk);
        }

        let n_packets = orig.packets.len();
        t!("Finalizing certificate with {} packets", n_packets);

        // Convert to tokens, but preserve packets if it fails.
        use std::convert::TryInto;
        let mut failed = false;
        let mut packets: Vec<Packet> = Vec::with_capacity(0);
        let mut tokens: Vec<Token> = Vec::with_capacity(n_packets);
        for p in orig.packets {
            if failed {
                // Just stash the packet.
                packets.push(p);
            } else {
                match p.try_into() {
                    Ok(t) => tokens.push(t),
                    Err(p) => {
                        // Conversion failed.  Revert the whole process.
                        packets.reserve(n_packets);
                        for t in tokens.drain(..) {
                            packets.push({
                                let p: Option<Packet> = t.into();
                                p.expect("token created with packet")
                            });
                        }
                        packets.push(p);
                        failed = true;
                    },
                }
            }
        }

        if failed {
            // There was at least one packet that doesn't belong in a
            // Cert.  Fail now.
            let err = Error::UnsupportedCert2(
                "Packet sequence includes non-Cert packets.".into(),
                packets);
            t!("Invalid certificate: {}", err);
            return Err(err.into());
        }
        t!("{} tokens: {:?}", tokens.len(), tokens);

        let certo = match CertLowLevelParser::new()
            .parse(Lexer::from_tokens(&tokens))
        {
            Ok(certo) => certo,
            Err(err) => {
                let err = low_level::parse_error_to_openpgp_error(
                    low_level::parse_error_downcast(err));
                t!("Low level parser: {}", err);
                return Err(err.into());
            }
        }.and_then(|cert| {
            for filter in &self.filter {
                if !filter(&cert, true) {
                    t!("Rejected by filter");
                    return None;
                }
            }

            Some(cert)
        }).and_then(|mut cert| {
            let primary_fp: KeyHandle = cert.key_handle();
            let primary_keyid = KeyHandle::KeyID(primary_fp.clone().into());

            // The parser puts all of the signatures on the
            // certifications field.  Split them now.

            split_sigs(&primary_fp, &primary_keyid, &mut cert.primary);

            for b in cert.userids.iter_mut() {
                split_sigs(&primary_fp, &primary_keyid, b);
            }
            for b in cert.user_attributes.iter_mut() {
                split_sigs(&primary_fp, &primary_keyid, b);
            }
            for b in cert.subkeys.iter_mut() {
                split_sigs(&primary_fp, &primary_keyid, b);
            }

            let cert = cert.canonicalize();

            // Make sure it is still wanted.
            for filter in &self.filter {
                if !filter(&cert, true) {
                    t!("Rejected by filter");
                    return None;
                }
            }

            Some(cert)
        });

        t!("Returning {:?}, constructed from {} packets",
           certo.as_ref().map(|c| c.fingerprint()),
           n_packets);

        Ok(certo)
    }
}

/// Splits the signatures in b.certifications into the correct
/// vectors.
pub(crate) fn split_sigs<C>(primary: &KeyHandle, primary_keyid: &KeyHandle,
                            b: &mut ComponentBundle<C>)
{
    let mut self_signatures = Vec::with_capacity(0);
    let mut certifications = Vec::with_capacity(0);
    let mut self_revs = Vec::with_capacity(0);
    let mut other_revs = Vec::with_capacity(0);

    for sig in mem::replace(&mut b.certifications, Vec::with_capacity(0)) {
        let typ = sig.typ();

        let issuers =
            sig.get_issuers();
        let is_selfsig =
            issuers.contains(primary)
            || issuers.contains(primary_keyid);

        use crate::SignatureType::*;
        if typ == KeyRevocation
            || typ == SubkeyRevocation
            || typ == CertificationRevocation
        {
            if is_selfsig {
                self_revs.push(sig);
            } else {
                other_revs.push(sig);
            }
        } else if is_selfsig {
            self_signatures.push(sig);
        } else {
            certifications.push(sig);
        }
    }

    b.self_signatures = self_signatures;
    b.certifications = certifications;
    b.self_revocations = self_revs;
    b.other_revocations = other_revs;
}

impl<'a> Iterator for CertParser<'a> {
    type Item = Result<Cert>;

    fn next(&mut self) -> Option<Self::Item> {
        tracer!(TRACE, "CertParser::next", 0);

        loop {
            match self.source.take() {
                None => {
                    t!("EOF.");

                    if let Some(err) = self.queued_error.take() {
                        return Some(Err(err));
                    }
                    if self.packets.is_empty() {
                        return None;
                    }
                    match self.cert(None) {
                        Ok(Some(cert)) => return Some(Ok(cert)),
                        Ok(None) => return None,
                        Err(err) => return Some(Err(err)),
                    }
                },
                Some(mut iter) => {
                    let r = match iter.next() {
                        Some(Ok(packet)) => {
                            t!("Got packet #{} ({}{})",
                               self.packets.len(), packet.tag(),
                               match &packet {
                                   Packet::PublicKey(k) =>
                                       Some(k.fingerprint().to_hex()),
                                   Packet::SecretKey(k) =>
                                       Some(k.fingerprint().to_hex()),
                                   Packet::PublicSubkey(k) =>
                                       Some(k.fingerprint().to_hex()),
                                   Packet::SecretSubkey(k) =>
                                       Some(k.fingerprint().to_hex()),
                                   Packet::UserID(u) =>
                                       Some(String::from_utf8_lossy(u.value())
                                                .into()),
                                   Packet::Signature(s) =>
                                       Some(format!("{}", s.typ())),
                                   _ => None,
                               }
                               .map(|s| format!(", {}", s))
                               .unwrap_or_else(|| "".into())
                            );
                            self.source = Some(iter);
                            self.parse(packet)
                        }
                        Some(Err(err)) => {
                            t!("Error getting packet: {}", err);
                            self.saw_error = true;

                            if ! self.packets.is_empty() {
                                // Returned any queued certificate first.
                                match self.cert(None) {
                                    Ok(Some(cert)) => {
                                        self.queued_error = Some(err);
                                        return Some(Ok(cert));
                                    }
                                    Ok(None) => {
                                        return Some(Err(err));
                                    }
                                    Err(err) => {
                                        return Some(Err(err));
                                    }
                                }
                            } else {
                                return Some(Err(err));
                            }
                        }
                        None if self.packets.is_empty() => {
                            t!("Packet iterator was empty");
                            Ok(None)
                        }
                        None => {
                            t!("Packet iterator exhausted after {} packets",
                               self.packets.len());
                            self.cert(None)
                        }
                    };

                    match r {
                        Ok(Some(cert)) => {
                            t!(" => {}", cert.fingerprint());
                            return Some(Ok(cert));
                        }
                        Ok(None) => (),
                        Err(err) => return Some(Err(err)),
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::collections::HashSet;
    use std::iter::FromIterator;

    use crate::Fingerprint;
    use crate::cert::prelude::*;
    use crate::packet::prelude::*;
    use crate::parse::RECOVERY_THRESHOLD;
    use crate::serialize::Serialize;
    use crate::types::DataFormat;

    use crate::tests;

    #[test]
    fn tokens() {
        use crate::cert::parser::low_level::lexer::{Token, Lexer};
        use crate::cert::parser::low_level::lexer::Token::*;
        use crate::cert::parser::low_level::CertParser;

        struct TestVector<'a> {
            s: &'a [Token],
            result: bool,
        }

        let test_vectors = [
            TestVector {
                s: &[ PublicKey(None) ],
                result: true,
            },
            TestVector {
                s: &[ SecretKey(None) ],
                result: true,
            },
            TestVector {
                s: &[ PublicKey(None), Signature(None) ],
                result: true,
            },
            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None) ],
                result: true,
            },

            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                     UserID(None) ],
                result: true,
            },
            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                     UserID(None), Signature(None) ],
                result: true,
            },
            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                     UserAttribute(None) ],
                result: true,
            },
            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                     UserAttribute(None), Signature(None) ],
                result: true,
            },
            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                     PublicSubkey(None) ],
                result: true,
            },
            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                     PublicSubkey(None), Signature(None) ],
                result: true,
            },
            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                     SecretSubkey(None) ],
                result: true,
            },
            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                     SecretSubkey(None), Signature(None) ],
                result: true,
            },

            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                      SecretSubkey(None), Signature(None),
                      SecretSubkey(None), Signature(None),
                      SecretSubkey(None), Signature(None),
                      SecretSubkey(None), Signature(None),
                      SecretSubkey(None), Signature(None),
                      UserID(None), Signature(None),
                        Signature(None), Signature(None),
                      SecretSubkey(None), Signature(None),
                      UserAttribute(None), Signature(None),
                      Signature(None), Signature(None),
                      SecretSubkey(None), Signature(None),
                      UserID(None),
                      UserAttribute(None), Signature(None),
                        Signature(None), Signature(None),
                ],
                result: true,
            },

            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                      PublicKey(None), Signature(None), Signature(None),
                ],
                result: false,
            },
            TestVector {
                s: &[ PublicKey(None), Signature(None), Signature(None),
                      SecretKey(None), Signature(None), Signature(None),
                ],
                result: false,
            },
            TestVector {
                s: &[ SecretKey(None), Signature(None), Signature(None),
                      SecretKey(None), Signature(None), Signature(None),
                ],
                result: false,
            },
            TestVector {
                s: &[ SecretKey(None), Signature(None), Signature(None),
                      PublicKey(None), Signature(None), Signature(None),
                ],
                result: false,
            },
            TestVector {
                s: &[ SecretSubkey(None), Signature(None), Signature(None),
                      PublicSubkey(None), Signature(None), Signature(None),
                ],
                result: false,
            },
        ];

        for v in &test_vectors {
            if v.result {
                let mut l = CertValidator::new();
                for token in v.s.into_iter() {
                    l.push_token((*token).clone());
                    assert_match!(CertValidity::CertPrefix = l.check());
                }

                l.finish();
                assert_match!(CertValidity::Cert = l.check());
            }

            match CertParser::new().parse(Lexer::from_tokens(v.s)) {
                Ok(r) => assert!(v.result, "Parsing: {:?} => {:?}", v.s, r),
                Err(e) => assert!(! v.result, "Parsing: {:?} => {:?}", v.s, e),
            }
        }
    }

    #[test]
    fn marker_packet_ignored() {
        use crate::serialize::Serialize;
        let mut testy_with_marker = Vec::new();
        Packet::Marker(Default::default())
            .serialize(&mut testy_with_marker).unwrap();
        testy_with_marker.extend_from_slice(crate::tests::key("testy.pgp"));
        CertParser::from(
            PacketParser::from_bytes(&testy_with_marker).unwrap())
            .next().unwrap().unwrap();

        let mut testy_with_marker = Vec::new();
        testy_with_marker.extend_from_slice(crate::tests::key("testy.pgp"));
        Packet::Marker(Default::default())
            .serialize(&mut testy_with_marker).unwrap();
        CertParser::from(
            PacketParser::from_bytes(&testy_with_marker).unwrap())
            .next().unwrap().unwrap();
    }

    #[test]
    fn invalid_packets() -> Result<()> {
        tracer!(TRACE, "invalid_packets", 0);

        fn cert_cmp(a: &Result<Cert>, b: &Vec<Packet>)
        {
            let a : Vec<Packet> = a.as_ref().unwrap().clone().into();

            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                if a != b {
                    panic!("Differ at element #{}:\n  {:?}\n  {:?}",
                           i, a, b);
                }
            }
            if a.len() != b.len() {
                panic!("Different lengths (common prefix identical): {} vs. {}",
                       a.len(), b.len());
            }
        }

        let (cert, _) =
            CertBuilder::general_purpose(None, Some("alice@example.org"))
            .generate()?;
        let cert : Vec<Packet> = cert.into();

        // A userid packet.
        let userid : Packet = cert.clone()
            .into_iter()
            .filter(|p| p.tag() == Tag::UserID)
            .next()
            .unwrap();

        // An unknown packet.
        let tag = Tag::Private(61);
        let unknown : Packet
            = Unknown::new(tag, Error::UnsupportedPacketType(tag).into())
            .into();

        // A literal packet.  (This is a valid OpenPGP Message.)
        let mut lit = Literal::new(DataFormat::Text);
        lit.set_body(b"test".to_vec());
        let lit = Packet::from(lit);

        // A compressed data packet containing a literal data packet.
        // (This is a valid OpenPGP Message.)
        let cd = {
            use crate::types::CompressionAlgorithm;
            use crate::packet;
            use crate::PacketPile;
            use crate::serialize::Serialize;
            use crate::parse::Parse;

            let mut cd = CompressedData::new(
                CompressionAlgorithm::Uncompressed);
            let mut body = Vec::new();
            lit.serialize(&mut body)?;
            cd.set_body(packet::Body::Processed(body));
            let cd = Packet::from(cd);

            // Make sure we created the message correctly: serialize,
            // parse it, and then check its form.
            let mut bytes = Vec::new();
            cd.serialize(&mut bytes)?;

            let pp = PacketPile::from_bytes(&bytes[..])?;

            assert_eq!(pp.descendants().count(), 2);
            assert_eq!(pp.path_ref(&[ 0 ]).unwrap().tag(),
                       packet::Tag::CompressedData);
            assert_eq!(pp.path_ref(&[ 0, 0 ]), Some(&lit));

            cd
        };

        t!("A single cert.");
        let cp = CertParser::from_iter(cert.clone()).collect::<Vec<_>>();
        assert_eq!(cp.len(), 1);
        cert_cmp(&cp[0], &cert);

        t!("Two certificates.");
        let cp = CertParser::from_iter(
            cert.clone().into_iter().chain(cert.clone())).collect::<Vec<_>>();
        assert_eq!(cp.len(), 2);
        cert_cmp(&cp[0], &cert);
        cert_cmp(&cp[1], &cert);

        fn interleave(cert: &Vec<Packet>, p: &Packet) {
            t!("A certificate, a {}.", p.tag());
            let cp = CertParser::from_iter(
                cert.clone().into_iter()
                    .chain(p.clone()))
                .collect::<Vec<_>>();
            assert_eq!(cp.len(), 2);
            cert_cmp(&cp[0], cert);
            assert!(cp[1].is_err());

            t!("A certificate, two {}.", p.tag());
            let cp = CertParser::from_iter(
                cert.clone().into_iter()
                    .chain(p.clone())
                    .chain(p.clone()))
                .collect::<Vec<_>>();
            assert_eq!(cp.len(), 3);
            cert_cmp(&cp[0], cert);
            assert!(cp[1].is_err());
            assert!(cp[2].is_err());

            t!("A {}, a certificate.", p.tag());
            let cp = CertParser::from_iter(
                p.clone().into_iter()
                    .chain(cert.clone()))
                .collect::<Vec<_>>();
            assert_eq!(cp.len(), 2);
            assert!(cp[0].is_err());
            cert_cmp(&cp[1], cert);

            t!("Two {}, a certificate.", p.tag());
            let cp = CertParser::from_iter(
                p.clone().into_iter()
                    .chain(p.clone())
                    .chain(cert.clone()))
                .collect::<Vec<_>>();
            assert_eq!(cp.len(), 3);
            assert!(cp[0].is_err());
            assert!(cp[1].is_err());
            cert_cmp(&cp[2], cert);

            t!("Two {}, a certificate, two {}.", p.tag(), p.tag());
            let cp = CertParser::from_iter(
                p.clone().into_iter()
                    .chain(p.clone())
                    .chain(cert.clone())
                    .chain(p.clone())
                    .chain(p.clone()))
                .collect::<Vec<_>>();
            assert_eq!(cp.len(), 5);
            assert!(cp[0].is_err());
            assert!(cp[1].is_err());
            cert_cmp(&cp[2], cert);
            assert!(cp[3].is_err());
            assert!(cp[4].is_err());

            t!("Two {}, two certificates, two {}, a certificate.");
            let cp = CertParser::from_iter(
                p.clone().into_iter()
                    .chain(p.clone())
                    .chain(cert.clone())
                    .chain(cert.clone())
                    .chain(p.clone())
                    .chain(p.clone())
                    .chain(cert.clone()))
                .collect::<Vec<_>>();
            assert_eq!(cp.len(), 7);
            assert!(cp[0].is_err());
            assert!(cp[1].is_err());
            cert_cmp(&cp[2], cert);
            cert_cmp(&cp[3], cert);
            assert!(cp[4].is_err());
            assert!(cp[5].is_err());
            cert_cmp(&cp[6], cert);
        }

        interleave(&cert, &lit);
        // The certificate parser shouldn't recurse into containers.
        // So, the compressed data packets should show up as a single
        // error.
        interleave(&cert, &cd);


        // The certificate parser should treat unknown packets as
        // valid certificate components.
        let mut cert_plus = cert.clone();
        cert_plus.push(unknown.clone());

        t!("A certificate, an unknown.");
        let cp = CertParser::from_iter(
            cert.clone().into_iter()
                .chain(unknown.clone()))
            .collect::<Vec<_>>();
        assert_eq!(cp.len(), 1);
        cert_cmp(&cp[0], &cert_plus);

        t!("An unknown, a certificate.");
        let cp = CertParser::from_iter(
            unknown.clone().into_iter()
                .chain(cert.clone()))
            .collect::<Vec<_>>();
        assert_eq!(cp.len(), 2);
        assert!(cp[0].is_err());
        cert_cmp(&cp[1], &cert);

        t!("A certificate, two unknowns.");
        let cp = CertParser::from_iter(
            cert.clone().into_iter()
                .chain(unknown.clone())
                .chain(unknown.clone()))
            .collect::<Vec<_>>();
        assert_eq!(cp.len(), 1);
        cert_cmp(&cp[0], &cert_plus);

        t!("A certificate, an unknown, a certificate.");
        let cp = CertParser::from_iter(
            cert.clone().into_iter()
                .chain(unknown.clone())
                .chain(cert.clone()))
            .collect::<Vec<_>>();
        assert_eq!(cp.len(), 2);
        cert_cmp(&cp[0], &cert_plus);
        cert_cmp(&cp[1], &cert);


        t!("A Literal, two User IDs");
        let cp = CertParser::from_iter(
            lit.clone().into_iter()
                .chain(userid.clone())
                .chain(userid.clone()))
            .collect::<Vec<_>>();
        assert_eq!(cp.len(), 3);
        assert!(cp[0].is_err());
        assert!(cp[1].is_err());
        assert!(cp[2].is_err());

        t!("A User ID, a certificate");
        let cp = CertParser::from_iter(
            userid.clone().into_iter()
                .chain(cert.clone()))
            .collect::<Vec<_>>();
        assert_eq!(cp.len(), 2);
        assert!(cp[0].is_err());
        cert_cmp(&cp[1], &cert);

        t!("Two User IDs, a certificate");
        let cp = CertParser::from_iter(
            userid.clone().into_iter()
                .chain(userid.clone())
                .chain(cert.clone()))
            .collect::<Vec<_>>();
        assert_eq!(cp.len(), 3);
        assert!(cp[0].is_err());
        assert!(cp[1].is_err());
        cert_cmp(&cp[2], &cert);

        Ok(())
    }

    #[test]
    fn concatenated_armored_certs() -> Result<()> {
        let mut keyring = Vec::new();
        keyring.extend_from_slice(b"some\ntext\n");
        keyring.extend_from_slice(crate::tests::key("testy.asc"));
        keyring.extend_from_slice(crate::tests::key("testy.asc"));
        keyring.extend_from_slice(b"some\ntext\n");
        keyring.extend_from_slice(crate::tests::key("testy.asc"));
        keyring.extend_from_slice(b"some\ntext\n");
        let certs = CertParser::from_bytes(&keyring)?.collect::<Vec<_>>();
        assert_eq!(certs.len(), 3);
        assert!(certs.iter().all(|c| c.is_ok()));
        Ok(())
    }

    fn parse_test(n: usize, literal: bool, bad: usize) -> Result<()> {
        tracer!(TRACE, "t", 0);

        // Parses keyrings with different numbers of keys and
        // different errors.

        // n: number of keys
        // literal: whether to interleave literal packets.
        // bad: whether to insert invalid data (NUL bytes where
        //      the start of a certificate is expected).
        let nulls = vec![ 0; bad ];

        t!("n: {}, literals: {}, bad data: {}",
           n, literal, bad);

        let mut data = Vec::new();

        let mut certs_orig = vec![];
        for i in 0..n {
            let (cert, _) =
                CertBuilder::general_purpose(
                    None, Some(format!("{}@example.org", i)))
                .generate()?;

            cert.as_tsk().serialize(&mut data)?;
            certs_orig.push(cert);

            if literal {
                let mut lit = Literal::new(DataFormat::Text);
                lit.set_body(b"data".to_vec());

                Packet::from(lit).serialize(&mut data)?;
            }
            // Push some NUL bytes.
            data.extend(&nulls[..bad]);
        }
        if n == 0 {
            // Push some NUL bytes even if we didn't add any packets.
            data.extend(&nulls[..bad]);
        }
        assert_eq!(certs_orig.len(), n);

        t!("Start of data: {} {}",
           if let Some(x) = data.get(0) {
               format!("{:02X}", x)
           } else {
               "XX".into()
           },
           if let Some(x) = data.get(1) {
               format!("{:02X}", x)
           } else {
               "XX".into()
           });

        let certs_parsed = CertParser::from_bytes(&data);

        let certs_parsed = if n == 0 && bad > 0 {
            // Junk at the beginning of the file results in an
            // immediate parse error.
            assert!(certs_parsed.is_err());
            return Ok(());
        } else {
            certs_parsed.expect("Valid init")
        };
        let certs_parsed: Vec<_> = certs_parsed.collect();

        certs_parsed.iter().enumerate().for_each(|(i, r)| {
            t!("{}. {}",
               i,
               match r {
                   Ok(c) => c.fingerprint().to_string(),
                   Err(err) => err.to_string(),
               });
        });

        let n = if bad > RECOVERY_THRESHOLD {
            // We stop once we see the junk.
            certs_orig.drain(1..);
            std::cmp::min(n, 1)
        } else {
            n
        };

        let modulus = if literal && bad > 0 {
            3
        } else {
            2
        };
        let certs_parsed: Vec<Cert> = certs_parsed.into_iter()
            .enumerate()
            .filter_map(|(i, c)| {
                if literal && i % modulus == 1 {
                    // Literals should be errors.
                    assert!(c.is_err());
                    None
                } else if bad > 0 && n == 0 && i == 0 {
                    // The first byte in the input is the NUL
                    // byte.
                    assert!(c.is_err());
                    None
                } else if bad > 0 && i % modulus == modulus - 1 {
                    // NUL bytes are inserted after the
                    // certificate / literal data packet.  So the
                    // second element will be the parse error.
                    assert!(c.is_err());
                    None
                } else {
                    Some(c.unwrap())
                }
            })
            .collect();

        assert_eq!(certs_orig.len(), certs_parsed.len(),
                   "number of parsed certificates: expected vs. got");

        let fpr_orig = certs_orig.iter()
            .map(|c| {
                c.fingerprint()
            })
            .collect::<Vec<_>>();
        let fpr_parsed = certs_parsed.iter()
            .map(|c| {
                c.fingerprint()
            })
            .collect::<Vec<_>>();
        if fpr_orig != fpr_parsed {
            t!("{} certificates in orig; {} is parsed",
               fpr_orig.len(), fpr_parsed.len());

            let fpr_set_orig: HashSet<&Fingerprint>
                = HashSet::from_iter(fpr_orig.iter());
            let fpr_set_parsed = HashSet::from_iter(fpr_parsed.iter());
            t!("Only in orig:\n  {}",
               fpr_set_orig.difference(&fpr_set_parsed)
               .map(|f| f.to_string())
               .collect::<Vec<_>>()
               .join(",\n  "));
            t!("Only in parsed:\n  {}",
               fpr_set_parsed.difference(&fpr_set_orig)
               .map(|f| f.to_string())
               .collect::<Vec<_>>()
               .join(",\n  "));

            assert_eq!(fpr_orig, fpr_parsed);
        }

        // Go packet by packet.  (This makes finding an error a
        // lot easier.)
        for (i, (c_orig, c_parsed)) in
            certs_orig
            .into_iter()
            .zip(certs_parsed.into_iter())
            .enumerate()
        {
            let ps_orig: Vec<Packet> = c_orig.into_packets().collect();
            let ps_parsed: Vec<Packet> = c_parsed.into_packets().collect();
            if bad > 0 && ! literal && i == n - 1 {
                // On a parse error, we lose the last successfully
                // parsed packet.  This is annoying.  But, the
                // file is corrupted anyway, so...
                assert_eq!(ps_orig.len() - 1, ps_parsed.len(),
                           "number of packets: expected vs. got");
            } else {
                assert_eq!(ps_orig.len(), ps_parsed.len(),
                           "number of packets: expected vs. got");
            }

            for (j, (p_orig, p_parsed)) in
                ps_orig
                .into_iter()
                .zip(ps_parsed.into_iter())
                .enumerate()
            {
                assert_eq!(p_orig, p_parsed,
                           "Cert {}, packet: {}", i, j);
            }
        }

        Ok(())
    }

    #[test]
    fn parse_keyring_simple() -> Result<()> {
        for n in [1, 100, 0].iter() {
            parse_test(*n, false, 0)?;
        }

        Ok(())
    }

    #[test]
    fn parse_keyring_interleaved_literals() -> Result<()> {
        for n in [1, 100, 0].iter() {
            parse_test(*n, true, 0)?;
        }

        Ok(())
    }

    #[test]
    fn parse_keyring_interleaved_small_junk() -> Result<()> {
        for n in [1, 100, 0].iter() {
            parse_test(*n, false, 1)?;
        }

        Ok(())
    }

    #[test]
    fn parse_keyring_interleaved_unrecoverable_junk() -> Result<()> {
        // PacketParser is pretty good at recovering from junk in the
        // middle: it will search the next RECOVERY_THRESHOLD bytes
        // for a valid packet.  If it finds it, it will turn the junk
        // into a reserved packet and resume.  Insert a lot of NULs to
        // prevent the recovery mechanism from working.
        for n in [1, 100, 0].iter() {
            parse_test(*n, false, 2 * RECOVERY_THRESHOLD)?;
        }

        Ok(())
    }

    #[test]
    fn parse_keyring_interleaved_literal_and_small_junk() -> Result<()> {
        for n in [1, 100, 0].iter() {
            parse_test(*n, true, 1)?;
        }

        Ok(())
    }

    #[test]
    fn parse_keyring_interleaved_literal_and_unrecoverable_junk() -> Result<()> {
        for n in [1, 100, 0].iter() {
            parse_test(*n, true, 2 * RECOVERY_THRESHOLD)?;
        }

        Ok(())
    }

    #[test]
    fn parse_keyring_no_public_key() -> Result<()> {
        tracer!(TRACE, "parse_keyring_no_public_key", 0);

        // The first few packets are not the valid start of a
        // certificate.  Each of those should return in an Error.
        // But, that shouldn't stop us from parsing the rest of the
        // keyring.

        let (cert_1, _) =
            CertBuilder::general_purpose(
                None, Some("a@example.org"))
            .generate()?;
        let cert_1_packets: Vec<Packet>
            = cert_1.into_packets().collect();

        let (cert_2, _) =
            CertBuilder::general_purpose(
                None, Some("b@example.org"))
            .generate()?;

        for n in 1..cert_1_packets.len() {
            t!("n: {}", n);

            let mut data = Vec::new();

            for i in n..cert_1_packets.len() {
                cert_1_packets[i].serialize(&mut data)?;
            }

            cert_2.as_tsk().serialize(&mut data)?;


            let certs_parsed = CertParser::from_bytes(&data)
                .expect("Valid parse");

            let mut iter = certs_parsed;
            for _ in n..cert_1_packets.len() {
                assert!(iter.next().unwrap().is_err());
            }
            assert_eq!(iter.next().unwrap().as_ref().unwrap(), &cert_2);
            assert!(iter.next().is_none());
            assert!(iter.next().is_none());
        }

        Ok(())
    }

    #[test]
    fn filter() {
        let fp = Fingerprint::from_hex(
            "CBCD8F030588653EEDD7E2659B7DD433F254904A",
        ).unwrap();

        let cp = CertParser::from_bytes(tests::key("bad-subkey-keyring.pgp"))
            .unwrap()
            .unvalidated_cert_filter(|cert, _| {
                cert.fingerprint() == fp
            });
        let certs = cp.collect::<Result<Vec<Cert>>>().unwrap();
        assert_eq!(certs.len(), 1);
        assert!(certs[0].fingerprint() == fp);
    }
}
