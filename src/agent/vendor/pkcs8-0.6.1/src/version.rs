use core::convert::{TryFrom, TryInto};

use der::{Encodable, Encoder, Tagged};

use crate::Error;

/// Version marker for PKCS#8 documents.
///
/// (RFC 5958 designates `0` and `1` as the only valid versions for PKCS#8 documents)
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum Version {
    /// Denotes PKCS#8 v1, used for [`crate::PrivateKeyInfo`] and [`crate::OneAsymmetricKey`]
    V1 = 0,

    /// Denotes PKCS#8 v2, only used for [`crate::OneAsymmetricKey`]
    V2 = 1,
}

impl From<Version> for u8 {
    fn from(version: Version) -> Self {
        version as u8
    }
}

impl TryFrom<u8> for Version {
    type Error = Error;
    fn try_from(byte: u8) -> Result<Version, Error> {
        match byte {
            0 => Ok(Version::V1),
            1 => Ok(Version::V2),
            _ => Err(Error::Decode),
        }
    }
}

impl<'a> TryFrom<der::Any<'a>> for Version {
    type Error = der::Error;
    fn try_from(any: der::Any<'a>) -> der::Result<Version> {
        u8::try_from(any)?.try_into().map_err(|_| {
            der::ErrorKind::Value {
                tag: der::Tag::Integer,
            }
            .into()
        })
    }
}

impl Encodable for Version {
    fn encoded_len(&self) -> der::Result<der::Length> {
        der::Length::from(1u8).for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> der::Result<()> {
        der::Header::new(Self::TAG, 1u8)?.encode(encoder)?;

        encoder.encode(self)
    }
}

impl Tagged for Version {
    const TAG: der::Tag = der::Tag::Integer;
}
