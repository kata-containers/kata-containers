//! ASN.1 DER headers.

use crate::{Decodable, Decoder, Encodable, Encoder, ErrorKind, Length, Result, Tag};
use core::convert::TryInto;

/// ASN.1 DER headers: tag + length component of TLV-encoded values
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Header {
    /// Tag representing the type of the encoded value
    pub tag: Tag,

    /// Length of the encoded value
    pub length: Length,
}

impl Header {
    /// Create a new [`Header`] from a [`Tag`] and a specified length.
    ///
    /// Returns an error if the length exceeds the limits of [`Length`].
    pub fn new(tag: Tag, length: impl TryInto<Length>) -> Result<Self> {
        let length = length.try_into().map_err(|_| ErrorKind::Overflow)?;
        Ok(Self { tag, length })
    }
}

impl Decodable<'_> for Header {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Header> {
        let tag = Tag::decode(decoder)?;

        let length = Length::decode(decoder).map_err(|e| {
            if e.kind() == ErrorKind::Overlength {
                ErrorKind::Length { tag }.into()
            } else {
                e
            }
        })?;

        Ok(Self { tag, length })
    }
}

impl Encodable for Header {
    fn encoded_len(&self) -> Result<Length> {
        self.tag.encoded_len()? + self.length.encoded_len()?
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        self.tag.encode(encoder)?;
        self.length.encode(encoder)
    }
}
