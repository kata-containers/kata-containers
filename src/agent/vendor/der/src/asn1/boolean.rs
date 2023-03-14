//! ASN.1 `BOOLEAN` support.

use crate::{
    asn1::Any, ByteSlice, DecodeValue, Decoder, EncodeValue, Encoder, Error, ErrorKind, FixedTag,
    Length, OrdIsValueOrd, Result, Tag,
};

/// Byte used to encode `true` in ASN.1 DER. From X.690 Section 11.1:
///
/// > If the encoding represents the boolean value TRUE, its single contents
/// > octet shall have all eight bits set to one.
const TRUE_OCTET: u8 = 0b11111111;

/// Byte used to encode `false` in ASN.1 DER.
const FALSE_OCTET: u8 = 0b00000000;

impl<'a> DecodeValue<'a> for bool {
    fn decode_value(decoder: &mut Decoder<'a>, length: Length) -> Result<Self> {
        if length != Length::ONE {
            return Err(decoder.error(ErrorKind::Length { tag: Self::TAG }));
        }

        match decoder.byte()? {
            FALSE_OCTET => Ok(false),
            TRUE_OCTET => Ok(true),
            _ => Err(Self::TAG.non_canonical_error()),
        }
    }
}

impl EncodeValue for bool {
    fn value_len(&self) -> Result<Length> {
        Ok(Length::ONE)
    }

    fn encode_value(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        encoder.byte(if *self { TRUE_OCTET } else { FALSE_OCTET })
    }
}

impl FixedTag for bool {
    const TAG: Tag = Tag::Boolean;
}

impl OrdIsValueOrd for bool {}

impl From<bool> for Any<'static> {
    fn from(value: bool) -> Any<'static> {
        let value = ByteSlice::from(match value {
            false => &[FALSE_OCTET],
            true => &[TRUE_OCTET],
        });

        Any::from_tag_and_value(Tag::Boolean, value)
    }
}

impl TryFrom<Any<'_>> for bool {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<bool> {
        any.try_into()
    }
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
