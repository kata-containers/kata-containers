use crate::{Asn1DerError, Result};

/// A trait that allows you to convert all unsigned integers from a `u128` (if possible)
pub trait UInt: Sized + Copy {
    /// Converts `num` into `Self`
    fn from_u128(num: u128) -> Result<Self>;
}
macro_rules! impl_uint {
	($type:ident) => {
		impl UInt for $type {
			fn from_u128(num: u128) -> Result<Self> {
				const MAX: u128 = $type::max_value() as u128;
				match num {
					_ if num > MAX => Err(Asn1DerError::UnsupportedValue),
					_ => Ok(num as Self)
				}
			}
		}
	};
	($($type:ident),+) => ($( impl_uint!($type); )+)
}
impl_uint!(usize, u128, u64, u32, u16, u8);

/// A deserializer for unsigned integers
pub struct UnsignedInteger;
impl UnsignedInteger {
    /// The deserialized integer for `data`
    pub fn deserialize<T: UInt>(data: &[u8]) -> Result<T> {
        // Check that we have some data
        if data.is_empty() {
            return Err(Asn1DerError::TruncatedData);
        }

        // Check first byte (number is signed, has leading zero, ...)
        let data = match data[0] {
            128..=255 => return Err(Asn1DerError::UnsupportedValue),
            0 if data.len() > 1 && data[1] < 128 => return Err(Asn1DerError::InvalidData),
            0 => &data[1..],
            _ => data,
        };
        // Check the data length
        if data.len() > 16 {
            return Err(Asn1DerError::UnsupportedValue);
        }

        // Deserialize data
        let mut num = [0; 16];
        num[16 - data.len()..].copy_from_slice(data);
        T::from_u128(u128::from_be_bytes(num))
    }
}
