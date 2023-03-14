use crate::*;
use alloc::borrow::Cow;
use alloc::vec;
use core::convert::{TryFrom, TryInto};

#[cfg(feature = "bigint")]
#[cfg_attr(docsrs, doc(cfg(feature = "bigint")))]
pub use num_bigint::{BigInt, BigUint, Sign};

/// Decode an unsigned integer into a big endian byte slice with all leading
/// zeroes removed (if positive) and extra 0xff remove (if negative)
fn trim_slice<'a>(any: &'a Any<'_>) -> Result<&'a [u8]> {
    let bytes = any.data;

    if bytes.is_empty() || (bytes[0] != 0x00 && bytes[0] != 0xff) {
        return Ok(bytes);
    }

    match bytes.iter().position(|&b| b != 0) {
        // first byte is not 0
        Some(0) => (),
        // all bytes are 0
        None => return Ok(&bytes[bytes.len() - 1..]),
        Some(first) => return Ok(&bytes[first..]),
    }

    // same for negative integers : skip byte 0->n if byte 0->n = 0xff AND byte n+1 >= 0x80
    match bytes.windows(2).position(|s| match s {
        &[a, b] => !(a == 0xff && b >= 0x80),
        _ => true,
    }) {
        // first byte is not 0xff
        Some(0) => (),
        // all bytes are 0xff
        None => return Ok(&bytes[bytes.len() - 1..]),
        Some(first) => return Ok(&bytes[first..]),
    }

    Ok(bytes)
}

/// Decode an unsigned integer into a byte array of the requested size
/// containing a big endian integer.
fn decode_array_uint<const N: usize>(any: &Any<'_>) -> Result<[u8; N]> {
    if is_highest_bit_set(any.data) {
        return Err(Error::IntegerNegative);
    }
    let input = trim_slice(any)?;

    if input.len() > N {
        return Err(Error::IntegerTooLarge);
    }

    // Input has leading zeroes removed, so we need to add them back
    let mut output = [0u8; N];
    assert!(input.len() <= N);
    output[N.saturating_sub(input.len())..].copy_from_slice(input);
    Ok(output)
}

/// Decode an unsigned integer of the specified size.
///
/// Returns a byte array of the requested size containing a big endian integer.
fn decode_array_int<const N: usize>(any: &Any<'_>) -> Result<[u8; N]> {
    if any.data.len() > N {
        return Err(Error::IntegerTooLarge);
    }

    // any.tag().assert_eq(Tag::Integer)?;
    let mut output = [0xFFu8; N];
    let offset = N.saturating_sub(any.as_bytes().len());
    output[offset..].copy_from_slice(any.as_bytes());
    Ok(output)
}

/// Is the highest bit of the first byte in the slice 1? (if present)
#[inline]
fn is_highest_bit_set(bytes: &[u8]) -> bool {
    bytes
        .get(0)
        .map(|byte| byte & 0b10000000 != 0)
        .unwrap_or(false)
}

macro_rules! impl_int {
    ($uint:ty => $int:ty) => {
        impl<'a> TryFrom<Any<'a>> for $int {
            type Error = Error;

            fn try_from(any: Any<'a>) -> Result<Self> {
                TryFrom::try_from(&any)
            }
        }

        impl<'a, 'b> TryFrom<&'b Any<'a>> for $int {
            type Error = Error;

            fn try_from(any: &'b Any<'a>) -> Result<Self> {
                any.tag().assert_eq(Self::TAG)?;
                any.header.assert_primitive()?;
                let result = if is_highest_bit_set(any.as_bytes()) {
                    <$uint>::from_be_bytes(decode_array_int(&any)?) as $int
                } else {
                    Self::from_be_bytes(decode_array_uint(&any)?)
                };
                Ok(result)
            }
        }

        impl<'a> CheckDerConstraints for $int {
            fn check_constraints(any: &Any) -> Result<()> {
                check_der_int_constraints(any)
            }
        }

        impl DerAutoDerive for $int {}

        impl Tagged for $int {
            const TAG: Tag = Tag::Integer;
        }

        #[cfg(feature = "std")]
        impl ToDer for $int {
            fn to_der_len(&self) -> Result<usize> {
                let int = Integer::from(*self);
                int.to_der_len()
            }

            fn write_der(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
                let int = Integer::from(*self);
                int.write_der(writer)
            }

            fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
                let int = Integer::from(*self);
                int.write_der_header(writer)
            }

            fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
                let int = Integer::from(*self);
                int.write_der_content(writer)
            }
        }
    };
}

macro_rules! impl_uint {
    ($ty:ty) => {
        impl<'a> TryFrom<Any<'a>> for $ty {
            type Error = Error;

            fn try_from(any: Any<'a>) -> Result<Self> {
                TryFrom::try_from(&any)
            }
        }
        impl<'a, 'b> TryFrom<&'b Any<'a>> for $ty {
            type Error = Error;

            fn try_from(any: &'b Any<'a>) -> Result<Self> {
                any.tag().assert_eq(Self::TAG)?;
                any.header.assert_primitive()?;
                let result = Self::from_be_bytes(decode_array_uint(any)?);
                Ok(result)
            }
        }
        impl<'a> CheckDerConstraints for $ty {
            fn check_constraints(any: &Any) -> Result<()> {
                check_der_int_constraints(any)
            }
        }

        impl DerAutoDerive for $ty {}

        impl Tagged for $ty {
            const TAG: Tag = Tag::Integer;
        }

        #[cfg(feature = "std")]
        impl ToDer for $ty {
            fn to_der_len(&self) -> Result<usize> {
                let int = Integer::from(*self);
                int.to_der_len()
            }

            fn write_der(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
                let int = Integer::from(*self);
                int.write_der(writer)
            }

            fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
                let int = Integer::from(*self);
                int.write_der_header(writer)
            }

            fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
                let int = Integer::from(*self);
                int.write_der_content(writer)
            }
        }
    };
}

impl_uint!(u8);
impl_uint!(u16);
impl_uint!(u32);
impl_uint!(u64);
impl_uint!(u128);
impl_int!(u8 => i8);
impl_int!(u16 => i16);
impl_int!(u32 => i32);
impl_int!(u64 => i64);
impl_int!(u128 => i128);

/// ASN.1 `INTEGER` type
///
/// Generic representation for integer types.
/// BER/DER integers can be of any size, so it is not possible to store them as simple integers (they
/// are stored as raw bytes).
///
/// The internal representation can be obtained using `.as_ref()`.
///
/// # Note
///
/// Methods from/to BER and DER encodings are also implemented for primitive types
/// (`u8`, `u16` to `u128`, and `i8` to `i128`).
/// In most cases, it is easier to use these types directly.
///
/// # Examples
///
/// Creating an `Integer`
///
/// ```
/// use asn1_rs::Integer;
///
/// // unsigned
/// let i = Integer::from(4);
/// assert_eq!(i.as_ref(), &[4]);
/// // signed
/// let j = Integer::from(-2);
/// assert_eq!(j.as_ref(), &[0x0, 0xff, 0xff, 0xff, 0xfe]);
/// ```
///
/// Converting an `Integer` to a primitive type (using the `TryInto` trait)
///
/// ```
/// use asn1_rs::{Error, Integer};
/// use std::convert::TryInto;
///
/// let i = Integer::new(&[0x12, 0x34, 0x56, 0x78]);
/// // converts to an u32
/// let n: u32 = i.try_into().unwrap();
///
/// // Same, but converting to an u16: will fail, value cannot fit into an u16
/// let i = Integer::new(&[0x12, 0x34, 0x56, 0x78]);
/// assert_eq!(i.try_into() as Result<u16, _>, Err(Error::IntegerTooLarge));
/// ```
///
/// Encoding an `Integer` to DER
///
/// ```
/// use asn1_rs::{Integer, ToDer};
///
/// let i = Integer::from(4);
/// let v = i.to_der_vec().unwrap();
/// assert_eq!(&v, &[2, 1, 4]);
///
/// // same, with primitive types
/// let v = 4.to_der_vec().unwrap();
/// assert_eq!(&v, &[2, 1, 4]);
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct Integer<'a> {
    pub(crate) data: Cow<'a, [u8]>,
}

impl<'a> Integer<'a> {
    /// Creates a new `Integer` containing the given value (borrowed).
    #[inline]
    pub const fn new(s: &'a [u8]) -> Self {
        Integer {
            data: Cow::Borrowed(s),
        }
    }

    /// Creates a borrowed `Any` for this object
    #[inline]
    pub fn any(&'a self) -> Any<'a> {
        Any::from_tag_and_data(Self::TAG, &self.data)
    }

    /// Returns a `BigInt` built from this `Integer` value.
    #[cfg(feature = "bigint")]
    #[cfg_attr(docsrs, doc(cfg(feature = "bigint")))]
    pub fn as_bigint(&self) -> BigInt {
        BigInt::from_signed_bytes_be(&self.data)
    }

    /// Returns a `BigUint` built from this `Integer` value.
    #[cfg(feature = "bigint")]
    #[cfg_attr(docsrs, doc(cfg(feature = "bigint")))]
    pub fn as_biguint(&self) -> Result<BigUint> {
        if is_highest_bit_set(&self.data) {
            Err(Error::IntegerNegative)
        } else {
            Ok(BigUint::from_bytes_be(&self.data))
        }
    }

    /// Build an `Integer` from a constant array of bytes representation of an integer.
    pub fn from_const_array<const N: usize>(b: [u8; N]) -> Self {
        let mut idx = 0;
        // skip leading 0s
        while idx < b.len() {
            if b[idx] == 0 {
                idx += 1;
                continue;
            }
            break;
        }
        if idx == b.len() {
            Integer {
                data: Cow::Borrowed(&[0]),
            }
        } else {
            Integer {
                data: Cow::Owned(b[idx..].to_vec()),
            }
        }
    }

    fn from_const_array_negative<const N: usize>(b: [u8; N]) -> Self {
        let mut out = vec![0];
        out.extend_from_slice(&b);

        Integer {
            data: Cow::Owned(out),
        }
    }
}

macro_rules! impl_from_to {
    ($ty:ty, $sty:expr, $from:ident, $to:ident) => {
        impl From<$ty> for Integer<'_> {
            fn from(i: $ty) -> Self {
                Self::$from(i)
            }
        }

        impl TryFrom<Integer<'_>> for $ty {
            type Error = Error;

            fn try_from(value: Integer<'_>) -> Result<Self> {
                value.$to()
            }
        }

        impl Integer<'_> {
            #[doc = "Attempts to convert an `Integer` to a `"]
            #[doc = $sty]
            #[doc = "`."]
            #[doc = ""]
            #[doc = "This function returns an `IntegerTooLarge` error if the integer will not fit into the output type."]
            pub fn $to(&self) -> Result<$ty> {
                self.any().try_into()
            }
        }
    };
    (IMPL SIGNED $ty:ty, $sty:expr, $from:ident, $to:ident) => {
        impl_from_to!($ty, $sty, $from, $to);

        impl Integer<'_> {
            #[doc = "Converts a `"]
            #[doc = $sty]
            #[doc = "` to an `Integer`"]
            #[doc = ""]
            #[doc = "Note: this function allocates data."]
            pub fn $from(i: $ty) -> Self {
                let b = i.to_be_bytes();
                if i >= 0 {
                    Self::from_const_array(b)
                } else {
                    Self::from_const_array_negative(b)
                }
            }
        }
    };
    (IMPL UNSIGNED $ty:ty, $sty:expr, $from:ident, $to:ident) => {
        impl_from_to!($ty, $sty, $from, $to);

        impl Integer<'_> {
            #[doc = "Converts a `"]
            #[doc = $sty]
            #[doc = "` to an `Integer`"]
            #[doc = ""]
            #[doc = "Note: this function allocates data."]
            pub fn $from(i: $ty) -> Self {
                Self::from_const_array(i.to_be_bytes())
            }
        }
    };
    (SIGNED $ty:ty, $from:ident, $to:ident) => {
        impl_from_to!(IMPL SIGNED $ty, stringify!($ty), $from, $to);
    };
    (UNSIGNED $ty:ty, $from:ident, $to:ident) => {
        impl_from_to!(IMPL UNSIGNED $ty, stringify!($ty), $from, $to);
    };
}

impl_from_to!(SIGNED i8, from_i8, as_i8);
impl_from_to!(SIGNED i16, from_i16, as_i16);
impl_from_to!(SIGNED i32, from_i32, as_i32);
impl_from_to!(SIGNED i64, from_i64, as_i64);
impl_from_to!(SIGNED i128, from_i128, as_i128);

impl_from_to!(UNSIGNED u8, from_u8, as_u8);
impl_from_to!(UNSIGNED u16, from_u16, as_u16);
impl_from_to!(UNSIGNED u32, from_u32, as_u32);
impl_from_to!(UNSIGNED u64, from_u64, as_u64);
impl_from_to!(UNSIGNED u128, from_u128, as_u128);

impl<'a> AsRef<[u8]> for Integer<'a> {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl<'a> TryFrom<Any<'a>> for Integer<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Integer<'a>> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for Integer<'a> {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<Integer<'a>> {
        any.tag().assert_eq(Self::TAG)?;
        Ok(Integer {
            data: Cow::Borrowed(any.data),
        })
    }
}

impl<'a> CheckDerConstraints for Integer<'a> {
    fn check_constraints(any: &Any) -> Result<()> {
        check_der_int_constraints(any)
    }
}

fn check_der_int_constraints(any: &Any) -> Result<()> {
    any.header.assert_primitive()?;
    any.header.length.assert_definite()?;
    match any.as_bytes() {
        [] => Err(Error::DerConstraintFailed(DerConstraint::IntegerEmpty)),
        [0] => Ok(()),
        // leading zeroes
        [0, byte, ..] if *byte < 0x80 => Err(Error::DerConstraintFailed(
            DerConstraint::IntegerLeadingZeroes,
        )),
        // negative integer with non-minimal encoding
        [0xff, byte, ..] if *byte >= 0x80 => {
            Err(Error::DerConstraintFailed(DerConstraint::IntegerLeadingFF))
        }
        _ => Ok(()),
    }
}

impl DerAutoDerive for Integer<'_> {}

impl<'a> Tagged for Integer<'a> {
    const TAG: Tag = Tag::Integer;
}

#[cfg(feature = "std")]
impl ToDer for Integer<'_> {
    fn to_der_len(&self) -> Result<usize> {
        let sz = self.data.len();
        if sz < 127 {
            // 1 (class+tag) + 1 (length) + len
            Ok(2 + sz)
        } else {
            // hmm, a very long integer. anyway:
            // 1 (class+tag) + n (length) + len
            let n = Length::Definite(sz).to_der_len()?;
            Ok(1 + n + sz)
        }
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let header = Header::new(
            Class::Universal,
            false,
            Self::TAG,
            Length::Definite(self.data.len()),
        );
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(&self.data).map_err(Into::into)
    }
}

/// Helper macro to declare integers at compile-time
///
/// [`Integer`] stores the encoded representation of the integer, so declaring
/// an integer requires to either use a runtime function or provide the encoded value.
/// This macro simplifies this task by encoding the value.
/// It can be used the following ways:
///
/// - `int!(1234)`: Create a const expression for the corresponding `Integer<'static>`
/// - `int!(raw 1234)`: Return the DER encoded form as a byte array (hex-encoded, big-endian
///    representation from the integer, with leading zeroes removed).
///
/// # Examples
///
/// ```rust
/// use asn1_rs::{int, Integer};
///
/// const INT0: Integer = int!(1234);
/// ```
#[macro_export]
macro_rules! int {
    (raw $item:expr) => {
        $crate::exports::asn1_rs_impl::encode_int!($item)
    };
    (rel $item:expr) => {
        $crate::exports::asn1_rs_impl::encode_int!(rel $item)
    };
    ($item:expr) => {
        $crate::Integer::new(
            &$crate::int!(raw $item),
        )
    };
}

#[cfg(test)]
mod tests {
    use crate::{Any, FromDer, Header, Tag};
    use std::convert::TryInto;

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
        assert_eq!(0, i8::from_der(I0_BYTES).unwrap().1);
        assert_eq!(127, i8::from_der(I127_BYTES).unwrap().1);
        assert_eq!(-128, i8::from_der(INEG128_BYTES).unwrap().1);
    }

    #[test]
    fn decode_i16() {
        assert_eq!(0, i16::from_der(I0_BYTES).unwrap().1);
        assert_eq!(127, i16::from_der(I127_BYTES).unwrap().1);
        assert_eq!(128, i16::from_der(I128_BYTES).unwrap().1);
        assert_eq!(255, i16::from_der(I255_BYTES).unwrap().1);
        assert_eq!(256, i16::from_der(I256_BYTES).unwrap().1);
        assert_eq!(32767, i16::from_der(I32767_BYTES).unwrap().1);
        assert_eq!(-128, i16::from_der(INEG128_BYTES).unwrap().1);
        assert_eq!(-129, i16::from_der(INEG129_BYTES).unwrap().1);
        assert_eq!(-32768, i16::from_der(INEG32768_BYTES).unwrap().1);
    }

    #[test]
    fn decode_u8() {
        assert_eq!(0, u8::from_der(I0_BYTES).unwrap().1);
        assert_eq!(127, u8::from_der(I127_BYTES).unwrap().1);
        assert_eq!(255, u8::from_der(I255_BYTES).unwrap().1);
    }

    #[test]
    fn decode_u16() {
        assert_eq!(0, u16::from_der(I0_BYTES).unwrap().1);
        assert_eq!(127, u16::from_der(I127_BYTES).unwrap().1);
        assert_eq!(255, u16::from_der(I255_BYTES).unwrap().1);
        assert_eq!(256, u16::from_der(I256_BYTES).unwrap().1);
        assert_eq!(32767, u16::from_der(I32767_BYTES).unwrap().1);
        assert_eq!(65535, u16::from_der(I65535_BYTES).unwrap().1);
    }

    /// Integers must be encoded with a minimum number of octets
    #[test]
    fn reject_non_canonical() {
        assert!(i8::from_der(&[0x02, 0x02, 0x00, 0x00]).is_err());
        assert!(i16::from_der(&[0x02, 0x02, 0x00, 0x00]).is_err());
        assert!(u8::from_der(&[0x02, 0x02, 0x00, 0x00]).is_err());
        assert!(u16::from_der(&[0x02, 0x02, 0x00, 0x00]).is_err());
    }

    #[test]
    fn declare_int() {
        let int = super::int!(1234);
        assert_eq!(int.try_into(), Ok(1234));
    }

    #[test]
    fn trim_slice() {
        use super::trim_slice;
        let h = Header::new_simple(Tag(0));
        // no zero nor ff - nothing to remove
        let input: &[u8] = &[0x7f, 0xff, 0x00, 0x02];
        assert_eq!(Ok(input), trim_slice(&Any::new(h.clone(), input)));
        //
        // 0x00
        //
        // empty - nothing to remove
        let input: &[u8] = &[];
        assert_eq!(Ok(input), trim_slice(&Any::new(h.clone(), input)));
        // one zero - nothing to remove
        let input: &[u8] = &[0];
        assert_eq!(Ok(input), trim_slice(&Any::new(h.clone(), input)));
        // all zeroes - keep only one
        let input: &[u8] = &[0, 0, 0];
        assert_eq!(Ok(&input[2..]), trim_slice(&Any::new(h.clone(), input)));
        // some zeroes - keep only the non-zero part
        let input: &[u8] = &[0, 0, 1];
        assert_eq!(Ok(&input[2..]), trim_slice(&Any::new(h.clone(), input)));
        //
        // 0xff
        //
        // one ff - nothing to remove
        let input: &[u8] = &[0xff];
        assert_eq!(Ok(input), trim_slice(&Any::new(h.clone(), input)));
        // all ff - keep only one
        let input: &[u8] = &[0xff, 0xff, 0xff];
        assert_eq!(Ok(&input[2..]), trim_slice(&Any::new(h.clone(), input)));
        // some ff - keep only the non-zero part
        let input: &[u8] = &[0xff, 0xff, 1];
        assert_eq!(Ok(&input[1..]), trim_slice(&Any::new(h.clone(), input)));
        // some ff and a MSB 1 - keep only the non-zero part
        let input: &[u8] = &[0xff, 0xff, 0x80, 1];
        assert_eq!(Ok(&input[2..]), trim_slice(&Any::new(h.clone(), input)));
    }
}
