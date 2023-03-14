//! Length calculations for encoded ASN.1 DER values

use crate::{Decodable, Decoder, Encodable, Encoder, Error, ErrorKind, Result};
use core::{
    convert::{TryFrom, TryInto},
    fmt,
    ops::Add,
};

/// Maximum length as a `u32` (1 MiB).
const MAX_U32: u32 = 0xf_ffff;

/// ASN.1-encoded length.
///
/// Maximum length is defined by the [`Length::MAX`] constant (1 MiB).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, PartialOrd, Ord)]
pub struct Length(u32);

impl Length {
    /// Length of `0`
    pub const ZERO: Self = Self(0);

    /// Length of `1`
    pub const ONE: Self = Self(1);

    /// Maximum length currently supported: 1 MiB
    pub const MAX: Self = Self(MAX_U32);

    /// Create a new [`Length`] for any value which fits inside of a [`u16`].
    ///
    /// This function is const-safe and therefore useful for [`Length`] constants.
    pub const fn new(value: u16) -> Self {
        Length(value as u32)
    }

    /// Return a length of `0`.
    #[deprecated(since = "0.3.3", note = "please use Length::ZERO")]
    pub const fn zero() -> Self {
        Self::ZERO
    }

    /// Return a length of `1`.
    #[deprecated(since = "0.3.3", note = "please use Length::ONE")]
    pub const fn one() -> Self {
        Self::ONE
    }

    /// Get the maximum length supported by this crate
    #[deprecated(since = "0.3.3", note = "please use Length::MAX")]
    pub const fn max() -> Self {
        Self::MAX
    }

    /// Get the length of DER Tag-Length-Value (TLV) encoded data if `self`
    /// is the length of the inner "value" portion of the message.
    pub fn for_tlv(self) -> Result<Self> {
        Length(1) + self.encoded_len()? + self
    }

    /// Get initial octet of the encoded length (if one is required).
    ///
    /// From X.690 Section 8.1.3.5:
    /// > In the long form, the length octets shall consist of an initial octet
    /// > and one or more subsequent octets. The initial octet shall be encoded
    /// > as follows:
    /// >
    /// > a) bit 8 shall be one;
    /// > b) bits 7 to 1 shall encode the number of subsequent octets in the
    /// >    length octets, as an unsigned binary integer with bit 7 as the
    /// >    most significant bit;
    /// > c) the value 11111111₂ shall not be used.
    fn initial_octet(self) -> Option<u8> {
        match self.0 {
            0x80..=0xFF => Some(0x81),
            0x100..=0xFFFF => Some(0x82),
            0x10000..=MAX_U32 => Some(0x83),
            _ => None,
        }
    }
}

impl Add for Length {
    type Output = Result<Self>;

    fn add(self, other: Self) -> Result<Self> {
        self.0
            .checked_add(other.0)
            .ok_or_else(|| ErrorKind::Overflow.into())
            .and_then(TryInto::try_into)
    }
}

impl Add<u8> for Length {
    type Output = Result<Self>;

    fn add(self, other: u8) -> Result<Self> {
        self + Length::from(other)
    }
}

impl Add<u16> for Length {
    type Output = Result<Self>;

    fn add(self, other: u16) -> Result<Self> {
        self + Length::from(other)
    }
}

impl Add<u32> for Length {
    type Output = Result<Self>;

    fn add(self, other: u32) -> Result<Self> {
        self + Length::try_from(other)?
    }
}

impl Add<usize> for Length {
    type Output = Result<Self>;

    fn add(self, other: usize) -> Result<Self> {
        self + Length::try_from(other)?
    }
}

impl Add<Length> for Result<Length> {
    type Output = Self;

    fn add(self, other: Length) -> Self {
        self? + other
    }
}

impl From<u8> for Length {
    fn from(len: u8) -> Length {
        Length(len as u32)
    }
}

impl From<u16> for Length {
    fn from(len: u16) -> Length {
        Length(len as u32)
    }
}

impl TryFrom<u32> for Length {
    type Error = Error;

    fn try_from(len: u32) -> Result<Length> {
        if len <= Self::MAX.0 {
            Ok(Length(len))
        } else {
            Err(ErrorKind::Overflow.into())
        }
    }
}

impl TryFrom<usize> for Length {
    type Error = Error;

    fn try_from(len: usize) -> Result<Length> {
        u32::try_from(len)
            .map_err(|_| ErrorKind::Overflow)?
            .try_into()
    }
}

impl TryFrom<Length> for usize {
    type Error = Error;

    fn try_from(len: Length) -> Result<usize> {
        len.0.try_into().map_err(|_| ErrorKind::Overflow.into())
    }
}

impl Decodable<'_> for Length {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Length> {
        match decoder.byte()? {
            // Note: per X.690 Section 8.1.3.6.1 the byte 0x80 encodes indefinite
            // lengths, which are not allowed in DER, so disallow that byte.
            len if len < 0x80 => Ok(len.into()),
            tag @ 0x81..=0x83 => {
                let nbytes = tag.checked_sub(0x80).ok_or(ErrorKind::Overlength)? as usize;
                let mut decoded_len = 0;

                for _ in 0..nbytes {
                    decoded_len = (decoded_len << 8) | decoder.byte()? as u32;
                }

                let length = Length::try_from(decoded_len)?;

                // X.690 Section 10.1: DER lengths must be encoded with a minimum
                // number of octets
                if length.initial_octet() == Some(tag) {
                    Ok(length)
                } else {
                    Err(ErrorKind::Noncanonical.into())
                }
            }
            _ => {
                // We specialize to a maximum 4-byte length (including initial octet)
                Err(ErrorKind::Overlength.into())
            }
        }
    }
}

impl Encodable for Length {
    fn encoded_len(&self) -> Result<Length> {
        match self.0 {
            0..=0x7F => Ok(Length(1)),
            0x80..=0xFF => Ok(Length(2)),
            0x100..=0xFFFF => Ok(Length(3)),
            0x10000..=MAX_U32 => Ok(Length(4)),
            _ => Err(ErrorKind::Overflow.into()),
        }
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        if let Some(tag_byte) = self.initial_octet() {
            encoder.byte(tag_byte)?;

            match self.0.to_be_bytes() {
                [0, 0, 0, byte] => encoder.byte(byte),
                [0, 0, bytes @ ..] => encoder.bytes(&bytes),
                [0, bytes @ ..] => encoder.bytes(&bytes),
                _ => Err(ErrorKind::Overlength.into()),
            }
        } else {
            encoder.byte(self.0 as u8)
        }
    }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::Length;
    use crate::{Decodable, Encodable, ErrorKind};
    use core::convert::TryFrom;

    #[test]
    fn decode() {
        assert_eq!(Length::ZERO, Length::from_der(&[0x00]).unwrap());

        assert_eq!(Length::from(0x7Fu8), Length::from_der(&[0x7F]).unwrap());

        assert_eq!(
            Length::from(0x80u8),
            Length::from_der(&[0x81, 0x80]).unwrap()
        );

        assert_eq!(
            Length::from(0xFFu8),
            Length::from_der(&[0x81, 0xFF]).unwrap()
        );

        assert_eq!(
            Length::from(0x100u16),
            Length::from_der(&[0x82, 0x01, 0x00]).unwrap()
        );

        assert_eq!(
            Length::try_from(0x10000u32).unwrap(),
            Length::from_der(&[0x83, 0x01, 0x00, 0x00]).unwrap()
        );
    }

    #[test]
    fn encode() {
        let mut buffer = [0u8; 4];

        assert_eq!(&[0x00], Length::ZERO.encode_to_slice(&mut buffer).unwrap());

        assert_eq!(
            &[0x7F],
            Length::from(0x7Fu8).encode_to_slice(&mut buffer).unwrap()
        );

        assert_eq!(
            &[0x81, 0x80],
            Length::from(0x80u8).encode_to_slice(&mut buffer).unwrap()
        );

        assert_eq!(
            &[0x81, 0xFF],
            Length::from(0xFFu8).encode_to_slice(&mut buffer).unwrap()
        );

        assert_eq!(
            &[0x82, 0x01, 0x00],
            Length::from(0x100u16).encode_to_slice(&mut buffer).unwrap()
        );

        assert_eq!(
            &[0x83, 0x01, 0x00, 0x00],
            Length::try_from(0x10000u32)
                .unwrap()
                .encode_to_slice(&mut buffer)
                .unwrap()
        );
    }

    #[test]
    fn reject_indefinite_lengths() {
        assert!(Length::from_der(&[0x80]).is_err());
    }

    #[test]
    fn add_overflows_when_max_length_exceeded() {
        let result = Length::MAX + Length::ONE;
        assert_eq!(
            result.err().map(|err| err.kind()),
            Some(ErrorKind::Overflow)
        );
    }
}
