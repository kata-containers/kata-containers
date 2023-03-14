//! Context-specific field.

use crate::{
    Any, Choice, Decodable, Encodable, Encoder, Error, ErrorKind, Header, Length, Result, Tag,
};
use core::convert::TryFrom;

/// Context-specific field.
///
/// This type encodes a field which is specific to a particular context,
/// and has a special "context-specific tag" (presently 0-15 supported).
///
/// Any context-specific field can be decoded/encoded with this type.
/// The intended use is to dynamically dispatch off of the context-specific
/// tag when decoding, which allows support for extensions, which are denoted
/// in an ASN.1 schema using the `...` ellipsis extension marker.
///
///
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct ContextSpecific<'a> {
    /// Context-specific tag value sans the leading `0b10000000` class
    /// identifier bit and `0b100000` constructed flag.
    pub(crate) tag: u8,

    /// Value of the field.
    pub(crate) value: Any<'a>,
}

impl<'a> ContextSpecific<'a> {
    /// Create a new context-specific field.
    ///
    /// The tag value includes only lower 6-bits of the context specific tag,
    /// sans the leading `10` high bits identifying the context-specific tag
    /// class as well as the constructed flag.
    pub fn new(tag: u8, value: Any<'a>) -> Result<Self> {
        // Ensure we consider the context-specific tag valid
        Tag::context_specific(tag)?;

        Ok(Self { tag, value })
    }

    /// Get the context-specific tag for this field.
    ///
    /// The tag value includes only lower 6-bits of the context specific tag,
    /// sans the leading `10` high bits identifying the context-specific tag
    /// class as well as the constructed flag.
    pub fn tag(self) -> u8 {
        self.tag
    }

    /// Get the value of this context-specific tag.
    pub fn value(self) -> Any<'a> {
        self.value
    }
}

impl<'a> Choice<'a> for ContextSpecific<'a> {
    fn can_decode(tag: Tag) -> bool {
        tag.is_context_specific()
    }
}

impl<'a> Encodable for ContextSpecific<'a> {
    fn encoded_len(&self) -> Result<Length> {
        self.value.encoded_len()?.for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        let tag = Tag::context_specific(self.tag)?;
        Header::new(tag, self.value.encoded_len()?)?.encode(encoder)?;
        self.value.encode(encoder)
    }
}

impl<'a> From<&ContextSpecific<'a>> for ContextSpecific<'a> {
    fn from(value: &ContextSpecific<'a>) -> ContextSpecific<'a> {
        *value
    }
}

impl<'a> TryFrom<Any<'a>> for ContextSpecific<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<ContextSpecific<'a>> {
        let tag = if any.tag().is_context_specific() {
            (any.tag() as u8)
                .checked_sub(0xA0)
                .ok_or(ErrorKind::Overflow)?
        } else {
            return Err(ErrorKind::UnexpectedTag {
                expected: None,
                actual: any.tag(),
            }
            .into());
        };

        let value = Any::from_der(any.as_bytes())?;

        Self::new(tag, value)
    }
}

#[cfg(test)]
mod tests {
    use super::ContextSpecific;
    use crate::{Decodable, Encodable, Tag};
    use hex_literal::hex;

    // Public key data from `pkcs8` crate's `ed25519-pkcs8-v2.der`
    const EXAMPLE_BYTES: &[u8] =
        &hex!("A123032100A3A7EAE3A8373830BC47E1167BC50E1DB551999651E0E2DC587623438EAC3F31");

    #[test]
    fn round_trip() {
        let field = ContextSpecific::from_der(EXAMPLE_BYTES).unwrap();
        assert_eq!(field.tag(), 1);

        let value = field.value();
        assert_eq!(value.tag(), Tag::BitString);
        assert_eq!(value.as_bytes(), &EXAMPLE_BYTES[5..]);

        let mut buf = [0u8; 128];
        let encoded = field.encode_to_slice(&mut buf).unwrap();
        assert_eq!(encoded, EXAMPLE_BYTES);
    }
}
