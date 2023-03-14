use crate::error::*;

/// Decode an unsigned integer into a big endian byte slice with all leading
/// zeroes removed.
///
/// Returns a byte array of the requested size containing a big endian integer.
fn remove_zeroes(bytes: &[u8]) -> Result<&[u8], BerError> {
    // skip leading 0s
    match bytes {
        // [] => Err(BerError::DerConstraintFailed),
        [0] => Ok(bytes),
        // [0, byte, ..] if *byte < 0x80 => Err(BerError::DerConstraintFailed),
        // [0, rest @ ..] => Ok(&rest),
        [0, rest @ ..] => remove_zeroes(rest),
        // [byte, ..] if *byte >= 0x80 => Err(BerError::IntegerTooLarge),
        _ => Ok(bytes),
    }
}

// XXX const generics require rustc >= 1.51
// /// Decode an unsigned integer into a byte array of the requested size
// /// containing a big endian integer.
// pub(crate) fn decode_array_uint<const N: usize>(bytes: &[u8]) -> Result<[u8; N], BerError> {
//     // Check if MSB is set *before* leading zeroes
//     if is_highest_bit_set(bytes) {
//         return Err(BerError::IntegerNegative);
//     }
//     let input = remove_zeroes(bytes)?;

//     if input.len() > N {
//         return Err(BerError::IntegerTooLarge);
//     }

//     // Input has leading zeroes removed, so we need to add them back
//     let mut output = [0u8; N];
//     assert!(input.len() <= N);
//     output[N.saturating_sub(input.len())..].copy_from_slice(input);
//     Ok(output)
// }

pub(crate) fn decode_array_uint8(bytes: &[u8]) -> Result<[u8; 8], BerError> {
    // Check if MSB is set *before* leading zeroes
    if is_highest_bit_set(bytes) {
        return Err(BerError::IntegerNegative);
    }
    let input = remove_zeroes(bytes)?;

    if input.len() > 8 {
        return Err(BerError::IntegerTooLarge);
    }

    // Input has leading zeroes removed, so we need to add them back
    let mut output = [0u8; 8];
    assert!(input.len() <= 8);
    output[8_usize.saturating_sub(input.len())..].copy_from_slice(input);
    Ok(output)
}

pub(crate) fn decode_array_uint4(bytes: &[u8]) -> Result<[u8; 4], BerError> {
    // Check if MSB is set *before* leading zeroes
    if is_highest_bit_set(bytes) {
        return Err(BerError::IntegerNegative);
    }
    let input = remove_zeroes(bytes)?;

    if input.len() > 4 {
        return Err(BerError::IntegerTooLarge);
    }

    // Input has leading zeroes removed, so we need to add them back
    let mut output = [0u8; 4];
    assert!(input.len() <= 4);
    output[4_usize.saturating_sub(input.len())..].copy_from_slice(input);
    Ok(output)
}

// XXX const generics require rustc >= 1.51
// /// Decode an unsigned integer of the specified size.
// ///
// /// Returns a byte array of the requested size containing a big endian integer.
// pub(crate) fn decode_array_int<const N: usize>(input: &[u8]) -> Result<[u8; N], BerError> {
//     let input = remove_zeroes(input)?;

//     if input.len() > N {
//         return Err(BerError::IntegerTooLarge);
//     }

//     // any.tag().assert_eq(Tag::Integer)?;
//     let mut output = [0xFFu8; N];
//     let offset = N.saturating_sub(input.len());
//     output[offset..].copy_from_slice(input);
//     Ok(output)
// }

pub(crate) fn decode_array_int8(input: &[u8]) -> Result<[u8; 8], BerError> {
    let input = remove_zeroes(input)?;

    if input.len() > 8 {
        return Err(BerError::IntegerTooLarge);
    }

    // any.tag().assert_eq(Tag::Integer)?;
    let mut output = [0xFFu8; 8];
    let offset = 8_usize.saturating_sub(input.len());
    output[offset..].copy_from_slice(input);
    Ok(output)
}

pub(crate) fn decode_array_int4(input: &[u8]) -> Result<[u8; 4], BerError> {
    let input = remove_zeroes(input)?;

    if input.len() > 4 {
        return Err(BerError::IntegerTooLarge);
    }

    // any.tag().assert_eq(Tag::Integer)?;
    let mut output = [0xFFu8; 4];
    let offset = 4_usize.saturating_sub(input.len());
    output[offset..].copy_from_slice(input);
    Ok(output)
}

/// Is the highest bit of the first byte in the slice 1? (if present)
#[inline]
pub(crate) fn is_highest_bit_set(bytes: &[u8]) -> bool {
    bytes
        .get(0)
        .map(|byte| byte & 0b10000000 != 0)
        .unwrap_or(false)
}
