//! ASN.1 `NULL` support.

use crate::{Any, ByteSlice, Encodable, Encoder, Error, ErrorKind, Length, Result, Tag, Tagged};
use core::convert::TryFrom;

/// ASN.1 `NULL` type.
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct Null;

impl TryFrom<Any<'_>> for Null {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<Null> {
        let tag = any.tag().assert_eq(Tag::Null)?;

        if any.is_empty() {
            Ok(Null)
        } else {
            Err(ErrorKind::Length { tag }.into())
        }
    }
}

impl<'a> From<Null> for Any<'a> {
    fn from(_: Null) -> Any<'a> {
        Any::from_tag_and_value(Tag::Null, ByteSlice::default())
    }
}

impl Encodable for Null {
    fn encoded_len(&self) -> Result<Length> {
        Any::from(*self).encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Any::from(*self).encode(encoder)
    }
}

impl Tagged for Null {
    const TAG: Tag = Tag::Integer;
}

impl TryFrom<Any<'_>> for () {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<()> {
        let tag = any.tag().assert_eq(Tag::Null)?;

        if any.is_empty() {
            Ok(())
        } else {
            Err(ErrorKind::Length { tag }.into())
        }
    }
}

impl<'a> From<()> for Any<'a> {
    fn from(_: ()) -> Any<'a> {
        Null.into()
    }
}

impl Encodable for () {
    fn encoded_len(&self) -> Result<Length> {
        Any::from(()).encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Any::from(()).encode(encoder)
    }
}

impl Tagged for () {
    const TAG: Tag = Tag::Null;
}

#[cfg(test)]
mod tests {
    use super::Null;
    use crate::{Decodable, Encodable};

    #[test]
    fn decode() {
        assert!(Null::from_der(&[0x05, 0x00]).is_ok());
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
