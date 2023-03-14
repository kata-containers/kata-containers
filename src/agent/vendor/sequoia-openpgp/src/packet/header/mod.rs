//! OpenPGP packet headers.
//!
//! An OpenPGP packet header contains packet meta-data.  Specifically,
//! it includes the [packet's type] (its so-called *tag*), and the
//! [packet's length].
//!
//! Decades ago, when OpenPGP was conceived, saving even a few bits
//! was considered important.  As such, OpenPGP uses very compact
//! encodings.  The encoding schemes have evolved so that there are
//! now two families: the so-called old format, and new format
//! encodings.
//!
//! [packet's type]: https://tools.ietf.org/html/rfc4880#section-4.3
//! [packet's length]: https://tools.ietf.org/html/rfc4880#section-4.2.1

use crate::{
    Error,
    Result,
};
use crate::packet::tag::Tag;
mod ctb;
pub use self::ctb::{
    CTB,
    CTBOld,
    CTBNew,
    PacketLengthType,
};

/// A packet's header.
///
/// See [Section 4.2 of RFC 4880] for details.
///
/// [Section 4.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2
#[derive(Clone, Debug)]
pub struct Header {
    /// The packet's CTB.
    ctb: CTB,
    /// The packet's length.
    length: BodyLength,
}
assert_send_and_sync!(Header);

impl Header {
    /// Creates a new header.
    pub fn new(ctb: CTB, length: BodyLength) -> Self {
        Header { ctb, length }
    }

    /// Returns the header's CTB.
    pub fn ctb(&self) -> &CTB {
        &self.ctb
    }

    /// Returns the header's length.
    pub fn length(&self) -> &BodyLength {
        &self.length
    }

    /// Checks the header for validity.
    ///
    /// A header is consider invalid if:
    ///
    ///   - The tag is [`Tag::Reserved`].
    ///   - The tag is [`Tag::Unknown`] or [`Tag::Private`] and
    ///     `future_compatible` is false.
    ///   - The [length encoding] is invalid for the packet (e.g.,
    ///     partial body encoding may not be used for [`PKESK`] packets)
    ///   - The lengths are unreasonable for a packet (e.g., a
    ///     `PKESK` or [`SKESK`] larger than 10 KB).
    ///
    /// [`Tag::Reserved`]: super::Tag::Reserved
    /// [`Tag::Unknown`]: super::Tag::Unknown
    /// [`Tag::Private`]: super::Tag::Private
    /// [length encoding]: https://tools.ietf.org/html/rfc4880#section-4.2.2.4
    /// [`PKESK`]: super::PKESK
    /// [`SKESK`]: super::SKESK
    // Note: To check the packet's content, use
    //       `PacketParser::plausible`.
    pub fn valid(&self, future_compatible: bool) -> Result<()> {
        let tag = self.ctb.tag();

        match tag {
            // Reserved packets are never valid.
            Tag::Reserved =>
                return Err(Error::UnsupportedPacketType(tag).into()),
            // Unknown packets are not valid unless we want future compatibility.
            Tag::Unknown(_) | Tag::Private(_) if !future_compatible =>
                return Err(Error::UnsupportedPacketType(tag).into()),
            _ => (),
        }

        // An implementation MAY use Partial Body Lengths for data
        // packets, be they literal, compressed, or encrypted.  The
        // first partial length MUST be at least 512 octets long.
        // Partial Body Lengths MUST NOT be used for any other packet
        // types.
        //
        // https://tools.ietf.org/html/rfc4880#section-4.2.2.4
        if tag == Tag::Literal || tag == Tag::CompressedData
            || tag == Tag::SED || tag == Tag::SEIP
            || tag == Tag::AED
        {
            // Data packet.
            match self.length {
                BodyLength::Indeterminate => (),
                BodyLength::Partial(l) => {
                    if l < 512 {
                        return Err(Error::MalformedPacket(
                            format!("Partial body length must be \
                                     at least 512 (got: {})",
                                    l)).into());
                    }
                }
                BodyLength::Full(l) => {
                    // In the following block cipher length checks, we
                    // conservatively assume a block size of 8 bytes,
                    // because Twofish, TripleDES, IDEA, and CAST-5
                    // have a block size of 64 bits.
                    if tag == Tag::SED && (l < (8       // Random block.
                                                + 2     // Quickcheck bytes.
                                                + 6)) { // Smallest literal.
                        return Err(Error::MalformedPacket(
                            format!("{} packet's length must be \
                                     at least 16 bytes in length (got: {})",
                                    tag, l)).into());
                    } else if tag == Tag::SEIP
                        && (l < (1       // Version.
                                 + 8     // Random block.
                                 + 2     // Quickcheck bytes.
                                 + 6     // Smallest literal.
                                 + 20))  // MDC packet.
                    {
                        return Err(Error::MalformedPacket(
                            format!("{} packet's length minus 1 must be \
                                     at least 37 bytes in length (got: {})",
                                    tag, l)).into());
                    } else if tag == Tag::CompressedData && l == 0 {
                        // One byte header.
                        return Err(Error::MalformedPacket(
                            format!("{} packet's length must be \
                                     at least 1 byte (got ({})",
                                    tag, l)).into());
                    } else if tag == Tag::Literal && l < 6 {
                        // Smallest literal packet consists of 6 octets.
                        return Err(Error::MalformedPacket(
                            format!("{} packet's length must be \
                                     at least 6 bytes (got: ({})",
                                    tag, l)).into());
                    }
                }
            }
        } else {
            // Non-data packet.
            match self.length {
                BodyLength::Indeterminate =>
                    return Err(Error::MalformedPacket(
                        format!("Indeterminite length encoding \
                                 not allowed for {} packets",
                                tag)).into()),
                BodyLength::Partial(_) =>
                    return Err(Error::MalformedPacket(
                        format!("Partial Body Chunking not allowed \
                                 for {} packets",
                                tag)).into()),
                BodyLength::Full(l) => {
                    let valid = match tag {
                        Tag::Signature =>
                            // A V3 signature is 19 bytes plus the
                            // MPIs.  A V4 is 10 bytes plus the hash
                            // areas and the MPIs.
                            (10..(10  // Header, fixed sized fields.
                                    + 2 * 64 * 1024 // Hashed & Unhashed areas.
                                    + 64 * 1024 // MPIs.
                                   )).contains(&l),
                        Tag::SKESK =>
                            // 2 bytes of fixed header.  An s2k
                            // specification (at least 1 byte), an
                            // optional encryption session key.
                            (3..10 * 1024).contains(&l),
                        Tag::PKESK =>
                            // 10 bytes of fixed header, plus the
                            // encrypted session key.
                            10 < l && l < 10 * 1024,
                        Tag::OnePassSig if ! future_compatible => l == 13,
                        Tag::OnePassSig => l < 1024,
                        Tag::PublicKey | Tag::PublicSubkey
                            | Tag::SecretKey | Tag::SecretSubkey =>
                            // A V3 key is 8 bytes of fixed header
                            // plus MPIs.  A V4 key is 6 bytes of
                            // fixed headers plus MPIs.
                            6 < l && l < 1024 * 1024,
                        Tag::Trust => true,
                        Tag::UserID =>
                            // Avoid insane user ids.
                            l < 32 * 1024,
                        Tag::UserAttribute =>
                            // The header is at least 2 bytes.
                            2 <= l,
                        Tag::MDC => l == 20,

                        Tag::Literal | Tag::CompressedData
                            | Tag::SED | Tag::SEIP | Tag::AED =>
                            unreachable!("handled in the data-packet branch"),
                        Tag::Unknown(_) | Tag::Private(_) => true,

                        Tag::Marker => l == 3,
                        Tag::Reserved => true,
                    };

                    if ! valid {
                        return Err(Error::MalformedPacket(
                            format!("Invalid size ({} bytes) for a {} packet",
                                    l, tag)).into())
                    }
                }
            }
        }

        Ok(())
    }
}

/// A packet's size.
///
/// A packet's size can be expressed in three different ways.  Either
/// the size of the packet is fully known (`Full`), the packet is
/// chunked using OpenPGP's partial body encoding (`Partial`), or the
/// packet extends to the end of the file (`Indeterminate`).  See
/// [Section 4.2 of RFC 4880] for more details.
///
///   [Section 4.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2
#[derive(Debug)]
// We need PartialEq so that assert_eq! works.
#[derive(PartialEq)]
#[derive(Clone, Copy)]
pub enum BodyLength {
    /// The packet's size is known.
    Full(u32),
    /// The parameter is the number of bytes in the current chunk.
    ///
    /// This type is only used with new format packets.
    Partial(u32),
    /// The packet extends until an EOF is encountered.
    ///
    /// This type is only used with old format packets.
    Indeterminate,
}
assert_send_and_sync!(BodyLength);
