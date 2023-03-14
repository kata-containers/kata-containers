//! ASN.1 `OBJECT IDENTIFIER`

use crate::{Any, Encodable, Encoder, Error, Length, ObjectIdentifier, Result, Tag, Tagged};
use core::convert::{TryFrom, TryInto};

impl TryFrom<Any<'_>> for ObjectIdentifier {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<ObjectIdentifier> {
        any.tag().assert_eq(Tag::ObjectIdentifier)?;
        Ok(ObjectIdentifier::from_bytes(any.as_bytes())?)
    }
}

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

impl Encodable for ObjectIdentifier {
    fn encoded_len(&self) -> Result<Length> {
        Any::from(self).encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Any::from(self).encode(encoder)
    }
}

impl<'a> Tagged for ObjectIdentifier {
    const TAG: Tag = Tag::ObjectIdentifier;
}

#[cfg(test)]
mod tests {
    use crate::{Decodable, Encodable, Length, ObjectIdentifier};
    use core::convert::TryInto;

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
        assert!(ObjectIdentifier::MAX_LENGTH <= Length::MAX.try_into().unwrap());
    }
}
