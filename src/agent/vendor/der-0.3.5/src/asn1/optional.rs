//! ASN.1 `OPTIONAL` as mapped to Rust's `Option` type

use crate::{Choice, Decodable, Decoder, Encodable, Encoder, Length, Result, Tag};
use core::convert::TryFrom;

impl<'a, T> Decodable<'a> for Option<T>
where
    T: Choice<'a>, // NOTE: all `Decodable + Tagged` types receive a blanket `Choice` impl
{
    fn decode(decoder: &mut Decoder<'a>) -> Result<Option<T>> {
        if let Some(byte) = decoder.peek() {
            if T::can_decode(Tag::try_from(byte)?) {
                return T::decode(decoder).map(Some);
            }
        }

        Ok(None)
    }
}

impl<T> Encodable for Option<T>
where
    T: Encodable,
{
    fn encoded_len(&self) -> Result<Length> {
        if let Some(encodable) = self {
            encodable.encoded_len()
        } else {
            Ok(0u8.into())
        }
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        if let Some(encodable) = self {
            encodable.encode(encoder)
        } else {
            Ok(())
        }
    }
}
