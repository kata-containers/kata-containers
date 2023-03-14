use crate::misc::WriteExt;
use crate::{Result, Serializer};
use picky_asn1::tag::Tag;

/// A trait that allows you to map all unsigned integers to a `u128`
pub trait UInt: Sized + Copy {
    /// Converts `self` into a `u128`
    fn into_u128(self) -> u128;
}
macro_rules! impl_uint {
	($type:ident) => {
		impl UInt for $type {
			fn into_u128(self) -> u128 {
				self as u128
			}
		}
	};
	($($type:ident),+) => ($( impl_uint!($type); )+)
}
impl_uint!(usize, u128, u64, u32, u16, u8);

/// A serializer for unsigned integers
pub struct UnsignedInteger;
impl UnsignedInteger {
    /// Serializes `value` into `writer`
    pub fn serialize<T: UInt>(value: T, ser: &mut Serializer) -> Result<usize> {
        // Convert the value and compute the amount of bytes to skip
        let value = value.into_u128();
        let skip = match value.leading_zeros() as usize {
            n if n % 8 == 0 => n / 8,
            n => (n / 8) + 1,
        };
        let length = 17 - skip;

        let mut written = ser.h_write_header(Tag::INTEGER, length)?;

        // Serialize the value and write the bytes
        let mut bytes = [0; 17];
        bytes[1..].copy_from_slice(&value.to_be_bytes());
        written += ser.writer.write_exact(&bytes[skip..])?;

        Ok(written)
    }
}
