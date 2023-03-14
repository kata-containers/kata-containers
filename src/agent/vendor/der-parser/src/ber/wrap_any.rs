use super::{BerObject, BerObjectContent, BitStringObject};
use crate::ber::{ber_get_object_content, MAX_OBJECT_SIZE};
use crate::error::{BerError, BerResult};
use alloc::vec::Vec;
use asn1_rs::*;
use nom::Err;
use rusticata_macros::custom_check;

/// Parse any BER object recursively, specifying the maximum recursion depth and expected tag
///
/// Raise an error if the maximum recursion depth was reached.
pub fn parse_ber_any_with_tag_r(i: &[u8], tag: Tag, max_depth: usize) -> BerResult {
    custom_check!(i, max_depth == 0, BerError::BerMaxDepth)?;
    let (rem, any) = Any::from_ber(i)?;
    any.header.assert_tag(tag)?;
    let obj = try_berobject_from_any(any, max_depth)?;
    Ok((rem, obj))
}

/// Parse any BER object recursively, specifying the maximum recursion depth
///
/// Raise an error if the maximum recursion depth was reached.
pub fn parse_ber_any_r(i: &[u8], max_depth: usize) -> BerResult {
    custom_check!(i, max_depth == 0, BerError::BerMaxDepth)?;
    let (rem, any) = Any::from_ber(i)?;
    let obj = try_berobject_from_any(any, max_depth)?;
    Ok((rem, obj))
}

/// Parse any BER object (not recursive)
pub fn parse_ber_any(i: &[u8]) -> BerResult<Any> {
    Any::from_ber(i)
}

macro_rules! from_obj {
    ($header:ident, $content:expr) => {
        BerObject::from_header_and_content($header, $content)
    };
    (STRING $ty:ident, $any:ident, $header:ident) => {
        from_obj!(STRING $ty, $ty, $any, $header)
    };
    // macro variant when enum variant is not the same as char type
    (STRING $ty:ident, $variant:ident, $any:ident, $header:ident) => {{
        custom_check!($any.data, $header.constructed(), BerError::Unsupported)?; // XXX valid in BER (8.21)
        <$ty>::test_valid_charset($any.data)?;
        let s = core::str::from_utf8($any.data)?;
        Ok(BerObject::from_header_and_content(
            $header,
            BerObjectContent::$variant(s),
        ))
    }};
}

/// Read element content as Universal object, or Unknown
// TODO implement the function for BerObjectContent (to replace ber_read_element_content_as)
// note: we cannot implement TryFrom because of the `max_depth` argument
pub(crate) fn try_read_berobjectcontent_as(
    i: &[u8],
    tag: Tag,
    length: Length,
    constructed: bool,
    max_depth: usize,
) -> BerResult<BerObjectContent> {
    if let Length::Definite(l) = length {
        custom_check!(i, l > MAX_OBJECT_SIZE, BerError::InvalidLength)?;
        if i.len() < l {
            return Err(Err::Incomplete(Needed::new(l)));
        }
    }
    let header = Header::new(Class::Universal, constructed, tag, length);
    let (rem, i) = ber_get_object_content(i, &header, max_depth)?;
    let any = Any::new(header, i);
    let object = try_berobject_from_any(any, max_depth)?;
    Ok((rem, object.content))
}

// note: we cannot implement TryFrom because of the `max_depth` argument
fn try_berobject_from_any(any: Any, max_depth: usize) -> Result<BerObject> {
    custom_check!(any.data, max_depth == 0, BerError::BerMaxDepth)?;
    let obj_from = BerObject::from_header_and_content;
    let header = any.header.clone();
    if any.class() != Class::Universal {
        return Ok(obj_from(header, BerObjectContent::Unknown(any)));
    }
    match any.tag() {
        Tag::BitString => {
            if any.data.is_empty() {
                return Err(BerError::BerValueError);
            }
            custom_check!(any.data, header.constructed(), BerError::Unsupported)?; // XXX valid in BER (8.6.3)
            let ignored_bits = any.data[0];
            let data = &any.data[1..];
            Ok(obj_from(
                header,
                BerObjectContent::BitString(ignored_bits, BitStringObject { data }),
            ))
        }
        Tag::BmpString => from_obj!(STRING BmpString, any, header),
        Tag::Boolean => {
            let b = any.bool()?;
            Ok(obj_from(header, BerObjectContent::Boolean(b)))
        }
        Tag::EndOfContent => Ok(obj_from(header, BerObjectContent::EndOfContent)),
        Tag::Enumerated => {
            let obj = any.enumerated()?;
            Ok(obj_from(header, BerObjectContent::Enum(obj.0 as u64)))
        }
        Tag::GeneralizedTime => {
            let time = any.generalizedtime()?;
            Ok(obj_from(header, BerObjectContent::GeneralizedTime(time.0)))
        }
        Tag::GeneralString => from_obj!(STRING GeneralString, any, header),
        Tag::GraphicString => from_obj!(STRING GraphicString, any, header),
        Tag::Ia5String => from_obj!(STRING Ia5String, IA5String, any, header),
        Tag::Integer => {
            let obj = obj_from(header, BerObjectContent::Integer(any.data));
            Ok(obj)
        }
        Tag::Null => Ok(obj_from(header, BerObjectContent::Null)),
        Tag::NumericString => from_obj!(STRING NumericString, any, header),
        Tag::ObjectDescriptor => from_obj!(STRING ObjectDescriptor, any, header),
        Tag::OctetString => Ok(obj_from(header, BerObjectContent::OctetString(any.data))),
        Tag::Oid => {
            let oid = any.oid()?;
            Ok(obj_from(header, BerObjectContent::OID(oid)))
        }
        Tag::PrintableString => from_obj!(STRING PrintableString, any, header),
        Tag::RelativeOid => {
            let oid = any.relative_oid()?;
            Ok(obj_from(header, BerObjectContent::RelativeOID(oid)))
        }
        Tag::Sequence => {
            header.assert_constructed()?;
            let objects: Result<Vec<_>> = SequenceIterator::<Any, BerParser>::new(any.data)
                .map(|item| {
                    let item = item?;
                    try_berobject_from_any(item, max_depth - 1)
                })
                .collect();
            let objects = objects?;
            Ok(obj_from(header, BerObjectContent::Sequence(objects)))
        }
        Tag::Set => {
            header.assert_constructed()?;
            let objects: Result<Vec<_>> = SetIterator::<Any, BerParser>::new(any.data)
                .map(|item| {
                    let item = item?;
                    try_berobject_from_any(item, max_depth - 1)
                })
                .collect();
            let objects = objects?;
            Ok(obj_from(header, BerObjectContent::Set(objects)))
        }
        Tag::TeletexString => from_obj!(STRING TeletexString, T61String, any, header),
        Tag::UtcTime => {
            let time = any.utctime()?;
            Ok(obj_from(header, BerObjectContent::UTCTime(time.0)))
        }
        Tag::UniversalString => {
            custom_check!(any.data, header.constructed(), BerError::Unsupported)?; // XXX valid in BER (8.21)

            // as detailed in asn1-rs, UniversalString allocates memory since the UCS-4 to UTF-8 conversion requires a memory allocation.
            // so, the charset is not checked here
            Ok(obj_from(
                header,
                BerObjectContent::UniversalString(any.data),
            ))
        }
        Tag::Utf8String => from_obj!(STRING Utf8String, UTF8String, any, header),
        Tag::VideotexString => from_obj!(STRING VideotexString, any, header),
        Tag::VisibleString => from_obj!(STRING VisibleString, any, header),
        _ => {
            // Note for: Tag::EmbeddedPdv | Tag::External | Tag::RealType
            // these types have no mapping in the BerObjectContent enum,
            // so we use the Unknown type
            Ok(obj_from(header, BerObjectContent::Unknown(any)))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ber::{BerObject, BerObjectContent, MAX_RECURSION};
    use crate::error::BerError;
    use hex_literal::hex;
    use test_case::test_case;

    use super::parse_ber_any_r;

    #[test_case(&hex!("01 01 00") => matches Ok(BerObject{header:_, content:BerObjectContent::Boolean(false)}) ; "val false")]
    #[test_case(&hex!("01 01 ff") => matches Ok(BerObject{header:_, content:BerObjectContent::Boolean(true)}) ; "val true")]
    #[test_case(&hex!("01 01 7f") => matches Ok(BerObject{header:_, content:BerObjectContent::Boolean(true)}) ; "true not ff")]
    #[test_case(&hex!("02 02 00 ff") => matches Ok(BerObject{header:_, content:BerObjectContent::Integer(_)}) ; "u32-255")]
    #[test_case(&hex!("02 02 01 23") => matches Ok(BerObject{header:_, content:BerObjectContent::Integer(_)}) ; "u32-0x123")]
    #[test_case(&hex!("02 04 ff ff ff ff") => matches Ok(BerObject{header:_, content:BerObjectContent::Integer(_)}) ; "u32-long-neg")]
    #[test_case(&hex!("0c 04 31 32 33 34") => matches Ok(BerObject{header:_, content:BerObjectContent::UTF8String("1234")}) ; "utf8: numeric")]
    #[test_case(&hex!("0d 04 c2 7b 03 02") => matches Ok(BerObject{header:_, content:BerObjectContent::RelativeOID(_)}) ; "relative OID")]
    #[test_case(&hex!("12 04 31 32 33 34") => matches Ok(BerObject{header:_, content:BerObjectContent::NumericString("1234")}) ; "numeric string")]
    #[test_case(&hex!("12 04 01 02 03 04") => matches Err(BerError::StringInvalidCharset) ; "numeric string err")]
    #[test_case(&hex!("13 04 31 32 33 34") => matches Ok(BerObject{header:_, content:BerObjectContent::PrintableString("1234")}) ; "printable string")]
    #[test_case(&hex!("13 04 01 02 03 04") => matches Err(BerError::StringInvalidCharset) ; "printable string err")]
    #[test_case(&hex!("16 04 31 32 33 34") => matches Ok(BerObject{header:_, content:BerObjectContent::IA5String("1234")}) ; "ia5: numeric")]
    #[test_case(&hex!("1a 04 31 32 33 34") => matches Ok(BerObject{header:_, content:BerObjectContent::VisibleString("1234")}) ; "visible: numeric")]
    #[test_case(&hex!("1e 08 00 55 00 73 00 65 00 72") => matches Ok(BerObject{header:_, content:BerObjectContent::BmpString("\x00U\x00s\x00e\x00r")}) ; "bmp")]
    #[test_case(&hex!("30 80 04 03 56 78 90 00 00") => matches Ok(BerObject{header:_, content:BerObjectContent::Sequence(_)}) ; "indefinite length")]
    #[test_case(&hex!("c0 03 01 00 01") => matches Ok(BerObject{header:_, content:BerObjectContent::Unknown(_)}) ; "private")]
    fn ber_from_any(i: &[u8]) -> Result<BerObject, BerError> {
        let (rem, res) = parse_ber_any_r(i, MAX_RECURSION)?;
        assert!(rem.is_empty());
        // dbg!(&res);
        Ok(res)
    }
}
