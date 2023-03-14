//! Value traits

use crate::{Decoder, Encoder, Header, Length, Result, Tagged};

#[cfg(doc)]
use crate::Tag;

/// Decode the value part of a Tag-Length-Value encoded field, sans the [`Tag`]
/// and [`Length`].
pub trait DecodeValue<'a>: Sized {
    /// Attempt to decode this message using the provided [`Decoder`].
    fn decode_value(decoder: &mut Decoder<'a>, length: Length) -> Result<Self>;
}

/// Encode the value part of a Tag-Length-Value encoded field, sans the [`Tag`]
/// and [`Length`].
pub trait EncodeValue {
    /// Get the [`Header`] used to encode this value.
    fn header(&self) -> Result<Header>
    where
        Self: Tagged,
    {
        Header::new(self.tag(), self.value_len()?)
    }

    /// Compute the length of this value (sans [`Tag`]+[`Length`] header) when
    /// encoded as ASN.1 DER.
    fn value_len(&self) -> Result<Length>;

    /// Encode value (sans [`Tag`]+[`Length`] header) as ASN.1 DER using the
    /// provided [`Encoder`].
    fn encode_value(&self, encoder: &mut Encoder<'_>) -> Result<()>;
}
