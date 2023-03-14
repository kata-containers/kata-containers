//! The [`Message`] pattern provided by this crate simplifies writing ASN.1 DER
//! decoders and encoders which map ASN.1 `SEQUENCE` types to Rust structs.

use crate::{Decodable, Encodable, Encoder, Header, Length, Result, Tag, Tagged};

/// Messages encoded as an ASN.1 `SEQUENCE`.
///
/// The "message" pattern this trait provides is not an ASN.1 concept,
/// but rather a pattern for writing ASN.1 DER decoders and encoders which
/// map ASN.1 `SEQUENCE` types to Rust structs with a minimum of code.
///
/// Types which impl this trait receive blanket impls for the [`Decodable`],
/// [`Encodable`], and [`Tagged`] traits.
pub trait Message<'a>: Decodable<'a> {
    /// Call the provided function with a slice of [`Encodable`] trait objects
    /// representing the fields of this message.
    ///
    /// This method uses a callback because structs with fields which aren't
    /// directly [`Encodable`] may need to construct temporary values from
    /// their fields prior to encoding.
    fn fields<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&[&dyn Encodable]) -> Result<T>;
}

impl<'a, M> Encodable for M
where
    M: Message<'a>,
{
    fn encoded_len(&self) -> Result<Length> {
        self.fields(|fields| {
            let inner_len = encoded_len_inner(fields)?;
            Header::new(Tag::Sequence, inner_len)?.encoded_len() + inner_len
        })
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        self.fields(|fields| encoder.message(fields))
    }
}

impl<'a, M> Tagged for M
where
    M: Message<'a>,
{
    const TAG: Tag = Tag::Sequence;
}

/// Obtain the length of an ASN.1 `SEQUENCE` consisting of the given
/// [`Encodable`] fields when serialized as ASN.1 DER, including the header
/// (i.e. tag and length)
pub fn encoded_len(fields: &[&dyn Encodable]) -> Result<Length> {
    let inner_len = encoded_len_inner(fields)?;
    Header::new(Tag::Sequence, inner_len)?.encoded_len() + inner_len
}

/// Obtain the length of an ASN.1 message `SEQUENCE` consisting of the given
/// [`Encodable`] fields when serialized as ASN.1 DER, including the header
/// (i.e. tag and length)
pub(crate) fn encoded_len_inner(fields: &[&dyn Encodable]) -> Result<Length> {
    fields.iter().fold(Ok(Length::ZERO), |sum, encodable| {
        sum + encodable.encoded_len()?
    })
}
