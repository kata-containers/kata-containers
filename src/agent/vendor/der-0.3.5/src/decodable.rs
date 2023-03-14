//! Trait definition for [`Decodable`].

use crate::{Any, Decoder, Error, Result};
use core::convert::TryFrom;

/// Decoding trait.
///
/// This trait provides the core abstraction upon which all decoding operations
/// are based.
///
/// # Blanket impl for `TryFrom<Any>`
///
/// In almost all cases you do not need to impl this trait yourself, but rather
/// can instead impl `TryFrom<Any<'a>, Error = Error>` and receive a blanket
/// impl of this trait.
pub trait Decodable<'a>: Sized {
    /// Attempt to decode this message using the provided decoder.
    fn decode(decoder: &mut Decoder<'a>) -> Result<Self>;

    /// Parse `Self` from the provided DER-encoded byte slice.
    fn from_der(bytes: &'a [u8]) -> Result<Self> {
        let mut decoder = Decoder::new(bytes);
        let result = Self::decode(&mut decoder)?;
        decoder.finish(result)
    }
}

impl<'a, T> Decodable<'a> for T
where
    T: TryFrom<Any<'a>, Error = Error>,
{
    fn decode(decoder: &mut Decoder<'a>) -> Result<T> {
        Any::decode(decoder)
            .and_then(Self::try_from)
            .or_else(|e| decoder.error(e.kind()))
    }
}
