//! Packet parsing infrastructure.
//!
//! OpenPGP defines a binary representation suitable for storing and
//! communicating OpenPGP data structures (see [Section 3 ff. of RFC
//! 4880]).  Parsing is the process of interpreting the binary
//! representation.
//!
//!   [Section 3 ff. of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3
//!
//! An OpenPGP stream represents a sequence of packets.  Some of the
//! packets contain other packets.  These so called containers include
//! encrypted data packets (the SED and [SEIP] packets), and
//! [compressed data] packets.  This structure results in a tree,
//! which is laid out in depth-first order.
//!
//!   [SEIP]: crate::packet::SEIP
//!   [compressed data]: crate::packet::CompressedData
//!
//! OpenPGP defines objects consisting of several packets with a
//! specific structure.  These objects are [`Message`]s, [`Cert`]s and
//! sequences of [`Cert`]s ("keyrings").  Verifying the structure of
//! these objects is also an act of parsing.
//!
//!   [`Message`]: super::Message
//!   [`Cert`]: crate::cert::Cert
//!
//! This crate provides several interfaces to parse OpenPGP data.
//! They fall in roughly three categories:
//!
//!  - First, most data structures in this crate implement the
//!    [`Parse`] trait.  It provides a uniform interface to parse data
//!    from an [`io::Read`]er, a file identified by its [`Path`], or
//!    simply a byte slice.
//!
//!  - Second, there is a convenient interface to decrypt and/or
//!    verify OpenPGP messages in a streaming fashion.  Encrypted
//!    and/or signed data is read using the [`Parse`] interface, and
//!    decrypted and/or verified data can be read using [`io::Read`].
//!
//!  - Finally, we expose the low-level [`PacketParser`], allowing
//!    fine-grained control over the parsing.
//!
//!   [`io::Read`]: std::io::Read
//!   [`Path`]: std::path::Path
//!
//! The choice of interface depends on the specific use case.  In many
//! circumstances, OpenPGP data can not be trusted until it has been
//! authenticated.  Therefore, it has to be treated as attacker
//! controlled data, and it has to be treated with great care.  See
//! the section [Security Considerations] below.
//!
//!   [Security Considerations]: #security-considerations
//!
//! # Common Operations
//!
//!  - *Decrypt a message*: Use a [streaming `Decryptor`].
//!  - *Verify a message*: Use a [streaming `Verifier`].
//!  - *Verify a detached signature*: Use a [`DetachedVerifier`].
//!  - *Parse a [`Cert`]*: Use [`Cert`]'s [`Parse`] interface.
//!  - *Parse a keyring*: Use [`CertParser`]'s [`Parse`] interface.
//!  - *Parse an unstructured sequence of small packets from a trusted
//!     source*: Use [`PacketPile`]s [`Parse`] interface (e.g.
//!     [`PacketPile::from_file`]).
//!  - *Parse an unstructured sequence of packets*: Use the
//!    [`PacketPileParser`].
//!  - *Parse an unstructured sequence of packets with full control
//!    over the parser*: Use a [`PacketParser`].
//!  - *Customize the parser behavior even more*: Use a
//!    [`PacketParserBuilder`].
//!
//!   [`CertParser`]: crate::cert::CertParser
//!   [streaming `Decryptor`]: stream::Decryptor
//!   [streaming `Verifier`]: stream::Verifier
//!   [`DetachedVerifier`]: stream::DetachedVerifier
//!   [`PacketPile`]: crate::PacketPile
//!   [`PacketPile::from_file`]: super::PacketPile::from_file()
//!
//! # Data Structures and Interfaces
//!
//! This crate provides several interfaces for parsing OpenPGP
//! streams, ordered from the most convenient but least flexible to
//! the least convenient but most flexible:
//!
//!   - The streaming [`Verifier`], [`DetachedVerifier`], and
//!     [`Decryptor`] are the most convenient way to parse OpenPGP
//!     messages.
//!
//!   - The [`PacketPile::from_file`] (and related methods) is the
//!     most convenient, but least flexible way to parse an arbitrary
//!     sequence of OpenPGP packets.  Whereas a [`PacketPileParser`]
//!     allows the caller to determine how to handle individual
//!     packets, the [`PacketPile::from_file`] parses the whole stream
//!     at once and returns a [`PacketPile`].
//!
//!   - The [`PacketPileParser`] abstraction builds on the
//!     [`PacketParser`] abstraction and provides a similar interface.
//!     However, after each iteration, the [`PacketPileParser`] adds the
//!     packet to a [`PacketPile`], which is returned once the packets are
//!     completely processed.
//!
//!     This interface should only be used if the caller actually
//!     wants a [`PacketPile`]; if the OpenPGP stream is parsed in place,
//!     then using a [`PacketParser`] is better.
//!
//!     This interface should only be used if the caller is certain
//!     that the parsed stream will fit in memory.
//!
//!   - The [`PacketParser`] abstraction produces one packet at a
//!     time.  What is done with those packets is completely up to the
//!     caller.
//!
//! The behavior of the [`PacketParser`] can be configured using a
//! [`PacketParserBuilder`].
//!
//!   [`Decryptor`]: stream::Decryptor
//!   [`Verifier`]: stream::Verifier
//!
//! # ASCII armored data
//!
//! The [`PacketParser`] will by default automatically detect and
//! remove any ASCII armor encoding (see [Section 6 of RFC 4880]).
//! This automatism can be disabled and fine-tuned using
//! [`PacketParserBuilder::dearmor`].
//!
//!   [Section 6 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-6
//!   [`PacketParserBuilder::dearmor`]: PacketParserBuilder::dearmor()
//!
//! # Security Considerations
//!
//! In general, OpenPGP data must be considered attacker controlled
//! and thus treated with great care.  Even though we use a
//! memory-safe language, there are several aspects to be aware of:
//!
//!  - OpenPGP messages may be compressed.  Therefore, one cannot
//!    predict the uncompressed size of a message by looking at the
//!    compressed representation.  Operations that parse OpenPGP
//!    streams and buffer the packet data (like using the
//!    [`PacketPile`]'s [`Parse`] interface) are inherently unsafe and
//!    must only be used on trusted data.
//!
//!  - The authenticity of an OpenPGP message can only be checked once
//!    it has been fully processed.  Therefore, the plaintext must be
//!    buffered and not be trusted until the whole message is
//!    processed and signatures and/or ciphertext integrity are
//!    verified.  On the other hand, buffering an unbounded amount of
//!    data is problematic and can lead to out-of-memory situations
//!    resulting in denial of service.  The streaming message
//!    processing interfaces address this problem by buffering an
//!    configurable amount of data before releasing any data to the
//!    caller, and only revert to streaming unverified data if the
//!    message exceeds the buffer.  See [`DEFAULT_BUFFER_SIZE`] for
//!    more information.
//!
//!  - Not all parts of signed-then-encrypted OpenPGP messages are
//!    authenticated.  Notably, all packets outside the encryption
//!    container (any [`PKESK`] and [`SKESK`] packets, as well as the
//!    encryption container itself), the [`Literal`] packet's headers,
//!    as well as parts of the [`Signature`] are not covered by the
//!    signatures.
//!
//!  - Ciphertext integrity is provided by the [`SEIP`] packet's
//!    [`MDC`] mechanism, but the integrity can only be checked after
//!    decrypting the whole container.  Proper authenticated
//!    encryption is provided by the [`AED`] container, but as of this
//!    writing it is not standardized.
//!
//!   [`DEFAULT_BUFFER_SIZE`]: stream::DEFAULT_BUFFER_SIZE
//!   [`PKESK`]: crate::packet::PKESK
//!   [`SKESK`]: crate::packet::PKESK
//!   [`Literal`]: crate::packet::Literal
//!   [`Signature`]: crate::packet::Signature
//!   [`SEIP`]: crate::packet::SEIP
//!   [`MDC`]: crate::packet::MDC
//!   [`AED`]: crate::packet::AED

use std::io;
use std::io::prelude::*;
use std::convert::TryFrom;
use std::cmp;
use std::str;
use std::mem;
use std::fmt;
use std::path::Path;
use std::result::Result as StdResult;

use xxhash_rust::xxh3::Xxh3;

use ::buffered_reader::*;

use crate::{
    cert::CertValidator,
    cert::CertValidity,
    cert::KeyringValidator,
    cert::KeyringValidity,
    crypto::{aead, hash::Hash},
    Result,
    packet::header::{
        CTB,
        BodyLength,
        PacketLengthType,
    },
    crypto::S2K,
    Error,
    packet::{
        Container,
        Header,
    },
    packet::signature::Signature3,
    packet::signature::Signature4,
    packet::prelude::*,
    Packet,
    Fingerprint,
    KeyID,
    crypto::SessionKey,
};
use crate::types::{
    AEADAlgorithm,
    CompressionAlgorithm,
    Features,
    HashAlgorithm,
    KeyFlags,
    KeyServerPreferences,
    PublicKeyAlgorithm,
    RevocationKey,
    SignatureType,
    SymmetricAlgorithm,
    Timestamp,
};
use crate::crypto::{self, mpi::{PublicKey, MPI}};
use crate::crypto::symmetric::{Decryptor, BufferedReaderDecryptor};
use crate::message;
use crate::message::MessageValidator;

mod partial_body;
use self::partial_body::BufferedReaderPartialBodyFilter;

use crate::packet::signature::subpacket::{
    NotationData,
    NotationDataFlags,
    Subpacket,
    SubpacketArea,
    SubpacketLength,
    SubpacketTag,
    SubpacketValue,
};

use crate::serialize::MarshalInto;

mod packet_pile_parser;
pub use self::packet_pile_parser::PacketPileParser;

mod hashed_reader;
pub(crate) use self::hashed_reader::{
    HashingMode,
    HashedReader,
};

mod packet_parser_builder;
pub use self::packet_parser_builder::{Dearmor, PacketParserBuilder};
use packet_parser_builder::ARMOR_READER_LEVEL;

pub mod map;
mod mpis;
pub mod stream;

// Whether to trace execution by default (on stderr).
const TRACE : bool = false;

// How much junk the packet parser is willing to skip when recovering.
// This is an internal implementation detail and hence not exported.
pub(crate) const RECOVERY_THRESHOLD: usize = 32 * 1024;

/// Parsing of packets and related structures.
///
/// This is a uniform interface to parse packets, messages, keys, and
/// related data structures.
pub trait Parse<'a, T> {
    /// Reads from the given reader.
    fn from_reader<R: 'a + Read + Send + Sync>(reader: R) -> Result<T>;

    /// Reads from the given file.
    ///
    /// The default implementation just uses [`from_reader(..)`], but
    /// implementations can provide their own specialized version.
    ///
    /// [`from_reader(..)`]: Parse::from_reader
    fn from_file<P: AsRef<Path>>(path: P) -> Result<T>
    {
        Self::from_reader(::std::fs::File::open(path)?)
    }

    /// Reads from the given slice.
    ///
    /// The default implementation just uses [`from_reader(..)`], but
    /// implementations can provide their own specialized version.
    ///
    /// [`from_reader(..)`]: Parse::from_reader
    fn from_bytes<D: AsRef<[u8]> + ?Sized + Send + Sync>(data: &'a D) -> Result<T> {
        Self::from_reader(io::Cursor::new(data))
    }
}

macro_rules! impl_parse_generic_packet {
    ($typ: ident) => {
        impl<'a> Parse<'a, $typ> for $typ {
            fn from_reader<R: 'a + Read + Send + Sync>(reader: R) -> Result<Self> {
                let bio = buffered_reader::Generic::with_cookie(
                    reader, None, Cookie::default());
                let parser = PacketHeaderParser::new_naked(bio);

                let mut pp = Self::parse(parser)?;
                pp.buffer_unread_content()?;

                match pp.next()? {
                    (Packet::$typ(o), PacketParserResult::EOF(_))
                        => Ok(o),
                    (p, PacketParserResult::EOF(_)) =>
                        Err(Error::InvalidOperation(
                            format!("Not a {} packet: {:?}", stringify!($typ),
                                    p)).into()),
                    (_, PacketParserResult::Some(_)) =>
                        Err(Error::InvalidOperation(
                            "Excess data after packet".into()).into()),
                }
            }
        }
    };
}

/// The default amount of acceptable nesting.
///
/// The default is `16`.
///
/// Typically, we expect a message to looking like:
///
/// ```text
/// [ encryption container: [ compression container: [ signature: [ literal data ]]]]
/// ```
///
/// So, this should be more than enough.
///
/// To change the maximum recursion depth, use
/// [`PacketParserBuilder::max_recursion_depth`].
///
///   [`PacketParserBuilder::max_recursion_depth`]: PacketParserBuilder::max_recursion_depth()
pub const DEFAULT_MAX_RECURSION_DEPTH : u8 = 16;

/// The default maximum size of non-container packets.
///
/// The default is `1 MiB`.
///
/// Packets that exceed this limit will be returned as
/// `Packet::Unknown`, with the error set to `Error::PacketTooLarge`.
///
/// This limit applies to any packet type that is *not* a container
/// packet, i.e. any packet that is not a literal data packet, a
/// compressed data packet, a symmetrically encrypted data packet, or
/// an AEAD encrypted data packet.
///
/// To change the maximum recursion depth, use
/// [`PacketParserBuilder::max_packet_size`].
///
///   [`PacketParserBuilder::max_packet_size`]: PacketParserBuilder::max_packet_size()
pub const DEFAULT_MAX_PACKET_SIZE: u32 = 1 << 20; // 1 MiB

// Used to parse an OpenPGP packet's header (note: in this case, the
// header means a Packet's fixed data, not the OpenPGP framing
// information, such as the CTB, and length information).
//
// This struct is not exposed to the user.  Instead, when a header has
// been successfully parsed, a `PacketParser` is returned.
pub(crate) struct PacketHeaderParser<T: BufferedReader<Cookie>> {
    // The reader stack wrapped in a buffered_reader::Dup so that if
    // there is a parse error, we can abort and still return an
    // Unknown packet.
    reader: buffered_reader::Dup<T, Cookie>,

    // The current packet's header.
    header: Header,
    header_bytes: Vec<u8>,

    // This packet's path.
    path: Vec<usize>,

    // The `PacketParser`'s state.
    state: PacketParserState,

    /// A map of this packet.
    map: Option<map::Map>,
}

/// Creates a local marco called php_try! that returns an Unknown
/// packet instead of an Error like try! on parsing-related errors.
/// (Errors like read errors are still returned as usual.)
///
/// If you want to fail like this in a non-try! context, use
/// php.fail("reason").
macro_rules! make_php_try {
    ($parser:expr) => {
        macro_rules! php_try {
            ($e:expr) => {
                match $e {
                    Ok(b) => {
                        Ok(b)
                    },
                    Err(e) => {
                        let e = match e.downcast::<io::Error>() {
                            Ok(e) =>
                                if let io::ErrorKind::UnexpectedEof = e.kind() {
                                    return $parser.error(e.into());
                                } else {
                                    e.into()
                                },
                            Err(e) => e,
                        };
                        let e = match e.downcast::<Error>() {
                            Ok(e) => return $parser.error(e.into()),
                            Err(e) => e,
                        };

                        Err(e)
                    },
                }?
            };
        }
    };
}

impl<T: BufferedReader<Cookie>> std::fmt::Debug for PacketHeaderParser<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("PacketHeaderParser")
            .field("header", &self.header)
            .field("path", &self.path)
            .field("reader", &self.reader)
            .field("state", &self.state)
            .field("map", &self.map)
            .finish()
    }
}

impl<'a, T: 'a + BufferedReader<Cookie>> PacketHeaderParser<T> {
    // Returns a `PacketHeaderParser` to parse an OpenPGP packet.
    // `inner` points to the start of the OpenPGP framing information,
    // i.e., the CTB.
    fn new(inner: T,
           state: PacketParserState,
           path: Vec<usize>, header: Header,
           header_bytes: Vec<u8>) -> Self
    {
        assert!(!path.is_empty());

        let cookie = Cookie {
            level: inner.cookie_ref().level,
            ..Default::default()
        };
        let map = if state.settings.map {
            Some(map::Map::new(header_bytes.clone()))
        } else {
            None
        };
        PacketHeaderParser {
            reader: buffered_reader::Dup::with_cookie(inner, cookie),
            header,
            header_bytes,
            path,
            state,
            map,
        }
    }

    // Returns a `PacketHeaderParser` that parses a bare packet.  That
    // is, `inner` points to the start of the packet; the OpenPGP
    // framing has already been processed, and `inner` already
    // includes any required filters (e.g., a
    // `BufferedReaderPartialBodyFilter`, etc.).
    fn new_naked(inner: T) -> Self {
        PacketHeaderParser::new(inner,
                                PacketParserState::new(Default::default()),
                                vec![ 0 ],
                                Header::new(CTB::new(Tag::Reserved),
                                            BodyLength::Full(0)),
                                Vec::new())
    }

    // Consumes the bytes belonging to the packet's header (i.e., the
    // number of bytes read) from the reader, and returns a
    // `PacketParser` that can be returned to the user.
    //
    // Only call this function if the packet's header has been
    // completely and correctly parsed.  If a failure occurs while
    // parsing the header, use `fail()` instead.
    fn ok(mut self, packet: Packet) -> Result<PacketParser<'a>> {
        let total_out = self.reader.total_out();

        let mut reader = if self.state.settings.map {
            // Read the body for the map.  Note that
            // `total_out` does not account for the body.
            //
            // XXX avoid the extra copy.
            let body = self.reader.steal_eof()?;
            if !body.is_empty() {
                self.field("body", body.len());
            }

            // This is a buffered_reader::Dup, so this always has an
            // inner.
            let inner = Box::new(self.reader).into_inner().unwrap();

            // Combine the header with the body for the map.
            let mut data = Vec::with_capacity(total_out + body.len());
            // We know that the inner reader must have at least
            // `total_out` bytes buffered, otherwise we could never
            // have read that much from the `buffered_reader::Dup`.
            data.extend_from_slice(&inner.buffer()[..total_out]);
            data.extend(body);
            self.map.as_mut().unwrap().finalize(data);

            inner
        } else {
            // This is a buffered_reader::Dup, so this always has an
            // inner.
            Box::new(self.reader).into_inner().unwrap()
        };

        if total_out > 0 {
            // We know the data has been read, so this cannot fail.
            reader.data_consume_hard(total_out).unwrap();
        }

        Ok(PacketParser {
            header: self.header,
            packet,
            path: self.path,
            last_path: vec![],
            reader,
            content_was_read: false,
            processed: true,
            finished: false,
            map: self.map,
            body_hash: Some(Container::make_body_hash()),
            state: self.state,
        })
    }

    // Something went wrong while parsing the packet's header.  Aborts
    // and returns an Unknown packet instead.
    fn fail(self, reason: &'static str) -> Result<PacketParser<'a>> {
        self.error(Error::MalformedPacket(reason.into()).into())
    }

    fn error(mut self, error: anyhow::Error) -> Result<PacketParser<'a>> {
        // Rewind the dup reader, so that the caller has a chance to
        // buffer the whole body of the unknown packet.
        self.reader.rewind();
        Unknown::parse(self, error)
    }

    fn field(&mut self, name: &'static str, size: usize) {
        if let Some(ref mut map) = self.map {
            map.add(name, size)
        }
    }

    fn parse_u8(&mut self, name: &'static str) -> Result<u8> {
        let r = self.reader.data_consume_hard(1)?[0];
        self.field(name, 1);
        Ok(r)
    }

    fn parse_be_u16(&mut self, name: &'static str) -> Result<u16> {
        let r = self.reader.read_be_u16()?;
        self.field(name, 2);
        Ok(r)
    }

    fn parse_be_u32(&mut self, name: &'static str) -> Result<u32> {
        let r = self.reader.read_be_u32()?;
        self.field(name, 4);
        Ok(r)
    }

    fn parse_bool(&mut self, name: &'static str) -> Result<bool> {
        let v = self.reader.data_consume_hard(1)?[0];
        self.field(name, 1);
        match v {
            0 => Ok(false),
            1 => Ok(true),
            n => Err(Error::MalformedPacket(
                format!("Invalid value for bool: {}", n)).into()),
        }
    }

    fn parse_bytes(&mut self, name: &'static str, amount: usize)
                   -> Result<Vec<u8>> {
        let r = self.reader.steal(amount)?;
        self.field(name, amount);
        Ok(r)
    }

    fn parse_bytes_eof(&mut self, name: &'static str) -> Result<Vec<u8>> {
        let r = self.reader.steal_eof()?;
        self.field(name, r.len());
        Ok(r)
    }

    fn recursion_depth(&self) -> isize {
        self.path.len() as isize - 1
    }
}


/// What the hash in the Cookie is for.
#[derive(Copy, Clone, PartialEq, Debug)]
#[allow(clippy::upper_case_acronyms)]
pub(crate) enum HashesFor {
    Nothing,
    MDC,
    Signature,
    CleartextSignature,
}

/// Controls whether or not a hashed reader hashes data.
#[derive(Copy, Clone, PartialEq, Debug)]
enum Hashing {
    /// Hashing is enabled.
    Enabled,
    /// Hashing is enabled for notarized signatures.
    Notarized,
    /// Hashing is disabled.
    Disabled,
}

/// Private state used by the `PacketParser`.
///
/// This is not intended to be used.  It is possible to explicitly
/// create `Cookie` instances using its `Default` implementation for
/// low-level interfacing with parsing code.
#[derive(Debug)]
pub struct Cookie {
    // `BufferedReader`s managed by a `PacketParser` have
    // `Some(level)`; an external `BufferedReader` (i.e., the
    // underlying `BufferedReader`) has no level.
    //
    // Before parsing a top-level packet, we may push a
    // `buffered_reader::Limitor` in front of the external
    // `BufferedReader`.  Such `BufferedReader`s are assigned a level
    // of 0.
    //
    // When a top-level packet (i.e., a packet with a recursion depth
    // of 0) reads from the `BufferedReader` stack, the top
    // `BufferedReader` will have a level of at most 0.
    //
    // If the top-level packet is a container, say, a `CompressedData`
    // packet, then it pushes a decompression filter with a level of 0
    // onto the `BufferedReader` stack, and it recursively invokes the
    // parser.
    //
    // When the parser encounters the `CompressedData`'s first child,
    // say, a `Literal` packet, it pushes a `buffered_reader::Limitor` on
    // the `BufferedReader` stack with a level of 1.  Then, a
    // `PacketParser` for the `Literal` data packet is created with a
    // recursion depth of 1.
    //
    // There are several things to note:
    //
    //   - When a `PacketParser` with a recursion depth of N reads
    //     from the `BufferedReader` stack, the top `BufferedReader`'s
    //     level is (at most) N.
    //
    //     - Because we sometimes don't need to push a limitor
    //       (specifically, when the length is indeterminate), the
    //       `BufferedReader` at the top of the stack may have a level
    //       less than the current `PacketParser`'s recursion depth.
    //
    //   - When a packet at depth N is a container that filters the
    //     data, it pushes a `BufferedReader` at level N onto the
    //     `BufferedReader` stack.
    //
    //   - When we finish parsing a packet at depth N, we pop all
    //     `BufferedReader`s from the `BufferedReader` stack that are
    //     at level N.  The intuition is: the `BufferedReaders` at
    //     level N are associated with the packet at depth N.
    //
    //   - If a OnePassSig packet occurs at the top level, then we
    //     need to push a HashedReader above the current level.  The
    //     top level is level 0, thus we push the HashedReader at
    //     level -1.
    level: Option<isize>,

    hashes_for: HashesFor,
    hashing: Hashing,

    /// Keeps track of whether the last one pass signature packet had
    /// the last flag set.
    saw_last: bool,
    sig_groups: Vec<SignatureGroup>,
    /// Keep track of the maximal size of sig_groups to compute
    /// signature levels.
    sig_groups_max_len: usize,

    /// Stashed bytes that need to be hashed.
    ///
    /// When checking nested signatures, we need to hash the framing.
    /// However, at the time we know that we want to hash it, it has
    /// already been consumed.  Deferring the consumption of headers
    /// failed due to complications with the partial body decoder
    /// eagerly consuming data.  I (Justus) decided that doing the
    /// right thing is not worth the trouble, at least for now.  Also,
    /// hash stash sounds funny.
    hash_stash: Option<Vec<u8>>,

    /// Whether this `BufferedReader` is actually an interior EOF in a
    /// container.
    ///
    /// This is used by the SEIP parser to prevent a child packet from
    /// accidentally swallowing the trailing MDC packet.  This can
    /// happen when there is a compressed data packet with an
    /// indeterminate body length encoding.  In this case, due to
    /// buffering, the decompressor consumes data beyond the end of
    /// the compressed data.
    ///
    /// When set, buffered_reader_stack_pop will return early when it
    /// encounters a fake EOF at the level it is popping to.
    fake_eof: bool,

    /// Indicates that this is the top-level armor reader that is
    /// doing a transformation of a message using the cleartext
    /// signature framework into a signed message.
    csf_transformation: bool,
}
assert_send_and_sync!(Cookie);

/// Contains hashes for consecutive one pass signature packets ending
/// in one with the last flag set.
#[derive(Default)]
pub(crate) struct SignatureGroup {
    /// Counts the number of one pass signature packets this group is
    /// for.  Once this drops to zero, we pop the group from the
    /// stack.
    ops_count: usize,

    /// The hash contexts.
    pub(crate) hashes: Vec<HashingMode<Box<dyn crypto::hash::Digest>>>,
}

impl fmt::Debug for SignatureGroup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let algos = self.hashes.iter().map(|mode| mode.map(|ctx| ctx.algo()))
            .collect::<Vec<_>>();

        f.debug_struct("Cookie")
            .field("ops_count", &self.ops_count)
            .field("hashes", &algos)
            .finish()
    }
}

impl SignatureGroup {
    /// Clears the signature group.
    fn clear(&mut self) {
        self.ops_count = 0;
        self.hashes.clear();
    }
}

impl Default for Cookie {
    fn default() -> Self {
        Cookie {
            level: None,
            hashing: Hashing::Enabled,
            hashes_for: HashesFor::Nothing,
            saw_last: false,
            sig_groups: vec![Default::default()],
            sig_groups_max_len: 1,
            hash_stash: None,
            fake_eof: false,
            csf_transformation: false,
        }
    }
}

impl Cookie {
    fn new(level: isize) -> Cookie {
        Cookie {
            level: Some(level),
            hashing: Hashing::Enabled,
            hashes_for: HashesFor::Nothing,
            saw_last: false,
            sig_groups: vec![Default::default()],
            sig_groups_max_len: 1,
            hash_stash: None,
            fake_eof: false,
            csf_transformation: false,
        }
    }

    /// Returns a reference to the topmost signature group.
    pub(crate) fn sig_group(&self) -> &SignatureGroup {
        assert!(!self.sig_groups.is_empty());
        &self.sig_groups[self.sig_groups.len() - 1]
    }

    /// Returns a mutable reference to the topmost signature group.
    pub(crate) fn sig_group_mut(&mut self) -> &mut SignatureGroup {
        assert!(!self.sig_groups.is_empty());
        let len = self.sig_groups.len();
        &mut self.sig_groups[len - 1]
    }

    /// Returns the level of the currently parsed signature.
    fn signature_level(&self) -> usize {
        // The signature with the deepest "nesting" is closest to the
        // data, and hence level 0.
        self.sig_groups_max_len - self.sig_groups.len()
    }

    /// Tests whether the topmost signature group is no longer used.
    fn sig_group_unused(&self) -> bool {
        assert!(!self.sig_groups.is_empty());
        self.sig_groups[self.sig_groups.len() - 1].ops_count == 0
    }

    /// Pushes a new signature group to the stack.
    fn sig_group_push(&mut self) {
        self.sig_groups.push(Default::default());
        self.sig_groups_max_len += 1;
    }

    /// Pops a signature group from the stack.
    fn sig_group_pop(&mut self) {
        if self.sig_groups.len() == 1 {
            // Don't pop the last one, just clear it.
            self.sig_groups[0].clear();
            self.hashes_for = HashesFor::Nothing;
        } else {
            self.sig_groups.pop();
        }
    }
}

impl Cookie {
    // Enables or disables signature hashers (HashesFor::Signature) at
    // level `level`.
    //
    // Thus to disable the hashing of a level 3 literal packet's
    // meta-data, we disable hashing at level 2.
    fn hashing(reader: &mut dyn BufferedReader<Cookie>,
               how: Hashing, level: isize) {
        let mut reader : Option<&mut dyn BufferedReader<Cookie>>
            = Some(reader);
        while let Some(r) = reader {
            {
                let cookie = r.cookie_mut();
                if let Some(br_level) = cookie.level {
                    if br_level < level {
                        break;
                    }
                    if br_level == level
                        && (cookie.hashes_for == HashesFor::Signature
                            || cookie.hashes_for == HashesFor::CleartextSignature)
                    {
                        cookie.hashing = how;
                    }
                } else {
                    break;
                }
            }
            reader = r.get_mut();
        }
    }

    /// Signals that we are processing a message using the Cleartext
    /// Signature Framework.
    ///
    /// This is used by the armor reader to signal that it has
    /// encountered such a message and is transforming it into an
    /// inline signed message.
    pub(crate) fn set_processing_csf_message(&mut self) {
        tracer!(TRACE, "set_processing_csf_message", self.level.unwrap_or(0));
        t!("Enabling CSF Transformation mode");
        self.csf_transformation = true;
    }

    /// Checks if we are processing a signed message using the
    /// Cleartext Signature Framework.
    fn processing_csf_message(reader: &dyn BufferedReader<Cookie>)
                              -> bool {
        let mut reader: Option<&dyn BufferedReader<Cookie>>
            = Some(reader);
        while let Some(r) = reader {
            if r.cookie_ref().level == Some(ARMOR_READER_LEVEL) {
                return r.cookie_ref().csf_transformation;
            } else {
                reader = r.get_ref();
            }
        }
        false
    }
}

// Pops readers from a buffered reader stack at the specified level.
fn buffered_reader_stack_pop<'a>(
    mut reader: Box<dyn BufferedReader<Cookie> + 'a>, depth: isize)
    -> Result<(bool, Box<dyn BufferedReader<Cookie> + 'a>)>
{
    tracer!(TRACE, "buffered_reader_stack_pop", depth);
    t!("(reader level: {:?}, pop through: {})",
       reader.cookie_ref().level, depth);

    while let Some(level) = reader.cookie_ref().level {
        assert!(level <= depth // Peel off exactly one level.
                || depth < 0); // Except for the topmost filters.

        if level >= depth {
            let fake_eof = reader.cookie_ref().fake_eof;

            t!("top reader at level {:?} (fake eof: {}), pop through: {}",
               reader.cookie_ref().level, fake_eof, depth);

            t!("popping level {:?} reader, reader: {:?}",
               reader.cookie_ref().level,
               reader);

            if reader.eof() && ! reader.consummated() {
                return Err(Error::MalformedPacket("Truncated packet".into())
                           .into());
            }
            reader.drop_eof()?;
            reader = reader.into_inner().unwrap();

            if level == depth && fake_eof {
                t!("Popped a fake EOF reader at level {}, stopping.", depth);
                return Ok((true, reader));
            }

            t!("now at level {:?} reader: {:?}",
               reader.cookie_ref().level, reader);
        } else {
            break;
        }
    }

    Ok((false, reader))
}


// A `PacketParser`'s settings.
#[derive(Clone, Debug)]
struct PacketParserSettings {
    // The maximum allowed recursion depth.
    //
    // There is absolutely no reason that this should be more than
    // 255.  (GnuPG defaults to 32.)  Moreover, if it is too large,
    // then a read from the reader pipeline could blow the stack.
    max_recursion_depth: u8,

    // The maximum size of non-container packets.
    //
    // Packets that exceed this limit will be returned as
    // `Packet::Unknown`, with the error set to
    // `Error::PacketTooLarge`.
    //
    // This limit applies to any packet type that is *not* a
    // container packet, i.e. any packet that is not a literal data
    // packet, a compressed data packet, a symmetrically encrypted
    // data packet, or an AEAD encrypted data packet.
    max_packet_size: u32,

    // Whether a packet's contents should be buffered or dropped when
    // the next packet is retrieved.
    buffer_unread_content: bool,

    // Whether or not to create a map.
    map: bool,
}

// The default `PacketParser` settings.
impl Default for PacketParserSettings {
    fn default() -> Self {
        PacketParserSettings {
            max_recursion_depth: DEFAULT_MAX_RECURSION_DEPTH,
            max_packet_size: DEFAULT_MAX_PACKET_SIZE,
            buffer_unread_content: false,
            map: false,
        }
    }
}

impl S2K {
    /// Reads an S2K from `php`.
    fn parse_v4<T: BufferedReader<Cookie>>(php: &mut PacketHeaderParser<T>)
                                           -> Result<Self> {
        Self::parse_common(php, None)
    }

    /// Reads an S2K from `php` with optional explicit S2K length.
    fn parse_common<T: BufferedReader<Cookie>>(php: &mut PacketHeaderParser<T>,
                                               s2k_len: Option<u8>)
                                               -> Result<Self>
    {
        if s2k_len == Some(0) {
            return Err(Error::MalformedPacket(
                "Invalid size for S2K object: 0 octets".into()).into());
        }

        let check_size = |expected| {
            if let Some(got) = s2k_len {
                if got != expected {
                    return Err(Error::MalformedPacket(format!(
                        "Invalid size for S2K object: {} octets, expected {}",
                        got, expected)));
                }
            }
            Ok(())
        };

        let s2k = php.parse_u8("s2k_type")?;
        #[allow(deprecated)]
        let ret = match s2k {
            0 => {
                check_size(2)?;
                S2K::Simple {
                    hash: HashAlgorithm::from(php.parse_u8("s2k_hash_algo")?),
                }
            },
            1 => {
                check_size(10)?;
                S2K::Salted {
                    hash: HashAlgorithm::from(php.parse_u8("s2k_hash_algo")?),
                    salt: Self::read_salt(php)?,
                }
            },
            3 => {
                check_size(11)?;
                S2K::Iterated {
                    hash: HashAlgorithm::from(php.parse_u8("s2k_hash_algo")?),
                    salt: Self::read_salt(php)?,
                    hash_bytes: S2K::decode_count(php.parse_u8("s2k_count")?),
                }
            },
            100..=110 => S2K::Private {
                tag: s2k,
                parameters: if let Some(l) = s2k_len {
                    Some(
                        php.parse_bytes("parameters", l as usize - 1 /* Tag */)?
                            .into())
                } else {
                    None
                },
            },
            u => S2K::Unknown {
                tag: u,
                parameters: if let Some(l) = s2k_len {
                    Some(
                        php.parse_bytes("parameters", l as usize - 1 /* Tag */)?
                            .into())
                } else {
                    None
                },
            },
        };

        Ok(ret)
    }

    fn read_salt<'a, T: 'a + BufferedReader<Cookie>>(php: &mut PacketHeaderParser<T>) -> Result<[u8; 8]> {
        let mut b = [0u8; 8];
        b.copy_from_slice(&php.parse_bytes("s2k_salt", 8)?);

        Ok(b)
    }
}

impl<'a> Parse<'a, S2K> for S2K {
    /// Reads an S2K from `reader`.
    fn from_reader<R: 'a + Read + Send + Sync>(reader: R) -> Result<Self> {
        let bio = buffered_reader::Generic::with_cookie(
            reader, None, Cookie::default());
        let mut parser = PacketHeaderParser::new_naked(bio);
        Self::parse_v4(&mut parser)
    }
}

impl Header {
    pub(crate) fn parse<R: BufferedReader<C>, C: fmt::Debug + Send + Sync> (bio: &mut R)
        -> Result<Header>
    {
        let ctb = CTB::try_from(bio.data_consume_hard(1)?[0])?;
        let length = match ctb {
            CTB::New(_) => BodyLength::parse_new_format(bio)?,
            CTB::Old(ref ctb) =>
                BodyLength::parse_old_format(bio, ctb.length_type())?,
        };
        Ok(Header::new(ctb, length))
    }
}

impl<'a> Parse<'a, Header> for Header {
    /// Parses an OpenPGP packet's header as described in [Section 4.2
    /// of RFC 4880].
    ///
    ///   [Section 4.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2
    fn from_reader<R: 'a + Read + Send + Sync>(reader: R) -> Result<Self>
    {
        let mut reader = buffered_reader::Generic::with_cookie(
            reader, None, Cookie::default());
        Header::parse(&mut reader)
    }
}

impl BodyLength {
    /// Decodes a new format body length as described in [Section
    /// 4.2.2 of RFC 4880].
    ///
    ///   [Section 4.2.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2.2
    pub(crate) fn parse_new_format<T: BufferedReader<C>, C: fmt::Debug + Send + Sync> (bio: &mut T)
        -> io::Result<BodyLength>
    {
        let octet1 : u8 = bio.data_consume_hard(1)?[0];
        match octet1 {
            0..=191 => // One octet.
                Ok(BodyLength::Full(octet1 as u32)),
            192..=223 => { // Two octets length.
                let octet2 = bio.data_consume_hard(1)?[0];
                Ok(BodyLength::Full(((octet1 as u32 - 192) << 8)
                                    + octet2 as u32 + 192))
            },
            224..=254 => // Partial body length.
                Ok(BodyLength::Partial(1 << (octet1 & 0x1F))),
            255 => // Five octets.
                Ok(BodyLength::Full(bio.read_be_u32()?)),
        }
    }

    /// Decodes an old format body length as described in [Section
    /// 4.2.1 of RFC 4880].
    ///
    ///   [Section 4.2.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2.1
    pub(crate) fn parse_old_format<T: BufferedReader<C>, C: fmt::Debug + Send + Sync>
        (bio: &mut T, length_type: PacketLengthType)
         -> Result<BodyLength>
    {
        match length_type {
            PacketLengthType::OneOctet =>
                Ok(BodyLength::Full(bio.data_consume_hard(1)?[0] as u32)),
            PacketLengthType::TwoOctets =>
                Ok(BodyLength::Full(bio.read_be_u16()? as u32)),
            PacketLengthType::FourOctets =>
                Ok(BodyLength::Full(bio.read_be_u32()? as u32)),
            PacketLengthType::Indeterminate =>
                Ok(BodyLength::Indeterminate),
        }
    }
}

#[test]
fn body_length_new_format() {
    fn test(input: &[u8], expected_result: BodyLength) {
        assert_eq!(
            BodyLength::parse_new_format(
                &mut buffered_reader::Memory::new(input)).unwrap(),
            expected_result);
    }

    // Examples from Section 4.2.3 of RFC4880.

    // Example #1.
    test(&[0x64][..], BodyLength::Full(100));

    // Example #2.
    test(&[0xC5, 0xFB][..], BodyLength::Full(1723));

    // Example #3.
    test(&[0xFF, 0x00, 0x01, 0x86, 0xA0][..], BodyLength::Full(100000));

    // Example #4.
    test(&[0xEF][..], BodyLength::Partial(32768));
    test(&[0xE1][..], BodyLength::Partial(2));
    test(&[0xF0][..], BodyLength::Partial(65536));
    test(&[0xC5, 0xDD][..], BodyLength::Full(1693));
}

#[test]
fn body_length_old_format() {
    fn test(input: &[u8], plt: PacketLengthType,
            expected_result: BodyLength, expected_rest: &[u8]) {
        let mut bio = buffered_reader::Memory::new(input);
        assert_eq!(BodyLength::parse_old_format(&mut bio, plt).unwrap(),
                   expected_result);
        let rest = bio.data_eof();
        assert_eq!(rest.unwrap(), expected_rest);
    }

    test(&[1], PacketLengthType::OneOctet, BodyLength::Full(1), &b""[..]);
    test(&[1, 2], PacketLengthType::TwoOctets,
         BodyLength::Full((1 << 8) + 2), &b""[..]);
    test(&[1, 2, 3, 4], PacketLengthType::FourOctets,
         BodyLength::Full((1 << 24) + (2 << 16) + (3 << 8) + 4), &b""[..]);
    test(&[1, 2, 3, 4, 5, 6], PacketLengthType::FourOctets,
         BodyLength::Full((1 << 24) + (2 << 16) + (3 << 8) + 4), &[5, 6][..]);
    test(&[1, 2, 3, 4], PacketLengthType::Indeterminate,
         BodyLength::Indeterminate, &[1, 2, 3, 4][..]);
}

impl Unknown {
    /// Parses the body of any packet and returns an Unknown.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(php: PacketHeaderParser<T>, error: anyhow::Error)
                 -> Result<PacketParser<'a>>
    {
        let tag = php.header.ctb().tag();
        php.ok(Packet::Unknown(Unknown::new(tag, error)))
    }
}

// Read the next packet as an unknown packet.
//
// The `reader` must point to the packet's header, i.e., the CTB.
// This buffers the packet's contents.
//
// Note: we only need this function for testing purposes in a
// different module.
#[cfg(test)]
pub(crate) fn to_unknown_packet<R: Read + Send + Sync>(reader: R) -> Result<Unknown>
{
    let mut reader = buffered_reader::Generic::with_cookie(
        reader, None, Cookie::default());
    let header = Header::parse(&mut reader)?;

    let reader : Box<dyn BufferedReader<Cookie>>
        = match header.length() {
            &BodyLength::Full(len) =>
                Box::new(buffered_reader::Limitor::with_cookie(
                    reader, len as u64, Cookie::default())),
            &BodyLength::Partial(len) =>
                Box::new(BufferedReaderPartialBodyFilter::with_cookie(
                    reader, len, true, Cookie::default())),
            _ => Box::new(reader),
    };

    let parser = PacketHeaderParser::new(
        reader, PacketParserState::new(Default::default()), vec![ 0 ], header, Vec::new());
    let mut pp =
        Unknown::parse(parser,
                       anyhow::anyhow!("explicit conversion to unknown"))?;
    pp.buffer_unread_content()?;
    pp.finish()?;

    if let Packet::Unknown(packet) = pp.packet {
        Ok(packet)
    } else {
        panic!("Internal inconsistency.");
    }
}

impl Signature {
    // Parses a signature packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>)
        -> Result<PacketParser<'a>>
    {
        let indent = php.recursion_depth();
        tracer!(TRACE, "Signature::parse", indent);

        make_php_try!(php);

        let version = php_try!(php.parse_u8("version"));

        match version {
            3 => Signature3::parse(php),
            4 => Signature4::parse(php),
            _ => {
                t!("Ignoring version {} packet.", version);
                php.fail("unknown version")
            },
        }
    }

    /// Returns whether the data appears to be a signature (no promises).
    fn plausible<T: BufferedReader<Cookie>>(
        bio: &mut buffered_reader::Dup<T, Cookie>, header: &Header)
                 -> Result<()> {
        Signature4::plausible(bio, header)
    }

    fn parse_finish(indent: isize, mut pp: PacketParser,
                    typ: SignatureType, hash_algo: HashAlgorithm)
        -> Result<PacketParser>
    {
        tracer!(TRACE, "Signature::parse_finish", indent);

        let need_hash = HashingMode::for_signature(hash_algo, typ);

        // Locate the corresponding HashedReader and extract the
        // computed hash.
        let mut computed_digest = None;
        {
            let recursion_depth = pp.recursion_depth();

            // We know that the top reader is not a HashedReader (it's
            // a buffered_reader::Dup).  So, start with it's child.
            let mut r = (&mut pp.reader).get_mut();
            while let Some(tmp) = r {
                {
                    let cookie = tmp.cookie_mut();

                    assert!(cookie.level.unwrap_or(-1)
                            <= recursion_depth);
                    // The HashedReader has to be at level
                    // 'recursion_depth - 1'.
                    if cookie.level.is_none()
                        || cookie.level.unwrap() < recursion_depth - 1 {
                            break
                        }

                    if cookie.hashes_for == HashesFor::Signature {
                        // When verifying cleartext signed messages,
                        // we may have more signatures than
                        // one-pass-signature packets, but are
                        // guaranteed to only have one signature
                        // group.
                        //
                        // Only decrement the count when hashing for
                        // signatures, not when hashing for cleartext
                        // signatures.
                        cookie.sig_group_mut().ops_count -= 1;
                    }

                    if cookie.hashes_for == HashesFor::Signature
                        || cookie.hashes_for == HashesFor::CleartextSignature
                    {
                        if let Some(hash) =
                            cookie.sig_group().hashes.iter().find_map(
                                |mode|
                                if mode.map(|ctx| ctx.algo()) == need_hash {
                                    Some(mode.as_ref())
                                } else {
                                    None
                                })
                        {
                            t!("found a {:?} HashedReader", need_hash);
                            computed_digest = Some((cookie.signature_level(),
                                                    hash.clone()));
                        }

                        if cookie.sig_group_unused() {
                            cookie.sig_group_pop();
                        }
                        break;
                    }
                }

                r = tmp.get_mut();
            }
        }

        if let Some((level, mut hash)) = computed_digest {
            if let Packet::Signature(ref mut sig) = pp.packet {
                sig.hash(&mut hash);

                let mut digest = vec![0u8; hash.digest_size()];
                let _ = hash.digest(&mut digest);

                sig.set_computed_digest(Some(digest));
                sig.set_level(level);
            } else {
                unreachable!()
            }
        }

        Ok(pp)
    }
}

impl Signature4 {
    // Parses a signature packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>)
        -> Result<PacketParser<'a>>
    {
        let indent = php.recursion_depth();
        tracer!(TRACE, "Signature4::parse", indent);

        make_php_try!(php);

        let typ = php_try!(php.parse_u8("type"));
        let pk_algo: PublicKeyAlgorithm = php_try!(php.parse_u8("pk_algo")).into();
        let hash_algo: HashAlgorithm =
            php_try!(php.parse_u8("hash_algo")).into();
        let hashed_area_len = php_try!(php.parse_be_u16("hashed_area_len"));
        let hashed_area
            = php_try!(SubpacketArea::parse(&mut php,
                                            hashed_area_len as usize,
                                            hash_algo));
        let unhashed_area_len = php_try!(php.parse_be_u16("unhashed_area_len"));
        let unhashed_area
            = php_try!(SubpacketArea::parse(&mut php,
                                            unhashed_area_len as usize,
                                            hash_algo));
        let digest_prefix1 = php_try!(php.parse_u8("digest_prefix1"));
        let digest_prefix2 = php_try!(php.parse_u8("digest_prefix2"));
        if ! pk_algo.for_signing() {
            return php.fail("not a signature algorithm");
        }
        let mpis = php_try!(
            crypto::mpi::Signature::_parse(pk_algo, &mut php));

        let typ = typ.into();
        let pp = php.ok(Packet::Signature(Signature4::new(
            typ, pk_algo, hash_algo,
            hashed_area,
            unhashed_area,
            [digest_prefix1, digest_prefix2],
            mpis).into()))?;

        Signature::parse_finish(indent, pp, typ, hash_algo)
    }

    /// Returns whether the data appears to be a signature (no promises).
    fn plausible<T: BufferedReader<Cookie>>(
        bio: &mut buffered_reader::Dup<T, Cookie>, header: &Header)
                 -> Result<()> {
        // The absolute minimum size for the header is 11 bytes (this
        // doesn't include the signature MPIs).

        if let BodyLength::Full(len) = header.length() {
            if *len < 11 {
                // Much too short.
                return Err(
                    Error::MalformedPacket("Packet too short".into()).into());
            }
        } else {
            return Err(
                Error::MalformedPacket(
                    format!("Unexpected body length encoding: {:?}",
                            header.length())).into());
        }

        // Make sure we have a minimum header.
        let data = bio.data(11)?;
        if data.len() < 11 {
            return Err(
                Error::MalformedPacket("Short read".into()).into());
        }

        // Assume unknown == bad.
        let version = data[0];
        let typ : SignatureType = data[1].into();
        let pk_algo : PublicKeyAlgorithm = data[2].into();
        let hash_algo : HashAlgorithm = data[3].into();

        if version == 4
            && !matches!(typ, SignatureType::Unknown(_))
            && !matches!(pk_algo, PublicKeyAlgorithm::Unknown(_))
            && !matches!(hash_algo, HashAlgorithm::Unknown(_))
        {
            Ok(())
        } else {
            Err(Error::MalformedPacket("Invalid or unsupported data".into())
                .into())
        }
    }
}

impl Signature3 {
    // Parses a v3 signature packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>)
        -> Result<PacketParser<'a>>
    {
        let indent = php.recursion_depth();
        tracer!(TRACE, "Signature3::parse", indent);

        make_php_try!(php);

        let len = php_try!(php.parse_u8("hashed length"));
        if len != 5 {
            return php.fail("invalid length \
                             (a v3 sig has 5 bytes of hashed data)");
        }
        let typ = php_try!(php.parse_u8("type"));
        let creation_time: Timestamp
            = php_try!(php.parse_be_u32("creation_time")).into();
        let issuer: KeyID
            = KeyID::from_bytes(&php_try!(php.parse_bytes("issuer", 8))[..]);
        let pk_algo: PublicKeyAlgorithm
            = php_try!(php.parse_u8("pk_algo")).into();
        let hash_algo: HashAlgorithm =
            php_try!(php.parse_u8("hash_algo")).into();
        let digest_prefix1 = php_try!(php.parse_u8("digest_prefix1"));
        let digest_prefix2 = php_try!(php.parse_u8("digest_prefix2"));
        if ! pk_algo.for_signing() {
            return php.fail("not a signature algorithm");
        }
        let mpis = php_try!(
            crypto::mpi::Signature::_parse(pk_algo, &mut php));

        let typ = typ.into();
        let pp = php.ok(Packet::Signature(Signature3::new(
            typ, creation_time, issuer, pk_algo, hash_algo,
            [digest_prefix1, digest_prefix2],
            mpis).into()))?;

        Signature::parse_finish(indent, pp, typ, hash_algo)
    }
}

impl_parse_generic_packet!(Signature);

#[test]
fn signature_parser_test () {
    use crate::serialize::MarshalInto;
    let data = crate::tests::message("sig.gpg");

    {
        let pp = PacketParser::from_bytes(data).unwrap().unwrap();
        assert_eq!(pp.header.length(), &BodyLength::Full(307));
        if let Packet::Signature(ref p) = pp.packet {
            assert_eq!(p.version(), 4);
            assert_eq!(p.typ(), SignatureType::Binary);
            assert_eq!(p.pk_algo(), PublicKeyAlgorithm::RSAEncryptSign);
            assert_eq!(p.hash_algo(), HashAlgorithm::SHA512);
            assert_eq!(p.hashed_area().iter().count(), 2);
            assert_eq!(p.unhashed_area().iter().count(), 1);
            assert_eq!(p.digest_prefix(), &[0x65u8, 0x74]);
            assert_eq!(p.mpis().serialized_len(), 258);
        } else {
            panic!("Wrong packet!");
        }
    }
}

impl SubpacketArea {
    // Parses a subpacket area.
    fn parse<'a, T>(php: &mut PacketHeaderParser<T>,
                    mut limit: usize,
                    hash_algo: HashAlgorithm)
                    -> Result<Self>
    where T: 'a + BufferedReader<Cookie>,
    {
        let indent = php.recursion_depth();
        tracer!(TRACE, "SubpacketArea::parse", indent);

        let mut packets = Vec::new();
        while limit > 0 {
            let r = Subpacket::parse(php, limit, hash_algo);
            t!("Subpacket::parse(_, {}, {:?}) => {:?}",
               limit, hash_algo, r);
            let p = r?;
            assert!(limit >= p.length.len() + p.length.serialized_len());
            limit -= p.length.len() + p.length.serialized_len();
            packets.push(p);
        }
        assert!(limit == 0);
        Self::new(packets)
    }
}

impl Subpacket {
    // Parses a raw subpacket.
    fn parse<'a, T>(php: &mut PacketHeaderParser<T>,
                    limit: usize,
                    hash_algo: HashAlgorithm)
                    -> Result<Self>
    where T: 'a + BufferedReader<Cookie>,
    {
        let length = SubpacketLength::parse(&mut php.reader)?;
        php.field("subpacket length", length.serialized_len());
        let len = length.len() as usize;

        if limit < length.serialized_len() + len {
            return Err(Error::MalformedPacket(
                "Subpacket extends beyond the end of the subpacket area".into())
                       .into());
        }

        if len == 0 {
            return Err(Error::MalformedPacket("Zero-length subpacket".into())
                       .into());
        }

        let tag = php.parse_u8("subpacket tag")?;
        let len = len - 1;

        // Remember our position in the reader to check subpacket boundaries.
        let total_out_before = php.reader.total_out();

        // The critical bit is the high bit.  Extract it.
        let critical = tag & (1 << 7) != 0;
        // Then clear it from the type and convert it.
        let tag: SubpacketTag = (tag & !(1 << 7)).into();

        let value = match tag {
            SubpacketTag::SignatureCreationTime =>
                SubpacketValue::SignatureCreationTime(
                    php.parse_be_u32("sig creation time")?.into()),
            SubpacketTag::SignatureExpirationTime =>
                SubpacketValue::SignatureExpirationTime(
                    php.parse_be_u32("sig expiry time")?.into()),
            SubpacketTag::ExportableCertification =>
                SubpacketValue::ExportableCertification(
                    php.parse_bool("exportable")?),
            SubpacketTag::TrustSignature =>
                SubpacketValue::TrustSignature {
                    level: php.parse_u8("trust level")?,
                    trust: php.parse_u8("trust value")?,
                },
            SubpacketTag::RegularExpression => {
                let mut v = php.parse_bytes("regular expr", len)?;
                if v.is_empty() || v[v.len() - 1] != 0 {
                    return Err(Error::MalformedPacket(
                        "Regular expression not 0-terminated".into())
                               .into());
                }
                v.pop();
                SubpacketValue::RegularExpression(v)
            },
            SubpacketTag::Revocable =>
                SubpacketValue::Revocable(php.parse_bool("revocable")?),
            SubpacketTag::KeyExpirationTime =>
                SubpacketValue::KeyExpirationTime(
                    php.parse_be_u32("key expiry time")?.into()),
            SubpacketTag::PreferredSymmetricAlgorithms =>
                SubpacketValue::PreferredSymmetricAlgorithms(
                    php.parse_bytes("pref sym algos", len)?
                        .iter().map(|o| (*o).into()).collect()),
            SubpacketTag::RevocationKey => {
                // 1 octet of class, 1 octet of pk algorithm, 20 bytes
                // for a v4 fingerprint and 32 bytes for a v5
                // fingerprint.
                if len < 22 {
                    return Err(Error::MalformedPacket(
                        "Short revocation key subpacket".into())
                               .into());
                }
                let class = php.parse_u8("class")?;
                let pk_algo = php.parse_u8("pk algo")?.into();
                let fp = Fingerprint::from_bytes(
                    &php.parse_bytes("fingerprint", len - 2)?);
                SubpacketValue::RevocationKey(
                    RevocationKey::from_bits(pk_algo, fp, class)?)
            },
            SubpacketTag::Issuer =>
                SubpacketValue::Issuer(
                    KeyID::from_bytes(&php.parse_bytes("issuer", len)?)),
            SubpacketTag::NotationData => {
                let flags = php.parse_bytes("flags", 4)?;
                let name_len = php.parse_be_u16("name len")? as usize;
                let value_len = php.parse_be_u16("value len")? as usize;

                if len != 8 + name_len + value_len {
                    return Err(Error::MalformedPacket(
                        format!("Malformed notation data subpacket: \
                                 expected {} bytes, got {}",
                                8 + name_len + value_len,
                                len)).into());
                }
                SubpacketValue::NotationData(
                    NotationData::new(
                        std::str::from_utf8(
                            &php.parse_bytes("notation name", name_len)?)
                            .map_err(|e| anyhow::Error::from(
                                Error::MalformedPacket(
                                    format!("Malformed notation name: {}", e)))
                            )?,
                        &php.parse_bytes("notation value", value_len)?,
                        Some(NotationDataFlags::new(&flags)?)))
            },
            SubpacketTag::PreferredHashAlgorithms =>
                SubpacketValue::PreferredHashAlgorithms(
                    php.parse_bytes("pref hash algos", len)?
                        .iter().map(|o| (*o).into()).collect()),
            SubpacketTag::PreferredCompressionAlgorithms =>
                SubpacketValue::PreferredCompressionAlgorithms(
                    php.parse_bytes("pref compression algos", len)?
                        .iter().map(|o| (*o).into()).collect()),
            SubpacketTag::KeyServerPreferences =>
                SubpacketValue::KeyServerPreferences(
                    KeyServerPreferences::new(
                        &php.parse_bytes("key server pref", len)?
                    )),
            SubpacketTag::PreferredKeyServer =>
                SubpacketValue::PreferredKeyServer(
                    php.parse_bytes("pref key server", len)?),
            SubpacketTag::PrimaryUserID =>
                SubpacketValue::PrimaryUserID(
                    php.parse_bool("primary user id")?),
            SubpacketTag::PolicyURI =>
                SubpacketValue::PolicyURI(php.parse_bytes("policy URI", len)?),
            SubpacketTag::KeyFlags =>
                SubpacketValue::KeyFlags(KeyFlags::new(
                    &php.parse_bytes("key flags", len)?)),
            SubpacketTag::SignersUserID =>
                SubpacketValue::SignersUserID(
                    php.parse_bytes("signers user id", len)?),
            SubpacketTag::ReasonForRevocation => {
                if len == 0 {
                    return Err(Error::MalformedPacket(
                        "Short reason for revocation subpacket".into()).into());
                }
                SubpacketValue::ReasonForRevocation {
                    code: php.parse_u8("revocation reason")?.into(),
                    reason: php.parse_bytes("human-readable", len - 1)?,
                }
            },
            SubpacketTag::Features =>
                SubpacketValue::Features(Features::new(
                    &php.parse_bytes("features", len)?)),
            SubpacketTag::SignatureTarget => {
                if len < 2 {
                    return Err(Error::MalformedPacket(
                        "Short reason for revocation subpacket".into()).into());
                }
                SubpacketValue::SignatureTarget {
                    pk_algo: php.parse_u8("pk algo")?.into(),
                    hash_algo: php.parse_u8("hash algo")?.into(),
                    digest: php.parse_bytes("digest", len - 2)?,
                }
            },
            SubpacketTag::EmbeddedSignature =>
                SubpacketValue::EmbeddedSignature(
                    Signature::from_bytes(
                        &php.parse_bytes("embedded sig", len)?)?),
            SubpacketTag::IssuerFingerprint => {
                if len == 0 {
                    return Err(Error::MalformedPacket(
                        "Short issuer fingerprint subpacket".into()).into());
                }
                let version = php.parse_u8("version")?;
                if let Some(expect_len) = match version {
                    4 => Some(1 + 20),
                    5 => Some(1 + 32),
                    _ => None,
                } {
                    if len != expect_len {
                        return Err(Error::MalformedPacket(
                            format!("Malformed issuer fingerprint subpacket: \
                                     expected {} bytes, got {}",
                                    expect_len, len)).into());
                    }
                }
                let bytes = php.parse_bytes("issuer fp", len - 1)?;
                SubpacketValue::IssuerFingerprint(
                    Fingerprint::from_bytes(&bytes))
            },
            SubpacketTag::PreferredAEADAlgorithms =>
                SubpacketValue::PreferredAEADAlgorithms(
                    php.parse_bytes("pref aead algos", len)?
                        .iter().map(|o| (*o).into()).collect()),
            SubpacketTag::IntendedRecipient => {
                if len == 0 {
                    return Err(Error::MalformedPacket(
                        "Short intended recipient subpacket".into()).into());
                }
                let version = php.parse_u8("version")?;
                if let Some(expect_len) = match version {
                    4 => Some(1 + 20),
                    5 => Some(1 + 32),
                    _ => None,
                } {
                    if len != expect_len {
                        return Err(Error::MalformedPacket(
                            format!("Malformed intended recipient subpacket: \
                                     expected {} bytes, got {}",
                                    expect_len, len)).into());
                    }
                }
                let bytes = php.parse_bytes("intended rcpt", len - 1)?;
                SubpacketValue::IntendedRecipient(
                    Fingerprint::from_bytes(&bytes))
            },
            SubpacketTag::AttestedCertifications => {
                // If we don't know the hash algorithm, put all digest
                // into one bucket.  That way, at least it will
                // roundtrip.  It will never verify, because we don't
                // know the hash.
                let digest_size =
                    hash_algo.context().map(|c| c.digest_size())
                    .unwrap_or(len);

                if digest_size == 0 {
                    // Empty body with unknown hash algorithm.
                    SubpacketValue::AttestedCertifications(
                        Vec::with_capacity(0))
                } else {
                    if len % digest_size != 0 {
                        return Err(Error::BadSignature(
                            "Wrong number of bytes in certification subpacket"
                                .into()).into());
                    }
                    let bytes = php.parse_bytes("attested crts", len)?;
                    SubpacketValue::AttestedCertifications(
                        bytes.chunks(digest_size).map(Into::into).collect())
                }
            },
            SubpacketTag::Reserved(_)
                | SubpacketTag::PlaceholderForBackwardCompatibility
                | SubpacketTag::Private(_)
                | SubpacketTag::Unknown(_) =>
                SubpacketValue::Unknown {
                    tag,
                    body: php.parse_bytes("unknown subpacket", len)?,
                },
        };

        let total_out = php.reader.total_out();
        if total_out_before + len != total_out {
            return Err(Error::MalformedPacket(
                format!("Malformed subpacket: \
                         body length is {} bytes, but read {}",
                        len, total_out - total_out_before)).into());
        }

        Ok(Subpacket::with_length(
            length,
            value,
            critical,
        ))
    }
}

impl SubpacketLength {
    /// Parses a subpacket length.
    fn parse<R: BufferedReader<C>, C: fmt::Debug + Send + Sync>(bio: &mut R) -> Result<Self> {
        let octet1 = bio.data_consume_hard(1)?[0];
        if octet1 < 192 {
            // One octet.
            Ok(Self::new(
                octet1 as u32,
                // Unambiguous.
                None))
        } else if (192..255).contains(&octet1) {
            // Two octets length.
            let octet2 = bio.data_consume_hard(1)?[0];
            let len = ((octet1 as u32 - 192) << 8) + octet2 as u32 + 192;
            Ok(Self::new(
                len,
                if Self::len_optimal_encoding(len) == 2 {
                    None
                } else {
                    Some(vec![octet1, octet2])
                }))
        } else {
            // Five octets.
            assert_eq!(octet1, 255);
            let len = bio.read_be_u32()?;
            Ok(Self::new(
                len,
                if Self::len_optimal_encoding(len) == 5 {
                    None
                } else {
                    let mut out = Vec::with_capacity(5);
                    out.push(octet1);
                    out.extend_from_slice(&len.to_be_bytes());
                    Some(out)
                }))
        }
    }
}

#[cfg(test)]
quickcheck! {
    fn length_roundtrip(l: u32) -> bool {
        use crate::serialize::Marshal;

        let length = SubpacketLength::from(l);
        let mut encoded = Vec::new();
        length.serialize(&mut encoded).unwrap();
        assert_eq!(encoded.len(), length.serialized_len());
        let mut reader = buffered_reader::Memory::new(&encoded);
        SubpacketLength::parse(&mut reader).unwrap().len() == l as usize
    }
}

impl OnePassSig {
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(php: PacketHeaderParser<T>)
        -> Result<PacketParser<'a>>
    {
        OnePassSig3::parse(php)
    }
}

impl_parse_generic_packet!(OnePassSig);

impl OnePassSig3 {
    #[allow(clippy::blocks_in_if_conditions)]
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>)
        -> Result<PacketParser<'a>>
    {
        let indent = php.recursion_depth();
        tracer!(TRACE, "OnePassSig", indent);

        make_php_try!(php);

        let version = php_try!(php.parse_u8("version"));
        if version != 3 {
            t!("Ignoring version {} packet", version);

            // Unknown version.  Return an unknown packet.
            return php.fail("unknown version");
        }

        let typ = php_try!(php.parse_u8("type"));
        let hash_algo = php_try!(php.parse_u8("hash_algo"));
        let pk_algo = php_try!(php.parse_u8("pk_algo"));
        let mut issuer = [0u8; 8];
        issuer.copy_from_slice(&php_try!(php.parse_bytes("issuer", 8)));
        let last = php_try!(php.parse_u8("last"));

        let hash_algo = hash_algo.into();
        let typ = typ.into();
        let mut sig = OnePassSig3::new(typ);
        sig.set_hash_algo(hash_algo);
        sig.set_pk_algo(pk_algo.into());
        sig.set_issuer(KeyID::from_bytes(&issuer));
        sig.set_last_raw(last);
        let need_hash = HashingMode::for_signature(hash_algo, typ);

        let recursion_depth = php.recursion_depth();

        // Check if we are processing a cleartext signed message.
        let want_hashes_for = if Cookie::processing_csf_message(&php.reader) {
            HashesFor::CleartextSignature
        } else {
            HashesFor::Signature
        };

        // Walk up the reader chain to see if there is already a
        // hashed reader on level recursion_depth - 1.
        let done = {
            let mut done = false;
            let mut reader : Option<&mut dyn BufferedReader<Cookie>>
                = Some(&mut php.reader);
            while let Some(r) = reader {
                {
                    let cookie = r.cookie_mut();
                    if let Some(br_level) = cookie.level {
                        if br_level < recursion_depth - 1 {
                            break;
                        }
                        if br_level == recursion_depth - 1
                            && cookie.hashes_for == want_hashes_for {
                                // We found a suitable hashed reader.
                                if cookie.saw_last {
                                    cookie.sig_group_push();
                                    cookie.saw_last = false;
                                    cookie.hash_stash =
                                        Some(php.header_bytes.clone());
                                }

                                // Make sure that it uses the required
                                // hash algorithm.
                                if ! cookie.sig_group().hashes.iter()
                                    .any(|mode| {
                                        mode.map(|ctx| ctx.algo()) == need_hash
                                    })
                                {
                                    if let Ok(ctx) = hash_algo.context() {
                                        cookie.sig_group_mut().hashes.push(
                                            HashingMode::for_signature(Box::new(ctx), typ)
                                        );
                                    }
                                }

                                // Account for this OPS packet.
                                cookie.sig_group_mut().ops_count += 1;

                                // Keep track of the last flag.
                                cookie.saw_last = last > 0;

                                // We're done.
                                done = true;
                                break;
                            }
                    } else {
                        break;
                    }
                }
                reader = r.get_mut();
            }
            done
        };
        // Commit here after potentially pushing a signature group.
        let mut pp = php.ok(Packet::OnePassSig(sig.into()))?;
        if done {
            return Ok(pp);
        }

        // We create an empty hashed reader even if we don't support
        // the hash algorithm so that we have something to match
        // against when we get to the Signature packet.
        let mut algos = Vec::new();
        if hash_algo.is_supported() {
            algos.push(HashingMode::for_signature(hash_algo, typ));
        }

        // We can't push the HashedReader on the BufferedReader stack:
        // when we finish processing this OnePassSig packet, it will
        // be popped.  Instead, we need to insert it at the next
        // higher level.  Unfortunately, this isn't possible.  But,
        // since we're done reading the current packet, we can pop the
        // readers associated with it, and then push the HashedReader.
        // This is a bit of a layering violation, but I (Neal) can't
        // think of a more elegant solution.

        assert!(pp.reader.cookie_ref().level <= Some(recursion_depth));
        let (fake_eof, reader)
            = buffered_reader_stack_pop(Box::new(pp.take_reader()),
                                        recursion_depth)?;
        // We only pop the buffered readers for the OPS, and we
        // (currently) never use a fake eof for OPS packets.
        assert!(! fake_eof);

        let mut reader = HashedReader::new(
            reader, want_hashes_for, algos);
        reader.cookie_mut().level = Some(recursion_depth - 1);
        // Account for this OPS packet.
        reader.cookie_mut().sig_group_mut().ops_count += 1;
        // Keep track of the last flag.
        reader.cookie_mut().saw_last = last > 0;

        t!("Pushed a hashed reader, level {:?}", reader.cookie_mut().level);

        // We add an empty limitor on top of the hashed reader,
        // because when we are done processing a packet,
        // PacketParser::finish discards any unread data from the top
        // reader.  Since the top reader is the HashedReader, this
        // discards any following packets.  To prevent this, we push a
        // Limitor on the reader stack.
        let mut reader = buffered_reader::Limitor::with_cookie(
            reader, 0, Cookie::default());
        reader.cookie_mut().level = Some(recursion_depth);

        pp.reader = Box::new(reader);

        Ok(pp)
    }
}

#[test]
fn one_pass_sig_parser_test () {
    use crate::SignatureType;
    use crate::PublicKeyAlgorithm;

    // This test assumes that the first packet is a OnePassSig packet.
    let data = crate::tests::message("signed-1.gpg");
    let mut pp = PacketParser::from_bytes(data).unwrap().unwrap();
    let p = pp.finish().unwrap();
    // eprintln!("packet: {:?}", p);

    if let &Packet::OnePassSig(ref p) = p {
        assert_eq!(p.version(), 3);
        assert_eq!(p.typ(), SignatureType::Binary);
        assert_eq!(p.hash_algo(), HashAlgorithm::SHA512);
        assert_eq!(p.pk_algo(), PublicKeyAlgorithm::RSAEncryptSign);
        assert_eq!(format!("{:X}", p.issuer()), "7223B56678E02528");
        assert_eq!(p.last_raw(), 1);
    } else {
        panic!("Wrong packet!");
    }
}

impl<'a> Parse<'a, OnePassSig3> for OnePassSig3 {
    fn from_reader<R: 'a + Read + Send + Sync>(reader: R) -> Result<Self> {
        OnePassSig::from_reader(reader).map(|p| match p {
            OnePassSig::V3(p) => p,
            // XXX: Once we have a second variant.
            //
            // p => Err(Error::InvalidOperation(
            //     format!("Not a OnePassSig::V3 packet: {:?}", p)).into()),
        })
    }
}

#[test]
fn one_pass_sig_test () {
    struct Test<'a> {
        filename: &'a str,
        digest_prefix: Vec<[u8; 2]>,
    }

    let tests = [
            Test {
                filename: "signed-1.gpg",
                digest_prefix: vec![ [ 0x83, 0xF5 ] ],
            },
            Test {
                filename: "signed-2-partial-body.gpg",
                digest_prefix: vec![ [ 0x2F, 0xBE ] ],
            },
            Test {
                filename: "signed-3-partial-body-multiple-sigs.gpg",
                digest_prefix: vec![ [ 0x29, 0x64 ], [ 0xff, 0x7d ] ],
            },
    ];

    for test in tests.iter() {
        eprintln!("Trying {}...", test.filename);
        let mut ppr = PacketParserBuilder::from_bytes(
            crate::tests::message(test.filename))
            .expect(&format!("Reading {}", test.filename)[..])
            .build().unwrap();

        let mut one_pass_sigs = 0;
        let mut sigs = 0;

        while let PacketParserResult::Some(pp) = ppr {
            if let Packet::OnePassSig(_) = pp.packet {
                one_pass_sigs += 1;
            } else if let Packet::Signature(ref sig) = pp.packet {
                eprintln!("  {}:\n  prefix: expected: {}, in sig: {}",
                          test.filename,
                          crate::fmt::to_hex(&test.digest_prefix[sigs][..], false),
                          crate::fmt::to_hex(sig.digest_prefix(), false));
                eprintln!("  computed hash: {}",
                          crate::fmt::to_hex(sig.computed_digest().unwrap(),
                                             false));

                assert_eq!(&test.digest_prefix[sigs], sig.digest_prefix());
                assert_eq!(&test.digest_prefix[sigs][..],
                           &sig.computed_digest().unwrap()[..2]);

                sigs += 1;
            } else if one_pass_sigs > 0 {
                assert_eq!(one_pass_sigs, test.digest_prefix.len(),
                           "Number of OnePassSig packets does not match \
                            number of expected OnePassSig packets.");
            }

            ppr = pp.recurse().expect("Parsing message").1;
        }
        assert_eq!(one_pass_sigs, sigs,
                   "Number of OnePassSig packets does not match \
                    number of signature packets.");

        eprintln!("done.");
    }
}

// Key::parse doesn't actually use the Key type parameters.  So, we
// can just set them to anything.  This avoids the caller having to
// set them to something.
impl Key<key::UnspecifiedParts, key::UnspecifiedRole>
{
    /// Parses the body of a public key, public subkey, secret key or
    /// secret subkey packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);
        let tag = php.header.ctb().tag();
        assert!(tag == Tag::Reserved
                || tag == Tag::PublicKey
                || tag == Tag::PublicSubkey
                || tag == Tag::SecretKey
                || tag == Tag::SecretSubkey);
        let version = php_try!(php.parse_u8("version"));

        match version {
            4 => Key4::parse(php),
            _ => php.fail("unknown version"),
        }
    }

    /// Returns whether the data appears to be a key (no promises).
    fn plausible<T: BufferedReader<Cookie>>(
        bio: &mut buffered_reader::Dup<T, Cookie>, header: &Header)
                 -> Result<()> {
        Key4::plausible(bio, header)
    }
}

// Key4::parse doesn't actually use the Key4 type parameters.  So, we
// can just set them to anything.  This avoids the caller having to
// set them to something.
impl Key4<key::UnspecifiedParts, key::UnspecifiedRole>
{
    /// Parses the body of a public key, public subkey, secret key or
    /// secret subkey packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);
        let tag = php.header.ctb().tag();
        assert!(tag == Tag::Reserved
                || tag == Tag::PublicKey
                || tag == Tag::PublicSubkey
                || tag == Tag::SecretKey
                || tag == Tag::SecretSubkey);

        let creation_time = php_try!(php.parse_be_u32("creation_time"));
        let pk_algo: PublicKeyAlgorithm = php_try!(php.parse_u8("pk_algo")).into();
        let mpis = php_try!(PublicKey::_parse(pk_algo, &mut php));
        let secret = if let Ok(s2k_usage) = php.parse_u8("s2k_usage") {
            use crypto::mpi;
            let sec = match s2k_usage {
                // Unencrypted
                0 => {
                    let sec = php_try!(
                        mpi::SecretKeyMaterial::_parse(
                            pk_algo, &mut php,
                            Some(mpi::SecretKeyChecksum::Sum16)));
                    sec.into()
                }
                // Encrypted & MD5 for key derivation: unsupported
                1..=253 => {
                    return php.fail("unsupported secret key encryption");
                }
                // Encrypted, S2K & SHA-1 checksum
                254 | 255 => {
                    let sk: SymmetricAlgorithm = php_try!(php.parse_u8("sym_algo")).into();
                    let s2k = php_try!(S2K::parse_v4(&mut php));
                    let s2k_supported = s2k.is_supported();
                    let cipher =
                        php_try!(php.parse_bytes_eof("encrypted_mpis"))
                        .into_boxed_slice();

                    crate::packet::key::Encrypted::new_raw(
                        s2k, sk,
                        if s2k_usage == 254 {
                            Some(mpi::SecretKeyChecksum::SHA1)
                        } else {
                            Some(mpi::SecretKeyChecksum::Sum16)
                        },
                        if s2k_supported {
                            Ok(cipher)
                        } else {
                            Err(cipher)
                        },
                    ).into()
                }
            };

            Some(sec)
        } else {
            None
        };

        let have_secret = secret.is_some();
        if have_secret {
            if tag == Tag::PublicKey || tag == Tag::PublicSubkey {
                return php.error(Error::MalformedPacket(
                    format!("Unexpected secret key found in {:?} packet", tag)
                ).into());
            }
        } else if tag == Tag::SecretKey || tag == Tag::SecretSubkey {
            return php.error(Error::MalformedPacket(
                format!("Expected secret key in {:?} packet", tag)
            ).into());
        }

        fn k<R>(creation_time: u32,
                pk_algo: PublicKeyAlgorithm,
                mpis: PublicKey)
            -> Result<Key4<key::PublicParts, R>>
            where R: key::KeyRole
        {
            Key4::make(creation_time, pk_algo, mpis, None)
        }
        fn s<R>(creation_time: u32,
                pk_algo: PublicKeyAlgorithm,
                mpis: PublicKey,
                secret: SecretKeyMaterial)
            -> Result<Key4<key::SecretParts, R>>
            where R: key::KeyRole
        {
            Key4::make(creation_time, pk_algo, mpis, Some(secret))
        }

        let tag = php.header.ctb().tag();

        let p : Packet = match tag {
            // For the benefit of Key::from_bytes.
            Tag::Reserved => if have_secret {
                Packet::SecretKey(
                    php_try!(s(creation_time, pk_algo, mpis, secret.unwrap()))
                        .into())
            } else {
                Packet::PublicKey(
                    php_try!(k(creation_time, pk_algo, mpis)).into())
            },
            Tag::PublicKey => Packet::PublicKey(
                php_try!(k(creation_time, pk_algo, mpis)).into()),
            Tag::PublicSubkey => Packet::PublicSubkey(
                php_try!(k(creation_time, pk_algo, mpis)).into()),
            Tag::SecretKey => Packet::SecretKey(
                php_try!(s(creation_time, pk_algo, mpis, secret.unwrap()))
                    .into()),
            Tag::SecretSubkey => Packet::SecretSubkey(
                php_try!(s(creation_time, pk_algo, mpis, secret.unwrap()))
                    .into()),
            _ => unreachable!(),
        };

        php.ok(p)
    }

    /// Returns whether the data appears to be a key (no promises).
    fn plausible<T: BufferedReader<Cookie>>(
        bio: &mut buffered_reader::Dup<T, Cookie>, header: &Header)
                 -> Result<()> {
        // The packet's header is 6 bytes.
        if let BodyLength::Full(len) = header.length() {
            if *len < 6 {
                // Much too short.
                return Err(Error::MalformedPacket(
                    format!("Packet too short ({} bytes)", len)).into());
            }
        } else {
            return Err(
                Error::MalformedPacket(
                    format!("Unexpected body length encoding: {:?}",
                            header.length())).into());
        }

        // Make sure we have a minimum header.
        let data = bio.data(6)?;
        if data.len() < 6 {
            return Err(
                Error::MalformedPacket("Short read".into()).into());
        }

        // Assume unknown == bad.
        let version = data[0];
        let pk_algo : PublicKeyAlgorithm = data[5].into();

        if version == 4 && !matches!(pk_algo, PublicKeyAlgorithm::Unknown(_)) {
            Ok(())
        } else {
            Err(Error::MalformedPacket("Invalid or unsupported data".into())
                .into())
        }
    }
}

impl<'a> Parse<'a, key::UnspecifiedKey> for key::UnspecifiedKey {
    fn from_reader<R: 'a + Read + Send + Sync>(reader: R) -> Result<Self> {
        let bio = buffered_reader::Generic::with_cookie(
            reader, None, Cookie::default());
        let parser = PacketHeaderParser::new_naked(bio);

        let mut pp = Self::parse(parser)?;
        pp.buffer_unread_content()?;

        match pp.next()? {
            (Packet::PublicKey(o), PacketParserResult::EOF(_)) => Ok(o.into()),
            (Packet::PublicSubkey(o), PacketParserResult::EOF(_)) => Ok(o.into()),
            (Packet::SecretKey(o), PacketParserResult::EOF(_)) => Ok(o.into()),
            (Packet::SecretSubkey(o), PacketParserResult::EOF(_)) => Ok(o.into()),
            (p, PacketParserResult::EOF(_)) =>
                Err(Error::InvalidOperation(
                    format!("Not a Key packet: {:?}", p)).into()),
            (_, PacketParserResult::Some(_)) =>
                Err(Error::InvalidOperation(
                    "Excess data after packet".into()).into()),
        }
    }
}

impl Trust {
    /// Parses the body of a trust packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);
        let value = php_try!(php.parse_bytes_eof("value"));
        php.ok(Packet::Trust(Trust::from(value)))
    }
}

impl_parse_generic_packet!(Trust);

impl UserID {
    /// Parses the body of a user id packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);

        let value = php_try!(php.parse_bytes_eof("value"));

        php.ok(Packet::UserID(UserID::from(value)))
    }
}

impl_parse_generic_packet!(UserID);

impl UserAttribute {
    /// Parses the body of a user attribute packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);

        let value = php_try!(php.parse_bytes_eof("value"));

        php.ok(Packet::UserAttribute(UserAttribute::from(value)))
    }
}

impl_parse_generic_packet!(UserAttribute);

impl Marker {
    /// Parses the body of a marker packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>>
    {
        make_php_try!(php);
        let marker = php_try!(php.parse_bytes("marker", Marker::BODY.len()));
        if &marker[..] == Marker::BODY {
            php.ok(Marker::default().into())
        } else {
            php.fail("invalid marker")
        }
    }

    /// Returns whether the data is a marker packet.
    fn plausible<T>(bio: &mut buffered_reader::Dup<T, Cookie>, header: &Header)
                    -> Result<()>
        where T: BufferedReader<Cookie>,
    {
        if let BodyLength::Full(len) = header.length() {
            let len = *len;
            if len as usize != Marker::BODY.len() {
                return Err(Error::MalformedPacket(
                    format!("Unexpected packet length {}", len)).into());
            }
        } else {
            return Err(Error::MalformedPacket(
                format!("Unexpected body length encoding: {:?}",
                        header.length())).into());
        }

        // Check the body.
        let data = bio.data(Marker::BODY.len())?;
        if data.len() < Marker::BODY.len() {
            return Err(Error::MalformedPacket("Short read".into()).into());
        }

        if data == Marker::BODY {
            Ok(())
        } else {
            Err(Error::MalformedPacket("Invalid or unsupported data".into())
                .into())
        }
    }
}

impl_parse_generic_packet!(Marker);

impl Literal {
    /// Parses the body of a literal packet.
    ///
    /// Condition: Hashing has been disabled by the callee.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>>
    {
        make_php_try!(php);

        // Directly hashing a literal data packet is... strange.
        // Neither the packet's header, the packet's meta-data nor the
        // length encoding information is included in the hash.

        let format = php_try!(php.parse_u8("format"));
        let filename_len = php_try!(php.parse_u8("filename_len"));

        let filename = if filename_len > 0 {
            Some(php_try!(php.parse_bytes("filename", filename_len as usize)))
        } else {
            None
        };

        let date = php_try!(php.parse_be_u32("date"));

        // The header is consumed while hashing is disabled.
        let recursion_depth = php.recursion_depth();

        let mut literal = Literal::new(format.into());
        if let Some(filename) = filename {
            literal.set_filename(&filename)
                .expect("length checked above");
        }
        literal.set_date(
            Some(std::time::SystemTime::from(Timestamp::from(date))))?;
        let mut pp = php.ok(Packet::Literal(literal))?;

        // Enable hashing of the body.
        Cookie::hashing(pp.mut_reader(), Hashing::Enabled,
                        recursion_depth - 1);

        Ok(pp)
    }
}

impl_parse_generic_packet!(Literal);

#[test]
fn literal_parser_test () {
    use crate::types::DataFormat;
    {
        let data = crate::tests::message("literal-mode-b.gpg");
        let mut pp = PacketParser::from_bytes(data).unwrap().unwrap();
        assert_eq!(pp.header.length(), &BodyLength::Full(18));
        let content = pp.steal_eof().unwrap();
        let p = pp.finish().unwrap();
        // eprintln!("{:?}", p);
        if let &Packet::Literal(ref p) = p {
            assert_eq!(p.format(), DataFormat::Binary);
            assert_eq!(p.filename().unwrap()[..], b"foobar"[..]);
            assert_eq!(p.date().unwrap(), Timestamp::from(1507458744).into());
            assert_eq!(content, b"FOOBAR");
        } else {
            panic!("Wrong packet!");
        }
    }

    {
        let data = crate::tests::message("literal-mode-t-partial-body.gpg");
        let mut pp = PacketParser::from_bytes(data).unwrap().unwrap();
        assert_eq!(pp.header.length(), &BodyLength::Partial(4096));
        let content = pp.steal_eof().unwrap();
        let p = pp.finish().unwrap();
        if let &Packet::Literal(ref p) = p {
            assert_eq!(p.format(), DataFormat::Text);
            assert_eq!(p.filename().unwrap()[..],
                       b"manifesto.txt"[..]);
            assert_eq!(p.date().unwrap(), Timestamp::from(1508000649).into());

            let expected = crate::tests::manifesto();

            assert_eq!(&content[..], expected);
        } else {
            panic!("Wrong packet!");
        }
    }
}

impl CompressedData {
    /// Parses the body of a compressed data packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        let recursion_depth = php.recursion_depth();
        tracer!(TRACE, "CompressedData::parse", recursion_depth);

        make_php_try!(php);
        let algo: CompressionAlgorithm =
            php_try!(php.parse_u8("algo")).into();

        let recursion_depth = php.recursion_depth();
        let mut pp = php.ok(Packet::CompressedData(CompressedData::new(algo)))?;

        #[allow(unreachable_patterns)]
        match algo {
            CompressionAlgorithm::Uncompressed => (),
            #[cfg(feature = "compression-deflate")]
            CompressionAlgorithm::Zip
                | CompressionAlgorithm::Zlib => (),
            #[cfg(feature = "compression-bzip2")]
            CompressionAlgorithm::BZip2 => (),
            _ => {
                // We don't know or support this algorithm.  Return a
                // CompressedData packet without pushing a filter, so
                // that it has an opaque body.
                t!("Algorithm {} unknown or unsupported.", algo);
                return Ok(pp.set_processed(false));
            },
        }

        t!("Pushing a decompressor for {}, recursion depth = {:?}.",
           algo, recursion_depth);

        let reader = pp.take_reader();
        let reader = match algo {
            CompressionAlgorithm::Uncompressed => {
                if TRACE {
                    eprintln!("CompressedData::parse(): Actually, no need \
                               for a compression filter: this is an \
                               \"uncompressed compression packet\".");
                }
                let _ = recursion_depth;
                reader
            },
            #[cfg(feature = "compression-deflate")]
            CompressionAlgorithm::Zip =>
                Box::new(buffered_reader::Deflate::with_cookie(
                    reader, Cookie::new(recursion_depth))),
            #[cfg(feature = "compression-deflate")]
            CompressionAlgorithm::Zlib =>
                Box::new(buffered_reader::Zlib::with_cookie(
                    reader, Cookie::new(recursion_depth))),
            #[cfg(feature = "compression-bzip2")]
            CompressionAlgorithm::BZip2 =>
                Box::new(buffered_reader::Bzip::with_cookie(
                    reader, Cookie::new(recursion_depth))),
            _ => unreachable!(), // Validated above.
        };
        pp.set_reader(reader);

        Ok(pp)
    }
}

impl_parse_generic_packet!(CompressedData);

#[cfg(any(feature = "compression-deflate", feature = "compression-bzip2"))]
#[test]
fn compressed_data_parser_test () {
    use crate::types::DataFormat;

    let expected = crate::tests::manifesto();

    for i in 1..4 {
        match CompressionAlgorithm::from(i) {
            #[cfg(feature = "compression-deflate")]
            CompressionAlgorithm::Zip | CompressionAlgorithm::Zlib => (),
            #[cfg(feature = "compression-bzip2")]
            CompressionAlgorithm::BZip2 => (),
            _ => continue,
        }
        let pp = PacketParser::from_bytes(crate::tests::message(
            &format!("compressed-data-algo-{}.gpg", i))).unwrap().unwrap();

        // We expect a compressed packet containing a literal data
        // packet, and that is it.
        if let Packet::CompressedData(ref compressed) = pp.packet {
            assert_eq!(compressed.algo(), i.into());
        } else {
            panic!("Wrong packet!");
        }

        let ppr = pp.recurse().unwrap().1;

        // ppr should be the literal data packet.
        let mut pp = ppr.unwrap();

        // It is a child.
        assert_eq!(pp.recursion_depth(), 1);

        let content = pp.steal_eof().unwrap();

        let (literal, ppr) = pp.recurse().unwrap();

        if let Packet::Literal(literal) = literal {
            assert_eq!(literal.filename(), None);
            assert_eq!(literal.format(), DataFormat::Binary);
            assert_eq!(literal.date().unwrap(),
                       Timestamp::from(1509219866).into());
            assert_eq!(content, expected.to_vec());
        } else {
            panic!("Wrong packet!");
        }

        // And, we're done...
        assert!(ppr.is_eof());
    }
}

impl SKESK {
    /// Parses the body of an SK-ESK packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>)
                                                 -> Result<PacketParser<'a>>
    {
        make_php_try!(php);
        let version = php_try!(php.parse_u8("version"));
        match version {
            4 => SKESK4::parse(php),
            5 => SKESK5::parse(php),
            _ => php.fail("unknown version"),
        }
    }
}

impl SKESK4 {
    /// Parses the body of an SK-ESK packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>)
                                                 -> Result<PacketParser<'a>>
    {
        make_php_try!(php);
        let sym_algo = php_try!(php.parse_u8("sym_algo"));
        let s2k = php_try!(S2K::parse_v4(&mut php));
        let s2k_supported = s2k.is_supported();
        let esk = php_try!(php.parse_bytes_eof("esk"));

        let skesk = php_try!(SKESK4::new_raw(
            sym_algo.into(),
            s2k,
            if s2k_supported || esk.is_empty() {
                Ok(if ! esk.is_empty() {
                    Some(esk.into())
                } else {
                    None
                })
            } else {
                Err(esk.into())
            },
        ));

        php.ok(skesk.into())
    }
}

impl SKESK5 {
    /// Parses the body of an SK-ESK packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>)
                                                 -> Result<PacketParser<'a>>
    {
        make_php_try!(php);
        let sym_algo: SymmetricAlgorithm =
            php_try!(php.parse_u8("sym_algo")).into();
        let aead_algo: AEADAlgorithm =
            php_try!(php.parse_u8("aead_algo")).into();
        let s2k = php_try!(S2K::parse_v4(&mut php));
        let s2k_supported = s2k.is_supported();
        let iv_size = php_try!(aead_algo.nonce_size());
        let digest_size = php_try!(aead_algo.digest_size());

        // The rest of the packet is (potentially) the S2K
        // parameters, the AEAD IV, the ESK, and the AEAD
        // digest.  We don't know the size of the S2K
        // parameters if the S2K method is not supported, and
        // we don't know the size of the ESK.
        let mut esk = php_try!(php.reader.steal_eof()
                               .map_err(anyhow::Error::from));
        let aead_iv = if s2k_supported && esk.len() >= iv_size {
            // We know the S2K method, so the parameters have
            // been parsed into the S2K object.  So, `esk`
            // starts with iv_size bytes of IV.
            let mut iv = esk;
            esk = iv.split_off(iv_size);
            iv
        } else {
            Vec::with_capacity(0) // A dummy value.
        };

        let l = esk.len();
        let aead_digest = esk.split_off(l.saturating_sub(digest_size));
        // Now fix the map.
        if s2k_supported {
            php.field("aead_iv", iv_size);
        }
        php.field("esk", esk.len());
        php.field("aead_digest", aead_digest.len());

        let skesk = php_try!(SKESK5::new_raw(
            sym_algo,
            aead_algo,
            s2k,
            if s2k_supported {
                Ok((aead_iv.into(), esk.into()))
            } else {
                Err(esk.into())
            },
            aead_digest.into_boxed_slice(),
        ));

        php.ok(skesk.into())
    }
}

impl_parse_generic_packet!(SKESK);

#[test]
fn skesk_parser_test() {
    use crate::crypto::Password;
    struct Test<'a> {
        filename: &'a str,
        s2k: S2K,
        cipher_algo: SymmetricAlgorithm,
        password: Password,
        key_hex: &'a str,
    }

    let tests = [
        Test {
            filename: "s2k/mode-3-encrypted-key-password-bgtyhn.gpg",
            cipher_algo: SymmetricAlgorithm::AES128,
            s2k: S2K::Iterated {
                hash: HashAlgorithm::SHA1,
                salt: [0x82, 0x59, 0xa0, 0x6e, 0x98, 0xda, 0x94, 0x1c],
                hash_bytes: S2K::decode_count(238),
            },
            password: "bgtyhn".into(),
            key_hex: "474E5C373BA18AF0A499FCAFE6093F131DF636F6A3812B9A8AE707F1F0214AE9",
        },
    ];

    for test in tests.iter() {
        let pp = PacketParser::from_bytes(
            crate::tests::message(test.filename)).unwrap().unwrap();
        if let Packet::SKESK(SKESK::V4(ref skesk)) = pp.packet {
            eprintln!("{:?}", skesk);

            assert_eq!(skesk.symmetric_algo(), test.cipher_algo);
            assert_eq!(skesk.s2k(), &test.s2k);

            match skesk.decrypt(&test.password) {
                Ok((_sym_algo, key)) => {
                    let key = crate::fmt::to_hex(&key[..], false);
                    assert_eq!(&key[..], test.key_hex);
                }
                Err(e) => {
                    panic!("No session key, got: {:?}", e);
                }
            }
        } else {
            panic!("Wrong packet!");
        }
    }
}

impl SEIP {
    /// Parses the body of a SEIP packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);
        let version = php_try!(php.parse_u8("version"));
        if version != 1 {
            return php.fail("unknown version");
        }

        php.ok(SEIP1::new().into())
            .map(|pp| pp.set_processed(false))
    }
}

impl_parse_generic_packet!(SEIP);

impl MDC {
    /// Parses the body of an MDC packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);

        // Find the HashedReader pushed by the containing SEIP packet.
        // In a well-formed message, this will be the outer most
        // HashedReader on the BufferedReader stack: we pushed it
        // there when we started decrypting the SEIP packet, and an
        // MDC packet is the last packet in a SEIP container.
        // Nevertheless, we take some basic precautions to check
        // whether it is really the matching HashedReader.

        let mut computed_digest : [u8; 20] = Default::default();
        {
            let mut r : Option<&mut dyn BufferedReader<Cookie>>
                = Some(&mut php.reader);
            while let Some(bio) = r {
                {
                    let state = bio.cookie_mut();
                    if state.hashes_for == HashesFor::MDC {
                        if !state.sig_group().hashes.is_empty() {
                            let h = state.sig_group_mut().hashes
                                .iter_mut().find_map(
                                    |mode|
                                    if mode.map(|ctx| ctx.algo()) ==
                                        HashingMode::Binary(HashAlgorithm::SHA1)
                                    {
                                        Some(mode.as_mut())
                                    } else {
                                        None
                                    }).unwrap();
                            let _ = h.digest(&mut computed_digest);
                        }

                        // If the outer most HashedReader is not the
                        // matching HashedReader, then the message is
                        // malformed.
                        break;
                    }
                }

                r = bio.get_mut();
            }
        }

        let mut digest: [u8; 20] = Default::default();
        digest.copy_from_slice(&php_try!(php.parse_bytes("digest", 20)));

        php.ok(Packet::MDC(MDC::new(digest, computed_digest)))
    }
}

impl_parse_generic_packet!(MDC);

impl AED {
    /// Parses the body of a AED packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);
        let version = php_try!(php.parse_u8("version"));

        match version {
            1 => AED1::parse(php),
            _ => php.fail("unknown version"),
        }
    }
}

impl_parse_generic_packet!(AED);

impl AED1 {
    /// Parses the body of a AED packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);
        let cipher: SymmetricAlgorithm =
            php_try!(php.parse_u8("sym_algo")).into();
        let aead: AEADAlgorithm =
            php_try!(php.parse_u8("aead_algo")).into();
        let chunk_size = php_try!(php.parse_u8("chunk_size"));

        // DRAFT 4880bis-08, section 5.16: "An implementation MUST
        // support chunk size octets with values from 0 to 56.  Chunk
        // size octets with other values are reserved for future
        // extensions."
        if chunk_size > 56 {
            return php.fail("unsupported chunk size");
        }
        let chunk_size: u64 = 1 << (chunk_size + 6);

        let iv_size = php_try!(aead.nonce_size());
        let iv = php_try!(php.parse_bytes("iv", iv_size));

        let aed = php_try!(Self::new(
            cipher, aead, chunk_size, iv.into_boxed_slice()
        ));
        php.ok(aed.into()).map(|pp| pp.set_processed(false))
    }
}

impl MPI {
    /// Parses an OpenPGP MPI.
    ///
    /// See [Section 3.2 of RFC 4880] for details.
    ///
    ///   [Section 3.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.2
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(
        name_len: &'static str,
        name: &'static str,
        php: &mut PacketHeaderParser<T>)
                 -> Result<Self> {
        // This function is used to parse MPIs from unknown
        // algorithms, which may use an encoding unknown to us.
        // Therefore, we need to be extra careful only to consume the
        // data once we found a well-formed MPI.
        let bits = {
            let buf = php.reader.data_hard(2)?;
            u16::from_be_bytes([buf[0], buf[1]]) as usize
        };
        if bits == 0 {
            // Now consume the data.
            php.parse_be_u16(name_len).expect("worked before");
            return Ok(vec![].into());
        }

        let bytes = (bits + 7) / 8;
        let value = {
            let buf = php.reader.data_hard(2 + bytes)?;
            Vec::from(&buf[2..2 + bytes])
        };

        let unused_bits = bytes * 8 - bits;
        assert_eq!(bytes * 8 - unused_bits, bits);

        // Make sure the unused bits are zeroed.
        if unused_bits > 0 {
            let mask = !((1 << (8 - unused_bits)) - 1);
            let unused_value = value[0] & mask;

            if unused_value != 0 {
                return Err(Error::MalformedMPI(
                        format!("{} unused bits not zeroed: ({:x})",
                        unused_bits, unused_value)).into());
            }
        }

        let first_used_bit = 8 - unused_bits;
        if value[0] & (1 << (first_used_bit - 1)) == 0 {
            return Err(Error::MalformedMPI(
                    format!("leading bit is not set: \
                             expected bit {} to be set in {:8b} ({:x})",
                             first_used_bit, value[0], value[0])).into());
        }

        // Now consume the data.
        php.parse_be_u16(name_len).expect("worked before");
        php.parse_bytes(name, bytes).expect("worked before");
        Ok(value.into())
    }
}

impl<'a> Parse<'a, MPI> for MPI {
    // Reads an MPI from `reader`.
    fn from_reader<R: io::Read + Send + Sync>(reader: R) -> Result<Self> {
        let bio = buffered_reader::Generic::with_cookie(
            reader, None, Cookie::default());
        let mut parser = PacketHeaderParser::new_naked(bio);
        Self::parse("(none_len)", "(none)", &mut parser)
    }
}

impl PKESK {
    /// Parses the body of an PK-ESK packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);
        let version = php_try!(php.parse_u8("version"));
        match version {
            3 => PKESK3::parse(php),
            _ => php.fail("unknown version"),
        }
    }
}

impl_parse_generic_packet!(PKESK);

impl PKESK3 {
    /// Parses the body of an PK-ESK packet.
    fn parse<'a, T: 'a + BufferedReader<Cookie>>(mut php: PacketHeaderParser<T>) -> Result<PacketParser<'a>> {
        make_php_try!(php);
        let mut keyid = [0u8; 8];
        keyid.copy_from_slice(&php_try!(php.parse_bytes("keyid", 8)));
        let pk_algo: PublicKeyAlgorithm = php_try!(php.parse_u8("pk_algo")).into();
        if ! pk_algo.for_encryption() {
            return php.fail("not an encryption algorithm");
        }
        let mpis = crypto::mpi::Ciphertext::_parse(pk_algo, &mut php)?;

        let pkesk = php_try!(PKESK3::new(KeyID::from_bytes(&keyid),
                                         pk_algo, mpis));
        php.ok(pkesk.into())
    }
}

impl<'a> Parse<'a, PKESK3> for PKESK3 {
    fn from_reader<R: 'a + Read + Send + Sync>(reader: R) -> Result<Self> {
        PKESK::from_reader(reader).map(|p| match p {
            PKESK::V3(p) => p,
            // XXX: Once we have a second variant.
            //
            // p => Err(Error::InvalidOperation(
            //     format!("Not a PKESKv3 packet: {:?}", p)).into()),
        })
    }
}

impl<'a> Parse<'a, Packet> for Packet {
    fn from_reader<R: 'a + Read + Send + Sync>(reader: R) -> Result<Self> {
        let ppr =
            PacketParserBuilder::from_reader(reader)
            ?.buffer_unread_content().build()?;

        let (p, ppr) = match ppr {
            PacketParserResult::Some(pp) => {
                pp.next()?
            },
            PacketParserResult::EOF(_) =>
                return Err(Error::InvalidOperation(
                    "Unexpected EOF".into()).into()),
        };

        match (p, ppr) {
            (p, PacketParserResult::EOF(_)) =>
                Ok(p),
            (_, PacketParserResult::Some(_)) =>
                Err(Error::InvalidOperation(
                    "Excess data after packet".into()).into()),
        }
    }
}

// State that lives for the life of the packet parser, not the life of
// an individual packet.
#[derive(Debug)]
struct PacketParserState {
    // The `PacketParser`'s settings
    settings: PacketParserSettings,

    /// Whether the packet sequence is a valid OpenPGP Message.
    message_validator: MessageValidator,

    /// Whether the packet sequence is a valid OpenPGP keyring.
    keyring_validator: KeyringValidator,

    /// Whether the packet sequence is a valid OpenPGP Cert.
    cert_validator: CertValidator,

    // Whether this is the first packet in the packet sequence.
    first_packet: bool,
}

impl PacketParserState {
    fn new(settings: PacketParserSettings) -> Self {
        PacketParserState {
            settings,
            message_validator: Default::default(),
            keyring_validator: Default::default(),
            cert_validator: Default::default(),
            first_packet: true,
        }
    }
}

/// A low-level OpenPGP message parser.
///
/// A `PacketParser` provides a low-level, iterator-like interface to
/// parse OpenPGP messages.
///
/// For each iteration, the user is presented with a [`Packet`]
/// corresponding to the last packet, a `PacketParser` for the next
/// packet, and their positions within the message.
///
/// Using the `PacketParser`, the user is able to configure how the
/// new packet will be parsed.  For instance, it is possible to stream
/// the packet's contents (a `PacketParser` implements the
/// [`std::io::Read`] and the [`BufferedReader`] traits), buffer them
/// within the [`Packet`], or drop them.  The user can also decide to
/// recurse into the packet, if it is a container, instead of getting
/// the following packet.
///
/// See the [`PacketParser::next`] and [`PacketParser::recurse`]
/// methods for more details.
///
///   [`Packet`]: super::Packet
///   [`BufferedReader`]: https://docs.rs/buffered-reader/*/buffered_reader/trait.BufferedReader.html
///   [`PacketParser::next`]: PacketParser::next()
///   [`PacketParser::recurse`]: PacketParser::recurse()
///
/// # Examples
///
/// These examples demonstrate how to process packet bodies by parsing
/// the simplest possible OpenPGP message containing just a single
/// literal data packet with the body "Hello world.".  There are three
/// options.  First, the body can be dropped.  Second, it can be
/// buffered.  Lastly, the body can be streamed.  In general,
/// streaming should be preferred, because it avoids buffering in
/// Sequoia.
///
/// This example demonstrates simply ignoring the packet body:
///
/// ```rust
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use sequoia_openpgp as openpgp;
/// use openpgp::Packet;
/// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
///
/// // By default, the `PacketParser` will drop packet bodies.
/// let mut ppr =
///     PacketParser::from_bytes(b"\xcb\x12b\x00\x00\x00\x00\x00Hello world.")?;
/// while let PacketParserResult::Some(pp) = ppr {
///     // Get the packet out of the parser and start parsing the next
///     // packet, recursing.
///     let (packet, next_ppr) = pp.recurse()?;
///     ppr = next_ppr;
///
///     // Process the packet.
///     if let Packet::Literal(literal) = packet {
///         // The body was dropped.
///         assert_eq!(literal.body(), b"");
///     } else {
///         unreachable!("We know it is a literal packet.");
///     }
/// }
/// # Ok(()) }
/// ```
///
/// This example demonstrates how the body can be buffered by
/// configuring the `PacketParser` to buffer all packet bodies:
///
/// ```rust
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use sequoia_openpgp as openpgp;
/// use openpgp::Packet;
/// use openpgp::parse::{Parse, PacketParserResult, PacketParserBuilder};
///
/// // By default, the `PacketParser` will drop packet bodies.  Use a
/// // `PacketParserBuilder` to change that.
/// let mut ppr =
///     PacketParserBuilder::from_bytes(
///         b"\xcb\x12b\x00\x00\x00\x00\x00Hello world.")?
///     .buffer_unread_content()
///     .build()?;
/// while let PacketParserResult::Some(pp) = ppr {
///     // Get the packet out of the parser and start parsing the next
///     // packet, recursing.
///     let (packet, next_ppr) = pp.recurse()?;
///     ppr = next_ppr;
///
///     // Process the packet.
///     if let Packet::Literal(literal) = packet {
///         // The body was buffered.
///         assert_eq!(literal.body(), b"Hello world.");
///     } else {
///         unreachable!("We know it is a literal packet.");
///     }
/// }
/// # Ok(()) }
/// ```
///
/// This example demonstrates how the body can be buffered by
/// buffering an individual packet:
///
/// ```rust
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use sequoia_openpgp as openpgp;
/// use openpgp::Packet;
/// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
///
/// // By default, the `PacketParser` will drop packet bodies.
/// let mut ppr =
///     PacketParser::from_bytes(b"\xcb\x12b\x00\x00\x00\x00\x00Hello world.")?;
/// while let PacketParserResult::Some(mut pp) = ppr {
///     if let Packet::Literal(_) = pp.packet {
///         // Buffer this packet's body.
///         pp.buffer_unread_content()?;
///     }
///
///     // Get the packet out of the parser and start parsing the next
///     // packet, recursing.
///     let (packet, next_ppr) = pp.recurse()?;
///     ppr = next_ppr;
///
///     // Process the packet.
///     if let Packet::Literal(literal) = packet {
///         // The body was buffered.
///         assert_eq!(literal.body(), b"Hello world.");
///     } else {
///         unreachable!("We know it is a literal packet.");
///     }
/// }
/// # Ok(()) }
/// ```
///
/// This example demonstrates how to stream the packet body:
///
/// ```rust
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use std::io::Read;
///
/// use sequoia_openpgp as openpgp;
/// use openpgp::Packet;
/// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
///
/// let mut ppr =
///     PacketParser::from_bytes(b"\xcb\x12b\x00\x00\x00\x00\x00Hello world.")?;
/// while let PacketParserResult::Some(mut pp) = ppr {
///     if let Packet::Literal(_) = pp.packet {
///         // Stream the body.
///         let mut buf = Vec::new();
///         pp.read_to_end(&mut buf)?;
///         assert_eq!(buf, b"Hello world.");
///     } else {
///         unreachable!("We know it is a literal packet.");
///     }
///
///     // Get the packet out of the parser and start parsing the next
///     // packet, recursing.
///     let (packet, next_ppr) = pp.recurse()?;
///     ppr = next_ppr;
///
///     // Process the packet.
///     if let Packet::Literal(literal) = packet {
///         // The body was streamed, not buffered.
///         assert_eq!(literal.body(), b"");
///     } else {
///         unreachable!("We know it is a literal packet.");
///     }
/// }
/// # Ok(()) }
/// ```
///
/// # Packet Parser Design
///
/// There are two major concerns that inform the design of the parsing
/// API.
///
/// First, when processing a container, it is possible to either
/// recurse into the container, and process its children, or treat the
/// contents of the container as an opaque byte stream, and process
/// the packet following the container.  The low-level
/// [`PacketParser`] and mid-level [`PacketPileParser`] abstractions
/// allow the caller to choose the behavior by either calling the
/// [`PacketParser::recurse`] method or the [`PacketParser::next`]
/// method, as appropriate.  OpenPGP doesn't impose any restrictions
/// on the amount of nesting.  So, to prevent a denial of service
/// attack, the parsers don't recurse more than
/// [`DEFAULT_MAX_RECURSION_DEPTH`] times, by default.
///
///
/// Second, packets can contain an effectively unbounded amount of
/// data.  To avoid errors due to memory exhaustion, the
/// `PacketParser` and [`PacketPileParser`] abstractions support
/// parsing packets in a streaming manner, i.e., never buffering more
/// than O(1) bytes of data.  To do this, the parsers initially only
/// parse a packet's header (which is rarely more than a few kilobytes
/// of data), and return control to the caller.  After inspecting that
/// data, the caller can decide how to handle the packet's contents.
/// If the content is deemed interesting, it can be streamed or
/// buffered.  Otherwise, it can be dropped.  Streaming is possible
/// not only for literal data packets, but also containers (other
/// packets also support the interface, but just return EOF).  For
/// instance, encryption can be stripped by saving the decrypted
/// content of an encryption packet, which is just an OpenPGP message.
///
/// ## Iterator Design
///
/// We explicitly chose to not use a callback-based API, but something
/// that is closer to Rust's iterator API.  Unfortunately, because a
/// `PacketParser` needs mutable access to the input stream (so that
/// the content can be streamed), only a single `PacketParser` item
/// can be live at a time (without a fair amount of unsafe nastiness).
/// This is incompatible with Rust's iterator concept, which allows
/// any number of items to be live at any time.  For instance:
///
/// ```rust
/// let mut v = vec![1, 2, 3, 4];
/// let mut iter = v.iter_mut();
///
/// let x = iter.next().unwrap();
/// let y = iter.next().unwrap();
///
/// *x += 10; // This does not cause an error!
/// *y += 10;
/// ```
pub struct PacketParser<'a> {
    /// The current packet's header.
    header: Header,

    /// The packet that is being parsed.
    pub packet: Packet,

    // The path of the packet that is currently being parsed.
    path: Vec<usize>,
    // The path of the packet that was most recently returned by
    // `next()` or `recurse()`.
    last_path: Vec<usize>,

    reader: Box<dyn BufferedReader<Cookie> + 'a>,

    // Whether the caller read the packet's content.  If so, then we
    // can't recurse, because we're missing some of the packet!
    content_was_read: bool,

    // Whether PacketParser::finish has been called.
    finished: bool,

    // Whether the content has been processed.
    processed: bool,

    /// A map of this packet.
    map: Option<map::Map>,

    /// We compute a hashsum over the body to implement comparison on
    /// containers that have been streamed.
    body_hash: Option<Box<Xxh3>>,

    state: PacketParserState,
}
assert_send_and_sync!(PacketParser<'_>);

impl<'a> std::fmt::Display for PacketParser<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "PacketParser")
    }
}

impl<'a> std::fmt::Debug for PacketParser<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("PacketParser")
            .field("header", &self.header)
            .field("packet", &self.packet)
            .field("path", &self.path)
            .field("last_path", &self.last_path)
            .field("processed", &self.processed)
            .field("content_was_read", &self.content_was_read)
            .field("settings", &self.state.settings)
            .field("map", &self.map)
            .finish()
    }
}

/// The return value of PacketParser::parse.
#[allow(clippy::upper_case_acronyms)]
enum ParserResult<'a> {
    Success(PacketParser<'a>),
    EOF((Box<dyn BufferedReader<Cookie> + 'a>, PacketParserState, Vec<usize>)),
}

/// Information about the stream of packets parsed by the
/// `PacketParser`.
///
/// Once the [`PacketParser`] reaches the end of the input stream, it
/// returns a [`PacketParserResult::EOF`] with a `PacketParserEOF`.
/// This object provides information about the parsed stream, notably
/// whether or not the packet stream was a well-formed [`Message`],
/// [`Cert`] or keyring.
///
///   [`Message`]: super::Message
///   [`Cert`]: crate::cert::Cert
///
/// # Examples
///
/// Parse some OpenPGP stream using a [`PacketParser`] and detects the
/// kind of data:
///
/// ```rust
/// # fn main() -> sequoia_openpgp::Result<()> {
/// use sequoia_openpgp as openpgp;
/// use openpgp::Packet;
/// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
///
/// let openpgp_data: &[u8] = // ...
/// #    include_bytes!("../tests/data/keys/public-key.gpg");
/// let mut ppr = PacketParser::from_bytes(openpgp_data)?;
/// while let PacketParserResult::Some(mut pp) = ppr {
///     // Start parsing the next packet, recursing.
///     ppr = pp.recurse()?.1;
/// }
///
/// if let PacketParserResult::EOF(eof) = ppr {
///     if eof.is_message().is_ok() {
///         // ...
///     } else if eof.is_cert().is_ok() {
///         // ...
///     } else if eof.is_keyring().is_ok() {
///         // ...
///     } else {
///         // ...
///     }
/// }
/// # Ok(()) }
/// ```
#[derive(Debug)]
pub struct PacketParserEOF<'a> {
    state: PacketParserState,
    reader: Box<dyn BufferedReader<Cookie> + 'a>,
    last_path: Vec<usize>,
}
assert_send_and_sync!(PacketParserEOF<'_>);

impl<'a> PacketParserEOF<'a> {
    /// Copies the important information in `pp` into a new
    /// `PacketParserEOF` instance.
    fn new(mut state: PacketParserState,
           reader: Box<dyn BufferedReader<Cookie> + 'a>)
           -> Self {
        state.message_validator.finish();
        state.keyring_validator.finish();
        state.cert_validator.finish();

        PacketParserEOF {
            state,
            reader,
            last_path: vec![],
        }
    }

    /// Creates a placeholder instance for PacketParserResult::take.
    fn empty() -> Self {
        Self::new(
            PacketParserState::new(Default::default()),
            buffered_reader::Memory::with_cookie(b"", Default::default())
                .as_boxed())
    }

    /// Returns whether the stream is an OpenPGP Message.
    ///
    /// A [`Message`] has a very specific structure.  Returns `true`
    /// if the stream is of that form, as opposed to a [`Cert`] or
    /// just a bunch of packets.
    ///
    ///   [`Message`]: super::Message
    ///   [`Cert`]: crate::cert::Cert
    ///
    /// # Examples
    ///
    /// Parse some OpenPGP stream using a [`PacketParser`] and detects the
    /// kind of data:
    ///
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// let openpgp_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/keys/public-key.gpg");
    /// let mut ppr = PacketParser::from_bytes(openpgp_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    ///
    /// if let PacketParserResult::EOF(eof) = ppr {
    ///     if eof.is_message().is_ok() {
    ///         // ...
    ///     }
    /// }
    /// # Ok(()) }
    /// ```
    pub fn is_message(&self) -> Result<()> {
        use crate::message::MessageValidity;

        match self.state.message_validator.check() {
            MessageValidity::Message => Ok(()),
            MessageValidity::MessagePrefix => unreachable!(),
            MessageValidity::Error(err) => Err(err),
        }
    }

    /// Returns whether the message is an OpenPGP keyring.
    ///
    /// A keyring has a very specific structure.  Returns `true` if
    /// the stream is of that form, as opposed to a [`Message`] or
    /// just a bunch of packets.
    ///
    ///   [`Message`]: super::Message
    ///
    /// # Examples
    ///
    /// Parse some OpenPGP stream using a [`PacketParser`] and detects the
    /// kind of data:
    ///
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// let openpgp_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/keys/public-key.gpg");
    /// let mut ppr = PacketParser::from_bytes(openpgp_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    ///
    /// if let PacketParserResult::EOF(eof) = ppr {
    ///     if eof.is_keyring().is_ok() {
    ///         // ...
    ///     }
    /// }
    /// # Ok(()) }
    /// ```
    pub fn is_keyring(&self) -> Result<()> {
        match self.state.keyring_validator.check() {
            KeyringValidity::Keyring => Ok(()),
            KeyringValidity::KeyringPrefix => unreachable!(),
            KeyringValidity::Error(err) => Err(err),
        }
    }

    /// Returns whether the message is an OpenPGP Cert.
    ///
    /// A [`Cert`] has a very specific structure.  Returns `true` if
    /// the stream is of that form, as opposed to a [`Message`] or
    /// just a bunch of packets.
    ///
    ///   [`Message`]: super::Message
    ///   [`Cert`]: crate::cert::Cert
    ///
    /// # Examples
    ///
    /// Parse some OpenPGP stream using a [`PacketParser`] and detects the
    /// kind of data:
    ///
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// let openpgp_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/keys/public-key.gpg");
    /// let mut ppr = PacketParser::from_bytes(openpgp_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    ///
    /// if let PacketParserResult::EOF(eof) = ppr {
    ///     if eof.is_cert().is_ok() {
    ///         // ...
    ///     }
    /// }
    /// # Ok(()) }
    /// ```
    pub fn is_cert(&self) -> Result<()> {
        match self.state.cert_validator.check() {
            CertValidity::Cert => Ok(()),
            CertValidity::CertPrefix => unreachable!(),
            CertValidity::Error(err) => Err(err),
        }
    }

    /// Returns the path of the last packet.
    ///
    /// # Examples
    ///
    /// Parse some OpenPGP stream using a [`PacketParser`] and returns
    /// the path (see [`PacketPile::path_ref`]) of the last packet:
    ///
    ///   [`PacketPile::path_ref`]: super::PacketPile::path_ref()
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// let openpgp_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/keys/public-key.gpg");
    /// let mut ppr = PacketParser::from_bytes(openpgp_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    ///
    /// if let PacketParserResult::EOF(eof) = ppr {
    ///     let _ = eof.last_path();
    /// }
    /// # Ok(()) }
    /// ```
    pub fn last_path(&self) -> &[usize] {
        &self.last_path[..]
    }

    /// The last packet's recursion depth.
    ///
    /// A top-level packet has a recursion depth of 0.  Packets in a
    /// top-level container have a recursion depth of 1, etc.
    ///
    /// # Examples
    ///
    /// Parse some OpenPGP stream using a [`PacketParser`] and returns
    /// the recursion depth of the last packet:
    ///
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// let openpgp_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/keys/public-key.gpg");
    /// let mut ppr = PacketParser::from_bytes(openpgp_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    ///
    /// if let PacketParserResult::EOF(eof) = ppr {
    ///     let _ = eof.last_recursion_depth();
    /// }
    /// # Ok(()) }
    /// ```
    pub fn last_recursion_depth(&self) -> Option<isize> {
        if self.last_path.is_empty() {
            None
        } else {
            Some(self.last_path.len() as isize - 1)
        }
    }

    /// Returns the exhausted reader.
    pub fn into_reader(self) -> Box<dyn BufferedReader<Cookie> + 'a> {
        self.reader
    }
}

/// The result of parsing a packet.
///
/// This type is returned by [`PacketParser::next`],
/// [`PacketParser::recurse`], [`PacketParserBuilder::build`], and the
/// implementation of [`PacketParser`]'s [`Parse` trait].  The result
/// is either `Some(PacketParser)`, indicating successful parsing of a
/// packet, or `EOF(PacketParserEOF)` if the end of the input stream
/// has been reached.
///
///   [`PacketParser::next`]: PacketParser::next()
///   [`PacketParser::recurse`]: PacketParser::recurse()
///   [`PacketParserBuilder::build`]: PacketParserBuilder::build()
///   [`Parse` trait]: struct.PacketParser.html#impl-Parse%3C%27a%2C%20PacketParserResult%3C%27a%3E%3E
#[derive(Debug)]
pub enum PacketParserResult<'a> {
    /// A `PacketParser` for the next packet.
    Some(PacketParser<'a>),
    /// Information about a fully parsed packet sequence.
    EOF(PacketParserEOF<'a>),
}
assert_send_and_sync!(PacketParserResult<'_>);

impl<'a> PacketParserResult<'a> {
    /// Returns `true` if the result is `EOF`.
    pub fn is_eof(&self) -> bool {
        matches!(self, PacketParserResult::EOF(_))
    }

    /// Returns `true` if the result is `Some`.
    pub fn is_some(&self) -> bool {
        ! Self::is_eof(self)
    }

    /// Unwraps a result, yielding the content of an `Some`.
    ///
    /// # Panics
    ///
    /// Panics if the value is an `EOF`, with a panic message
    /// including the passed message, and the information in the
    /// [`PacketParserEOF`] object.
    ///
    pub fn expect(self, msg: &str) -> PacketParser<'a> {
        if let PacketParserResult::Some(pp) = self {
            pp
        } else {
            panic!("{}", msg);
        }
    }

    /// Unwraps a result, yielding the content of an `Some`.
    ///
    /// # Panics
    ///
    /// Panics if the value is an `EOF`, with a panic message
    /// including the information in the [`PacketParserEOF`] object.
    ///
    pub fn unwrap(self) -> PacketParser<'a> {
        self.expect("called `PacketParserResult::unwrap()` on a \
                     `PacketParserResult::PacketParserEOF` value")
    }

    /// Converts from `PacketParserResult` to `Result<&PacketParser,
    /// &PacketParserEOF>`.
    ///
    /// Produces a new `Result`, containing references into the
    /// original `PacketParserResult`, leaving the original in place.
    pub fn as_ref(&self)
                  -> StdResult<&PacketParser<'a>, &PacketParserEOF> {
        match self {
            PacketParserResult::Some(pp) => Ok(pp),
            PacketParserResult::EOF(eof) => Err(eof),
        }
    }

    /// Converts from `PacketParserResult` to `Result<&mut
    /// PacketParser, &mut PacketParserEOF>`.
    ///
    /// Produces a new `Result`, containing mutable references into the
    /// original `PacketParserResult`, leaving the original in place.
    pub fn as_mut(&mut self)
                  -> StdResult<&mut PacketParser<'a>, &mut PacketParserEOF<'a>>
    {
        match self {
            PacketParserResult::Some(pp) => Ok(pp),
            PacketParserResult::EOF(eof) => Err(eof),
        }
    }

    /// Takes the value out of the `PacketParserResult`, leaving a
    /// `EOF` in its place.
    ///
    /// The `EOF` left in place carries a [`PacketParserEOF`] with
    /// default values.
    ///
    pub fn take(&mut self) -> Self {
        mem::replace(
            self,
            PacketParserResult::EOF(PacketParserEOF::empty()))
    }

    /// Maps a `PacketParserResult` to `Result<PacketParser,
    /// PacketParserEOF>` by applying a function to a contained `Some`
    /// value, leaving an `EOF` value untouched.
    pub fn map<U, F>(self, f: F) -> StdResult<U, PacketParserEOF<'a>>
        where F: FnOnce(PacketParser<'a>) -> U
    {
        match self {
            PacketParserResult::Some(x) => Ok(f(x)),
            PacketParserResult::EOF(e) => Err(e),
        }
    }
}

impl<'a> Parse<'a, PacketParserResult<'a>> for PacketParser<'a> {
    /// Starts parsing an OpenPGP message stored in a `std::io::Read` object.
    ///
    /// This function returns a `PacketParser` for the first packet in
    /// the stream.
    fn from_reader<R: io::Read + 'a + Send + Sync>(reader: R)
            -> Result<PacketParserResult<'a>> {
        PacketParserBuilder::from_reader(reader)?.build()
    }

    /// Starts parsing an OpenPGP message stored in a file named `path`.
    ///
    /// This function returns a `PacketParser` for the first packet in
    /// the stream.
    fn from_file<P: AsRef<Path>>(path: P)
            -> Result<PacketParserResult<'a>> {
        PacketParserBuilder::from_file(path)?.build()
    }

    /// Starts parsing an OpenPGP message stored in a buffer.
    ///
    /// This function returns a `PacketParser` for the first packet in
    /// the stream.
    fn from_bytes<D: AsRef<[u8]> + ?Sized + Send + Sync>(data: &'a D)
            -> Result<PacketParserResult<'a>> {
        PacketParserBuilder::from_bytes(data)?.build()
    }
}

impl <'a> PacketParser<'a> {
    /// Starts parsing an OpenPGP message stored in a `BufferedReader`
    /// object.
    ///
    /// This function returns a `PacketParser` for the first packet in
    /// the stream.
    pub(crate) fn from_buffered_reader(bio: Box<dyn BufferedReader<Cookie> + 'a>)
            -> Result<PacketParserResult<'a>> {
        PacketParserBuilder::from_buffered_reader(bio)?.build()
    }

    /// Returns the reader stack, replacing it with a
    /// `buffered_reader::EOF` reader.
    ///
    /// This function may only be called when the `PacketParser` is in
    /// State::Body.
    fn take_reader(&mut self) -> Box<dyn BufferedReader<Cookie> + 'a> {
        self.set_reader(
            Box::new(buffered_reader::EOF::with_cookie(Default::default())))
    }

    /// Replaces the reader stack.
    ///
    /// This function may only be called when the `PacketParser` is in
    /// State::Body.
    fn set_reader(&mut self, reader: Box<dyn BufferedReader<Cookie> + 'a>)
        -> Box<dyn BufferedReader<Cookie> + 'a>
    {
        mem::replace(&mut self.reader, reader)
    }

    /// Returns a mutable reference to the reader stack.
    fn mut_reader(&mut self) -> &mut dyn BufferedReader<Cookie> {
        &mut self.reader
    }

    /// Marks the packet's contents as processed or not.
    fn set_processed(mut self, v: bool) -> Self {
        self.processed = v;
        self
    }

    /// Returns whether the packet's contents have been processed.
    ///
    /// This function returns `true` while processing an encryption
    /// container before it is decrypted using
    /// [`PacketParser::decrypt`].  Once successfully decrypted, it
    /// returns `false`.
    ///
    ///   [`PacketParser::decrypt`]: PacketParser::decrypt()
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::fmt::hex;
    /// use openpgp::types::SymmetricAlgorithm;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse an encrypted message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/encrypted-aes256-password-123.gpg");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     if let Packet::SEIP(_) = pp.packet {
    ///         assert!(!pp.processed());
    ///         pp.decrypt(SymmetricAlgorithm::AES256,
    ///                    &hex::decode("7EF4F08C44F780BEA866961423306166\
    ///                                  B8912C43352F3D9617F745E4E3939710")?
    ///                        .into())?;
    ///         assert!(pp.processed());
    ///     }
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn processed(&self) -> bool {
        self.processed
    }

    /// Returns whether the packet's contents are encrypted.
    ///
    /// This function has been obsoleted by the negation of
    /// [`PacketParser::processed`].
    #[deprecated(since = "1.10.0", note = "Use !processed()")]
    pub fn encrypted(&self) -> bool {
        !self.processed()
    }

    /// Returns the path of the last packet.
    ///
    /// This function returns the path (see [`PacketPile::path_ref`]
    /// for a description of paths) of the packet last returned by a
    /// call to [`PacketParser::recurse`] or [`PacketParser::next`].
    /// If no packet has been returned (i.e. the current packet is the
    /// first packet), this returns the empty slice.
    ///
    ///   [`PacketPile::path_ref`]: super::PacketPile::path_ref()
    ///   [`PacketParser::recurse`]: PacketParser::recurse()
    ///   [`PacketParser::next`]: PacketParser::next()
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a compressed message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     match pp.packet {
    ///         Packet::CompressedData(_) => assert_eq!(pp.last_path(), &[]),
    ///         Packet::Literal(_) => assert_eq!(pp.last_path(), &[0]),
    ///         _ => (),
    ///     }
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn last_path(&self) -> &[usize] {
        &self.last_path[..]
    }

    /// Returns the path of the current packet.
    ///
    /// This function returns the path (see [`PacketPile::path_ref`]
    /// for a description of paths) of the packet currently being
    /// processed (see [`PacketParser::packet`]).
    ///
    ///   [`PacketPile::path_ref`]: super::PacketPile::path_ref()
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a compressed message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     match pp.packet {
    ///         Packet::CompressedData(_) => assert_eq!(pp.path(), &[0]),
    ///         Packet::Literal(_) => assert_eq!(pp.path(), &[0, 0]),
    ///         _ => (),
    ///     }
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn path(&self) -> &[usize] {
        &self.path[..]
    }

    /// The current packet's recursion depth.
    ///
    /// A top-level packet has a recursion depth of 0.  Packets in a
    /// top-level container have a recursion depth of 1, etc.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a compressed message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     match pp.packet {
    ///         Packet::CompressedData(_) => assert_eq!(pp.recursion_depth(), 0),
    ///         Packet::Literal(_) => assert_eq!(pp.recursion_depth(), 1),
    ///         _ => (),
    ///     }
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn recursion_depth(&self) -> isize {
        self.path.len() as isize - 1
    }

    /// The last packet's recursion depth.
    ///
    /// A top-level packet has a recursion depth of 0.  Packets in a
    /// top-level container have a recursion depth of 1, etc.
    ///
    /// Note: if no packet has been returned yet, this returns None.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a compressed message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     match pp.packet {
    ///         Packet::CompressedData(_) => assert_eq!(pp.last_recursion_depth(), None),
    ///         Packet::Literal(_) => assert_eq!(pp.last_recursion_depth(), Some(0)),
    ///         _ => (),
    ///     }
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn last_recursion_depth(&self) -> Option<isize> {
        if self.last_path.is_empty() {
            assert_eq!(&self.path[..], &[ 0 ]);
            None
        } else {
            Some(self.last_path.len() as isize - 1)
        }
    }

    /// Returns whether the message appears to be an OpenPGP Message.
    ///
    /// Only when the whole message has been processed is it possible
    /// to say whether the message is definitely an OpenPGP Message.
    /// Before that, it is only possible to say that the message is a
    /// valid prefix or definitely not an OpenPGP message (see
    /// [`PacketParserEOF::is_message`]).
    ///
    ///   [`PacketParserEOF::is_message`]: PacketParserEOF::is_message()
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a compressed message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     pp.possible_message()?;
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn possible_message(&self) -> Result<()> {
        use crate::message::MessageValidity;

        match self.state.message_validator.check() {
            MessageValidity::Message => unreachable!(),
            MessageValidity::MessagePrefix => Ok(()),
            MessageValidity::Error(err) => Err(err),
        }
    }

    /// Returns whether the message appears to be an OpenPGP keyring.
    ///
    /// Only when the whole message has been processed is it possible
    /// to say whether the message is definitely an OpenPGP keyring.
    /// Before that, it is only possible to say that the message is a
    /// valid prefix or definitely not an OpenPGP keyring (see
    /// [`PacketParserEOF::is_keyring`]).
    ///
    ///   [`PacketParserEOF::is_keyring`]: PacketParserEOF::is_keyring()
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a certificate.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/keys/testy.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     pp.possible_keyring()?;
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn possible_keyring(&self) -> Result<()> {
        match self.state.keyring_validator.check() {
            KeyringValidity::Keyring => unreachable!(),
            KeyringValidity::KeyringPrefix => Ok(()),
            KeyringValidity::Error(err) => Err(err),
        }
    }

    /// Returns whether the message appears to be an OpenPGP Cert.
    ///
    /// Only when the whole message has been processed is it possible
    /// to say whether the message is definitely an OpenPGP Cert.
    /// Before that, it is only possible to say that the message is a
    /// valid prefix or definitely not an OpenPGP Cert (see
    /// [`PacketParserEOF::is_cert`]).
    ///
    ///   [`PacketParserEOF::is_cert`]: PacketParserEOF::is_cert()
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a certificate.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/keys/testy.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     pp.possible_cert()?;
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn possible_cert(&self) -> Result<()> {
        match self.state.cert_validator.check() {
            CertValidity::Cert => unreachable!(),
            CertValidity::CertPrefix => Ok(()),
            CertValidity::Error(err) => Err(err),
        }
    }

    /// Tests whether the data appears to be a legal cert packet.
    ///
    /// This is just a heuristic.  It can be used for recovering from
    /// garbage.
    ///
    /// Successfully reading the header only means that the top bit of
    /// the ptag is 1.  Assuming a uniform distribution, there's a 50%
    /// chance that that is the case.
    ///
    /// To improve our chances of a correct recovery, we make sure the
    /// tag is known (for new format CTBs, there are 64 possible tags,
    /// but only a third of them are reasonable; for old format
    /// packets, there are only 16 and nearly all are plausible), and
    /// we make sure the packet contents are reasonable.
    ///
    /// Currently, we only try to recover the most interesting
    /// packets.
    fn plausible_cert<T: BufferedReader<Cookie>>(
        bio: &mut buffered_reader::Dup<T, Cookie>, header: &Header)
                 -> Result<()> {
        let bad = Err(
            Error::MalformedPacket("Can't make an educated case".into()).into());

        match header.ctb().tag() {
            Tag::Reserved
            | Tag::Unknown(_) | Tag::Private(_) =>
                Err(Error::MalformedPacket("Looks like garbage".into()).into()),

            Tag::Marker => Marker::plausible(bio, header),
            Tag::Signature => Signature::plausible(bio, header),

            Tag::SecretKey => Key::plausible(bio, header),
            Tag::PublicKey => Key::plausible(bio, header),
            Tag::SecretSubkey => Key::plausible(bio, header),
            Tag::PublicSubkey => Key::plausible(bio, header),

            Tag::UserID => bad,
            Tag::UserAttribute => bad,

            // It is reasonable to try and ignore garbage in Certs,
            // because who knows what the keyservers return, etc.
            // But, if we have what appears to be an OpenPGP message,
            // then, ignore.
            Tag::PKESK => bad,
            Tag::SKESK => bad,
            Tag::OnePassSig => bad,
            Tag::CompressedData => bad,
            Tag::SED => bad,
            Tag::Literal => bad,
            Tag::Trust => bad,
            Tag::SEIP => bad,
            Tag::MDC => bad,
            Tag::AED => bad,
        }
    }

    /// Returns a `PacketParser` for the next OpenPGP packet in the
    /// stream.  If there are no packets left, this function returns
    /// `bio`.
    fn parse(mut bio: Box<dyn BufferedReader<Cookie> + 'a>,
             state: PacketParserState,
             path: Vec<usize>)
        -> Result<ParserResult<'a>>
    {
        assert!(!path.is_empty());

        let indent = path.len() as isize - 1;
        tracer!(TRACE, "PacketParser::parse", indent);
        t!("Parsing packet at {:?}", path);

        let recursion_depth = path.len() as isize - 1;

        // When header encounters an EOF, it returns an error.  But,
        // we want to return None.  Try a one byte read.
        if bio.data(1)?.is_empty() {
            t!("No packet at {:?} (EOF).", path);
            return Ok(ParserResult::EOF((bio, state, path)));
        }

        // When computing a hash for a signature, most of the
        // signature packet should not be included in the hash.  That
        // is:
        //
        //    [ one pass sig ] [ ... message ... ] [ sig ]
        //                     ^^^^^^^^^^^^^^^^^^^
        //                        hash only this
        //
        // (The special logic for the Signature packet is in
        // Signature::parse.)
        //
        // To avoid this, we use a Dup reader to figure out if the
        // next packet is a sig packet without consuming the headers,
        // which would cause the headers to be hashed.  If so, we
        // extract the hash context.

        let mut bio = buffered_reader::Dup::with_cookie(bio, Cookie::default());
        let mut header;

        // Read the header.
        let mut skip = 0;
        let mut orig_error : Option<anyhow::Error> = None;
        loop {
            bio.rewind();
            bio.data_consume_hard(skip)?;

            match Header::parse(&mut bio) {
                Ok(header_) => {
                    if skip == 0 {
                        header = header_;
                        break;
                    }

                    match Self::plausible_cert(&mut bio, &header_) {
                        Ok(()) => {
                            header = header_;
                            break;
                        }
                        Err(_err) => (),
                    }
                }
                Err(err) => {
                    if orig_error.is_none() {
                        orig_error = Some(err);
                    }

                    if state.first_packet || skip > RECOVERY_THRESHOLD {
                        // Limit the search space.  This should be
                        // enough to find a reasonable recovery point
                        // in a Cert.
                        return Err(orig_error.unwrap());
                    }
                }
            }

            skip += 1;
        }

        // Prepare to actually consume the header or garbage.
        let consumed = if skip == 0 {
            bio.total_out()
        } else {
            t!("turning {} bytes of junk into an Unknown packet", skip);

            // Fabricate a header.
            header = Header::new(CTB::new(Tag::Reserved),
                                 BodyLength::Full(skip as u32));
            0
        };

        let tag = header.ctb().tag();

        // A buffered_reader::Dup always has an inner.
        let mut bio = Box::new(bio).into_inner().unwrap();

        // Disable hashing for literal packets, Literal::parse will
        // enable it for the body.  Signatures and OnePassSig packets
        // are only hashed by notarizing signatures.
        if tag == Tag::Literal {
            Cookie::hashing(
                &mut bio, Hashing::Disabled, recursion_depth - 1);
        } else if tag == Tag::OnePassSig || tag == Tag::Signature {
            if Cookie::processing_csf_message(&bio) {
                // When processing a CSF message, the hashing reader
                // is not peeled off, because the number of signature
                // packets cannot be known from the number of OPS
                // packets.  Instead, we simply disable hashing.
                //
                // XXX: It would be nice to peel off the hashing
                // reader and drop this workaround.
                Cookie::hashing(
                    &mut bio, Hashing::Disabled, recursion_depth - 1);
            } else {
                Cookie::hashing(
                    &mut bio, Hashing::Notarized, recursion_depth - 1);
            }
        }

        // Save header for the map or nested signatures.
        let header_bytes =
            Vec::from(&bio.data_consume_hard(consumed)?[..consumed]);

        let bio : Box<dyn BufferedReader<Cookie>>
            = match header.length() {
                &BodyLength::Full(len) => {
                    t!("Pushing a limitor ({} bytes), level: {}.",
                       len, recursion_depth);
                    Box::new(buffered_reader::Limitor::with_cookie(
                        bio, len as u64,
                        Cookie::new(recursion_depth)))
                },
                &BodyLength::Partial(len) => {
                    t!("Pushing a partial body chunk decoder, level: {}.",
                       recursion_depth);
                    Box::new(BufferedReaderPartialBodyFilter::with_cookie(
                        bio, len,
                        // When hashing a literal data packet, we only
                        // hash the packet's contents; we don't hash
                        // the literal data packet's meta-data or the
                        // length information, which includes the
                        // partial body headers.
                        tag != Tag::Literal,
                        Cookie::new(recursion_depth)))
                },
                BodyLength::Indeterminate => {
                    t!("Indeterminate length packet, not adding a limitor.");
                    bio
                },
        };

        // Our parser should not accept packets that fail our header
        // syntax check.  Doing so breaks roundtripping, and seems
        // like a bad idea anyway.
        let mut header_syntax_error = header.valid(true).err();

        // Check packet size.
        if header_syntax_error.is_none() {
            let max_size = state.settings.max_packet_size;
            match tag {
                // Don't check the size for container packets, those
                // can be safely streamed.
                Tag::Literal | Tag::CompressedData | Tag::SED | Tag::SEIP
                    | Tag::AED => (),
                _ => match header.length() {
                    BodyLength::Full(l) => if *l > max_size {
                        header_syntax_error = Some(
                            Error::PacketTooLarge(tag, *l, max_size).into());
                    },
                    _ => unreachable!("non-data packets have full length, \
                                       syntax check above"),
                }
            }
        }

        let parser = PacketHeaderParser::new(bio, state, path,
                                             header, header_bytes);

        let mut result = match tag {
            Tag::Reserved if skip > 0 => Unknown::parse(
                parser, Error::MalformedPacket(format!(
                    "Skipped {} bytes of junk", skip)).into()),
            _ if header_syntax_error.is_some() =>
                Unknown::parse(parser, header_syntax_error.unwrap()),
            Tag::Signature =>           Signature::parse(parser),
            Tag::OnePassSig =>          OnePassSig::parse(parser),
            Tag::PublicSubkey =>        Key::parse(parser),
            Tag::PublicKey =>           Key::parse(parser),
            Tag::SecretKey =>           Key::parse(parser),
            Tag::SecretSubkey =>        Key::parse(parser),
            Tag::Trust =>               Trust::parse(parser),
            Tag::UserID =>              UserID::parse(parser),
            Tag::UserAttribute =>       UserAttribute::parse(parser),
            Tag::Marker =>              Marker::parse(parser),
            Tag::Literal =>             Literal::parse(parser),
            Tag::CompressedData =>      CompressedData::parse(parser),
            Tag::SKESK =>               SKESK::parse(parser),
            Tag::SEIP =>                SEIP::parse(parser),
            Tag::MDC =>                 MDC::parse(parser),
            Tag::PKESK =>               PKESK::parse(parser),
            Tag::AED =>                 AED::parse(parser),
            _ => Unknown::parse(parser,
                                Error::UnsupportedPacketType(tag).into()),
        }?;

        if tag == Tag::OnePassSig {
            Cookie::hashing(
                &mut result, Hashing::Enabled, recursion_depth - 1);
        }

        result.state.first_packet = false;

        t!(" -> {:?}, path: {:?}, level: {:?}.",
           result.packet.tag(), result.path, result.cookie_ref().level);

        return Ok(ParserResult::Success(result));
    }

    /// Finishes parsing the current packet and starts parsing the
    /// next one.
    ///
    /// This function finishes parsing the current packet.  By
    /// default, any unread content is dropped.  (See
    /// [`PacketParsererBuilder`] for how to configure this.)  It then
    /// creates a new packet parser for the next packet.  If the
    /// current packet is a container, this function does *not*
    /// recurse into the container, but skips any packets it contains.
    /// To recurse into the container, use the [`recurse()`] method.
    ///
    ///   [`PacketParsererBuilder`]: PacketParserBuilder
    ///   [`recurse()`]: PacketParser::recurse()
    ///
    /// The return value is a tuple containing:
    ///
    ///   - A `Packet` holding the fully processed old packet;
    ///
    ///   - A `PacketParser` holding the new packet;
    ///
    /// To determine the two packet's position within the parse tree,
    /// you can use `last_path()` and `path()`, respectively.  To
    /// determine their depth, you can use `last_recursion_depth()`
    /// and `recursion_depth()`, respectively.
    ///
    /// Note: A recursion depth of 0 means that the packet is a
    /// top-level packet, a recursion depth of 1 means that the packet
    /// is an immediate child of a top-level-packet, etc.
    ///
    /// Since the packets are serialized in depth-first order and all
    /// interior nodes are visited, we know that if the recursion
    /// depth is the same, then the packets are siblings (they have a
    /// common parent) and not, e.g., cousins (they have a common
    /// grandparent).  This is because, if we move up the tree, the
    /// only way to move back down is to first visit a new container
    /// (e.g., an aunt).
    ///
    /// Using the two positions, we can compute the change in depth as
    /// new_depth - old_depth.  Thus, if the change in depth is 0, the
    /// two packets are siblings.  If the value is 1, the old packet
    /// is a container, and the new packet is its first child.  And,
    /// if the value is -1, the new packet is contained in the old
    /// packet's grandparent.  The idea is illustrated below:
    ///
    /// ```text
    ///             ancestor
    ///             |       \
    ///            ...      -n
    ///             |
    ///           grandparent
    ///           |          \
    ///         parent       -1
    ///         |      \
    ///      packet    0
    ///         |
    ///         1
    /// ```
    ///
    /// Note: since this function does not automatically recurse into
    /// a container, the change in depth will always be non-positive.
    /// If the current container is empty, this function DOES pop that
    /// container off the container stack, and returns the following
    /// packet in the parent container.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // Start parsing the next packet.
    ///     ppr = pp.next()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn next(mut self)
        -> Result<(Packet, PacketParserResult<'a>)>
    {
        let indent = self.recursion_depth();
        tracer!(TRACE, "PacketParser::next", indent);
        t!("({:?}, path: {:?}, level: {:?}).",
           self.packet.tag(), self.path, self.cookie_ref().level);

        self.finish()?;

        let (mut fake_eof, mut reader) = buffered_reader_stack_pop(
            mem::replace(&mut self.reader,
                         Box::new(buffered_reader::EOF::with_cookie(
                             Default::default()))),
            self.recursion_depth())?;

        self.last_path.clear();
        self.last_path.extend_from_slice(&self.path[..]);

        // Assume that we succeed in parsing the next packet.  If not,
        // then we'll adjust the path.
        *self.path.last_mut().expect("A path is never empty") += 1;

        // Now read the next packet.
        loop {
            // Parse the next packet.
            t!("Reading packet at {:?} from: {:?}", self.path, reader);

            let recursion_depth = self.recursion_depth();

            let ppr = PacketParser::parse(reader, self.state, self.path)?;
            match ppr {
                ParserResult::EOF((reader_, state_, path_)) => {
                    // We got EOF on the current container.  The
                    // container at recursion depth n is empty.  Pop
                    // it and any filters for it, i.e., those at level
                    // n (e.g., the limitor that caused us to hit
                    // EOF), and then try again.

                    t!("depth: {}, got EOF trying to read the next packet",
                       recursion_depth);

                    self.path = path_;

                    if ! fake_eof && recursion_depth == 0 {
                        t!("Popped top-level container, done reading message.");
                        // Pop topmost filters (e.g. the armor::Reader).
                        let (_, reader_) = buffered_reader_stack_pop(
                            reader_, ARMOR_READER_LEVEL)?;
                        let mut eof = PacketParserEOF::new(state_, reader_);
                        eof.last_path = self.last_path;
                        return Ok((self.packet,
                                   PacketParserResult::EOF(eof)));
                    } else {
                        self.state = state_;
                        self.finish()?;
                        let (fake_eof_, reader_) = buffered_reader_stack_pop(
                            reader_, recursion_depth - 1)?;
                        fake_eof = fake_eof_;
                        if ! fake_eof {
                            self.path.pop().unwrap();
                            *self.path.last_mut()
                                .expect("A path is never empty") += 1;
                        }
                        reader = reader_;
                    }
                },
                ParserResult::Success(mut pp) => {
                    let path = pp.path().to_vec();
                    pp.state.message_validator.push(
                        pp.packet.tag(), pp.packet.version(),
                        &path);
                    pp.state.keyring_validator.push(pp.packet.tag());
                    pp.state.cert_validator.push(pp.packet.tag());

                    pp.last_path = self.last_path;

                    return Ok((self.packet, PacketParserResult::Some(pp)));
                }
            }
        }
    }

    /// Finishes parsing the current packet and starts parsing the
    /// next one, recursing if possible.
    ///
    /// This method is similar to the [`next()`] method (see that
    /// method for more details), but if the current packet is a
    /// container (and we haven't reached the maximum recursion depth,
    /// and the user hasn't started reading the packet's contents), we
    /// recurse into the container, and return a `PacketParser` for
    /// its first child.  Otherwise, we return the next packet in the
    /// packet stream.  If this function recurses, then the new
    /// packet's recursion depth will be `last_recursion_depth() + 1`;
    /// because we always visit interior nodes, we can't recurse more
    /// than one level at a time.
    ///
    ///   [`next()`]: PacketParser::next()
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn recurse(self) -> Result<(Packet, PacketParserResult<'a>)> {
        let indent = self.recursion_depth();
        tracer!(TRACE, "PacketParser::recurse", indent);
        t!("({:?}, path: {:?}, level: {:?})",
           self.packet.tag(), self.path, self.cookie_ref().level);

        match self.packet {
            // Packets that recurse.
            Packet::CompressedData(_) | Packet::SEIP(_) | Packet::AED(_)
                if self.processed =>
            {
                if self.recursion_depth() as u8
                    >= self.state.settings.max_recursion_depth
                {
                    t!("Not recursing into the {:?} packet, maximum recursion \
                        depth ({}) reached.",
                       self.packet.tag(),
                       self.state.settings.max_recursion_depth);

                    // Drop through.
                } else if self.content_was_read {
                    t!("Not recursing into the {:?} packet, some data was \
                        already read.",
                       self.packet.tag());

                    // Drop through.
                } else {
                    let mut last_path = self.last_path;
                    last_path.clear();
                    last_path.extend_from_slice(&self.path[..]);

                    let mut path = self.path;
                    path.push(0);

                    match PacketParser::parse(self.reader, self.state,
                                              path.clone())?
                    {
                        ParserResult::Success(mut pp) => {
                            t!("Recursed into the {:?} packet, got a {:?}.",
                               self.packet.tag(), pp.packet.tag());

                            pp.state.message_validator.push(
                                pp.packet.tag(),
                                pp.packet.version(),
                                &path);
                            pp.state.keyring_validator.push(pp.packet.tag());
                            pp.state.cert_validator.push(pp.packet.tag());

                            pp.last_path = last_path;

                            return Ok((self.packet,
                                       PacketParserResult::Some(pp)));
                        },
                        ParserResult::EOF(_) => {
                            return Err(Error::MalformedPacket(
                                "Container is truncated".into()).into());
                        },
                    }
                }
            },
            // Packets that don't recurse.
            Packet::Unknown(_) | Packet::Signature(_) | Packet::OnePassSig(_)
                | Packet::PublicKey(_) | Packet::PublicSubkey(_)
                | Packet::SecretKey(_) | Packet::SecretSubkey(_)
                | Packet::Marker(_) | Packet::Trust(_)
                | Packet::UserID(_) | Packet::UserAttribute(_)
                | Packet::Literal(_) | Packet::PKESK(_) | Packet::SKESK(_)
                | Packet::SEIP(_) | Packet::MDC(_) | Packet::AED(_)
                | Packet::CompressedData(_) => {
                // Drop through.
                t!("A {:?} packet is not a container, not recursing.",
                   self.packet.tag());
            },
        }

        // No recursion.
        self.next()
    }

    /// Causes the PacketParser to buffer the packet's contents.
    ///
    /// The packet's contents can be retrieved using
    /// e.g. [`Container::body`].  In general, you should avoid
    /// buffering a packet's content and prefer streaming its content
    /// unless you are certain that the content is small.
    ///
    ///   [`Container::body`]: crate::packet::Container::body()
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a message.
    /// let message_data: &[u8] = // ...
    /// #   include_bytes!("../tests/data/messages/literal-mode-t-partial-body.gpg");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     // Process the packet.
    ///
    ///     if let Packet::Literal(_) = pp.packet {
    ///         assert!(pp.buffer_unread_content()?
    ///                     .starts_with(b"A Cypherpunk's Manifesto"));
    /// #       assert!(pp.buffer_unread_content()?
    /// #                   .starts_with(b"A Cypherpunk's Manifesto"));
    ///         if let Packet::Literal(l) = &pp.packet {
    ///             assert!(l.body().starts_with(b"A Cypherpunk's Manifesto"));
    ///             assert_eq!(l.body().len(), 5158);
    ///         } else {
    ///             unreachable!();
    ///         }
    ///     }
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn buffer_unread_content(&mut self) -> Result<&[u8]> {
        let rest = self.steal_eof()?;

        fn set_or_extend(rest: Vec<u8>, c: &mut Container, processed: bool)
                         -> Result<&[u8]> {
            if !rest.is_empty() {
                let current = match c.body() {
                    Body::Unprocessed(bytes) => &bytes[..],
                    Body::Processed(bytes) => &bytes[..],
                    Body::Structured(packets) if packets.is_empty() => &[][..],
                    Body::Structured(_) => return Err(Error::InvalidOperation(
                        "cannot append unread bytes to parsed packets"
                            .into()).into()),
                };
                let rest = if !current.is_empty() {
                    let mut new =
                        Vec::with_capacity(current.len() + rest.len());
                    new.extend_from_slice(current);
                    new.extend_from_slice(&rest);
                    new
                } else {
                    rest
                };

                c.set_body(if processed {
                    Body::Processed(rest)
                } else {
                    Body::Unprocessed(rest)
                });
            }

            match c.body() {
                Body::Unprocessed(bytes) => Ok(bytes),
                Body::Processed(bytes) => Ok(bytes),
                Body::Structured(packets) if packets.is_empty() => Ok(&[][..]),
                Body::Structured(_) => Err(Error::InvalidOperation(
                    "cannot append unread bytes to parsed packets"
                        .into()).into()),
            }
        }

        use std::ops::DerefMut;
        match &mut self.packet {
            Packet::Literal(p) => set_or_extend(rest, p.container_mut(), false),
            Packet::Unknown(p) => set_or_extend(rest, p.container_mut(), false),
            Packet::CompressedData(p) =>
                set_or_extend(rest, p.deref_mut(), self.processed),
            Packet::SEIP(p) =>
                set_or_extend(rest, p.deref_mut(), self.processed),
            Packet::AED(p) =>
                set_or_extend(rest, p.deref_mut(), self.processed),
            p => {
                if !rest.is_empty() {
                    Err(Error::MalformedPacket(
                        format!("Unexpected body data for {:?}: {}",
                                p, crate::fmt::hex::encode_pretty(rest)))
                        .into())
                } else {
                    Ok(&b""[..])
                }
            },
        }
    }

    /// Finishes parsing the current packet.
    ///
    /// By default, this drops any unread content.  Use, for instance,
    /// [`PacketParserBuilder`] to customize the default behavior.
    ///
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     let p = pp.finish()?;
    /// #   let _ = p;
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    // Note: this function is public and may be called multiple times!
    pub fn finish(&mut self) -> Result<&Packet> {
        let indent = self.recursion_depth();
        tracer!(TRACE, "PacketParser::finish", indent);

        if self.finished {
            return Ok(&self.packet);
        }

        let recursion_depth = self.recursion_depth();

        let unread_content = if self.state.settings.buffer_unread_content {
            t!("({:?} at depth {}): buffering {} bytes of unread content",
               self.packet.tag(), recursion_depth,
               self.data_eof().unwrap_or(&[]).len());

            !self.buffer_unread_content()?.is_empty()
        } else {
            t!("({:?} at depth {}): dropping {} bytes of unread content",
               self.packet.tag(), recursion_depth,
               self.data_eof().unwrap_or(&[]).len());

            self.drop_eof()?
        };

        if unread_content {
            match self.packet.tag() {
                Tag::SEIP | Tag::AED | Tag::SED | Tag::CompressedData => {
                    // We didn't (fully) process a container's content.  Add
                    // this as opaque content to the message validator.
                    let mut path = self.path().to_vec();
                    path.push(0);
                    #[allow(deprecated)]
                    self.state.message_validator.push_token(
                        message::Token::OpaqueContent, &path);
                }
                _ => {},
            }
        }

        if let Some(c) = self.packet.container_mut() {
            let h = self.body_hash.take()
                .expect("body_hash is Some");
            c.set_body_hash(h);
        }

        self.finished = true;

        Ok(&self.packet)
    }

    /// Hashes content that has been streamed.
    fn hash_read_content(&mut self, b: &[u8]) {
        if !b.is_empty() {
            assert!(self.body_hash.is_some());
            if let Some(h) = self.body_hash.as_mut() {
                h.update(b);
            }
            self.content_was_read = true;
        }
    }

    /// Returns a reference to the current packet's header.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse a message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/compressed-data-algo-0.pgp");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     pp.header().valid(false)?;
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Returns a reference to the map (if any is written).
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
    pub fn map(&self) -> Option<&map::Map> {
        self.map.as_ref()
    }

    /// Takes the map (if any is written).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::parse::{Parse, PacketParserBuilder};
    ///
    /// let message_data = b"\xcb\x12t\x00\x00\x00\x00\x00Hello world.";
    /// let mut pp = PacketParserBuilder::from_bytes(message_data)?
    ///     .map(true) // Enable mapping.
    ///     .build()?
    ///     .expect("One packet, not EOF");
    /// let map = pp.take_map().expect("Mapping is enabled");
    ///
    /// assert_eq!(map.iter().nth(0).unwrap().name(), "CTB");
    /// assert_eq!(map.iter().nth(0).unwrap().offset(), 0);
    /// assert_eq!(map.iter().nth(0).unwrap().as_bytes(), &[0xcb]);
    /// # Ok(()) }
    /// ```
    pub fn take_map(&mut self) -> Option<map::Map> {
        self.map.take()
    }

    /// Checks if we are processing a signed message using the
    /// Cleartext Signature Framework.
    pub(crate) fn processing_csf_message(&self) -> bool {
        Cookie::processing_csf_message(&self.reader)
    }
}

/// This interface allows a caller to read the content of a
/// `PacketParser` using the `Read` interface.  This is essential to
/// supporting streaming operation.
///
/// Note: it is safe to mix the use of the `std::io::Read` and
/// `BufferedReader` interfaces.
impl<'a> io::Read for PacketParser<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // The BufferedReader interface takes care of hashing the read
        // values.
        buffered_reader_generic_read_impl(self, buf)
    }
}

/// This interface allows a caller to read the content of a
/// `PacketParser` using the `BufferedReader` interface.  This is
/// essential to supporting streaming operation.
///
/// Note: it is safe to mix the use of the `std::io::Read` and
/// `BufferedReader` interfaces.
impl<'a> BufferedReader<Cookie> for PacketParser<'a> {
    fn buffer(&self) -> &[u8] {
        self.reader.buffer()
    }

    fn data(&mut self, amount: usize) -> io::Result<&[u8]> {
        // There is no need to set `content_was_read`, because this
        // doesn't actually consume any data.
        self.reader.data(amount)
    }

    fn data_hard(&mut self, amount: usize) -> io::Result<&[u8]> {
        // There is no need to set `content_was_read`, because this
        // doesn't actually consume any data.
        self.reader.data_hard(amount)
    }

    fn data_eof(&mut self) -> io::Result<&[u8]> {
        // There is no need to set `content_was_read`, because this
        // doesn't actually consume any data.
        self.reader.data_eof()
    }

    fn consume(&mut self, amount: usize) -> &[u8] {
        // This is awkward.  Juggle mutable references around.
        if let Some(mut body_hash) = self.body_hash.take() {
            let data = self.data_hard(amount)
                .expect("It is an error to consume more than data returns");
            body_hash.update(&data[..amount]);
            self.body_hash = Some(body_hash);
            self.content_was_read |= amount > 0;
        } else {
            panic!("body_hash is None");
        }

        self.reader.consume(amount)
    }

    fn data_consume(&mut self, mut amount: usize) -> io::Result<&[u8]> {
        // This is awkward.  Juggle mutable references around.
        if let Some(mut body_hash) = self.body_hash.take() {
            let data = self.data(amount)?;
            amount = cmp::min(data.len(), amount);
            body_hash.update(&data[..amount]);
            self.body_hash = Some(body_hash);
            self.content_was_read |= amount > 0;
        } else {
            panic!("body_hash is None");
        }

        self.reader.data_consume(amount)
    }

    fn data_consume_hard(&mut self, amount: usize) -> io::Result<&[u8]> {
        // This is awkward.  Juggle mutable references around.
        if let Some(mut body_hash) = self.body_hash.take() {
            let data = self.data_hard(amount)?;
            body_hash.update(&data[..amount]);
            self.body_hash = Some(body_hash);
            self.content_was_read |= amount > 0;
        } else {
            panic!("body_hash is None");
        }

        self.reader.data_consume_hard(amount)
    }

    fn steal(&mut self, amount: usize) -> io::Result<Vec<u8>> {
        let v = self.reader.steal(amount)?;
        self.hash_read_content(&v);
        Ok(v)
    }

    fn steal_eof(&mut self) -> io::Result<Vec<u8>> {
        let v = self.reader.steal_eof()?;
        self.hash_read_content(&v);
        Ok(v)
    }

    fn get_mut(&mut self) -> Option<&mut dyn BufferedReader<Cookie>> {
        None
    }

    fn get_ref(&self) -> Option<&dyn BufferedReader<Cookie>> {
        None
    }

    fn into_inner<'b>(self: Box<Self>)
            -> Option<Box<dyn BufferedReader<Cookie> + 'b>>
            where Self: 'b {
        None
    }

    fn cookie_set(&mut self, cookie: Cookie)
            -> Cookie {
        self.reader.cookie_set(cookie)
    }

    fn cookie_ref(&self) -> &Cookie {
        self.reader.cookie_ref()
    }

    fn cookie_mut(&mut self) -> &mut Cookie {
        self.reader.cookie_mut()
    }
}

// Check that we can use the read interface to stream the contents of
// a packet.
#[cfg(feature = "compression-deflate")]
#[test]
fn packet_parser_reader_interface() {
    // We need the Read trait.
    use std::io::Read;

    let expected = crate::tests::manifesto();

    // A message containing a compressed packet that contains a
    // literal packet.
    let pp = PacketParser::from_bytes(
        crate::tests::message("compressed-data-algo-1.gpg")).unwrap().unwrap();

    // The message has the form:
    //
    //   [ compressed data [ literal data ] ]
    //
    // packet is the compressed data packet; ppo is the literal data
    // packet.
    let packet_depth = pp.recursion_depth();
    let (packet, ppr) = pp.recurse().unwrap();
    let pp_depth = ppr.as_ref().unwrap().recursion_depth();
    if let Packet::CompressedData(_) = packet {
    } else {
        panic!("Expected a compressed data packet.");
    }

    let relative_position = pp_depth - packet_depth;
    assert_eq!(relative_position, 1);

    let mut pp = ppr.unwrap();

    if let Packet::Literal(_) = pp.packet {
    } else {
        panic!("Expected a literal data packet.");
    }

    // Check that we can read the packet's contents.  We do this one
    // byte at a time to exercise the cursor implementation.
    for i in 0..expected.len() {
        let mut buf = [0u8; 1];
        let r = pp.read(&mut buf).unwrap();
        assert_eq!(r, 1);
        assert_eq!(buf[0], expected[i]);
    }
    // And, now an EOF.
    let mut buf = [0u8; 1];
    let r = pp.read(&mut buf).unwrap();
    assert_eq!(r, 0);

    // Make sure we can still get the next packet (which in this case
    // is just EOF).
    let (packet, ppr) = pp.recurse().unwrap();
    assert!(ppr.is_eof());
    // Since we read all of the data, we expect content to be None.
    assert_eq!(packet.unprocessed_body().unwrap().len(), 0);
}

impl<'a> PacketParser<'a> {
    /// Tries to decrypt the current packet.
    ///
    /// On success, this function pushes one or more readers onto the
    /// `PacketParser`'s reader stack, and sets the packet parser's
    /// `processed` flag (see [`PacketParser::processed`]).
    ///
    ///   [`PacketParser::processed`]: PacketParser::processed()
    ///
    /// If this function is called on a packet that does not contain
    /// encrypted data, or some of the data was already read, then it
    /// returns [`Error::InvalidOperation`].
    ///
    ///   [`Error::InvalidOperation`]: super::Error::InvalidOperation
    ///
    /// # Examples
    ///
    /// ```rust
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::Packet;
    /// use openpgp::fmt::hex;
    /// use openpgp::types::SymmetricAlgorithm;
    /// use openpgp::parse::{Parse, PacketParserResult, PacketParser};
    ///
    /// // Parse an encrypted message.
    /// let message_data: &[u8] = // ...
    /// #    include_bytes!("../tests/data/messages/encrypted-aes256-password-123.gpg");
    /// let mut ppr = PacketParser::from_bytes(message_data)?;
    /// while let PacketParserResult::Some(mut pp) = ppr {
    ///     if let Packet::SEIP(_) = pp.packet {
    ///         pp.decrypt(SymmetricAlgorithm::AES256,
    ///                    &hex::decode("7EF4F08C44F780BEA866961423306166\
    ///                                  B8912C43352F3D9617F745E4E3939710")?
    ///                        .into())?;
    ///     }
    ///
    ///     // Start parsing the next packet, recursing.
    ///     ppr = pp.recurse()?.1;
    /// }
    /// # Ok(()) }
    /// ```
    ///
    /// # Security Considerations
    ///
    /// This functions returns rich errors in case the decryption
    /// fails.  In combination with certain asymmetric algorithms
    /// (RSA), this may lead to compromise of secret key material or
    /// (partial) recovery of the message's plain text.  See [Section
    /// 14 of RFC 4880].
    ///
    ///   [Section 14 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-14
    ///
    /// DO NOT relay these errors in situations where an attacker can
    /// request decryption of messages in an automated fashion.  The
    /// API of the streaming [`Decryptor`] prevents leaking rich
    /// decryption errors.
    ///
    ///   [`Decryptor`]: stream::Decryptor
    ///
    /// Nevertheless, decrypting messages that do not use an
    /// authenticated encryption mode in an automated fashion that
    /// relays or leaks information to a third party is NEVER SAFE due
    /// to unavoidable format oracles, see [Format Oracles on
    /// OpenPGP].
    ///
    ///   [Format Oracles on OpenPGP]: https://www.ssi.gouv.fr/uploads/2015/05/format-Oracles-on-OpenPGP.pdf
    pub fn decrypt(&mut self, algo: SymmetricAlgorithm, key: &SessionKey)
        -> Result<()>
    {
        let indent = self.recursion_depth();
        tracer!(TRACE, "PacketParser::decrypt", indent);

        if self.content_was_read {
            return Err(Error::InvalidOperation(
                "Packet's content has already been read.".to_string()).into());
        }
        if self.processed {
            return Err(Error::InvalidOperation(
                "Packet not encrypted.".to_string()).into());
        }

        if algo.key_size()? != key.len () {
            return Err(Error::InvalidOperation(
                format!("Bad key size: {} expected: {}",
                        key.len(), algo.key_size()?)).into());
        }

        match self.packet.clone() {
            Packet::SEIP(_) => {
                // Get the first blocksize plus two bytes and check
                // whether we can decrypt them using the provided key.
                // Don't actually consume them in case we can't.
                let bl = algo.block_size()?;

                {
                    let mut dec = Decryptor::new(
                        algo, key, &self.data_hard(bl + 2)?[..bl + 2])?;
                    let mut header = vec![ 0u8; bl + 2 ];
                    dec.read_exact(&mut header)?;

                    if !(header[bl - 2] == header[bl]
                         && header[bl - 1] == header[bl + 1]) {
                        return Err(Error::InvalidSessionKey(
                            "Decryption failed".into()).into());
                    }
                }

                // Ok, we can decrypt the data.  Push a Decryptor and
                // a HashedReader on the `BufferedReader` stack.

                // This can't fail, because we create a decryptor
                // above with the same parameters.
                let reader = self.take_reader();
                let mut reader = BufferedReaderDecryptor::with_cookie(
                    algo, key, reader, Cookie::default()).unwrap();
                reader.cookie_mut().level = Some(self.recursion_depth());

                t!("Pushing Decryptor, level {:?}.", reader.cookie_ref().level);

                // And the hasher.
                let mut reader = HashedReader::new(
                    reader, HashesFor::MDC,
                    vec![HashingMode::Binary(HashAlgorithm::SHA1)]);
                reader.cookie_mut().level = Some(self.recursion_depth());

                t!("Pushing HashedReader, level {:?}.",
                   reader.cookie_ref().level);

                // A SEIP packet is a container that always ends with
                // an MDC packet.  But, if the packet preceding the
                // MDC packet uses an indeterminate length encoding
                // (gpg generates these for compressed data packets,
                // for instance), the parser has to detect the EOF and
                // be careful to not read any further.  Unfortunately,
                // our decompressor buffers the data.  To stop the
                // decompressor from buffering the MDC packet, we use
                // a buffered_reader::Reserve.  Note: we do this
                // unconditionally, since it doesn't otherwise
                // interfere with parsing.

                // An MDC consists of a 1-byte CTB, a 1-byte length
                // encoding, and a 20-byte hash.
                let mut reader = buffered_reader::Reserve::with_cookie(
                    reader, 1 + 1 + 20,
                    Cookie::new(self.recursion_depth()));
                reader.cookie_mut().fake_eof = true;

                t!("Pushing buffered_reader::Reserve, level: {}.",
                   self.recursion_depth());

                // Consume the header.  This shouldn't fail, because
                // it worked when reading the header.
                reader.data_consume_hard(bl + 2).unwrap();

                self.reader = Box::new(reader);
                self.processed = true;

                Ok(())
            },

            Packet::AED(AED::V1(aed)) => {
                let chunk_size =
                    aead::chunk_size_usize(aed.chunk_size())?;

                // Read the first chunk and check whether we can
                // decrypt it using the provided key.  Don't actually
                // consume them in case we can't.
                {
                    // We need a bit more than one chunk so that
                    // `aead::Decryptor` won't see EOF and think that
                    // it has a partial block and it needs to verify
                    // the final chunk.
                    let amount = aead::chunk_size_usize(
                        aed.chunk_digest_size()?
                        + aed.aead().digest_size()? as u64)?;

                    let data = self.data(amount)?;
                    let schedule = aead::AEDv1Schedule::new(
                        aed.symmetric_algo(),
                        aed.aead(),
                        chunk_size,
                        aed.iv())?;

                    let dec = aead::Decryptor::new(
                        aed.symmetric_algo(), aed.aead(), chunk_size,
                        schedule, key.clone(),
                        &data[..cmp::min(data.len(), amount)])?;
                    let mut chunk = Vec::new();
                    dec.take(aed.chunk_size() as u64).read_to_end(&mut chunk)?;
                }

                // Ok, we can decrypt the data.  Push a Decryptor and
                // a HashedReader on the `BufferedReader` stack.

                // This can't fail, because we create a decryptor
                // above with the same parameters.
                let schedule = aead::AEDv1Schedule::new(
                    aed.symmetric_algo(),
                    aed.aead(),
                    chunk_size,
                    aed.iv())?;

                let reader = self.take_reader();
                let mut reader = aead::BufferedReaderDecryptor::with_cookie(
                    aed.symmetric_algo(), aed.aead(), chunk_size,
                    schedule, key.clone(), reader, Cookie::default()).unwrap();
                reader.cookie_mut().level = Some(self.recursion_depth());

                t!("Pushing aead::Decryptor, level {:?}.",
                   reader.cookie_ref().level);

                self.reader = Box::new(reader);
                self.processed = true;

                Ok(())
            },

            _ =>
                Err(Error::InvalidOperation(
                    format!("Can't decrypt {:?} packets.",
                            self.packet.tag())).into())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::serialize::Serialize;

    enum Data<'a> {
        File(&'a str),
        String(&'a [u8]),
    }

    impl<'a> Data<'a> {
        fn content(&self) -> Vec<u8> {
            match self {
                Data::File(filename) => crate::tests::message(filename).to_vec(),
                Data::String(data) => data.to_vec(),
            }
        }
    }

    struct DecryptTest<'a> {
        filename: &'a str,
        algo: SymmetricAlgorithm,
        key_hex: &'a str,
        plaintext: Data<'a>,
        paths: &'a[ (Tag, &'a[ usize ] ) ],
    }
    const DECRYPT_TESTS: &[DecryptTest] = &[
        // Messages with a relatively simple structure:
        //
        //   [ SKESK SEIP [ Literal MDC ] ].
        //
        // And simple length encodings (no indeterminate length
        // encodings).
        DecryptTest {
            filename: "encrypted-aes256-password-123.gpg",
            algo: SymmetricAlgorithm::AES256,
            key_hex: "7EF4F08C44F780BEA866961423306166B8912C43352F3D9617F745E4E3939710",
            plaintext: Data::File("a-cypherpunks-manifesto.txt"),
            paths: &[
                (Tag::SKESK, &[ 0 ]),
                (Tag::SEIP, &[ 1 ]),
                (Tag::Literal, &[ 1, 0 ]),
                (Tag::MDC, &[ 1, 1 ]),
            ],
        },
        DecryptTest {
            filename: "encrypted-aes192-password-123456.gpg",
            algo: SymmetricAlgorithm::AES192,
            key_hex: "B2F747F207EFF198A6C826F1D398DE037986218ED468DB61",
            plaintext: Data::File("a-cypherpunks-manifesto.txt"),
            paths: &[
                (Tag::SKESK, &[ 0 ]),
                (Tag::SEIP, &[ 1 ]),
                (Tag::Literal, &[ 1, 0 ]),
                (Tag::MDC, &[ 1, 1 ]),
            ],
        },
        DecryptTest {
            filename: "encrypted-aes128-password-123456789.gpg",
            algo: SymmetricAlgorithm::AES128,
            key_hex: "AC0553096429260B4A90B1CEC842D6A0",
            plaintext: Data::File("a-cypherpunks-manifesto.txt"),
            paths: &[
                (Tag::SKESK, &[ 0 ]),
                (Tag::SEIP, &[ 1 ]),
                (Tag::Literal, &[ 1, 0 ]),
                (Tag::MDC, &[ 1, 1 ]),
            ],
        },
        DecryptTest {
            filename: "encrypted-twofish-password-red-fish-blue-fish.gpg",
            algo: SymmetricAlgorithm::Twofish,
            key_hex: "96AFE1EDFA7C9CB7E8B23484C718015E5159CFA268594180D4DB68B2543393CB",
            plaintext: Data::File("a-cypherpunks-manifesto.txt"),
            paths: &[
                (Tag::SKESK, &[ 0 ]),
                (Tag::SEIP, &[ 1 ]),
                (Tag::Literal, &[ 1, 0 ]),
                (Tag::MDC, &[ 1, 1 ]),
            ],
        },

        // More complex messages.  In particular, some of these
        // messages include compressed data packets, and some are
        // signed.  But what makes these particularly complex is the
        // use of an indeterminate length encoding, which checks the
        // buffered_reader::Reserve hack.
        #[cfg(feature = "compression-deflate")]
        DecryptTest {
            filename: "seip/msg-compression-not-signed-password-123.pgp",
            algo: SymmetricAlgorithm::AES128,
            key_hex: "86A8C1C7961F55A3BE181A990D0ABB2A",
            plaintext: Data::String(b"compression, not signed\n"),
            paths: &[
                (Tag::SKESK, &[ 0 ]),
                (Tag::SEIP, &[ 1 ]),
                (Tag::CompressedData, &[ 1, 0 ]),
                (Tag::Literal, &[ 1, 0, 0 ]),
                (Tag::MDC, &[ 1, 1 ]),
            ],
        },
        #[cfg(feature = "compression-deflate")]
        DecryptTest {
            filename: "seip/msg-compression-signed-password-123.pgp",
            algo: SymmetricAlgorithm::AES128,
            key_hex: "1B195CD35CAD4A99D9399B4CDA4CDA4E",
            plaintext: Data::String(b"compression, signed\n"),
            paths: &[
                (Tag::SKESK, &[ 0 ]),
                (Tag::SEIP, &[ 1 ]),
                (Tag::CompressedData, &[ 1, 0 ]),
                (Tag::OnePassSig, &[ 1, 0, 0 ]),
                (Tag::Literal, &[ 1, 0, 1 ]),
                (Tag::Signature, &[ 1, 0, 2 ]),
                (Tag::MDC, &[ 1, 1 ]),
            ],
        },
        DecryptTest {
            filename: "seip/msg-no-compression-not-signed-password-123.pgp",
            algo: SymmetricAlgorithm::AES128,
            key_hex: "AFB43B83A4B9D971E4B4A4C53749076A",
            plaintext: Data::String(b"no compression, not signed\n"),
            paths: &[
                (Tag::SKESK, &[ 0 ]),
                (Tag::SEIP, &[ 1 ]),
                (Tag::Literal, &[ 1, 0 ]),
                (Tag::MDC, &[ 1, 1 ]),
            ],
        },
        DecryptTest {
            filename: "seip/msg-no-compression-signed-password-123.pgp",
            algo: SymmetricAlgorithm::AES128,
            key_hex: "9D5DB92F77F0E4A356EE53813EF2C3DC",
            plaintext: Data::String(b"no compression, signed\n"),
            paths: &[
                (Tag::SKESK, &[ 0 ]),
                (Tag::SEIP, &[ 1 ]),
                (Tag::OnePassSig, &[ 1, 0 ]),
                (Tag::Literal, &[ 1, 1 ]),
                (Tag::Signature, &[ 1, 2 ]),
                (Tag::MDC, &[ 1, 3 ]),
            ],
        },

        // AEAD encrypted messages.
        DecryptTest {
            filename: "aed/msg-aes128-eax-chunk-size-64-password-123.pgp",
            algo: SymmetricAlgorithm::AES128,
            key_hex: "E88151F2B6F6F6F0AE6B56ED247AA61B",
            plaintext: Data::File("a-cypherpunks-manifesto.txt"),
            paths: &[
                (Tag::SKESK, &[ 0 ]),
                (Tag::AED, &[ 1 ]),
                (Tag::Literal, &[ 1, 0 ]),
            ],
        },
        DecryptTest {
            filename: "aed/msg-aes128-eax-chunk-size-4194304-password-123.pgp",
            algo: SymmetricAlgorithm::AES128,
            key_hex: "918E6BF5C6CE4320D014735AF27BFA76",
            plaintext: Data::File("a-cypherpunks-manifesto.txt"),
            paths: &[
                (Tag::SKESK, &[ 0 ]),
                (Tag::AED, &[ 1 ]),
                (Tag::Literal, &[ 1, 0 ]),
            ],
        },
    ];

    // Consume packets until we get to one in `keep`.
    fn consume_until<'a>(mut ppr: PacketParserResult<'a>,
                         ignore_first: bool, keep: &[Tag], skip: &[Tag])
        -> PacketParserResult<'a>
    {
        if ignore_first {
            ppr = ppr.unwrap().recurse().unwrap().1;
        }

        while let PacketParserResult::Some(pp) = ppr {
            let tag = pp.packet.tag();
            for t in keep.iter() {
                if *t == tag {
                    return PacketParserResult::Some(pp);
                }
            }

            let mut ok = false;
            for t in skip.iter() {
                if *t == tag {
                    ok = true;
                }
            }
            if !ok {
                panic!("Packet not in keep ({:?}) or skip ({:?}) set: {:?}",
                       keep, skip, pp.packet);
            }

            ppr = pp.recurse().unwrap().1;
        }
        ppr
    }

    #[test]
    fn decrypt_test() {
        decrypt_test_common(false);
    }

    #[test]
    fn decrypt_test_stream() {
        decrypt_test_common(true);
    }

    fn decrypt_test_common(stream: bool) {
        for test in DECRYPT_TESTS.iter() {
            if !test.algo.is_supported() {
                eprintln!("Algorithm {} unsupported, skipping", test.algo);
                continue;
            }

            eprintln!("Decrypting {}, streaming content: {}",
                      test.filename, stream);

            let ppr = PacketParserBuilder::from_bytes(
                crate::tests::message(test.filename)).unwrap()
                .buffer_unread_content()
                .build()
                .expect(&format!("Error reading {}", test.filename)[..]);

            let mut ppr = consume_until(
                ppr, false, &[ Tag::SEIP, Tag::AED ][..],
                &[ Tag::SKESK, Tag::PKESK ][..] );
            if let PacketParserResult::Some(ref mut pp) = ppr {
                let key = crate::fmt::from_hex(test.key_hex, false)
                    .unwrap().into();

                pp.decrypt(test.algo, &key).unwrap();
            } else {
                panic!("Expected a SEIP/AED packet.  Got: {:?}", ppr);
            }

            let mut ppr = consume_until(
                ppr, true, &[ Tag::Literal ][..],
                &[ Tag::OnePassSig, Tag::CompressedData ][..]);
            if let PacketParserResult::Some(ref mut pp) = ppr {
                if stream {
                    let mut body = Vec::new();
                    loop {
                        let mut b = [0];
                        if pp.read(&mut b).unwrap() == 0 {
                            break;
                        }
                        body.push(b[0]);
                    }

                    assert_eq!(&body[..],
                               &test.plaintext.content()[..],
                               "{:?}", pp.packet);
                } else {
                    pp.buffer_unread_content().unwrap();
                    if let Packet::Literal(l) = &pp.packet {
                        assert_eq!(l.body(), &test.plaintext.content()[..],
                                   "{:?}", pp.packet);
                    } else {
                        panic!("Expected literal, got: {:?}", pp.packet);
                    }
                }
            } else {
                panic!("Expected a Literal packet.  Got: {:?}", ppr);
            }

            let ppr = consume_until(
                ppr, true, &[ Tag::MDC ][..], &[ Tag::Signature ][..]);
            if let PacketParserResult::Some(
                PacketParser { packet: Packet::MDC(ref mdc), .. }) = ppr
            {
                assert_eq!(mdc.computed_digest(), mdc.digest(),
                           "MDC doesn't match");
            }

            if ppr.is_eof() {
                // AED packets don't have an MDC packet.
                continue;
            }
            let ppr = consume_until(
                ppr, true, &[][..], &[][..]);
            assert!(ppr.is_eof());
        }
    }

    #[test]
    fn message_validator() {
      for marker in 0..4 {
        let marker_before = marker & 1 > 0;
        let marker_after = marker & 2 > 0;

        for test in DECRYPT_TESTS.iter() {
            if !test.algo.is_supported() {
                eprintln!("Algorithm {} unsupported, skipping", test.algo);
                continue;
            }

            let mut buf = Vec::new();
            if marker_before {
                Packet::Marker(Default::default()).serialize(&mut buf).unwrap();
            }
            buf.extend_from_slice(crate::tests::message(test.filename));
            if marker_after {
                Packet::Marker(Default::default()).serialize(&mut buf).unwrap();
            }

            let mut ppr = PacketParserBuilder::from_bytes(&buf)
                .unwrap()
                .build()
                .expect(&format!("Error reading {}", test.filename)[..]);

            // Make sure we actually decrypted...
            let mut saw_literal = false;
            while let PacketParserResult::Some(mut pp) = ppr {
                assert!(pp.possible_message().is_ok());

                match pp.packet {
                    Packet::SEIP(_) | Packet::AED(_) => {
                        let key = crate::fmt::from_hex(test.key_hex, false)
                            .unwrap().into();
                        pp.decrypt(test.algo, &key).unwrap();
                    },
                    Packet::Literal(_) => {
                        assert!(! saw_literal);
                        saw_literal = true;
                    },
                    _ => {},
                }

                ppr = pp.recurse().unwrap().1;
            }
            assert!(saw_literal);
            if let PacketParserResult::EOF(eof) = ppr {
                assert!(eof.is_message().is_ok());
            } else {
                unreachable!();
            }
        }
      }
    }

    #[test]
    fn keyring_validator() {
      for marker in 0..4 {
        let marker_before = marker & 1 > 0;
        let marker_after = marker & 2 > 0;

        for test in &["testy.pgp",
                      "lutz.gpg",
                      "testy-new.pgp",
                      "neal.pgp"]
        {
            let mut buf = Vec::new();
            if marker_before {
                Packet::Marker(Default::default()).serialize(&mut buf).unwrap();
            }
            buf.extend_from_slice(crate::tests::key("testy.pgp"));
            buf.extend_from_slice(crate::tests::key(test));
            if marker_after {
                Packet::Marker(Default::default()).serialize(&mut buf).unwrap();
            }

            let mut ppr = PacketParserBuilder::from_bytes(&buf)
                .unwrap()
                .build()
                .expect(&format!("Error reading {:?}", test));

            while let PacketParserResult::Some(pp) = ppr {
                assert!(pp.possible_keyring().is_ok());
                ppr = pp.recurse().unwrap().1;
            }
            if let PacketParserResult::EOF(eof) = ppr {
                assert!(eof.is_keyring().is_ok());
                assert!(eof.is_cert().is_err());
            } else {
                unreachable!();
            }
        }
      }
    }

    #[test]
    fn cert_validator() {
      for marker in 0..4 {
        let marker_before = marker & 1 > 0;
        let marker_after = marker & 2 > 0;

        for test in &["testy.pgp",
                      "lutz.gpg",
                      "testy-new.pgp",
                      "neal.pgp"]
        {
            let mut buf = Vec::new();
            if marker_before {
                Packet::Marker(Default::default()).serialize(&mut buf).unwrap();
            }
            buf.extend_from_slice(crate::tests::key(test));
            if marker_after {
                Packet::Marker(Default::default()).serialize(&mut buf).unwrap();
            }

            let mut ppr = PacketParserBuilder::from_bytes(&buf)
                .unwrap()
                .build()
                .expect(&format!("Error reading {:?}", test));

            while let PacketParserResult::Some(pp) = ppr {
                assert!(pp.possible_keyring().is_ok());
                assert!(pp.possible_cert().is_ok());
                ppr = pp.recurse().unwrap().1;
            }
            if let PacketParserResult::EOF(eof) = ppr {
                assert!(eof.is_keyring().is_ok());
                assert!(eof.is_cert().is_ok());
            } else {
                unreachable!();
            }
        }
      }
    }

    // If we don't decrypt the SEIP packet, it shows up as opaque
    // content.
    #[test]
    fn message_validator_opaque_content() {
        for test in DECRYPT_TESTS.iter() {
            let mut ppr = PacketParserBuilder::from_bytes(
                crate::tests::message(test.filename)).unwrap()
                .build()
                .expect(&format!("Error reading {}", test.filename)[..]);

            let mut saw_literal = false;
            while let PacketParserResult::Some(pp) = ppr {
                assert!(pp.possible_message().is_ok());

                match pp.packet {
                    Packet::Literal(_) => {
                        assert!(! saw_literal);
                        saw_literal = true;
                    },
                    _ => {},
                }

                ppr = pp.recurse().unwrap().1;
            }
            assert!(! saw_literal);
            if let PacketParserResult::EOF(eof) = ppr {
                eprintln!("eof: {:?}; message: {:?}", eof, eof.is_message());
                assert!(eof.is_message().is_ok());
            } else {
                unreachable!();
            }
        }
    }

    #[test]
    fn path() {
        for test in DECRYPT_TESTS.iter() {
            if !test.algo.is_supported() {
                eprintln!("Algorithm {} unsupported, skipping", test.algo);
                continue;
            }

            eprintln!("Decrypting {}", test.filename);

            let mut ppr = PacketParserBuilder::from_bytes(
                crate::tests::message(test.filename)).unwrap()
                .build()
                .expect(&format!("Error reading {}", test.filename)[..]);

            let mut last_path = vec![];

            let mut paths = test.paths.to_vec();
            // We pop from the end.
            paths.reverse();

            while let PacketParserResult::Some(mut pp) = ppr {
                let path = paths.pop().expect("Message longer than expect");
                assert_eq!(path.0, pp.packet.tag());
                assert_eq!(path.1, pp.path());

                assert_eq!(last_path, pp.last_path());
                last_path = pp.path.to_vec();

                eprintln!("  {}: {:?}", pp.packet.tag(), pp.path());

                match pp.packet {
                    Packet::SEIP(_) | Packet::AED(_) => {
                        let key = crate::fmt::from_hex(test.key_hex, false)
                            .unwrap().into();

                        pp.decrypt(test.algo, &key).unwrap();
                    }
                    _ => (),
                }

                ppr = pp.recurse().unwrap().1;
            }
            paths.reverse();
            assert_eq!(paths.len(), 0,
                       "Message shorter than expected (expecting: {:?})",
                       paths);

            if let PacketParserResult::EOF(eof) = ppr {
                assert_eq!(last_path, eof.last_path());
            } else {
                panic!("Expect an EOF");
            }
        }
    }

    #[test]
    fn corrupted_cert() {
        use crate::armor::{Reader, ReaderMode, Kind};

        // The following Cert is corrupted about a third the way
        // through.  Make sure we can recover.
        let mut ppr = PacketParser::from_reader(
            Reader::from_bytes(crate::tests::key("corrupted.pgp"),
                               ReaderMode::Tolerant(Some(Kind::PublicKey))))
            .unwrap();

        let mut sigs = 0;
        let mut subkeys = 0;
        let mut userids = 0;
        let mut uas = 0;
        let mut unknown = 0;
        while let PacketParserResult::Some(pp) = ppr {
            match pp.packet {
                Packet::Signature(_) => sigs += 1,
                Packet::PublicSubkey(_) => subkeys += 1,
                Packet::UserID(_) => userids += 1,
                Packet::UserAttribute(_) => uas += 1,
                Packet::Unknown(_) => {
                    unknown += 1;
                },
                _ => (),
            }

            ppr = pp.next().unwrap().1;
        }

        assert_eq!(sigs, 53);
        assert_eq!(subkeys, 3);
        assert_eq!(userids, 5);
        assert_eq!(uas, 0);
        assert_eq!(unknown, 2);
    }

    #[test]
    fn junk_prefix() {
        // Make sure we can read the first packet.
        let msg = crate::tests::message("sig.gpg");

        let ppr = PacketParserBuilder::from_bytes(msg).unwrap()
            .dearmor(packet_parser_builder::Dearmor::Disabled)
            .build();
        assert_match!(Ok(PacketParserResult::Some(ref _pp)) = ppr);


        // Prepend an invalid byte and make sure we fail.  Note: we
        // have a mechanism to skip corruption, however, that is only
        // activated once we've seen a good packet.  This test checks
        // that we don't try to recover.
        let mut msg2 = Vec::new();
        msg2.push(0);
        msg2.extend_from_slice(msg);

        let ppr = PacketParserBuilder::from_bytes(&msg2[..]).unwrap()
            .dearmor(packet_parser_builder::Dearmor::Disabled)
            .build();
        assert_match!(Err(_) = ppr);
    }

    /// Issue #141.
    #[test]
    fn truncated_packet() {
        for msg in &[crate::tests::message("literal-mode-b.gpg"),
                     crate::tests::message("literal-mode-t-partial-body.gpg"),
        ] {
            // Make sure we can read the first packet.
            let ppr = PacketParserBuilder::from_bytes(msg).unwrap()
                .dearmor(packet_parser_builder::Dearmor::Disabled)
                .build();
            assert_match!(Ok(PacketParserResult::Some(ref _pp)) = ppr);

            // Now truncate the packet.
            let msg2 = &msg[..msg.len() - 1];
            let ppr = PacketParserBuilder::from_bytes(msg2).unwrap()
                .dearmor(packet_parser_builder::Dearmor::Disabled)
                .build().unwrap();
            if let PacketParserResult::Some(pp) = ppr {
                let err = pp.next().err().unwrap();
                assert_match!(Some(&Error::MalformedPacket(_))
                              = err.downcast_ref());
            } else {
                panic!("No packet!?");
            }
        }
    }

    #[test]
    fn max_packet_size() {
        use crate::serialize::Serialize;
        let uid = Packet::UserID("foobar".into());
        let mut buf = Vec::new();
        uid.serialize(&mut buf).unwrap();

        // Make sure we can read it.
        let ppr = PacketParserBuilder::from_bytes(&buf).unwrap()
            .build().unwrap();
        if let PacketParserResult::Some(pp) = ppr {
            assert_eq!(Packet::UserID("foobar".into()), pp.packet);
        } else {
            panic!("failed to parse userid");
        }

        // But if we set the maximum packet size too low, it is parsed
        // into a unknown packet.
        let ppr = PacketParserBuilder::from_bytes(&buf).unwrap()
            .max_packet_size(5)
            .build().unwrap();
        if let PacketParserResult::Some(pp) = ppr {
            if let Packet::Unknown(ref u) = pp.packet {
                assert_eq!(u.tag(), Tag::UserID);
                assert_match!(Some(&Error::PacketTooLarge(_, _, _))
                              = u.error().downcast_ref());
            } else {
                panic!("expected an unknown packet, got {:?}", pp.packet);
            }
        } else {
            panic!("failed to parse userid");
        }

    }

    /// We erroneously assumed that when BufferedReader::next() is
    /// called, a SEIP container be opaque and hence there cannot be a
    /// buffered_reader::Reserve on the stack with Cookie::fake_eof
    /// set.  But, we could simply call BufferedReader::next() after
    /// the SEIP packet is decrypted, or buffer a SEIP packet's body,
    /// then call BufferedReader::recurse(), which falls back to
    /// BufferedReader::next() because some data has been read.
    #[test]
    fn issue_455() -> Result<()> {
        let sk: SessionKey =
            crate::fmt::hex::decode("3E99593760EE241488462BAFAE4FA268\
                                     260B14B82D310D196DCEC82FD4F67678")?.into();
        let algo = SymmetricAlgorithm::AES256;

        // Decrypt, then call BufferedReader::next().
        eprintln!("Decrypt, then next():\n");
        let mut ppr = PacketParser::from_bytes(
            crate::tests::message("encrypted-to-testy.gpg"))?;
        while let PacketParserResult::Some(mut pp) = ppr {
            match &pp.packet {
                Packet::SEIP(_) => {
                    pp.decrypt(algo, &sk)?;
                },
                _ => (),
            }
            // Used to trigger the assertion failure on the SEIP
            // packet:
            ppr = pp.next()?.1;
        }

        // Decrypt, buffer, then call BufferedReader::recurse().
        eprintln!("\nDecrypt, buffer, then recurse():\n");
        let mut ppr = PacketParser::from_bytes(
            crate::tests::message("encrypted-to-testy.gpg"))?;
        while let PacketParserResult::Some(mut pp) = ppr {
            match &pp.packet {
                Packet::SEIP(_) => {
                    pp.decrypt(algo, &sk)?;
                    pp.buffer_unread_content()?;
                },
                _ => (),
            }
            // Used to trigger the assertion failure on the SEIP
            // packet:
            ppr = pp.recurse()?.1;
        }
        Ok(())
    }

    /// Crash in the AED parser due to missing chunk size validation.
    #[test]
    fn issue_514() -> Result<()> {
        let data = &[212, 43, 1, 0, 0, 125, 212, 0, 10, 10, 10];
        let ppr = PacketParser::from_bytes(&data)?;
        let packet = &ppr.unwrap().packet;
        if let Packet::Unknown(_) = packet {
            Ok(())
        } else {
            panic!("expected unknown packet, got: {:?}", packet);
        }
    }

    /// Malformed subpackets must not cause a hard parsing error.
    #[test]
    fn malformed_embedded_signature() -> Result<()> {
        let ppr = PacketParser::from_bytes(
            crate::tests::file("edge-cases/malformed-embedded-sig.pgp"))?;
        let packet = &ppr.unwrap().packet;
        if let Packet::Unknown(_) = packet {
            Ok(())
        } else {
            panic!("expected unknown packet, got: {:?}", packet);
        }
    }

    /// Malformed notation names must not cause hard parsing errors.
    #[test]
    fn malformed_notation_name() -> Result<()> {
        let ppr = PacketParser::from_bytes(
            crate::tests::file("edge-cases/malformed-notation-name.pgp"))?;
        let packet = &ppr.unwrap().packet;
        if let Packet::Unknown(_) = packet {
            Ok(())
        } else {
            panic!("expected unknown packet, got: {:?}", packet);
        }
    }

    /// Checks that the content hash is correctly computed whether or
    /// not the content has been (fully) read.
    #[test]
    fn issue_537() -> Result<()> {
        // Buffer unread content.
        let ppr0 = PacketParserBuilder::from_bytes(
            crate::tests::message("literal-mode-b.gpg"))?
            .buffer_unread_content()
            .build()?;
        let pp0 = ppr0.unwrap();
        let (packet0, _) = pp0.recurse()?;

        // Drop unread content.
        let ppr1 = PacketParser::from_bytes(
            crate::tests::message("literal-mode-b.gpg"))?;
        let pp1 = ppr1.unwrap();
        let (packet1, _) = pp1.recurse()?;

        // Read content.
        let ppr2 = PacketParser::from_bytes(
            crate::tests::message("literal-mode-b.gpg"))?;
        let mut pp2 = ppr2.unwrap();
        io::copy(&mut pp2, &mut io::sink())?;
        let (packet2, _) = pp2.recurse()?;

        // Partially read content.
        let ppr3 = PacketParser::from_bytes(
            crate::tests::message("literal-mode-b.gpg"))?;
        let mut pp3 = ppr3.unwrap();
        let mut buf = [0];
        let nread = pp3.read(&mut buf)?;
        assert_eq!(buf.len(), nread);
        let (packet3, _) = pp3.recurse()?;

        assert_eq!(packet0, packet1);
        assert_eq!(packet1, packet2);
        assert_eq!(packet2, packet3);
        Ok(())
    }

    /// Checks that newlines are properly normalized when verifying
    /// text signatures.
    #[test]
    fn issue_530_verifying() -> Result<()> {
        use std::io::Write;
        use crate::*;
        use crate::packet::signature;
        use crate::serialize::stream::{Message, Signer};

        use crate::policy::StandardPolicy;
        use crate::{Result, Cert};
        use crate::parse::Parse;
        use crate::parse::stream::*;

        let data = b"one\r\ntwo\r\nthree";

        let p = &StandardPolicy::new();
        let cert: Cert =
            Cert::from_bytes(crate::tests::key("testy-new-private.pgp"))?;
        let signing_keypair = cert.keys().secret()
            .with_policy(p, None).alive().revoked(false).for_signing().next().unwrap()
            .key().clone().into_keypair()?;
        let mut signature = vec![];
        {
            let message = Message::new(&mut signature);
            let mut message = Signer::with_template(
                message, signing_keypair,
                signature::SignatureBuilder::new(SignatureType::Text)
            ).detached().build()?;
            message.write_all(data)?;
            message.finalize()?;
        }

        struct Helper {}
        impl VerificationHelper for Helper {
            fn get_certs(&mut self, _ids: &[KeyHandle]) -> Result<Vec<Cert>> {
                Ok(vec![Cert::from_bytes(crate::tests::key("testy-new.pgp"))?])
            }
            fn check(&mut self, structure: MessageStructure) -> Result<()> {
                for (i, layer) in structure.iter().enumerate() {
                    assert_eq!(i, 0);
                    if let MessageLayer::SignatureGroup { results } = layer {
                        assert_eq!(results.len(), 1);
                        results[0].as_ref().unwrap();
                        assert!(results[0].is_ok());
                        return Ok(());
                    } else {
                        unreachable!();
                    }
                }
                unreachable!()
            }
        }

        let h = Helper {};
        let mut v = DetachedVerifierBuilder::from_bytes(&signature)?
            .with_policy(p, None, h)?;

        for data in &[
            &b"one\r\ntwo\r\nthree"[..], // dos
            b"one\ntwo\nthree",          // unix
            b"one\ntwo\r\nthree",        // mixed
            b"one\r\ntwo\nthree",
            b"one\rtwo\rthree",          // classic mac
        ] {
            v.verify_bytes(data)?;
        }

        Ok(())
    }

    /// Tests for a panic in the SKESK parser.
    #[test]
    fn issue_588() -> Result<()> {
        let data = vec![0x8c, 0x34, 0x05, 0x12, 0x02, 0x00, 0xaf, 0x0d,
                        0xff, 0xff, 0x65];
        let _ = PacketParser::from_bytes(&data);
        Ok(())
    }

    /// Tests for a panic in the packet parser.
    #[test]
    fn packet_parser_on_mangled_cert() -> Result<()> {
        // The armored input cert is mangled.  Currently, Sequoia
        // doesn't grok the mangled armor, but it should not panic.
        let mut ppr = match PacketParser::from_bytes(
            crate::tests::key("bobs-cert-badly-mangled.asc")) {
            Ok(ppr) => ppr,
            Err(_) => return Ok(()),
        };
        while let PacketParserResult::Some(pp) = ppr {
            dbg!(&pp.packet);
            if let Ok((_, tmp)) = pp.recurse() {
                ppr = tmp;
            } else {
                break;
            }
        }
        Ok(())
    }
}
