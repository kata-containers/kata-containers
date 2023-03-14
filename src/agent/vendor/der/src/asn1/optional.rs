//! ASN.1 `OPTIONAL` as mapped to Rust's `Option` type

use crate::{Choice, Decodable, Decoder, DerOrd, Encodable, Encoder, Length, Result, Tag};
use core::cmp::Ordering;

impl<'a, T> Decodable<'a> for Option<T>
where
    T: Choice<'a>, // NOTE: all `Decodable + Tagged` types receive a blanket `Choice` impl
{
    fn decode(decoder: &mut Decoder<'a>) -> Result<Option<T>> {
        if let Some(byte) = decoder.peek_byte() {
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

impl<T> DerOrd for Option<T>
where
    T: DerOrd,
{
    fn der_cmp(&self, other: &Self) -> Result<Ordering> {
        if let Some(a) = self {
            if let Some(b) = other {
                a.der_cmp(b)
            } else {
                Ok(Ordering::Greater)
            }
        } else {
            Ok(Ordering::Less)
        }
    }
}

/// A reference to an ASN.1 `OPTIONAL` type, used for encoding only.
pub struct OptionalRef<'a, T>(pub Option<&'a T>);

impl<'a, T> Encodable for OptionalRef<'a, T>
where
    T: Encodable,
{
    fn encoded_len(&self) -> Result<Length> {
        if let Some(encodable) = self.0 {
            encodable.encoded_len()
        } else {
            Ok(0u8.into())
        }
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        if let Some(encodable) = self.0 {
            encodable.encode(encoder)
        } else {
            Ok(())
        }
    }
}
