//! Cipher Type Byte (CTB).
//!
//! The CTB encodes the packet's type and some length information.  It
//! has two variants: the so-called old format and the so-called new
//! format.  See [Section 4.2 of RFC 4880] for more details.
//!
//!   [Section 4.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2

use std::convert::TryFrom;

use crate::{
    packet::Tag,
    Error,
    Result
};
use crate::packet::header::BodyLength;

/// Data common to all CTB formats.
///
/// OpenPGP defines two packet formats: an old format and a new
/// format.  They both include the packet's so-called tag.
///
/// See [Section 4.2 of RFC 4880] for more details.
///
///   [Section 4.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2
#[derive(Clone, Debug)]
struct CTBCommon {
    /// RFC4880 Packet tag
    tag: Tag,
}

/// A CTB using the new format encoding.
///
/// See [Section 4.2 of RFC 4880] for more details.
///
///   [Section 4.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2
#[derive(Clone, Debug)]
pub struct CTBNew {
    /// Packet CTB fields
    common: CTBCommon,
}
assert_send_and_sync!(CTBNew);

impl CTBNew {
    /// Constructs a new-style CTB.
    pub fn new(tag: Tag) -> Self {
        CTBNew {
            common: CTBCommon {
                tag,
            },
        }
    }

    /// Returns the packet's tag.
    pub fn tag(&self) -> Tag {
        self.common.tag
    }
}

/// The length encoded for an old style CTB.
///
/// The `PacketLengthType` is only part of the [old CTB], and is
/// partially used to determine the packet's size.
///
/// See [Section 4.2.1 of RFC 4880] for more details.
///
///   [old CTB]: CTBOld
///   [Section 4.2.1 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2.1
#[derive(Debug)]
#[derive(Clone, Copy, PartialEq)]
pub enum PacketLengthType {
    /// A one-octet Body Length header encodes a length of 0 to 191 octets.
    ///
    /// The header is 2 octets long.  It contains the one byte CTB
    /// followed by the one octet length.
    OneOctet,
    /// A two-octet Body Length header encodes a length of 192 to 8383 octets.
    ///
    /// The header is 3 octets long.  It contains the one byte CTB
    /// followed by the two octet length.
    TwoOctets,
    /// A four-octet Body Length.
    ///
    /// The header is 5 octets long.  It contains the one byte CTB
    /// followed by the four octet length.
    FourOctets,
    /// The packet is of indeterminate length.
    ///
    /// Neither the packet header nor the packet itself contain any
    /// information about the length.  The end of the packet is clear
    /// from the context, e.g., EOF.
    Indeterminate,
}
assert_send_and_sync!(PacketLengthType);

impl TryFrom<u8> for PacketLengthType {
    type Error = anyhow::Error;

    fn try_from(u: u8) -> Result<Self> {
        match u {
            0 => Ok(PacketLengthType::OneOctet),
            1 => Ok(PacketLengthType::TwoOctets),
            2 => Ok(PacketLengthType::FourOctets),
            3 => Ok(PacketLengthType::Indeterminate),
            _ => Err(Error::InvalidArgument(
                format!("Invalid packet length: {}", u)).into()),
        }
    }
}

impl From<PacketLengthType> for u8 {
    fn from(l: PacketLengthType) -> Self {
        match l {
            PacketLengthType::OneOctet => 0,
            PacketLengthType::TwoOctets => 1,
            PacketLengthType::FourOctets => 2,
            PacketLengthType::Indeterminate => 3,
        }
    }
}

/// A CTB using the old format encoding.
///
/// See [Section 4.2 of RFC 4880] for more details.
///
///   [Section 4.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2
#[derive(Clone, Debug)]
pub struct CTBOld {
    /// Common CTB fields.
    common: CTBCommon,
    /// Type of length specifier.
    length_type: PacketLengthType,
}
assert_send_and_sync!(CTBOld);

impl CTBOld {
    /// Constructs an old-style CTB.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidArgument`] if the tag or the body
    /// length cannot be expressed using an old-style CTB.
    ///
    /// [`Error::InvalidArgument`]: super::super::Error::InvalidArgument
    pub fn new(tag: Tag, length: BodyLength) -> Result<Self> {
        let n: u8 = tag.into();

        // Only tags 0-15 are supported.
        if n > 15 {
            return Err(Error::InvalidArgument(
                format!("Only tags 0-15 are supported, got: {:?} ({})",
                        tag, n)).into());
        }

        let length_type = match length {
            // Assume an optimal encoding.
            BodyLength::Full(l) => {
                match l {
                    // One octet length.
                    0 ..= 0xFF => PacketLengthType::OneOctet,
                    // Two octet length.
                    0x1_00 ..= 0xFF_FF => PacketLengthType::TwoOctets,
                    // Four octet length,
                    _ => PacketLengthType::FourOctets,
                }
            },
            BodyLength::Partial(_) =>
                return Err(Error::InvalidArgument(
                    "Partial body lengths are not support for old format packets".
                        into()).into()),
            BodyLength::Indeterminate =>
                PacketLengthType::Indeterminate,
        };

        Ok(CTBOld {
            common: CTBCommon {
                tag,
            },
            length_type,
        })
    }

    /// Returns the packet's tag.
    pub fn tag(&self) -> Tag {
        self.common.tag
    }

    /// Returns the packet's length type.
    pub fn length_type(&self) -> PacketLengthType {
        self.length_type
    }
}

/// The CTB variants.
///
/// There are two CTB variants: the [old CTB format] and the [new CTB
/// format].
///
///   [old CTB format]: CTBOld
///   [new CTB format]: CTBNew
///
/// Note: CTB stands for Cipher Type Byte.
#[derive(Clone, Debug)]
pub enum CTB {
    /// New (current) packet header format.
    New(CTBNew),
    /// Old PGP 2.6 header format.
    Old(CTBOld),
}
assert_send_and_sync!(CTB);

impl CTB {
    /// Constructs a new-style CTB.
    pub fn new(tag: Tag) -> Self {
        CTB::New(CTBNew::new(tag))
    }

    /// Returns the packet's tag.
    pub fn tag(&self) -> Tag {
        match self {
            CTB::New(c) => c.tag(),
            CTB::Old(c) => c.tag(),
        }
    }
}

impl TryFrom<u8> for CTB {
    type Error = anyhow::Error;

    /// Parses a CTB as described in [Section 4.2 of RFC 4880].  This
    /// function parses both new and old format CTBs.
    ///
    ///   [Section 4.2 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-4.2
    fn try_from(ptag: u8) -> Result<CTB> {
        // The top bit of the ptag must be set.
        if ptag & 0b1000_0000 == 0 {
            return Err(
                Error::MalformedPacket(
                    format!("Malformed CTB: MSB of ptag ({:#010b}) not set{}.",
                            ptag,
                            if ptag == b'-' {
                                " (ptag is a dash, perhaps this is an \
                                 ASCII-armor encoded message)"
                            } else {
                                ""
                            })).into());
        }

        let new_format = ptag & 0b0100_0000 != 0;
        let ctb = if new_format {
            let tag = ptag & 0b0011_1111;
            CTB::New(CTBNew {
                common: CTBCommon {
                    tag: tag.into()
                }})
        } else {
            let tag = (ptag & 0b0011_1100) >> 2;
            let length_type = ptag & 0b0000_0011;

            CTB::Old(CTBOld {
                common: CTBCommon {
                    tag: tag.into(),
                },
                length_type: PacketLengthType::try_from(length_type)?,
            })
        };

        Ok(ctb)
    }
}

#[test]
fn ctb() {
    // 0x99 = public key packet
    if let CTB::Old(ctb) = CTB::try_from(0x99).unwrap() {
        assert_eq!(ctb.tag(), Tag::PublicKey);
        assert_eq!(ctb.length_type, PacketLengthType::TwoOctets);
    } else {
        panic!("Expected an old format packet.");
    }

    // 0xa3 = old compressed packet
    if let CTB::Old(ctb) = CTB::try_from(0xa3).unwrap() {
        assert_eq!(ctb.tag(), Tag::CompressedData);
        assert_eq!(ctb.length_type, PacketLengthType::Indeterminate);
    } else {
        panic!("Expected an old format packet.");
    }

    // 0xcb: new literal
    if let CTB::New(ctb) = CTB::try_from(0xcb).unwrap() {
        assert_eq!(ctb.tag(), Tag::Literal);
    } else {
        panic!("Expected a new format packet.");
    }
}
