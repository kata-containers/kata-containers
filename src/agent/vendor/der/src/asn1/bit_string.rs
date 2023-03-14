//! ASN.1 `BIT STRING` support.

use crate::{
    asn1::Any, ByteSlice, DecodeValue, Decoder, DerOrd, EncodeValue, Encoder, Error, ErrorKind,
    FixedTag, Length, Result, Tag, ValueOrd,
};
use core::{cmp::Ordering, iter::FusedIterator};

/// ASN.1 `BIT STRING` type.
///
/// This type contains a sequence of any number of bits, modeled internally as
/// a sequence of bytes with a known number of "unused bits".
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct BitString<'a> {
    /// Number of unused bits in the final octet.
    unused_bits: u8,

    /// Length of this `BIT STRING` in bits.
    bit_length: usize,

    /// Bitstring represented as a slice of bytes.
    inner: ByteSlice<'a>,
}

impl<'a> BitString<'a> {
    /// Maximum number of unused bits allowed.
    pub const MAX_UNUSED_BITS: u8 = 7;

    /// Create a new ASN.1 `BIT STRING` from a byte slice.
    ///
    /// Accepts an optional number of "unused bits" (0-7) which are omitted
    /// from the final octet. This number is 0 if the value is octet-aligned.
    pub fn new(unused_bits: u8, bytes: &'a [u8]) -> Result<Self> {
        if (unused_bits > Self::MAX_UNUSED_BITS) || (unused_bits != 0 && bytes.is_empty()) {
            return Err(Self::TAG.value_error());
        }

        let inner = ByteSlice::new(bytes).map_err(|_| Self::TAG.length_error())?;

        let bit_length = usize::try_from(inner.len())?
            .checked_mul(8)
            .and_then(|n| n.checked_sub(usize::from(unused_bits)))
            .ok_or(ErrorKind::Overflow)?;

        Ok(Self {
            unused_bits,
            bit_length,
            inner,
        })
    }

    /// Create a new ASN.1 `BIT STRING` from the given bytes.
    ///
    /// The "unused bits" are set to 0.
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self> {
        Self::new(0, bytes)
    }

    /// Get the number of unused bits in this byte slice.
    pub fn unused_bits(&self) -> u8 {
        self.unused_bits
    }

    /// Is the number of unused bits a value other than 0?
    pub fn has_unused_bits(&self) -> bool {
        self.unused_bits != 0
    }

    /// Get the length of this `BIT STRING` in bits.
    pub fn bit_len(&self) -> usize {
        self.bit_length
    }

    /// Get the number of bytes/octets needed to represent this `BIT STRING`
    /// when serialized in an octet-aligned manner.
    pub fn byte_len(&self) -> Length {
        self.inner.len()
    }

    /// Is the inner byte slice empty?
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Borrow the inner byte slice.
    ///
    /// Returns `None` if the number of unused bits is *not* equal to zero,
    /// i.e. if the `BIT STRING` is not octet aligned.
    ///
    /// Use [`BitString::raw_bytes`] to obtain access to the raw value
    /// regardless of the presence of unused bits.
    pub fn as_bytes(&self) -> Option<&'a [u8]> {
        if self.has_unused_bits() {
            None
        } else {
            Some(self.raw_bytes())
        }
    }

    /// Borrow the raw bytes of this `BIT STRING`.
    ///
    /// Note that the byte string may contain extra unused bits in the final
    /// octet. If the number of unused bits is expected to be 0, the
    /// [`BitString::as_bytes`] function can be used instead.
    pub fn raw_bytes(&self) -> &'a [u8] {
        self.inner.as_bytes()
    }

    /// Iterator over the bits of this `BIT STRING`.
    pub fn bits(self) -> BitStringIter<'a> {
        BitStringIter {
            bit_string: self,
            position: 0,
        }
    }
}

impl<'a> DecodeValue<'a> for BitString<'a> {
    fn decode_value(decoder: &mut Decoder<'a>, encoded_len: Length) -> Result<Self> {
        let unused_bits = decoder.byte()?;
        let inner = ByteSlice::decode_value(decoder, (encoded_len - Length::ONE)?)?;
        Self::new(unused_bits, inner.as_bytes())
    }
}

impl EncodeValue for BitString<'_> {
    fn value_len(&self) -> Result<Length> {
        self.byte_len() + Length::ONE
    }

    fn encode_value(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        encoder.byte(self.unused_bits)?;
        encoder.bytes(self.raw_bytes())
    }
}

impl ValueOrd for BitString<'_> {
    fn value_cmp(&self, other: &Self) -> Result<Ordering> {
        match self.unused_bits.cmp(&other.unused_bits) {
            Ordering::Equal => self.inner.der_cmp(&other.inner),
            ordering => Ok(ordering),
        }
    }
}

impl<'a> From<&BitString<'a>> for BitString<'a> {
    fn from(value: &BitString<'a>) -> BitString<'a> {
        *value
    }
}

impl<'a> TryFrom<Any<'a>> for BitString<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<BitString<'a>> {
        any.decode_into()
    }
}

impl<'a> TryFrom<&'a [u8]> for BitString<'a> {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<BitString<'a>> {
        BitString::from_bytes(bytes)
    }
}

/// Hack for simplifying the custom derive use case.
impl<'a> TryFrom<&&'a [u8]> for BitString<'a> {
    type Error = Error;

    fn try_from(bytes: &&'a [u8]) -> Result<BitString<'a>> {
        BitString::from_bytes(*bytes)
    }
}

impl<'a> TryFrom<BitString<'a>> for &'a [u8] {
    type Error = Error;

    fn try_from(bit_string: BitString<'a>) -> Result<&'a [u8]> {
        bit_string
            .as_bytes()
            .ok_or_else(|| Tag::BitString.value_error())
    }
}

impl<'a> FixedTag for BitString<'a> {
    const TAG: Tag = Tag::BitString;
}

/// Iterator over the bits of a [`BitString`].
pub struct BitStringIter<'a> {
    /// [`BitString`] being iterated over.
    bit_string: BitString<'a>,

    /// Current bit position within the iterator.
    position: usize,
}

impl<'a> Iterator for BitStringIter<'a> {
    type Item = bool;

    fn next(&mut self) -> Option<bool> {
        if self.position >= self.bit_string.bit_len() {
            return None;
        }

        let byte = self.bit_string.raw_bytes().get(self.position / 8)?;
        let bit = 1u8 << (7 - (self.position % 8));
        self.position = self.position.checked_add(1)?;
        Some(byte & bit != 0)
    }
}

impl<'a> ExactSizeIterator for BitStringIter<'a> {
    fn len(&self) -> usize {
        self.bit_string.bit_len()
    }
}

impl<'a> FusedIterator for BitStringIter<'a> {}

#[cfg(test)]
mod tests {
    use super::{BitString, Result, Tag};
    use crate::asn1::Any;
    use hex_literal::hex;

    /// Parse a `BitString` from an ASN.1 `Any` value to test decoding behaviors.
    fn parse_bitstring(bytes: &[u8]) -> Result<BitString<'_>> {
        Any::new(Tag::BitString, bytes)?.try_into()
    }

    #[test]
    fn decode_empty_bitstring() {
        let bs = parse_bitstring(&hex!("00")).unwrap();
        assert_eq!(bs.as_bytes().unwrap(), &[]);
    }

    #[test]
    fn decode_non_empty_bitstring() {
        let bs = parse_bitstring(&hex!("00010203")).unwrap();
        assert_eq!(bs.as_bytes().unwrap(), &[0x01, 0x02, 0x03]);
    }

    #[test]
    fn decode_bitstring_with_unused_bits() {
        let bs = parse_bitstring(&hex!("066e5dc0")).unwrap();
        assert_eq!(bs.unused_bits(), 6);
        assert_eq!(bs.raw_bytes(), &hex!("6e5dc0"));

        // Expected: 011011100101110111
        let mut bits = bs.bits();
        assert_eq!(bits.len(), 18);

        for bit in [0, 1, 1, 0, 1, 1, 1, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1] {
            assert_eq!(bits.next().unwrap() as u8, bit)
        }

        // Ensure `None` is returned on successive calls
        assert_eq!(bits.next(), None);
        assert_eq!(bits.next(), None);
    }

    #[test]
    fn reject_unused_bits_in_empty_string() {
        assert_eq!(
            parse_bitstring(&[0x03]).err().unwrap().kind(),
            Tag::BitString.value_error().kind()
        )
    }
}
