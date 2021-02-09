//! Utility functions and types for encoding and decoding Protobuf types.
//!
//! Meant to be used only from `Message` implementations.

use std::cmp::min;
use std::str;
use std::u32;
use std::usize;

use ::bytes::{Buf, BufMut};

use crate::DecodeError;
use crate::Message;

/// Encodes an integer value into LEB128 variable length format, and writes it to the buffer.
/// The buffer must have enough remaining space (maximum 10 bytes).
#[inline]
pub fn encode_varint<B>(mut value: u64, buf: &mut B)
where
    B: BufMut,
{
    // Safety notes:
    //
    // - bytes_mut is unsafe because it may return an uninitialized slice.
    //   The use here is safe because the slice is only written to, never read from.
    //
    // - advance_mut is unsafe because it could cause uninitialized memory to be
    //   advanced over. The use here is safe since each byte which is advanced over
    //   has been written to in the previous loop iteration.
    let mut i;
    'outer: loop {
        i = 0;

        for byte in unsafe { buf.bytes_mut() } {
            i += 1;
            if value < 0x80 {
                *byte = value as u8;
                break 'outer;
            } else {
                *byte = ((value & 0x7F) | 0x80) as u8;
                value >>= 7;
            }
        }

        unsafe {
            buf.advance_mut(i);
        }
        debug_assert!(buf.has_remaining_mut());
    }

    unsafe {
        buf.advance_mut(i);
    }
}

/// Decodes a LEB128-encoded variable length integer from the buffer.
pub fn decode_varint<B>(buf: &mut B) -> Result<u64, DecodeError>
where
    B: Buf,
{
    let bytes = buf.bytes();
    let len = bytes.len();
    if len == 0 {
        return Err(DecodeError::new("invalid varint"));
    }

    let byte = bytes[0];
    if byte < 0x80 {
        buf.advance(1);
        Ok(u64::from(byte))
    } else if len > 10 || bytes[len - 1] < 0x80 {
        let (value, advance) = decode_varint_slice(bytes)?;
        buf.advance(advance);
        Ok(value)
    } else {
        decode_varint_slow(buf)
    }
}

/// Decodes a LEB128-encoded variable length integer from the slice, returning the value and the
/// number of bytes read.
///
/// Based loosely on [`ReadVarint64FromArray`][1].
///
/// [1]: https://github.com/google/protobuf/blob/3.3.x/src/google/protobuf/io/coded_stream.cc#L365-L406
#[inline]
fn decode_varint_slice(bytes: &[u8]) -> Result<(u64, usize), DecodeError> {
    // Fully unrolled varint decoding loop. Splitting into 32-bit pieces gives better performance.

    let mut b: u8;
    let mut part0: u32;
    b = bytes[0];
    part0 = u32::from(b);
    if b < 0x80 {
        return Ok((u64::from(part0), 1));
    };
    part0 -= 0x80;
    b = bytes[1];
    part0 += u32::from(b) << 7;
    if b < 0x80 {
        return Ok((u64::from(part0), 2));
    };
    part0 -= 0x80 << 7;
    b = bytes[2];
    part0 += u32::from(b) << 14;
    if b < 0x80 {
        return Ok((u64::from(part0), 3));
    };
    part0 -= 0x80 << 14;
    b = bytes[3];
    part0 += u32::from(b) << 21;
    if b < 0x80 {
        return Ok((u64::from(part0), 4));
    };
    part0 -= 0x80 << 21;
    let value = u64::from(part0);

    let mut part1: u32;
    b = bytes[4];
    part1 = u32::from(b);
    if b < 0x80 {
        return Ok((value + (u64::from(part1) << 28), 5));
    };
    part1 -= 0x80;
    b = bytes[5];
    part1 += u32::from(b) << 7;
    if b < 0x80 {
        return Ok((value + (u64::from(part1) << 28), 6));
    };
    part1 -= 0x80 << 7;
    b = bytes[6];
    part1 += u32::from(b) << 14;
    if b < 0x80 {
        return Ok((value + (u64::from(part1) << 28), 7));
    };
    part1 -= 0x80 << 14;
    b = bytes[7];
    part1 += u32::from(b) << 21;
    if b < 0x80 {
        return Ok((value + (u64::from(part1) << 28), 8));
    };
    part1 -= 0x80 << 21;
    let value = value + ((u64::from(part1)) << 28);

    let mut part2: u32;
    b = bytes[8];
    part2 = u32::from(b);
    if b < 0x80 {
        return Ok((value + (u64::from(part2) << 56), 9));
    };
    part2 -= 0x80;
    b = bytes[9];
    part2 += u32::from(b) << 7;
    if b < 0x80 {
        return Ok((value + (u64::from(part2) << 56), 10));
    };

    // We have overrun the maximum size of a varint (10 bytes). Assume the data is corrupt.
    Err(DecodeError::new("invalid varint"))
}

/// Decodes a LEB128-encoded variable length integer from the buffer, advancing the buffer as
/// necessary.
#[inline(never)]
fn decode_varint_slow<B>(buf: &mut B) -> Result<u64, DecodeError>
where
    B: Buf,
{
    let mut value = 0;
    for count in 0..min(10, buf.remaining()) {
        let byte = buf.get_u8();
        value |= u64::from(byte & 0x7F) << (count * 7);
        if byte <= 0x7F {
            return Ok(value);
        }
    }

    Err(DecodeError::new("invalid varint"))
}

/// Returns the encoded length of the value in LEB128 variable length format.
/// The returned value will be between 1 and 10, inclusive.
#[inline]
pub fn encoded_len_varint(value: u64) -> usize {
    // Based on [VarintSize64][1].
    // [1]: https://github.com/google/protobuf/blob/3.3.x/src/google/protobuf/io/coded_stream.h#L1301-L1309
    ((((value | 1).leading_zeros() ^ 63) * 9 + 73) / 64) as usize
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum WireType {
    Varint = 0,
    SixtyFourBit = 1,
    LengthDelimited = 2,
    ThirtyTwoBit = 5,
}

pub const MIN_TAG: u32 = 1;
pub const MAX_TAG: u32 = (1 << 29) - 1;

impl WireType {
    // TODO: impl TryFrom<u8> when stable.
    #[inline]
    pub fn try_from(val: u8) -> Result<WireType, DecodeError> {
        match val {
            0 => Ok(WireType::Varint),
            1 => Ok(WireType::SixtyFourBit),
            2 => Ok(WireType::LengthDelimited),
            5 => Ok(WireType::ThirtyTwoBit),
            _ => Err(DecodeError::new(format!(
                "invalid wire type value: {}",
                val
            ))),
        }
    }
}

/// Encodes a Protobuf field key, which consists of a wire type designator and
/// the field tag.
#[inline]
pub fn encode_key<B>(tag: u32, wire_type: WireType, buf: &mut B)
where
    B: BufMut,
{
    debug_assert!(tag >= MIN_TAG && tag <= MAX_TAG);
    let key = (tag << 3) | wire_type as u32;
    encode_varint(u64::from(key), buf);
}

/// Decodes a Protobuf field key, which consists of a wire type designator and
/// the field tag.
#[inline]
pub fn decode_key<B>(buf: &mut B) -> Result<(u32, WireType), DecodeError>
where
    B: Buf,
{
    let key = decode_varint(buf)?;
    if key > u64::from(u32::MAX) {
        return Err(DecodeError::new(format!("invalid key value: {}", key)));
    }
    let wire_type = WireType::try_from(key as u8 & 0x07)?;
    let tag = key as u32 >> 3;

    if tag < MIN_TAG {
        return Err(DecodeError::new("invalid tag value: 0"));
    }

    Ok((tag, wire_type))
}

/// Returns the width of an encoded Protobuf field key with the given tag.
/// The returned width will be between 1 and 5 bytes (inclusive).
#[inline]
pub fn key_len(tag: u32) -> usize {
    encoded_len_varint(u64::from(tag << 3))
}

/// Checks that the expected wire type matches the actual wire type,
/// or returns an error result.
#[inline]
pub fn check_wire_type(expected: WireType, actual: WireType) -> Result<(), DecodeError> {
    if expected != actual {
        return Err(DecodeError::new(format!(
            "invalid wire type: {:?} (expected {:?})",
            actual, expected
        )));
    }
    Ok(())
}

/// Helper function which abstracts reading a length delimiter prefix followed
/// by decoding values until the length of bytes is exhausted.
pub fn merge_loop<T, M, B>(value: &mut T, buf: &mut B, mut merge: M) -> Result<(), DecodeError>
where
    M: FnMut(&mut T, &mut B) -> Result<(), DecodeError>,
    B: Buf,
{
    let len = decode_varint(buf)?;
    let remaining = buf.remaining();
    if len > remaining as u64 {
        return Err(DecodeError::new("buffer underflow"));
    }

    let limit = remaining - len as usize;
    while buf.remaining() > limit {
        merge(value, buf)?;
    }

    if buf.remaining() != limit {
        return Err(DecodeError::new("delimited length exceeded"));
    }
    Ok(())
}

pub fn skip_field<B>(wire_type: WireType, buf: &mut B) -> Result<(), DecodeError>
where
    B: Buf,
{
    let len = match wire_type {
        WireType::Varint => decode_varint(buf).map(|_| 0)?,
        WireType::ThirtyTwoBit => 4,
        WireType::SixtyFourBit => 8,
        WireType::LengthDelimited => decode_varint(buf)?,
    };

    if len > buf.remaining() as u64 {
        return Err(DecodeError::new("buffer underflow"));
    }

    buf.advance(len as usize);
    Ok(())
}

/// Helper macro which emits an `encode_repeated` function for the type.
macro_rules! encode_repeated {
    ($ty:ty) => {
        pub fn encode_repeated<B>(tag: u32, values: &[$ty], buf: &mut B)
        where
            B: BufMut,
        {
            for value in values {
                encode(tag, value, buf);
            }
        }
    };
}

/// Helper macro which emits a `merge_repeated_numeric` function for the numeric type.
macro_rules! merge_repeated_numeric {
    ($ty:ty,
     $wire_type:expr,
     $merge:ident,
     $merge_repeated:ident) => {
        pub fn $merge_repeated<B>(
            wire_type: WireType,
            values: &mut Vec<$ty>,
            buf: &mut B,
        ) -> Result<(), DecodeError>
        where
            B: Buf,
        {
            if wire_type == WireType::LengthDelimited {
                // Packed.
                merge_loop(values, buf, |values, buf| {
                    let mut value = Default::default();
                    $merge($wire_type, &mut value, buf)?;
                    values.push(value);
                    Ok(())
                })
            } else {
                // Unpacked.
                check_wire_type($wire_type, wire_type)?;
                let mut value = Default::default();
                $merge(wire_type, &mut value, buf)?;
                values.push(value);
                Ok(())
            }
        }
    };
}

/// Macro which emits a module containing a set of encoding functions for a
/// variable width numeric type.
macro_rules! varint {
    ($ty:ty,
     $proto_ty:ident) => (
        varint!($ty,
                $proto_ty,
                to_uint64(value) { *value as u64 },
                from_uint64(value) { value as $ty });
    );

    ($ty:ty,
     $proto_ty:ident,
     to_uint64($to_uint64_value:ident) $to_uint64:expr,
     from_uint64($from_uint64_value:ident) $from_uint64:expr) => (

         pub mod $proto_ty {
            use crate::encoding::*;

            pub fn encode<B>(tag: u32, $to_uint64_value: &$ty, buf: &mut B) where B: BufMut {
                encode_key(tag, WireType::Varint, buf);
                encode_varint($to_uint64, buf);
            }

            pub fn merge<B>(wire_type: WireType, value: &mut $ty, buf: &mut B) -> Result<(), DecodeError> where B: Buf {
                check_wire_type(WireType::Varint, wire_type)?;
                let $from_uint64_value = decode_varint(buf)?;
                *value = $from_uint64;
                Ok(())
            }

            encode_repeated!($ty);

            pub fn encode_packed<B>(tag: u32, values: &[$ty], buf: &mut B) where B: BufMut {
                if values.is_empty() { return; }

                encode_key(tag, WireType::LengthDelimited, buf);
                let len: usize = values.iter().map(|$to_uint64_value| {
                    encoded_len_varint($to_uint64)
                }).sum();
                encode_varint(len as u64, buf);

                for $to_uint64_value in values {
                    encode_varint($to_uint64, buf);
                }
            }

            merge_repeated_numeric!($ty, WireType::Varint, merge, merge_repeated);

            #[inline]
            pub fn encoded_len(tag: u32, $to_uint64_value: &$ty) -> usize {
                key_len(tag) + encoded_len_varint($to_uint64)
            }

            #[inline]
            pub fn encoded_len_repeated(tag: u32, values: &[$ty]) -> usize {
                key_len(tag) * values.len() + values.iter().map(|$to_uint64_value| {
                    encoded_len_varint($to_uint64)
                }).sum::<usize>()
            }

            #[inline]
            pub fn encoded_len_packed(tag: u32, values: &[$ty]) -> usize {
                if values.is_empty() {
                    0
                } else {
                    let len = values.iter()
                                    .map(|$to_uint64_value| encoded_len_varint($to_uint64))
                                    .sum::<usize>();
                    key_len(tag) + encoded_len_varint(len as u64) + len
                }
            }

            #[cfg(test)]
            mod test {
                use quickcheck::{quickcheck, TestResult};

                use crate::encoding::$proto_ty::*;
                use crate::encoding::test::{
                    check_collection_type,
                    check_type,
                };

                quickcheck! {
                    fn check(value: $ty, tag: u32) -> TestResult {
                        check_type(value, tag, WireType::Varint,
                                   encode, merge, encoded_len)
                    }
                    fn check_repeated(value: Vec<$ty>, tag: u32) -> TestResult {
                        check_collection_type(value, tag, WireType::Varint,
                                              encode_repeated, merge_repeated,
                                              encoded_len_repeated)
                    }
                    fn check_packed(value: Vec<$ty>, tag: u32) -> TestResult {
                        check_type(value, tag, WireType::LengthDelimited,
                                   encode_packed, merge_repeated,
                                   encoded_len_packed)
                    }
                }
            }
         }

    );
}
varint!(bool, bool,
        to_uint64(value) if *value { 1u64 } else { 0u64 },
        from_uint64(value) value != 0);
varint!(i32, int32);
varint!(i64, int64);
varint!(u32, uint32);
varint!(u64, uint64);
varint!(i32, sint32,
        to_uint64(value) {
            ((value << 1) ^ (value >> 31)) as u32 as u64
        },
        from_uint64(value) {
            let value = value as u32;
            ((value >> 1) as i32) ^ (-((value & 1) as i32))
        });
varint!(i64, sint64,
        to_uint64(value) {
            ((value << 1) ^ (value >> 63)) as u64
        },
        from_uint64(value) {
            ((value >> 1) as i64) ^ (-((value & 1) as i64))
        });

/// Macro which emits a module containing a set of encoding functions for a
/// fixed width numeric type.
macro_rules! fixed_width {
    ($ty:ty,
     $width:expr,
     $wire_type:expr,
     $proto_ty:ident,
     $put:ident,
     $get:ident) => {
        pub mod $proto_ty {
            use crate::encoding::*;

            pub fn encode<B>(tag: u32, value: &$ty, buf: &mut B)
            where
                B: BufMut,
            {
                encode_key(tag, $wire_type, buf);
                buf.$put(*value);
            }

            pub fn merge<B>(
                wire_type: WireType,
                value: &mut $ty,
                buf: &mut B,
            ) -> Result<(), DecodeError>
            where
                B: Buf,
            {
                check_wire_type($wire_type, wire_type)?;
                if buf.remaining() < $width {
                    return Err(DecodeError::new("buffer underflow"));
                }
                *value = buf.$get();
                Ok(())
            }

            encode_repeated!($ty);

            pub fn encode_packed<B>(tag: u32, values: &[$ty], buf: &mut B)
            where
                B: BufMut,
            {
                if values.is_empty() {
                    return;
                }

                encode_key(tag, WireType::LengthDelimited, buf);
                let len = values.len() as u64 * $width;
                encode_varint(len as u64, buf);

                for value in values {
                    buf.$put(*value);
                }
            }

            merge_repeated_numeric!($ty, $wire_type, merge, merge_repeated);

            #[inline]
            pub fn encoded_len(tag: u32, _: &$ty) -> usize {
                key_len(tag) + $width
            }

            #[inline]
            pub fn encoded_len_repeated(tag: u32, values: &[$ty]) -> usize {
                (key_len(tag) + $width) * values.len()
            }

            #[inline]
            pub fn encoded_len_packed(tag: u32, values: &[$ty]) -> usize {
                if values.is_empty() {
                    0
                } else {
                    let len = $width * values.len();
                    key_len(tag) + encoded_len_varint(len as u64) + len
                }
            }

            #[cfg(test)]
            mod test {
                use quickcheck::{quickcheck, TestResult};

                use super::super::test::{check_collection_type, check_type};
                use super::*;

                quickcheck! {
                    fn check(value: $ty, tag: u32) -> TestResult {
                        check_type(value, tag, $wire_type,
                                   encode, merge, encoded_len)
                    }
                    fn check_repeated(value: Vec<$ty>, tag: u32) -> TestResult {
                        check_collection_type(value, tag, $wire_type,
                                              encode_repeated, merge_repeated,
                                              encoded_len_repeated)
                    }
                    fn check_packed(value: Vec<$ty>, tag: u32) -> TestResult {
                        check_type(value, tag, WireType::LengthDelimited,
                                   encode_packed, merge_repeated,
                                   encoded_len_packed)
                    }
                }
            }
        }
    };
}
fixed_width!(
    f32,
    4,
    WireType::ThirtyTwoBit,
    float,
    put_f32_le,
    get_f32_le
);
fixed_width!(
    f64,
    8,
    WireType::SixtyFourBit,
    double,
    put_f64_le,
    get_f64_le
);
fixed_width!(
    u32,
    4,
    WireType::ThirtyTwoBit,
    fixed32,
    put_u32_le,
    get_u32_le
);
fixed_width!(
    u64,
    8,
    WireType::SixtyFourBit,
    fixed64,
    put_u64_le,
    get_u64_le
);
fixed_width!(
    i32,
    4,
    WireType::ThirtyTwoBit,
    sfixed32,
    put_i32_le,
    get_i32_le
);
fixed_width!(
    i64,
    8,
    WireType::SixtyFourBit,
    sfixed64,
    put_i64_le,
    get_i64_le
);

/// Macro which emits encoding functions for a length-delimited type.
macro_rules! length_delimited {
    ($ty:ty) => {
        encode_repeated!($ty);

        pub fn merge_repeated<B>(
            wire_type: WireType,
            values: &mut Vec<$ty>,
            buf: &mut B,
        ) -> Result<(), DecodeError>
        where
            B: Buf,
        {
            check_wire_type(WireType::LengthDelimited, wire_type)?;
            let mut value = Default::default();
            merge(wire_type, &mut value, buf)?;
            values.push(value);
            Ok(())
        }

        #[inline]
        pub fn encoded_len(tag: u32, value: &$ty) -> usize {
            key_len(tag) + encoded_len_varint(value.len() as u64) + value.len()
        }

        #[inline]
        pub fn encoded_len_repeated(tag: u32, values: &[$ty]) -> usize {
            key_len(tag) * values.len()
                + values
                    .iter()
                    .map(|value| encoded_len_varint(value.len() as u64) + value.len())
                    .sum::<usize>()
        }

        #[cfg(test)]
        mod test {
            use quickcheck::{quickcheck, TestResult};

            use super::super::test::{check_collection_type, check_type};
            use super::*;

            quickcheck! {
                fn check(value: $ty, tag: u32) -> TestResult {
                    super::test::check_type(value, tag, WireType::LengthDelimited,
                                            encode, merge, encoded_len)
                }
                fn check_repeated(value: Vec<$ty>, tag: u32) -> TestResult {
                    super::test::check_collection_type(value, tag, WireType::LengthDelimited,
                                                       encode_repeated, merge_repeated,
                                                       encoded_len_repeated)
                }
            }
        }
    };
}

pub mod string {
    use super::*;

    pub fn encode<B>(tag: u32, value: &String, buf: &mut B)
    where
        B: BufMut,
    {
        encode_key(tag, WireType::LengthDelimited, buf);
        encode_varint(value.len() as u64, buf);
        buf.put_slice(value.as_bytes());
    }
    pub fn merge<B>(wire_type: WireType, value: &mut String, buf: &mut B) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        unsafe {
            // String::as_mut_vec is unsafe because it doesn't check that the bytes
            // inserted into it the resulting vec are valid UTF-8. We check
            // explicitly in order to ensure this is safe.
            super::bytes::merge(wire_type, value.as_mut_vec(), buf)?;
            str::from_utf8(value.as_bytes())
                .map_err(|_| DecodeError::new("invalid string value: data is not UTF-8 encoded"))?;
        }
        Ok(())
    }

    length_delimited!(String);
}

pub mod bytes {
    use super::*;

    pub fn encode<B>(tag: u32, value: &Vec<u8>, buf: &mut B)
    where
        B: BufMut,
    {
        encode_key(tag, WireType::LengthDelimited, buf);
        encode_varint(value.len() as u64, buf);
        buf.put_slice(value);
    }

    pub fn merge<B>(
        wire_type: WireType,
        value: &mut Vec<u8>,
        buf: &mut B,
    ) -> Result<(), DecodeError>
    where
        B: Buf,
    {
        check_wire_type(WireType::LengthDelimited, wire_type)?;
        let len = decode_varint(buf)?;
        if len > buf.remaining() as u64 {
            return Err(DecodeError::new("buffer underflow"));
        }

        let mut remaining = len as usize;
        while remaining > 0 {
            let len = {
                let bytes = buf.bytes();
                debug_assert!(!bytes.is_empty(), "Buf::bytes returned empty slice");
                let len = min(remaining, bytes.len());
                let bytes = &bytes[..len];
                value.extend_from_slice(bytes);
                len
            };
            remaining -= len;
            buf.advance(len);
        }
        Ok(())
    }

    length_delimited!(Vec<u8>);
}

pub mod message {
    use super::*;

    pub fn encode<M, B>(tag: u32, msg: &M, buf: &mut B)
    where
        M: Message,
        B: BufMut,
    {
        encode_key(tag, WireType::LengthDelimited, buf);
        encode_varint(msg.encoded_len() as u64, buf);
        msg.encode_raw(buf);
    }

    pub fn merge<M, B>(wire_type: WireType, msg: &mut M, buf: &mut B) -> Result<(), DecodeError>
    where
        M: Message,
        B: Buf,
    {
        check_wire_type(WireType::LengthDelimited, wire_type)?;
        merge_loop(msg, buf, M::merge_field)
    }

    pub fn encode_repeated<M, B>(tag: u32, messages: &[M], buf: &mut B)
    where
        M: Message,
        B: BufMut,
    {
        for msg in messages {
            encode(tag, msg, buf);
        }
    }

    pub fn merge_repeated<M, B>(
        wire_type: WireType,
        messages: &mut Vec<M>,
        buf: &mut B,
    ) -> Result<(), DecodeError>
    where
        M: Message + Default,
        B: Buf,
    {
        check_wire_type(WireType::LengthDelimited, wire_type)?;
        let mut msg = M::default();
        merge(WireType::LengthDelimited, &mut msg, buf)?;
        messages.push(msg);
        Ok(())
    }

    #[inline]
    pub fn encoded_len<M>(tag: u32, msg: &M) -> usize
    where
        M: Message,
    {
        let len = msg.encoded_len();
        key_len(tag) + encoded_len_varint(len as u64) + msg.encoded_len()
    }

    #[inline]
    pub fn encoded_len_repeated<M>(tag: u32, messages: &[M]) -> usize
    where
        M: Message,
    {
        key_len(tag) * messages.len()
            + messages
                .iter()
                .map(Message::encoded_len)
                .map(|len| len + encoded_len_varint(len as u64))
                .sum::<usize>()
    }
}

/// Rust doesn't have a `Map` trait, so macros are currently the best way to be
/// generic over `HashMap` and `BTreeMap`.
macro_rules! map {
    ($map_ty:ident) => {
        use std::collections::$map_ty;
        use std::hash::Hash;

        use crate::encoding::*;

        /// Generic protobuf map encode function.
        pub fn encode<K, V, B, KE, KL, VE, VL>(
            key_encode: KE,
            key_encoded_len: KL,
            val_encode: VE,
            val_encoded_len: VL,
            tag: u32,
            values: &$map_ty<K, V>,
            buf: &mut B,
        ) where
            K: Default + Eq + Hash + Ord,
            V: Default + PartialEq,
            B: BufMut,
            KE: Fn(u32, &K, &mut B),
            KL: Fn(u32, &K) -> usize,
            VE: Fn(u32, &V, &mut B),
            VL: Fn(u32, &V) -> usize,
        {
            encode_with_default(
                key_encode,
                key_encoded_len,
                val_encode,
                val_encoded_len,
                &V::default(),
                tag,
                values,
                buf,
            )
        }

        /// Generic protobuf map merge function.
        pub fn merge<K, V, B, KM, VM>(
            key_merge: KM,
            val_merge: VM,
            values: &mut $map_ty<K, V>,
            buf: &mut B,
        ) -> Result<(), DecodeError>
        where
            K: Default + Eq + Hash + Ord,
            V: Default,
            B: Buf,
            KM: Fn(WireType, &mut K, &mut B) -> Result<(), DecodeError>,
            VM: Fn(WireType, &mut V, &mut B) -> Result<(), DecodeError>,
        {
            merge_with_default(key_merge, val_merge, V::default(), values, buf)
        }

        /// Generic protobuf map encode function.
        pub fn encoded_len<K, V, KL, VL>(
            key_encoded_len: KL,
            val_encoded_len: VL,
            tag: u32,
            values: &$map_ty<K, V>,
        ) -> usize
        where
            K: Default + Eq + Hash + Ord,
            V: Default + PartialEq,
            KL: Fn(u32, &K) -> usize,
            VL: Fn(u32, &V) -> usize,
        {
            encoded_len_with_default(key_encoded_len, val_encoded_len, &V::default(), tag, values)
        }

        /// Generic protobuf map encode function with an overriden value default.
        ///
        /// This is necessary because enumeration values can have a default value other
        /// than 0 in proto2.
        pub fn encode_with_default<K, V, B, KE, KL, VE, VL>(
            key_encode: KE,
            key_encoded_len: KL,
            val_encode: VE,
            val_encoded_len: VL,
            val_default: &V,
            tag: u32,
            values: &$map_ty<K, V>,
            buf: &mut B,
        ) where
            K: Default + Eq + Hash + Ord,
            V: PartialEq,
            B: BufMut,
            KE: Fn(u32, &K, &mut B),
            KL: Fn(u32, &K) -> usize,
            VE: Fn(u32, &V, &mut B),
            VL: Fn(u32, &V) -> usize,
        {
            for (key, val) in values.iter() {
                let skip_key = key == &K::default();
                let skip_val = val == val_default;

                let len = (if skip_key { 0 } else { key_encoded_len(1, key) })
                    + (if skip_val { 0 } else { val_encoded_len(2, val) });

                encode_key(tag, WireType::LengthDelimited, buf);
                encode_varint(len as u64, buf);
                if !skip_key {
                    key_encode(1, key, buf);
                }
                if !skip_val {
                    val_encode(2, val, buf);
                }
            }
        }

        /// Generic protobuf map merge function with an overriden value default.
        ///
        /// This is necessary because enumeration values can have a default value other
        /// than 0 in proto2.
        pub fn merge_with_default<K, V, B, KM, VM>(
            key_merge: KM,
            val_merge: VM,
            val_default: V,
            values: &mut $map_ty<K, V>,
            buf: &mut B,
        ) -> Result<(), DecodeError>
        where
            K: Default + Eq + Hash + Ord,
            B: Buf,
            KM: Fn(WireType, &mut K, &mut B) -> Result<(), DecodeError>,
            VM: Fn(WireType, &mut V, &mut B) -> Result<(), DecodeError>,
        {
            let mut key = Default::default();
            let mut val = val_default;
            merge_loop(
                &mut (&mut key, &mut val),
                buf,
                |&mut (ref mut key, ref mut val), buf| {
                    let (tag, wire_type) = decode_key(buf)?;
                    match tag {
                        1 => key_merge(wire_type, key, buf),
                        2 => val_merge(wire_type, val, buf),
                        _ => skip_field(wire_type, buf),
                    }
                },
            )?;
            values.insert(key, val);

            Ok(())
        }

        /// Generic protobuf map encode function with an overriden value default.
        ///
        /// This is necessary because enumeration values can have a default value other
        /// than 0 in proto2.
        pub fn encoded_len_with_default<K, V, KL, VL>(
            key_encoded_len: KL,
            val_encoded_len: VL,
            val_default: &V,
            tag: u32,
            values: &$map_ty<K, V>,
        ) -> usize
        where
            K: Default + Eq + Hash + Ord,
            V: PartialEq,
            KL: Fn(u32, &K) -> usize,
            VL: Fn(u32, &V) -> usize,
        {
            key_len(tag) * values.len()
                + values
                    .iter()
                    .map(|(key, val)| {
                        let len = (if key == &K::default() {
                            0
                        } else {
                            key_encoded_len(1, key)
                        }) + (if val == val_default {
                            0
                        } else {
                            val_encoded_len(2, val)
                        });
                        encoded_len_varint(len as u64) + len
                    })
                    .sum::<usize>()
        }
    };
}

pub mod hash_map {
    map!(HashMap);
}

pub mod btree_map {
    map!(BTreeMap);
}

#[cfg(test)]
mod test {
    use std::borrow::Borrow;
    use std::fmt::Debug;
    use std::io::Cursor;
    use std::u64;

    use ::bytes::{Bytes, BytesMut, IntoBuf};
    use quickcheck::TestResult;

    use crate::encoding::*;

    pub fn check_type<T, B>(
        value: T,
        tag: u32,
        wire_type: WireType,
        encode: fn(u32, &B, &mut BytesMut),
        merge: fn(WireType, &mut T, &mut Cursor<Bytes>) -> Result<(), DecodeError>,
        encoded_len: fn(u32, &B) -> usize,
    ) -> TestResult
    where
        T: Debug + Default + PartialEq + Borrow<B>,
        B: ?Sized,
    {
        if tag > MAX_TAG || tag < MIN_TAG {
            return TestResult::discard();
        }

        let expected_len = encoded_len(tag, value.borrow());

        let mut buf = BytesMut::with_capacity(expected_len);
        encode(tag, value.borrow(), &mut buf);

        let mut buf = buf.freeze().into_buf();

        if buf.remaining() != expected_len {
            return TestResult::error(format!(
                "encoded_len wrong; expected: {}, actual: {}",
                expected_len,
                buf.remaining()
            ));
        }

        if !buf.has_remaining() {
            // Short circuit for empty packed values.
            return TestResult::passed();
        }

        let (decoded_tag, decoded_wire_type) = match decode_key(&mut buf) {
            Ok(key) => key,
            Err(error) => return TestResult::error(format!("{:?}", error)),
        };

        if tag != decoded_tag {
            return TestResult::error(format!(
                "decoded tag does not match; expected: {}, actual: {}",
                tag, decoded_tag
            ));
        }

        if wire_type != decoded_wire_type {
            return TestResult::error(format!(
                "decoded wire type does not match; expected: {:?}, actual: {:?}",
                wire_type, decoded_wire_type
            ));
        }

        match wire_type {
            WireType::SixtyFourBit if buf.remaining() != 8 => {
                return TestResult::error(format!(
                    "64bit wire type illegal remaining: {}, tag: {}",
                    buf.remaining(),
                    tag
                ));
            }
            WireType::ThirtyTwoBit if buf.remaining() != 4 => {
                return TestResult::error(format!(
                    "32bit wire type illegal remaining: {}, tag: {}",
                    buf.remaining(),
                    tag
                ));
            }
            _ => (),
        }

        let mut roundtrip_value = T::default();
        if let Err(error) = merge(wire_type, &mut roundtrip_value, &mut buf) {
            return TestResult::error(error.to_string());
        };

        if buf.has_remaining() {
            return TestResult::error(format!(
                "expected buffer to be empty, remaining: {}",
                buf.remaining()
            ));
        }

        if value == roundtrip_value {
            TestResult::passed()
        } else {
            TestResult::failed()
        }
    }

    pub fn check_collection_type<T, B, E, M, L>(
        value: T,
        tag: u32,
        wire_type: WireType,
        encode: E,
        mut merge: M,
        encoded_len: L,
    ) -> TestResult
    where
        T: Debug + Default + PartialEq + Borrow<B>,
        B: ?Sized,
        E: FnOnce(u32, &B, &mut BytesMut),
        M: FnMut(WireType, &mut T, &mut Cursor<Bytes>) -> Result<(), DecodeError>,
        L: FnOnce(u32, &B) -> usize,
    {
        if tag > MAX_TAG || tag < MIN_TAG {
            return TestResult::discard();
        }

        let expected_len = encoded_len(tag, value.borrow());

        let mut buf = BytesMut::with_capacity(expected_len);
        encode(tag, value.borrow(), &mut buf);

        let mut buf = buf.freeze().into_buf();

        if buf.remaining() != expected_len {
            return TestResult::error(format!(
                "encoded_len wrong; expected: {}, actual: {}",
                expected_len,
                buf.remaining()
            ));
        }

        let mut roundtrip_value = Default::default();
        while buf.has_remaining() {
            let (decoded_tag, decoded_wire_type) = match decode_key(&mut buf) {
                Ok(key) => key,
                Err(error) => return TestResult::error(format!("{:?}", error)),
            };

            if tag != decoded_tag {
                return TestResult::error(format!(
                    "decoded tag does not match; expected: {}, actual: {}",
                    tag, decoded_tag
                ));
            }

            if wire_type != decoded_wire_type {
                return TestResult::error(format!(
                    "decoded wire type does not match; expected: {:?}, actual: {:?}",
                    wire_type, decoded_wire_type
                ));
            }

            if let Err(error) = merge(wire_type, &mut roundtrip_value, &mut buf) {
                return TestResult::error(error.to_string());
            };
        }

        if value == roundtrip_value {
            TestResult::passed()
        } else {
            TestResult::failed()
        }
    }

    #[test]
    fn varint() {
        fn check(value: u64, encoded: &[u8]) {
            // Small buffer.
            let mut buf = Vec::with_capacity(1);
            encode_varint(value, &mut buf);
            assert_eq!(buf, encoded);

            // Large buffer.
            let mut buf = Vec::with_capacity(100);
            encode_varint(value, &mut buf);
            assert_eq!(buf, encoded);

            assert_eq!(encoded_len_varint(value), encoded.len());

            let roundtrip_value = decode_varint(&mut encoded.into_buf()).expect("decoding failed");
            assert_eq!(value, roundtrip_value);

            let roundtrip_value =
                decode_varint_slow(&mut encoded.into_buf()).expect("slow decoding failed");
            assert_eq!(value, roundtrip_value);
        }

        check(2u64.pow(0) - 1, &[0x00]);
        check(2u64.pow(0), &[0x01]);

        check(2u64.pow(7) - 1, &[0x7F]);
        check(2u64.pow(7), &[0x80, 0x01]);
        check(300, &[0xAC, 0x02]);

        check(2u64.pow(14) - 1, &[0xFF, 0x7F]);
        check(2u64.pow(14), &[0x80, 0x80, 0x01]);

        check(2u64.pow(21) - 1, &[0xFF, 0xFF, 0x7F]);
        check(2u64.pow(21), &[0x80, 0x80, 0x80, 0x01]);

        check(2u64.pow(28) - 1, &[0xFF, 0xFF, 0xFF, 0x7F]);
        check(2u64.pow(28), &[0x80, 0x80, 0x80, 0x80, 0x01]);

        check(2u64.pow(35) - 1, &[0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
        check(2u64.pow(35), &[0x80, 0x80, 0x80, 0x80, 0x80, 0x01]);

        check(2u64.pow(42) - 1, &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F]);
        check(2u64.pow(42), &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01]);

        check(
            2u64.pow(49) - 1,
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F],
        );
        check(
            2u64.pow(49),
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
        );

        check(
            2u64.pow(56) - 1,
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F],
        );
        check(
            2u64.pow(56),
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
        );

        check(
            2u64.pow(63) - 1,
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F],
        );
        check(
            2u64.pow(63),
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
        );

        check(
            u64::MAX,
            &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01],
        );
    }

    /// This big bowl o' macro soup generates a quickcheck encoding test for each
    /// combination of map type, scalar map key, and value type.
    /// TODO: these tests take a long time to compile, can this be improved?
    macro_rules! map_tests {
        (keys: $keys:tt,
         vals: $vals:tt) => {
            mod hash_map {
                map_tests!(@private HashMap, hash_map, $keys, $vals);
            }
            mod btree_map {
                map_tests!(@private BTreeMap, btree_map, $keys, $vals);
            }
        };

        (@private $map_type:ident,
                  $mod_name:ident,
                  [$(($key_ty:ty, $key_proto:ident)),*],
                  $vals:tt) => {
            $(
                mod $key_proto {
                    use std::collections::$map_type;
                    use quickcheck::{quickcheck, TestResult};

                    use crate::encoding::*;
                    use crate::encoding::test::check_collection_type;

                    map_tests!(@private $map_type, $mod_name, ($key_ty, $key_proto), $vals);
                }
            )*
        };

        (@private $map_type:ident,
                  $mod_name:ident,
                  ($key_ty:ty, $key_proto:ident),
                  [$(($val_ty:ty, $val_proto:ident)),*]) => {
            $(
                quickcheck! {
                    fn $val_proto(values: $map_type<$key_ty, $val_ty>, tag: u32) -> TestResult {
                        check_collection_type(values, tag, WireType::LengthDelimited,
                                              |tag, values, buf| {
                                                  $mod_name::encode($key_proto::encode,
                                                                    $key_proto::encoded_len,
                                                                    $val_proto::encode,
                                                                    $val_proto::encoded_len,
                                                                    tag,
                                                                    values,
                                                                    buf)
                                              },
                                              |wire_type, values, buf| {
                                                  check_wire_type(WireType::LengthDelimited, wire_type)?;
                                                  $mod_name::merge($key_proto::merge,
                                                                   $val_proto::merge,
                                                                   values,
                                                                   buf)
                                              },
                                              |tag, values| {
                                                  $mod_name::encoded_len($key_proto::encoded_len,
                                                                         $val_proto::encoded_len,
                                                                         tag,
                                                                         values)
                                              })
                    }
                }
             )*
        };
    }

    map_tests!(keys: [
                   (i32, int32),
                   (i64, int64),
                   (u32, uint32),
                   (u64, uint64),
                   (i32, sint32),
                   (i64, sint64),
                   (u32, fixed32),
                   (u64, fixed64),
                   (i32, sfixed32),
                   (i64, sfixed64),
                   (bool, bool),
                   (String, string)
               ],
               vals: [
                   (f32, float),
                   (f64, double),
                   (i32, int32),
                   (i64, int64),
                   (u32, uint32),
                   (u64, uint64),
                   (i32, sint32),
                   (i64, sint64),
                   (u32, fixed32),
                   (u64, fixed64),
                   (i32, sfixed32),
                   (i64, sfixed64),
                   (bool, bool),
                   (String, string),
                   (Vec<u8>, bytes)
               ]);
}
