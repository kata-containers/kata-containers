//! ASN.1 `INTEGER` support.

// TODO(tarcieri): add support for `i32`/`u32`

use crate::{Any, Encodable, Encoder, Error, ErrorKind, Header, Length, Result, Tag, Tagged};
use core::convert::TryFrom;

//
// i8
//

impl TryFrom<Any<'_>> for i8 {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<i8> {
        let tag = any.tag().assert_eq(Tag::Integer)?;

        match *any.as_bytes() {
            [x] => Ok(x as i8),
            _ => Err(ErrorKind::Length { tag }.into()),
        }
    }
}

impl Encodable for i8 {
    fn encoded_len(&self) -> Result<Length> {
        Length::ONE.for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Header::new(Self::TAG, Length::ONE)?.encode(encoder)?;
        encoder.byte(*self as u8)
    }
}

impl Tagged for i8 {
    const TAG: Tag = Tag::Integer;
}

//
// i16
//

impl TryFrom<Any<'_>> for i16 {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<i16> {
        let tag = any.tag().assert_eq(Tag::Integer)?;

        match *any.as_bytes() {
            [_] => i8::try_from(any).map(|x| x as i16),
            [0, lo] if lo < 0x80 => Err(ErrorKind::Noncanonical.into()),
            [hi, lo] => Ok(i16::from_be_bytes([hi, lo])),
            _ => Err(ErrorKind::Length { tag }.into()),
        }
    }
}

impl Encodable for i16 {
    fn encoded_len(&self) -> Result<Length> {
        if let Ok(x) = i8::try_from(*self) {
            return x.encoded_len();
        }

        Length::from(2u8).for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        if let Ok(x) = i8::try_from(*self) {
            return x.encode(encoder);
        }

        Header::new(Self::TAG, Length::from(2u8))?.encode(encoder)?;
        encoder.bytes(&self.to_be_bytes())
    }
}

impl Tagged for i16 {
    const TAG: Tag = Tag::Integer;
}

//
// u8
//

impl TryFrom<Any<'_>> for u8 {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<u8> {
        let tag = any.tag().assert_eq(Tag::Integer)?;

        match *any.as_bytes() {
            [x] if x < 0x80 => Ok(x),
            [x] if x >= 0x80 => Err(ErrorKind::Noncanonical.into()),
            [0, x] if x < 0x80 => Err(ErrorKind::Noncanonical.into()),
            [0, x] if x >= 0x80 => Ok(x),
            _ => Err(ErrorKind::Length { tag }.into()),
        }
    }
}

impl Encodable for u8 {
    fn encoded_len(&self) -> Result<Length> {
        let inner_len = if *self < 0x80 { 1u8 } else { 2u8 };
        Length::from(inner_len).for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Header::new(Self::TAG, if *self < 0x80 { 1u8 } else { 2u8 })?.encode(encoder)?;

        if *self >= 0x80 {
            encoder.byte(0)?;
        }

        encoder.byte(*self as u8)
    }
}

impl Tagged for u8 {
    const TAG: Tag = Tag::Integer;
}

//
// u16
//

impl TryFrom<Any<'_>> for u16 {
    type Error = Error;

    fn try_from(any: Any<'_>) -> Result<u16> {
        let tag = any.tag().assert_eq(Tag::Integer)?;

        match *any.as_bytes() {
            [x] if x < 0x80 => Ok(x as u16),
            [x] if x >= 0x80 => Err(ErrorKind::Noncanonical.into()),
            [0, x] if x < 0x80 => Err(ErrorKind::Noncanonical.into()),
            [hi, lo] if hi < 0x80 => Ok(u16::from_be_bytes([hi, lo])),
            [0, hi, lo] if hi >= 0x80 => Ok(u16::from_be_bytes([hi, lo])),
            _ => Err(ErrorKind::Length { tag }.into()),
        }
    }
}

impl Encodable for u16 {
    fn encoded_len(&self) -> Result<Length> {
        if let Ok(x) = u8::try_from(*self) {
            return x.encoded_len();
        }

        let inner_len = if *self < 0x8000 { 2u16 } else { 3u16 };
        Length::from(inner_len).for_tlv()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        if let Ok(x) = u8::try_from(*self) {
            return x.encode(encoder);
        }

        Header::new(Self::TAG, if *self < 0x8000 { 2u16 } else { 3u16 })?.encode(encoder)?;

        if *self >= 0x8000 {
            encoder.byte(0)?;
        }

        encoder.bytes(&self.to_be_bytes())
    }
}

impl Tagged for u16 {
    const TAG: Tag = Tag::Integer;
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::{Decodable, Encodable};

    // Vectors from Section 5.7 of:
    // https://luca.ntop.org/Teaching/Appunti/asn1.html
    pub(crate) const I0_BYTES: &[u8] = &[0x02, 0x01, 0x00];
    pub(crate) const I127_BYTES: &[u8] = &[0x02, 0x01, 0x7F];
    pub(crate) const I128_BYTES: &[u8] = &[0x02, 0x02, 0x00, 0x80];
    pub(crate) const I256_BYTES: &[u8] = &[0x02, 0x02, 0x01, 0x00];
    pub(crate) const INEG128_BYTES: &[u8] = &[0x02, 0x01, 0x80];
    pub(crate) const INEG129_BYTES: &[u8] = &[0x02, 0x02, 0xFF, 0x7F];

    // Additional vectors
    pub(crate) const I255_BYTES: &[u8] = &[0x02, 0x02, 0x00, 0xFF];
    pub(crate) const I32767_BYTES: &[u8] = &[0x02, 0x02, 0x7F, 0xFF];
    pub(crate) const I65535_BYTES: &[u8] = &[0x02, 0x03, 0x00, 0xFF, 0xFF];
    pub(crate) const INEG32768_BYTES: &[u8] = &[0x02, 0x02, 0x80, 0x00];

    #[test]
    fn decode_i8() {
        assert_eq!(0, i8::from_der(I0_BYTES).unwrap());
        assert_eq!(127, i8::from_der(I127_BYTES).unwrap());
        assert_eq!(-128, i8::from_der(INEG128_BYTES).unwrap());
    }

    #[test]
    fn decode_i16() {
        assert_eq!(0, i16::from_der(I0_BYTES).unwrap());
        assert_eq!(127, i16::from_der(I127_BYTES).unwrap());
        assert_eq!(128, i16::from_der(I128_BYTES).unwrap());
        assert_eq!(255, i16::from_der(I255_BYTES).unwrap());
        assert_eq!(256, i16::from_der(I256_BYTES).unwrap());
        assert_eq!(32767, i16::from_der(I32767_BYTES).unwrap());
        assert_eq!(-128, i16::from_der(INEG128_BYTES).unwrap());
        assert_eq!(-129, i16::from_der(INEG129_BYTES).unwrap());
        assert_eq!(-32768, i16::from_der(INEG32768_BYTES).unwrap());
    }

    #[test]
    fn decode_u8() {
        assert_eq!(0, u8::from_der(I0_BYTES).unwrap());
        assert_eq!(127, u8::from_der(I127_BYTES).unwrap());
        assert_eq!(255, u8::from_der(I255_BYTES).unwrap());
    }

    #[test]
    fn decode_u16() {
        assert_eq!(0, u16::from_der(I0_BYTES).unwrap());
        assert_eq!(127, u16::from_der(I127_BYTES).unwrap());
        assert_eq!(255, u16::from_der(I255_BYTES).unwrap());
        assert_eq!(256, u16::from_der(I256_BYTES).unwrap());
        assert_eq!(32767, u16::from_der(I32767_BYTES).unwrap());
        assert_eq!(65535, u16::from_der(I65535_BYTES).unwrap());
    }

    #[test]
    fn encode_i8() {
        let mut buffer = [0u8; 3];

        assert_eq!(I0_BYTES, 0i8.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I127_BYTES, 127i8.encode_to_slice(&mut buffer).unwrap());

        assert_eq!(
            INEG128_BYTES,
            (-128i8).encode_to_slice(&mut buffer).unwrap()
        );
    }

    #[test]
    fn encode_i16() {
        let mut buffer = [0u8; 4];
        assert_eq!(I0_BYTES, 0i16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I127_BYTES, 127i16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I128_BYTES, 128i16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I255_BYTES, 255i16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I256_BYTES, 256i16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I32767_BYTES, 32767i16.encode_to_slice(&mut buffer).unwrap());

        assert_eq!(
            INEG128_BYTES,
            (-128i16).encode_to_slice(&mut buffer).unwrap()
        );

        assert_eq!(
            INEG129_BYTES,
            (-129i16).encode_to_slice(&mut buffer).unwrap()
        );

        assert_eq!(
            INEG32768_BYTES,
            (-32768i16).encode_to_slice(&mut buffer).unwrap()
        );
    }

    #[test]
    fn encode_u8() {
        let mut buffer = [0u8; 4];
        assert_eq!(I0_BYTES, 0u8.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I127_BYTES, 127u8.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I255_BYTES, 255u8.encode_to_slice(&mut buffer).unwrap());
    }

    #[test]
    fn encode_u16() {
        let mut buffer = [0u8; 5];
        assert_eq!(I0_BYTES, 0u16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I127_BYTES, 127u16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I128_BYTES, 128u16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I255_BYTES, 255u16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I256_BYTES, 256u16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I32767_BYTES, 32767u16.encode_to_slice(&mut buffer).unwrap());
        assert_eq!(I65535_BYTES, 65535u16.encode_to_slice(&mut buffer).unwrap());
    }

    /// Integers must be encoded with a minimum number of octets
    #[test]
    fn reject_non_canonical() {
        assert!(i8::from_der(&[0x02, 0x02, 0x00, 0x00]).is_err());
        assert!(i16::from_der(&[0x02, 0x02, 0x00, 0x00]).is_err());
        assert!(u8::from_der(&[0x02, 0x02, 0x00, 0x00]).is_err());
        assert!(u16::from_der(&[0x02, 0x02, 0x00, 0x00]).is_err());
    }
}
