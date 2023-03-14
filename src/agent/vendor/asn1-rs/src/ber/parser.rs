use crate::error::*;
use crate::header::*;
use crate::{FromDer, Length, Tag};
use nom::bytes::streaming::take;
use nom::{Err, Needed, Offset};
use rusticata_macros::custom_check;

/// Default maximum recursion limit
pub const MAX_RECURSION: usize = 50;

/// Default maximum object size (2^32)
// pub const MAX_OBJECT_SIZE: usize = 4_294_967_295;

/// Skip object content, and return true if object was End-Of-Content
pub(crate) fn ber_skip_object_content<'a>(
    i: &'a [u8],
    hdr: &Header,
    max_depth: usize,
) -> ParseResult<'a, bool> {
    if max_depth == 0 {
        return Err(Err::Error(Error::BerMaxDepth));
    }
    match hdr.length {
        Length::Definite(l) => {
            if l == 0 && hdr.tag == Tag::EndOfContent {
                return Ok((i, true));
            }
            let (i, _) = take(l)(i)?;
            Ok((i, false))
        }
        Length::Indefinite => {
            hdr.assert_constructed()?;
            // read objects until EndOfContent (00 00)
            // this is recursive
            let mut i = i;
            loop {
                let (i2, header2) = Header::from_der(i)?;
                let (i3, eoc) = ber_skip_object_content(i2, &header2, max_depth - 1)?;
                if eoc {
                    // return false, since top object was not EndOfContent
                    return Ok((i3, false));
                }
                i = i3;
            }
        }
    }
}

/// Read object raw content (bytes)
pub(crate) fn ber_get_object_content<'a>(
    i: &'a [u8],
    hdr: &Header,
    max_depth: usize,
) -> ParseResult<'a, &'a [u8]> {
    let start_i = i;
    let (i, _) = ber_skip_object_content(i, hdr, max_depth)?;
    let len = start_i.offset(i);
    let (content, i) = start_i.split_at(len);
    // if len is indefinite, there are 2 extra bytes for EOC
    if hdr.length == Length::Indefinite {
        let len = content.len();
        assert!(len >= 2);
        Ok((i, &content[..len - 2]))
    } else {
        Ok((i, content))
    }
}

/// Try to parse input bytes as u64
#[inline]
pub(crate) fn bytes_to_u64(s: &[u8]) -> core::result::Result<u64, Error> {
    let mut u: u64 = 0;
    for &c in s {
        if u & 0xff00_0000_0000_0000 != 0 {
            return Err(Error::IntegerTooLarge);
        }
        u <<= 8;
        u |= u64::from(c);
    }
    Ok(u)
}

pub(crate) fn parse_identifier(i: &[u8]) -> ParseResult<(u8, u8, u32, &[u8])> {
    if i.is_empty() {
        Err(Err::Incomplete(Needed::new(1)))
    } else {
        let a = i[0] >> 6;
        let b = if i[0] & 0b0010_0000 != 0 { 1 } else { 0 };
        let mut c = u32::from(i[0] & 0b0001_1111);

        let mut tag_byte_count = 1;

        if c == 0x1f {
            c = 0;
            loop {
                // Make sure we don't read past the end of our data.
                custom_check!(i, tag_byte_count >= i.len(), Error::InvalidTag)?;

                // With tag defined as u32 the most we can fit in is four tag bytes.
                // (X.690 doesn't actually specify maximum tag width.)
                custom_check!(i, tag_byte_count > 5, Error::InvalidTag)?;

                c = (c << 7) | (u32::from(i[tag_byte_count]) & 0x7f);
                let done = i[tag_byte_count] & 0x80 == 0;
                tag_byte_count += 1;
                if done {
                    break;
                }
            }
        }

        let (raw_tag, rem) = i.split_at(tag_byte_count);

        Ok((rem, (a, b, c, raw_tag)))
    }
}

/// Return the MSB and the rest of the first byte, or an error
pub(crate) fn parse_ber_length_byte(i: &[u8]) -> ParseResult<(u8, u8)> {
    if i.is_empty() {
        Err(Err::Incomplete(Needed::new(1)))
    } else {
        let a = i[0] >> 7;
        let b = i[0] & 0b0111_1111;
        Ok((&i[1..], (a, b)))
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! der_constraint_fail_if(
    ($slice:expr, $cond:expr, $constraint:expr) => (
        {
            if $cond {
                return Err(::nom::Err::Error(Error::DerConstraintFailed($constraint)));
            }
        }
    );
);
