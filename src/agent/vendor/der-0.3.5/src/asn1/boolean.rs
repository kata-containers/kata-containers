//! ASN.1 `BOOLEAN` support.

use crate::{Any, Encodable, Encoder, Error, ErrorKind, Header, Length, Result, Tag, Tagged};
use core::convert::TryFrom;

/// Byte used to encode `true` in ASN.1 DER. From X.690 Section 11.1:
///
/// > If the encoding represents the boolean value TRUE, its single contents
/// > octet shall have all eight bits set to one.
const TRUE_OCTET: u8 = 0b11111111;

/// Byte used to encode `false` in ASN.1 DER.
const FALSE_OCTET: u8 = 0b00000000;

impl TryFrom<Any<'_>> for bool {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<bool> {
        any.tag().assert_eq(Tag::Boolean)?;

        match any.as_bytes() {
            [FALSE_OCTET] => Ok(false),
            [TRUE_OCTET] => Ok(true),
            _ => Err(ErrorKind::Noncanonical.into()),
        }
    }
}

impl Encodable for bool {
    fn encoded_len(&self) -> Result<Length> {
        Length::ONE.for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Header::new(Self::TAG, Length::ONE)?.encode(encoder)?;
        let byte = if *self { TRUE_OCTET } else { FALSE_OCTET };
        encoder.byte(byte)
    }
}

impl Tagged for bool {
    const TAG: Tag = Tag::Boolean;
}

#[cfg(test)]
mod tests {
    use crate::{Decodable, Encodable};

    #[test]
    fn decode() {
        assert_eq!(true, bool::from_der(&[0x01, 0x01, 0xFF]).unwrap());
        assert_eq!(false, bool::from_der(&[0x01, 0x01, 0x00]).unwrap());
    }

    #[test]
    fn encode() {
        let mut buffer = [0u8; 3];
        assert_eq!(
            &[0x01, 0x01, 0xFF],
            true.encode_to_slice(&mut buffer).unwrap()
        );
        assert_eq!(
            &[0x01, 0x01, 0x00],
            false.encode_to_slice(&mut buffer).unwrap()
        );
    }

    #[test]
    fn reject_non_canonical() {
        assert!(bool::from_der(&[0x01, 0x01, 0x01]).is_err());
    }
}
