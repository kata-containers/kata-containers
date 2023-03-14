#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_root_url = "https://docs.rs/const-oid/0.7.1"
)]
#![forbid(unsafe_code, clippy::unwrap_used)]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg(feature = "std")]
extern crate std;

#[macro_use]
mod macros;

mod arcs;
mod encoder;
mod error;
mod parser;

pub use crate::{
    arcs::{Arc, Arcs},
    error::{Error, Result},
};

use crate::arcs::{RootArcs, ARC_MAX_BYTES, ARC_MAX_LAST_OCTET};
use core::{fmt, str::FromStr};

/// Object identifier (OID).
///
/// OIDs are hierarchical structures consisting of "arcs", i.e. integer
/// identifiers.
///
/// # Validity
///
/// In order for an OID to be considered valid by this library, it must meet
/// the following criteria:
///
/// - The OID MUST have at least 3 arcs
/// - The first arc MUST be within the range 0-2
/// - The second arc MUST be within the range 0-39
/// - The BER/DER encoding of the OID MUST be shorter than
///   [`ObjectIdentifier::MAX_SIZE`]
#[derive(Copy, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct ObjectIdentifier {
    /// Length in bytes
    length: u8,

    /// Array containing BER/DER-serialized bytes (no header)
    bytes: [u8; Self::MAX_SIZE],
}

#[allow(clippy::len_without_is_empty)]
impl ObjectIdentifier {
    /// Maximum size of a BER/DER-encoded OID in bytes.
    pub const MAX_SIZE: usize = 39; // 32-bytes total w\ 1-byte length

    /// Parse an [`ObjectIdentifier`] from the dot-delimited string form, e.g.:
    ///
    /// ```
    /// use const_oid::ObjectIdentifier;
    ///
    /// pub const MY_OID: ObjectIdentifier = ObjectIdentifier::new("1.2.840.113549.1.1.1");
    /// ```
    ///
    /// # Panics
    ///
    /// This method panics in the event the OID is malformed according to the
    /// "Validity" rules given in the toplevel documentation for this type.
    ///
    /// For that reason this method is *ONLY* recommended for use in constants
    /// (where it will generate a compiler error instead).
    ///
    /// To parse an OID from a `&str` slice fallibly and without panicking,
    /// use the [`FromStr`][1] impl instead (or via `str`'s [`parse`][2]
    /// method).
    ///
    /// [1]: ./struct.ObjectIdentifier.html#impl-FromStr
    /// [2]: https://doc.rust-lang.org/nightly/std/primitive.str.html#method.parse
    pub const fn new(s: &str) -> Self {
        parser::Parser::parse(s).finish()
    }

    /// Parse an OID from a slice of [`Arc`] values (i.e. integers).
    pub fn from_arcs(arcs: &[Arc]) -> Result<Self> {
        let mut bytes = [0u8; Self::MAX_SIZE];

        bytes[0] = match *arcs {
            [first, second, _, ..] => RootArcs::new(first, second)?.into(),
            _ => return Err(Error),
        };

        let mut offset = 1;

        for &arc in &arcs[2..] {
            offset += encoder::write_base128(&mut bytes[offset..], arc)?;
        }

        Ok(Self {
            bytes,
            length: offset as u8,
        })
    }

    /// Parse an OID from from its BER/DER encoding.
    pub fn from_bytes(ber_bytes: &[u8]) -> Result<Self> {
        let len = ber_bytes.len();

        if !(2..=Self::MAX_SIZE).contains(&len) {
            return Err(Error);
        }

        // Validate root arcs are in range
        ber_bytes
            .get(0)
            .cloned()
            .ok_or(Error)
            .and_then(RootArcs::try_from)?;

        // Validate lower arcs are well-formed
        let mut arc_offset = 1;
        let mut arc_bytes = 0;

        // TODO(tarcieri): consolidate this with `Arcs::next`?
        while arc_offset < len {
            match ber_bytes.get(arc_offset + arc_bytes).cloned() {
                Some(byte) => {
                    if (arc_bytes == ARC_MAX_BYTES) && (byte & ARC_MAX_LAST_OCTET != 0) {
                        // Overflowed `Arc` (u32)
                        return Err(Error);
                    }

                    arc_bytes += 1;

                    if byte & 0b10000000 == 0 {
                        arc_offset += arc_bytes;
                        arc_bytes = 0;
                    }
                }
                None => return Err(Error), // truncated OID
            }
        }

        let mut bytes = [0u8; Self::MAX_SIZE];
        bytes[..len].copy_from_slice(ber_bytes);

        Ok(Self {
            bytes,
            length: len as u8,
        })
    }

    /// Get the BER/DER serialization of this OID as bytes.
    ///
    /// Note that this encoding omits the tag/length, and only contains the
    /// value portion of the encoded OID.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length as usize]
    }

    /// Return the arc with the given index, if it exists.
    pub fn arc(&self, index: usize) -> Option<Arc> {
        self.arcs().nth(index)
    }

    /// Iterate over the arcs (a.k.a. nodes) of an [`ObjectIdentifier`].
    ///
    /// Returns [`Arcs`], an iterator over `Arc` values representing the value
    /// of each arc/node.
    pub fn arcs(&self) -> Arcs<'_> {
        Arcs::new(self)
    }
}

impl AsRef<[u8]> for ObjectIdentifier {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl FromStr for ObjectIdentifier {
    type Err = Error;

    fn from_str(string: &str) -> Result<Self> {
        let mut split = string.split('.');
        let first_arc = split.next().and_then(|s| s.parse().ok()).ok_or(Error)?;
        let second_arc = split.next().and_then(|s| s.parse().ok()).ok_or(Error)?;

        let mut bytes = [0u8; Self::MAX_SIZE];
        bytes[0] = RootArcs::new(first_arc, second_arc)?.into();

        let mut offset = 1;

        for s in split {
            let arc = s.parse().map_err(|_| Error)?;
            offset += encoder::write_base128(&mut bytes[offset..], arc)?;
        }

        if offset > 1 {
            Ok(Self {
                bytes,
                length: offset as u8,
            })
        } else {
            // Minimum 3 arcs
            Err(Error)
        }
    }
}

impl TryFrom<&[u8]> for ObjectIdentifier {
    type Error = Error;

    fn try_from(ber_bytes: &[u8]) -> Result<Self> {
        Self::from_bytes(ber_bytes)
    }
}

impl From<&ObjectIdentifier> for ObjectIdentifier {
    fn from(oid: &ObjectIdentifier) -> ObjectIdentifier {
        *oid
    }
}

impl fmt::Debug for ObjectIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectIdentifier({})", self)
    }
}

impl fmt::Display for ObjectIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.arcs().count();

        for (i, arc) in self.arcs().enumerate() {
            write!(f, "{}", arc)?;

            if i < len - 1 {
                write!(f, ".")?;
            }
        }

        Ok(())
    }
}
