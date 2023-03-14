use std::{
    borrow::Cow,
};

use crate::{
    packet::Header,
};

/// Remove whitespace, etc. from the base64 data.
///
/// This function returns the filtered base64 data (i.e., stripped of
/// all skipable data like whitespace), and the amount of unfiltered
/// data that corresponds to.  Thus, if we have the following 7 bytes:
///
/// ```text
///     ab  cde
///     0123456
/// ```
///
/// This function returns ("abcd", 6), because the 'd' is the last
/// character in the last complete base64 chunk, and it is at offset 5.
///
/// If 'd' is followed by whitespace, it is undefined whether that
/// whitespace is included in the count.
///
/// This function only returns full chunks of base64 data.  As a
/// consequence, if base64_data_max is less than 4, then this will not
/// return any data.
///
/// This function will stop after it sees base64 padding, and if it
/// sees invalid base64 data.
#[allow(clippy::single_match)]
pub fn base64_filter(mut bytes: Cow<[u8]>, base64_data_max: usize,
                     mut prefix_remaining: usize, prefix_len: usize)
    -> (Cow<[u8]>, usize, usize)
{
    let mut leading_whitespace = 0;

    // Round down to the nearest chunk size.
    let base64_data_max = base64_data_max / 4 * 4;

    // Number of bytes of base64 data.  Since we update `bytes` in
    // place, the base64 data is `&bytes[..base64_len]`.
    let mut base64_len = 0;

    // Offset of the next byte of unfiltered data to process.
    let mut unfiltered_offset = 0;

    // Offset of the last byte of the last ***complete*** base64 chunk
    // in the unfiltered data.
    let mut unfiltered_complete_len = 0;

    // Number of bytes of padding that we've seen so far.
    let mut padding = 0;

    while unfiltered_offset < bytes.len()
        && base64_len < base64_data_max
        // A valid base64 chunk never starts with padding.
        && ! (padding > 0 && base64_len % 4 == 0)
    {
        // If we have some prefix to skip, skip it.
        if prefix_remaining > 0 {
            prefix_remaining -= 1;
            if unfiltered_offset == 0 {
                match bytes {
                    Cow::Borrowed(s) => {
                        // We're at the beginning.  Avoid moving
                        // data by cutting off the start of the
                        // slice.
                        bytes = Cow::Borrowed(&s[1..]);
                        leading_whitespace += 1;
                        continue;
                    }
                    Cow::Owned(_) => (),
                }
            }
            unfiltered_offset += 1;
            continue;
        }
        match bytes[unfiltered_offset] {
            // White space.
            c if c.is_ascii_whitespace() => {
                if c == b'\n' {
                    prefix_remaining = prefix_len;
                }
                if unfiltered_offset == 0 {
                    match bytes {
                        Cow::Borrowed(s) => {
                            // We're at the beginning.  Avoid moving
                            // data by cutting off the start of the
                            // slice.
                            bytes = Cow::Borrowed(&s[1..]);
                            leading_whitespace += 1;
                            continue;
                        }
                        Cow::Owned(_) => (),
                    }
                }
            }

            // Padding.
            b'=' => {
                if padding == 2 {
                    // There can never be more than two bytes of
                    // padding.
                    break;
                }
                if base64_len % 4 == 0 {
                    // Padding can never occur at the start of a
                    // base64 chunk.
                    break;
                }

                if unfiltered_offset != base64_len {
                    bytes.to_mut()[base64_len] = b'=';
                }
                base64_len += 1;
                if base64_len % 4 == 0 {
                    unfiltered_complete_len = unfiltered_offset + 1;
                }
                padding += 1;
            }

            // The only thing that can occur after padding is
            // whitespace or padding.  Those cases were covered above.
            _ if padding > 0 => break,

            // Base64 data!
            b if is_base64_char(&b) => {
                if unfiltered_offset != base64_len {
                    bytes.to_mut()[base64_len] = b;
                }
                base64_len += 1;
                if base64_len % 4 == 0 {
                    unfiltered_complete_len = unfiltered_offset + 1;
                }
            }

            // Not base64 data.
            _ => break,
        }

        unfiltered_offset += 1;
    }

    let base64_len = base64_len - (base64_len % 4);
    unfiltered_complete_len += leading_whitespace;
    match bytes {
        Cow::Borrowed(s) =>
            (Cow::Borrowed(&s[..base64_len]), unfiltered_complete_len,
             prefix_remaining),
        Cow::Owned(mut v) => {
            crate::vec_truncate(&mut v, base64_len);
            (Cow::Owned(v), unfiltered_complete_len, prefix_remaining)
        }
    }
}

/// Checks whether the given bytes contain armored OpenPGP data.
pub fn is_armored_pgp_blob(bytes: &[u8]) -> bool {
    // Get up to 32 bytes of base64 data.  That's 24 bytes of data
    // (ignoring padding), which is more than enough to get the first
    // packet's header.
    let (bytes, _, _) = base64_filter(Cow::Borrowed(bytes), 32, 0, 0);

    match base64::decode_config(&bytes, base64::STANDARD) {
        Ok(d) => {
            // Don't consider an empty message to be valid.
            if d.is_empty() {
                false
            } else {
                let mut br = buffered_reader::Memory::new(&d);
                if let Ok(header) = Header::parse(&mut br) {
                    header.ctb().tag().valid_start_of_message()
                        && header.valid(false).is_ok()
                } else {
                    false
                }
            }
        },
        Err(_err) => false,
    }
}

/// Checks whether the given byte is in the base64 character set.
pub fn is_base64_char(b: &u8) -> bool {
    b.is_ascii_alphanumeric() || *b == b'+' || *b == b'/'
}

/// Returns the number of bytes of base64 data are needed to encode
/// `s` bytes of raw data.
pub fn base64_size(s: usize) -> usize {
    (s + 3 - 1) / 3 * 4
}

#[test]
fn base64_size_test() {
    assert_eq!(base64_size(0), 0);
    assert_eq!(base64_size(1), 4);
    assert_eq!(base64_size(2), 4);
    assert_eq!(base64_size(3), 4);
    assert_eq!(base64_size(4), 8);
    assert_eq!(base64_size(5), 8);
    assert_eq!(base64_size(6), 8);
    assert_eq!(base64_size(7), 12);
}
