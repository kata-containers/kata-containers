//! The [`Sequence`] trait simplifies writing decoders/encoders which map ASN.1
//! `SEQUENCE`s to Rust structs.

use crate::{Decodable, Encodable, EncodeValue, Encoder, FixedTag, Length, Result, Tag};

/// ASN.1 `SEQUENCE` trait.
///
/// Types which impl this trait receive blanket impls for the [`Decodable`],
/// [`Encodable`], and [`FixedTag`] traits.
pub trait Sequence<'a>: Decodable<'a> {
    /// Call the provided function with a slice of [`Encodable`] trait objects
    /// representing the fields of this `SEQUENCE`.
    ///
    /// This method uses a callback because structs with fields which aren't
    /// directly [`Encodable`] may need to construct temporary values from
    /// their fields prior to encoding.
    fn fields<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&[&dyn Encodable]) -> Result<T>;
}

impl<'a, M> EncodeValue for M
where
    M: Sequence<'a>,
{
    fn value_len(&self) -> Result<Length> {
        self.fields(|fields| {
            fields
                .iter()
                .fold(Ok(Length::ZERO), |len, field| len + field.encoded_len()?)
        })
    }

    fn encode_value(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        self.fields(|fields| {
            for &field in fields {
                field.encode(encoder)?;
            }

            Ok(())
        })
    }
}

impl<'a, M> FixedTag for M
where
    M: Sequence<'a>,
{
    const TAG: Tag = Tag::Sequence;
}
