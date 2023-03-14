//! ASN.1 `NULL` support.

use crate::{
    asn1::Any, ByteSlice, DecodeValue, Decoder, Encodable, EncodeValue, Encoder, Error, ErrorKind,
    FixedTag, Length, OrdIsValueOrd, Result, Tag,
};

/// ASN.1 `NULL` type.
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct Null;

impl DecodeValue<'_> for Null {
    fn decode_value(decoder: &mut Decoder<'_>, length: Length) -> Result<Self> {
        if length.is_zero() {
            Ok(Null)
        } else {
            Err(decoder.error(ErrorKind::Length { tag: Self::TAG }))
        }
    }
}

impl EncodeValue for Null {
    fn value_len(&self) -> Result<Length> {
        Ok(Length::ZERO)
    }

    fn encode_value(&self, _encoder: &mut Encoder<'_>) -> Result<()> {
        Ok(())
    }
}

impl FixedTag for Null {
    const TAG: Tag = Tag::Null;
}

impl OrdIsValueOrd for Null {}

impl<'a> From<Null> for Any<'a> {
    fn from(_: Null) -> Any<'a> {
        Any::from_tag_and_value(Tag::Null, ByteSlice::default())
    }
}

impl TryFrom<Any<'_>> for Null {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<Null> {
        any.decode_into()
    }
}

impl TryFrom<Any<'_>> for () {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<()> {
        Null::try_from(any).map(|_| ())
    }
}

impl<'a> From<()> for Any<'a> {
    fn from(_: ()) -> Any<'a> {
        Null.into()
    }
}

impl DecodeValue<'_> for () {
    fn decode_value(decoder: &mut Decoder<'_>, length: Length) -> Result<Self> {
        Null::decode_value(decoder, length)?;
        Ok(())
    }
}

impl Encodable for () {
    fn encoded_len(&self) -> Result<Length> {
        Null.encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Null.encode(encoder)
    }
}

impl FixedTag for () {
    const TAG: Tag = Tag::Null;
}

#[cfg(test)]
mod tests {
    use super::Null;
    use crate::{Decodable, Encodable};

    #[test]
    fn decode() {
        Null::from_der(&[0x05, 0x00]).unwrap();
    }

    #[test]
    fn encode() {
        let mut buffer = [0u8; 2];
        assert_eq!(&[0x05, 0x00], Null.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(&[0x05, 0x00], ().encode_to_slice(&mut buffer).unwrap());
    }

    #[test]
    fn reject_non_canonical() {
        assert!(Null::from_der(&[0x05, 0x81, 0x00]).is_err());
    }
}
