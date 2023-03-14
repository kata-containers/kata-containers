use crate::*;
use alloc::borrow::Cow;
#[cfg(feature = "bits")]
use bitvec::{order::Msb0, slice::BitSlice};
use core::convert::TryFrom;

/// ASN.1 `BITSTRING` type
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitString<'a> {
    pub unused_bits: u8,
    pub data: Cow<'a, [u8]>,
}

impl<'a> BitString<'a> {
    // Length must be >= 1 (first byte is number of ignored bits)
    pub const fn new(unused_bits: u8, s: &'a [u8]) -> Self {
        BitString {
            unused_bits,
            data: Cow::Borrowed(s),
        }
    }

    /// Test if bit `bitnum` is set
    pub fn is_set(&self, bitnum: usize) -> bool {
        let byte_pos = bitnum / 8;
        if byte_pos >= self.data.len() {
            return false;
        }
        let b = 7 - (bitnum % 8);
        (self.data[byte_pos] & (1 << b)) != 0
    }

    /// Constructs a shared `&BitSlice` reference over the object data.
    #[cfg(feature = "bits")]
    pub fn as_bitslice(&self) -> Option<&BitSlice<u8, Msb0>> {
        BitSlice::<_, Msb0>::try_from_slice(&self.data).ok()
    }
}

impl<'a> AsRef<[u8]> for BitString<'a> {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl<'a> TryFrom<Any<'a>> for BitString<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<BitString<'a>> {
        TryFrom::try_from(&any)
    }
}

// non-consuming version
impl<'a, 'b> TryFrom<&'b Any<'a>> for BitString<'a> {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<BitString<'a>> {
        any.tag().assert_eq(Self::TAG)?;
        if any.data.is_empty() {
            return Err(Error::InvalidLength);
        }
        let s = any.data;
        let (unused_bits, data) = (s[0], Cow::Borrowed(&s[1..]));
        Ok(BitString { unused_bits, data })
    }
}

impl<'a> CheckDerConstraints for BitString<'a> {
    fn check_constraints(any: &Any) -> Result<()> {
        // X.690 section 10.2
        any.header.assert_primitive()?;
        // Check that padding bits are all 0 (X.690 section 11.2.1)
        match any.data.len() {
            0 => Err(Error::InvalidLength),
            1 => {
                // X.690 section 11.2.2 Note 2
                if any.data[0] == 0 {
                    Ok(())
                } else {
                    Err(Error::InvalidLength)
                }
            }
            len => {
                let unused_bits = any.data[0];
                let last_byte = any.data[len - 1];
                if last_byte.trailing_zeros() < unused_bits as u32 {
                    return Err(Error::DerConstraintFailed(DerConstraint::UnusedBitsNotZero));
                }

                Ok(())
            }
        }
    }
}

impl DerAutoDerive for BitString<'_> {}

impl<'a> Tagged for BitString<'a> {
    const TAG: Tag = Tag::BitString;
}

#[cfg(feature = "std")]
impl ToDer for BitString<'_> {
    fn to_der_len(&self) -> Result<usize> {
        let sz = self.data.len();
        if sz < 127 {
            // 1 (class+tag) + 1 (length) +  1 (unused bits) + len
            Ok(3 + sz)
        } else {
            // 1 (class+tag) + n (length) + 1 (unused bits) + len
            let n = Length::Definite(sz + 1).to_der_len()?;
            Ok(2 + n + sz)
        }
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let header = Header::new(
            Class::Universal,
            false,
            Self::TAG,
            Length::Definite(1 + self.data.len()),
        );
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let sz = writer.write(&[self.unused_bits])?;
        let sz = sz + writer.write(&self.data)?;
        Ok(sz)
    }
}

#[cfg(test)]
mod tests {
    use super::BitString;

    #[test]
    fn test_bitstring_is_set() {
        let obj = BitString::new(0, &[0x0f, 0x00, 0x40]);
        assert!(!obj.is_set(0));
        assert!(obj.is_set(7));
        assert!(!obj.is_set(9));
        assert!(obj.is_set(17));
    }

    #[cfg(feature = "bits")]
    #[test]
    fn test_bitstring_to_bitvec() {
        let obj = BitString::new(0, &[0x0f, 0x00, 0x40]);
        let bv = obj.as_bitslice().expect("could not get bitslice");
        assert_eq!(bv.get(0).as_deref(), Some(&false));
        assert_eq!(bv.get(7).as_deref(), Some(&true));
        assert_eq!(bv.get(9).as_deref(), Some(&false));
        assert_eq!(bv.get(17).as_deref(), Some(&true));
    }
}
