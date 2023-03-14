//! This module contains functionality for decompression.

use crate::alloc::boxed::Box;
use crate::alloc::vec;
use crate::alloc::vec::Vec;
use ::core::cmp::min;
use ::core::usize;

pub mod core;
mod output_buffer;
pub mod stream;
use self::core::*;

const TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS: i32 = -4;
const TINFL_STATUS_BAD_PARAM: i32 = -3;
const TINFL_STATUS_ADLER32_MISMATCH: i32 = -2;
const TINFL_STATUS_FAILED: i32 = -1;
const TINFL_STATUS_DONE: i32 = 0;
const TINFL_STATUS_NEEDS_MORE_INPUT: i32 = 1;
const TINFL_STATUS_HAS_MORE_OUTPUT: i32 = 2;

/// Return status codes.
#[repr(i8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum TINFLStatus {
    /// More input data was expected, but the caller indicated that there was no more data, so the
    /// input stream is likely truncated.
    ///
    /// This can't happen if you have provided the
    /// [`TINFL_FLAG_HAS_MORE_INPUT`][core::inflate_flags::TINFL_FLAG_HAS_MORE_INPUT] flag to the
    /// decompression.  By setting that flag, you indicate more input exists but is not provided,
    /// and so reaching the end of the input data without finding the end of the compressed stream
    /// would instead return a [`NeedsMoreInput`][Self::NeedsMoreInput] status.
    FailedCannotMakeProgress = TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS as i8,

    /// The output buffer is an invalid size; consider the `flags` parameter.
    BadParam = TINFL_STATUS_BAD_PARAM as i8,

    /// The decompression went fine, but the adler32 checksum did not match the one
    /// provided in the header.
    Adler32Mismatch = TINFL_STATUS_ADLER32_MISMATCH as i8,

    /// Failed to decompress due to invalid data.
    Failed = TINFL_STATUS_FAILED as i8,

    /// Finished decompression without issues.
    ///
    /// This indicates the end of the compressed stream has been reached.
    Done = TINFL_STATUS_DONE as i8,

    /// The decompressor needs more input data to continue decompressing.
    ///
    /// This occurs when there's no more consumable input, but the end of the stream hasn't been
    /// reached, and you have supplied the
    /// [`TINFL_FLAG_HAS_MORE_INPUT`][core::inflate_flags::TINFL_FLAG_HAS_MORE_INPUT] flag to the
    /// decompressor.  Had you not supplied that flag (which would mean you were asserting that you
    /// believed all the data was available) you would have gotten a
    /// [`FailedCannotMakeProcess`][Self::FailedCannotMakeProgress] instead.
    NeedsMoreInput = TINFL_STATUS_NEEDS_MORE_INPUT as i8,

    /// There is still pending data that didn't fit in the output buffer.
    HasMoreOutput = TINFL_STATUS_HAS_MORE_OUTPUT as i8,
}

impl TINFLStatus {
    pub fn from_i32(value: i32) -> Option<TINFLStatus> {
        use self::TINFLStatus::*;
        match value {
            TINFL_STATUS_FAILED_CANNOT_MAKE_PROGRESS => Some(FailedCannotMakeProgress),
            TINFL_STATUS_BAD_PARAM => Some(BadParam),
            TINFL_STATUS_ADLER32_MISMATCH => Some(Adler32Mismatch),
            TINFL_STATUS_FAILED => Some(Failed),
            TINFL_STATUS_DONE => Some(Done),
            TINFL_STATUS_NEEDS_MORE_INPUT => Some(NeedsMoreInput),
            TINFL_STATUS_HAS_MORE_OUTPUT => Some(HasMoreOutput),
            _ => None,
        }
    }
}

/// Decompress the deflate-encoded data in `input` to a vector.
///
/// Returns a tuple of the [`Vec`] of decompressed data and the [status result][TINFLStatus].
#[inline]
pub fn decompress_to_vec(input: &[u8]) -> Result<Vec<u8>, TINFLStatus> {
    decompress_to_vec_inner(input, 0, usize::max_value())
}

/// Decompress the deflate-encoded data (with a zlib wrapper) in `input` to a vector.
///
/// Returns a tuple of the [`Vec`] of decompressed data and the [status result][TINFLStatus].
#[inline]
pub fn decompress_to_vec_zlib(input: &[u8]) -> Result<Vec<u8>, TINFLStatus> {
    decompress_to_vec_inner(
        input,
        inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER,
        usize::max_value(),
    )
}

/// Decompress the deflate-encoded data in `input` to a vector.
/// The vector is grown to at most `max_size` bytes; if the data does not fit in that size,
/// [`TINFLStatus::HasMoreOutput`] error is returned.
///
/// Returns a tuple of the [`Vec`] of decompressed data and the [status result][TINFLStatus].
#[inline]
pub fn decompress_to_vec_with_limit(input: &[u8], max_size: usize) -> Result<Vec<u8>, TINFLStatus> {
    decompress_to_vec_inner(input, 0, max_size)
}

/// Decompress the deflate-encoded data (with a zlib wrapper) in `input` to a vector.
/// The vector is grown to at most `max_size` bytes; if the data does not fit in that size,
/// [`TINFLStatus::HasMoreOutput`] error is returned.
///
/// Returns a tuple of the [`Vec`] of decompressed data and the [status result][TINFLStatus].
#[inline]
pub fn decompress_to_vec_zlib_with_limit(
    input: &[u8],
    max_size: usize,
) -> Result<Vec<u8>, TINFLStatus> {
    decompress_to_vec_inner(input, inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER, max_size)
}

/// Backend of various to-[`Vec`] decompressions.
///
/// Returns a tuple of the [`Vec`] of decompressed data and the [status result][TINFLStatus].
fn decompress_to_vec_inner(
    input: &[u8],
    flags: u32,
    max_output_size: usize,
) -> Result<Vec<u8>, TINFLStatus> {
    let flags = flags | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
    let mut ret: Vec<u8> = vec![0; min(input.len().saturating_mul(2), max_output_size)];

    let mut decomp = Box::<DecompressorOxide>::default();

    let mut in_pos = 0;
    let mut out_pos = 0;
    loop {
        // Wrap the whole output slice so we know we have enough of the
        // decompressed data for matches.
        let (status, in_consumed, out_consumed) =
            decompress(&mut decomp, &input[in_pos..], &mut ret, out_pos, flags);
        in_pos += in_consumed;
        out_pos += out_consumed;

        match status {
            TINFLStatus::Done => {
                ret.truncate(out_pos);
                return Ok(ret);
            }

            TINFLStatus::HasMoreOutput => {
                // We need more space, so check if we can resize the buffer and do it.
                let new_len = ret
                    .len()
                    .checked_add(out_pos)
                    .ok_or(TINFLStatus::HasMoreOutput)?;
                if new_len > max_output_size {
                    return Err(TINFLStatus::HasMoreOutput);
                };
                ret.resize(new_len, 0);
            }

            _ => return Err(status),
        }
    }
}

/// Decompress one or more source slices from an iterator into the output slice.
///
/// * On success, returns the number of bytes that were written.
/// * On failure, returns the failure status code.
///
/// This will fail if the output buffer is not large enough, but in that case
/// the output buffer will still contain the partial decompression.
///
/// * `out` the output buffer.
/// * `it` the iterator of input slices.
/// * `zlib_header` if the first slice out of the iterator is expected to have a
///   Zlib header. Otherwise the slices are assumed to be the deflate data only.
/// * `ignore_adler32` if the adler32 checksum should be calculated or not.
pub fn decompress_slice_iter_to_slice<'out, 'inp>(
    out: &'out mut [u8],
    it: impl Iterator<Item = &'inp [u8]>,
    zlib_header: bool,
    ignore_adler32: bool,
) -> Result<usize, TINFLStatus> {
    use self::core::inflate_flags::*;

    let mut it = it.peekable();
    let r = &mut DecompressorOxide::new();
    let mut out_pos = 0;
    while let Some(in_buf) = it.next() {
        let has_more = it.peek().is_some();
        let flags = {
            let mut f = TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
            if zlib_header {
                f |= TINFL_FLAG_PARSE_ZLIB_HEADER;
            }
            if ignore_adler32 {
                f |= TINFL_FLAG_IGNORE_ADLER32;
            }
            if has_more {
                f |= TINFL_FLAG_HAS_MORE_INPUT;
            }
            f
        };
        let (status, _input_read, bytes_written) = decompress(r, in_buf, out, out_pos, flags);
        out_pos += bytes_written;
        match status {
            TINFLStatus::NeedsMoreInput => continue,
            TINFLStatus::Done => return Ok(out_pos),
            e => return Err(e),
        }
    }
    // If we ran out of source slices without getting a `Done` from the
    // decompression we can call it a failure.
    Err(TINFLStatus::FailedCannotMakeProgress)
}

#[cfg(test)]
mod test {
    use super::{
        decompress_slice_iter_to_slice, decompress_to_vec_zlib, decompress_to_vec_zlib_with_limit,
        TINFLStatus,
    };
    const ENCODED: [u8; 20] = [
        120, 156, 243, 72, 205, 201, 201, 215, 81, 168, 202, 201, 76, 82, 4, 0, 27, 101, 4, 19,
    ];

    #[test]
    fn decompress_vec() {
        let res = decompress_to_vec_zlib(&ENCODED[..]).unwrap();
        assert_eq!(res.as_slice(), &b"Hello, zlib!"[..]);
    }

    #[test]
    fn decompress_vec_with_high_limit() {
        let res = decompress_to_vec_zlib_with_limit(&ENCODED[..], 100_000).unwrap();
        assert_eq!(res.as_slice(), &b"Hello, zlib!"[..]);
    }

    #[test]
    fn fail_to_decompress_with_limit() {
        let res = decompress_to_vec_zlib_with_limit(&ENCODED[..], 8);
        match res {
            Err(TINFLStatus::HasMoreOutput) => (), // expected result
            _ => panic!("Decompression output size limit was not enforced"),
        }
    }

    #[test]
    fn test_decompress_slice_iter_to_slice() {
        // one slice
        let mut out = [0_u8; 12_usize];
        let r =
            decompress_slice_iter_to_slice(&mut out, Some(&ENCODED[..]).into_iter(), true, false);
        assert_eq!(r, Ok(12));
        assert_eq!(&out[..12], &b"Hello, zlib!"[..]);

        // some chunks at a time
        for chunk_size in 1..13 {
            // Note: because of https://github.com/Frommi/miniz_oxide/issues/110 our
            // out buffer needs to have +1 byte available when the chunk size cuts
            // the adler32 data off from the last actual data.
            let mut out = [0_u8; 12_usize + 1];
            let r =
                decompress_slice_iter_to_slice(&mut out, ENCODED.chunks(chunk_size), true, false);
            assert_eq!(r, Ok(12));
            assert_eq!(&out[..12], &b"Hello, zlib!"[..]);
        }

        // output buffer too small
        let mut out = [0_u8; 3_usize];
        let r = decompress_slice_iter_to_slice(&mut out, ENCODED.chunks(7), true, false);
        assert!(r.is_err());
    }
}
