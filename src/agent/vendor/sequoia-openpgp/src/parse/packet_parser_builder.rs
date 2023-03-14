use std::io;
use std::path::Path;

use buffered_reader::BufferedReader;

use crate::Result;
use crate::parse::PacketParserResult;
use crate::parse::PacketParser;
use crate::parse::PacketParserEOF;
use crate::parse::PacketParserState;
use crate::parse::PacketParserSettings;
use crate::parse::ParserResult;
use crate::parse::Parse;
use crate::parse::Cookie;
use crate::armor;
use crate::packet;

/// Controls transparent stripping of ASCII armor when parsing.
///
/// When parsing OpenPGP data streams, the [`PacketParser`] will by
/// default automatically detect and remove any ASCII armor encoding
/// (see [Section 6 of RFC 4880]).  This automatism can be disabled
/// and fine-tuned using [`PacketParserBuilder::dearmor`].
///
///   [Section 6 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-6
///   [`PacketParserBuilder::dearmor`]: PacketParserBuilder::dearmor()
#[derive(PartialEq)]
pub enum Dearmor {
    /// Unconditionally treat the input as if it were an OpenPGP
    /// message encoded using ASCII armor.
    ///
    /// Parsing a binary encoded OpenPGP message using this mode will
    /// fail.  The [`ReaderMode`] allow further customization of the
    /// ASCII armor parser.
    ///
    ///   [`ReaderMode`]: crate::armor::ReaderMode
    Enabled(armor::ReaderMode),
    /// Unconditionally treat the input as if it were a binary OpenPGP
    /// message.
    ///
    /// Parsing an ASCII armor encoded OpenPGP message using this mode will
    /// fail.
    Disabled,
    /// If input does not appear to be a binary encoded OpenPGP
    /// message, treat it as if it were encoded using ASCII armor.
    ///
    /// This is the default.  The [`ReaderMode`] allow further
    /// customization of the ASCII armor parser.
    ///
    ///   [`ReaderMode`]: crate::armor::ReaderMode
    Auto(armor::ReaderMode),
}
assert_send_and_sync!(Dearmor);

impl Default for Dearmor {
    fn default() -> Self {
        Dearmor::Auto(Default::default())
    }
}

/// This is the level at which we insert the dearmoring filter into
/// the buffered reader stack.
pub(super) const ARMOR_READER_LEVEL: isize = -2;

/// A builder for configuring a `PacketParser`.
///
/// Since the default settings are usually appropriate, this mechanism
/// will only be needed in exceptional circumstances.  Instead use,
/// for instance, `PacketParser::from_file` or
/// `PacketParser::from_reader` to start parsing an OpenPGP message.
///
/// # Examples
///
/// ```rust
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use sequoia_openpgp as openpgp;
/// use openpgp::parse::{Parse, PacketParserResult, PacketParserBuilder};
///
/// // Parse a message.
/// let message_data: &[u8] = // ...
/// #    include_bytes!("../../tests/data/messages/compressed-data-algo-0.pgp");
/// let mut ppr = PacketParserBuilder::from_bytes(message_data)?
///     // Customize the `PacketParserBuilder` here.
///     .build()?;
/// while let PacketParserResult::Some(mut pp) = ppr {
///     // ...
///
///     // Start parsing the next packet, recursing.
///     ppr = pp.recurse()?.1;
/// }
/// # Ok(()) }
/// ```
pub struct PacketParserBuilder<'a> {
    bio: Box<dyn BufferedReader<Cookie> + 'a>,
    dearmor: Dearmor,
    settings: PacketParserSettings,
    csf_transformation: bool,
}
assert_send_and_sync!(PacketParserBuilder<'_>);

impl<'a> Parse<'a, PacketParserBuilder<'a>> for PacketParserBuilder<'a> {
    /// Creates a `PacketParserBuilder` for an OpenPGP message stored
    /// in a `std::io::Read` object.
    fn from_reader<R: io::Read + 'a + Send + Sync>(reader: R) -> Result<Self> {
        PacketParserBuilder::from_buffered_reader(
            Box::new(buffered_reader::Generic::with_cookie(
                reader, None, Cookie::default())))
    }

    /// Creates a `PacketParserBuilder` for an OpenPGP message stored
    /// in the file named `path`.
    fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        PacketParserBuilder::from_buffered_reader(
            Box::new(buffered_reader::File::with_cookie(path, Cookie::default())?))
    }

    /// Creates a `PacketParserBuilder` for an OpenPGP message stored
    /// in the specified buffer.
    fn from_bytes<D: AsRef<[u8]> + ?Sized>(data: &'a D) -> Result<PacketParserBuilder<'a>> {
        PacketParserBuilder::from_buffered_reader(
            Box::new(buffered_reader::Memory::with_cookie(
                data.as_ref(), Cookie::default())))
    }
}

impl<'a> PacketParserBuilder<'a> {
    // Creates a `PacketParserBuilder` for an OpenPGP message stored
    // in a `BufferedReader` object.
    //
    // Note: this clears the `level` field of the
    // `Cookie` cookie.
    pub(crate) fn from_buffered_reader(mut bio: Box<dyn BufferedReader<Cookie> + 'a>)
            -> Result<Self> {
        bio.cookie_mut().level = None;
        Ok(PacketParserBuilder {
            bio,
            dearmor: Default::default(),
            settings: PacketParserSettings::default(),
            csf_transformation: false,
        })
    }

    /// Sets the maximum recursion depth.
    ///
    /// Setting this to 0 means that the `PacketParser` will never
    /// recurse; it will only parse the top-level packets.
    ///
    /// This is a u8, because recursing more than 255 times makes no
    /// sense.  The default is [`DEFAULT_MAX_RECURSION_DEPTH`].
    /// (GnuPG defaults to a maximum recursion depth of 32.)
    ///
    /// [`DEFAULT_MAX_RECURSION_DEPTH`]: crate::parse::DEFAULT_MAX_RECURSION_DEPTH
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParserBuilder};
    ///
    /// // Parse a compressed message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParserBuilder::from_bytes(message_data)?
    ///     .max_recursion_depth(0)
    ///     .build()?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     assert_eq!(pp.recursion_depth(), 0);
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn max_recursion_depth(mut self, value: u8) -> Self {
        self.settings.max_recursion_depth = value;
        self
    }

    /// Sets the maximum size in bytes of non-container packets.
    ///
    /// Packets that exceed this limit will be returned as
    /// `Packet::Unknown`, with the error set to
    /// `Error::PacketTooLarge`.
    ///
    /// This limit applies to any packet type that is *not* a
    /// container packet, i.e. any packet that is not a literal data
    /// packet, a compressed data packet, a symmetrically encrypted
    /// data packet, or an AEAD encrypted data packet.
    ///
    /// The default is [`DEFAULT_MAX_PACKET_SIZE`].
    ///
    /// [`DEFAULT_MAX_PACKET_SIZE`]: crate::parse::DEFAULT_MAX_PACKET_SIZE
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::{Error, Packet};
    /// use openpgp::packet::Tag;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParserBuilder};
    /// use openpgp::serialize::MarshalInto;
    ///
    /// // Parse a signed message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../../tests/data/messages/signed-1.gpg");
    /// let mut ppr = PacketParserBuilder::from_bytes(message_data)?
    ///     .max_packet_size(256)    // Only parse 256 bytes of headers.
    ///     .buffer_unread_content() // Used below.
    ///     .build()?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     match &pp.packet {
    ///         Packet::OnePassSig(p) =>
    ///             // The OnePassSig packet was small enough.
    ///             assert!(p.serialized_len() < 256),
    ///         Packet::Literal(p) =>
    ///             // Likewise the `Literal` packet, excluding the body.
    ///             assert!(p.serialized_len() - p.body().len() < 256),
    ///         Packet::Unknown(p) =>
    ///             // The signature packet was too big.
    ///             assert_eq!(
    ///                 &Error::PacketTooLarge(Tag::Signature, 307, 256),
    ///                 p.error().downcast_ref().unwrap()),
    ///         _ => unreachable!(),
    ///     }
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn max_packet_size(mut self, value: u32) -> Self {
        self.settings.max_packet_size = value;
        self
    }

    /// Causes `PacketParser::build()` to buffer any unread content.
    ///
    /// The unread content can be accessed using [`Literal::body`],
    /// [`Unknown::body`], or [`Container::body`].
    ///
    ///   [`Literal::body`]: crate::packet::Literal::body()
    ///   [`Unknown::body`]: crate::packet::Unknown::body()
    ///   [`Container::body`]: crate::packet::Container::body()
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParserBuilder};
    ///
    /// // Parse a simple message.
    /// let message_data = b"\xcb\x12t\x00\x00\x00\x00\x00Hello world.";
    /// let mut ppr = PacketParserBuilder::from_bytes(message_data)?
    ///     .buffer_unread_content()
    ///     .build()?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // Start parsing the next packet, recursing.
    ///     let (packet, tmp) = pp.recurse()?;
    ///     ppr = tmp;
    ///
    ///     match packet {
    ///         Packet::Literal(l) => assert_eq!(l.body(), b"Hello world."),
    ///         _ => unreachable!(),
    ///     }
    /// }
    /// # Ok(()) }
    /// ```
    pub fn buffer_unread_content(mut self) -> Self {
        self.settings.buffer_unread_content = true;
        self
    }

    /// Causes `PacketParser::finish()` to drop any unread content.
    ///
    /// This is the default.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParserBuilder};
    ///
    /// // Parse a simple message.
    /// let message_data = b"\xcb\x12t\x00\x00\x00\x00\x00Hello world.";
    /// let mut ppr = PacketParserBuilder::from_bytes(message_data)?
    ///     .drop_unread_content()
    ///     .build()?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // Start parsing the next packet, recursing.
    ///     let (packet, tmp) = pp.recurse()?;
    ///     ppr = tmp;
    ///
    ///     match packet {
    ///         Packet::Literal(l) => assert_eq!(l.body(), b""),
    ///         _ => unreachable!(),
    ///     }
    /// }
    /// # Ok(()) }
    /// ```
    pub fn drop_unread_content(mut self) -> Self {
        self.settings.buffer_unread_content = false;
        self
    }

    /// Controls mapping.
    ///
    /// Note that enabling mapping buffers all the data.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::parse::{Parse, PacketParserBuilder};
    ///
    /// let message_data = b"\xcb\x12t\x00\x00\x00\x00\x00Hello world.";
    /// let pp = PacketParserBuilder::from_bytes(message_data)?
    ///     .map(true) // Enable mapping.
    ///     .build()?
    ///     .expect("One packet, not EOF");
    /// let map = pp.map().expect("Mapping is enabled");
    ///
    /// assert_eq!(map.iter().nth(0).unwrap().name(), "CTB");
    /// assert_eq!(map.iter().nth(0).unwrap().offset(), 0);
    /// assert_eq!(map.iter().nth(0).unwrap().as_bytes(), &[0xcb]);
    /// # Ok(()) }
    /// ```
    pub fn map(mut self, enable: bool) -> Self {
        self.settings.map = enable;
        self
    }

    /// Controls dearmoring.
    ///
    /// By default, if the input does not appear to be plain binary
    /// OpenPGP data, we assume that it is ASCII-armored.  This method
    /// can be used to tweak the behavior.  See [`Dearmor`] for
    /// details.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::parse::{Parse, PacketParserBuilder, Dearmor};
    ///
    /// let message_data = b"\xcb\x12t\x00\x00\x00\x00\x00Hello world.";
    /// let pp = PacketParserBuilder::from_bytes(message_data)?
    ///     .dearmor(Dearmor::Disabled) // Disable dearmoring.
    ///     .build()?
    ///     .expect("One packet, not EOF");
    /// # Ok(()) }
    /// ```
    pub fn dearmor(mut self, mode: Dearmor) -> Self {
        self.dearmor = mode;
        self
    }

    /// Controls transparent transformation of messages using the
    /// cleartext signature framework into signed messages.
    ///
    /// XXX: This could be controlled by `Dearmor`, but we cannot add
    /// values to that now.
    pub(crate) fn csf_transformation(mut self, enable: bool) -> Self {
        self.csf_transformation = enable;
        self
    }

    /// Builds the `PacketParser`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParserBuilder};
    ///
    /// // Parse a message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParserBuilder::from_bytes(message_data)?
    ///     // Customize the `PacketParserBuilder` here.
    ///     .build()?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // ...
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    #[allow(clippy::redundant_pattern_matching)]
    pub fn build(mut self)
        -> Result<PacketParserResult<'a>>
        where Self: 'a
    {
        let state = PacketParserState::new(self.settings);

        let dearmor_mode = match self.dearmor {
            Dearmor::Enabled(mode) => Some(mode),
            Dearmor::Disabled => None,
            Dearmor::Auto(mode) => {
                if self.bio.eof() {
                    None
                } else {
                    let mut reader = buffered_reader::Dup::with_cookie(
                        self.bio, Cookie::default());
                    let header = packet::Header::parse(&mut reader);
                    self.bio = Box::new(reader).into_inner().unwrap();
                    if let Ok(header) = header {
                        if let Err(_) = header.valid(false) {
                            // Invalid header: better try an ASCII armor
                            // decoder.
                            Some(mode)
                        } else {
                            None
                        }
                    } else {
                        // Failed to parse the header: better try an ASCII
                        // armor decoder.
                        Some(mode)
                    }
                }
            }
        };

        if let Some(mode) = dearmor_mode {
            // Add a top-level filter so that it is peeled off when
            // the packet parser is finished.  We use level -2 for that.
            self.bio =
                armor::Reader::from_buffered_reader_csft(self.bio, Some(mode),
                    Cookie::new(ARMOR_READER_LEVEL), self.csf_transformation)
                .as_boxed();
        }

        // Parse the first packet.
        match PacketParser::parse(Box::new(self.bio), state, vec![ 0 ])? {
            ParserResult::Success(mut pp) => {
                // We successfully parsed the first packet's header.
                pp.state.message_validator.push(
                    pp.packet.tag(), pp.packet.version(), &[0]);
                pp.state.keyring_validator.push(pp.packet.tag());
                pp.state.cert_validator.push(pp.packet.tag());
                Ok(PacketParserResult::Some(pp))
            },
            ParserResult::EOF((reader, state, _path)) => {
                // `bio` is empty.  We're done.
                Ok(PacketParserResult::EOF(PacketParserEOF::new(state, reader)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armor() {
        // Not ASCII armor encoded data.
        let msg = crate::tests::message("sig.gpg");

        // Make sure we can read the first packet.
        let ppr = PacketParserBuilder::from_bytes(msg).unwrap()
            .dearmor(Dearmor::Disabled)
            .build();
        assert_match!(Ok(PacketParserResult::Some(ref _pp)) = ppr);

        let ppr = PacketParserBuilder::from_bytes(msg).unwrap()
            .dearmor(Dearmor::Auto(Default::default()))
            .build();
        assert_match!(Ok(PacketParserResult::Some(ref _pp)) = ppr);

        let ppr = PacketParserBuilder::from_bytes(msg).unwrap()
            .dearmor(Dearmor::Enabled(Default::default()))
            .build();
        assert_match!(Err(_) = ppr);

        // ASCII armor encoded data.
        let msg = crate::tests::message("a-cypherpunks-manifesto.txt.ed25519.sig");

        // Make sure we can read the first packet.
        let ppr = PacketParserBuilder::from_bytes(msg).unwrap()
            .dearmor(Dearmor::Disabled)
            .build();
        assert_match!(Err(_) = ppr);

        let ppr = PacketParserBuilder::from_bytes(msg).unwrap()
            .dearmor(Dearmor::Auto(Default::default()))
            .build();
        assert_match!(Ok(PacketParserResult::Some(ref _pp)) = ppr);

        let ppr = PacketParserBuilder::from_bytes(msg).unwrap()
            .dearmor(Dearmor::Enabled(Default::default()))
            .build();
        assert_match!(Ok(PacketParserResult::Some(ref _pp)) = ppr);
    }
}
