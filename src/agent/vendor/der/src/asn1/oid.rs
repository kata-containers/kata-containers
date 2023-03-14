//! ASN.1 `OBJECT IDENTIFIER`

use crate::{
    asn1::Any, ByteSlice, DecodeValue, Decoder, EncodeValue, Encoder, Error, FixedTag, Length,
    OrdIsValueOrd, Result, Tag, Tagged,
};
use const_oid::ObjectIdentifier;

impl DecodeValue<'_> for ObjectIdentifier {
    fn decode_value(decoder: &mut Decoder<'_>, length: Length) -> Result<Self> {
        let bytes = ByteSlice::decode_value(decoder, length)?.as_bytes();
        Ok(Self::from_bytes(bytes)?)
    }
}

impl EncodeValue for ObjectIdentifier {
    fn value_len(&self) -> Result<Length> {
        Length::try_from(self.as_bytes().len())
    }

    fn encode_value(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        encoder.bytes(self.as_bytes())
    }
}

impl FixedTag for ObjectIdentifier {
    const TAG: Tag = Tag::ObjectIdentifier;
}

impl OrdIsValueOrd for ObjectIdentifier {}

impl<'a> From<&'a ObjectIdentifier> for Any<'a> {
    fn from(oid: &'a ObjectIdentifier) -> Any<'a> {
        // Note: ensuring an infallible conversion is possible relies on the
        // invariant that `const_oid::MAX_LEN <= Length::max()`.
        //
        // The `length()` test below ensures this is the case.
        let value = oid
            .as_bytes()
            .try_into()
            .expect("OID length invariant violated");

        Any::from_tag_and_value(Tag::ObjectIdentifier, value)
    }
}

impl TryFrom<Any<'_>> for ObjectIdentifier {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<ObjectIdentifier> {
        any.tag().assert_eq(Tag::ObjectIdentifier)?;
        Ok(ObjectIdentifier::from_bytes(any.value())?)
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectIdentifier;
    use crate::{Decodable, Encodable, Length};

    const EXAMPLE_OID: ObjectIdentifier = ObjectIdentifier::new("1.2.840.113549");
    const EXAMPLE_OID_BYTES: &[u8; 8] = &[0x06, 0x06, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d];

    #[test]
    fn decode() {
        let oid = ObjectIdentifier::from_der(EXAMPLE_OID_BYTES).unwrap();
        assert_eq!(EXAMPLE_OID, oid);
    }

    #[test]
    fn encode() {
        let mut buffer = [0u8; 8];
        assert_eq!(
            EXAMPLE_OID_BYTES,
            EXAMPLE_OID.encode_to_slice(&mut buffer).unwrap()
        );
    }

    #[test]
    fn length() {
        // Ensure an infallible `From` conversion to `Any` will never panic
        assert!(ObjectIdentifier::MAX_SIZE <= Length::MAX.try_into().unwrap());
    }
}
