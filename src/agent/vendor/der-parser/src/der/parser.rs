use crate::ber::*;
use crate::der::*;
use crate::der_constraint_fail_if;
use crate::error::*;
use alloc::borrow::ToOwned;
use asn1_rs::{Any, FromDer, Tag};
use nom::bytes::streaming::take;
use nom::number::streaming::be_u8;
use nom::{Err, Needed};
use rusticata_macros::custom_check;

/// Parse DER object recursively
///
/// Return a tuple containing the remaining (unparsed) bytes and the DER Object, or an error.
///
/// *Note: this is the same as calling `parse_der_recursive` with `MAX_RECURSION`.
///
/// ### Example
///
/// ```
/// use der_parser::der::{parse_der, Tag};
///
/// let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (_, obj) = parse_der(bytes).expect("parsing failed");
///
/// assert_eq!(obj.header.tag(), Tag::Integer);
/// ```
#[inline]
pub fn parse_der(i: &[u8]) -> DerResult {
    parse_der_recursive(i, MAX_RECURSION)
}

/// Parse DER object recursively, specifying the maximum recursion depth
///
/// Return a tuple containing the remaining (unparsed) bytes and the DER Object, or an error.
///
/// ### Example
///
/// ```
/// use der_parser::der::{parse_der_recursive, Tag};
///
/// let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (_, obj) = parse_der_recursive(bytes, 1).expect("parsing failed");
///
/// assert_eq!(obj.header.tag(), Tag::Integer);
/// ```
pub fn parse_der_recursive(i: &[u8], max_depth: usize) -> DerResult {
    let (i, hdr) = der_read_element_header(i)?;
    // safety check: length cannot be more than 2^32 bytes
    if let Length::Definite(l) = hdr.length() {
        custom_check!(i, l > MAX_OBJECT_SIZE, BerError::InvalidLength)?;
    }
    der_read_element_content_recursive(i, hdr, max_depth)
}

/// Parse a DER object, expecting a value with specified tag
///
/// The object is parsed recursively, with a maximum depth of `MAX_RECURSION`.
///
/// ### Example
///
/// ```
/// use der_parser::der::{parse_der_with_tag, Tag};
///
/// let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (_, obj) = parse_der_with_tag(bytes, Tag::Integer).expect("parsing failed");
///
/// assert_eq!(obj.header.tag(), Tag::Integer);
/// ```
pub fn parse_der_with_tag<T: Into<Tag>>(i: &[u8], tag: T) -> DerResult {
    let tag = tag.into();
    let (i, hdr) = der_read_element_header(i)?;
    hdr.assert_tag(tag)?;
    let (i, content) = der_read_element_content_as(
        i,
        hdr.tag(),
        hdr.length(),
        hdr.is_constructed(),
        MAX_RECURSION,
    )?;
    Ok((i, DerObject::from_header_and_content(hdr, content)))
}

/// Read end of content marker
#[inline]
pub fn parse_der_endofcontent(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::EndOfContent)
}

/// Read a boolean value
///
/// The encoding of a boolean value shall be primitive. The contents octets shall consist of a
/// single octet.
///
/// If the boolean value is FALSE, the octet shall be zero.
/// If the boolean value is TRUE, the octet shall be one byte, and have all bits set to one (0xff).
#[inline]
pub fn parse_der_bool(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::Boolean)
}

/// Read an integer value
///
/// The encoding of a boolean value shall be primitive. The contents octets shall consist of one or
/// more octets.
///
/// To access the content, use the [`as_u64`](struct.BerObject.html#method.as_u64),
/// [`as_u32`](struct.BerObject.html#method.as_u32),
/// [`as_biguint`](struct.BerObject.html#method.as_biguint) or
/// [`as_bigint`](struct.BerObject.html#method.as_bigint) methods.
/// Remember that a BER integer has unlimited size, so these methods return `Result` or `Option`
/// objects.
///
/// # Examples
///
/// ```rust
/// # use der_parser::der::{parse_der_integer, DerObject, DerObjectContent};
/// let empty = &b""[..];
/// let bytes = [0x02, 0x03, 0x01, 0x00, 0x01];
/// let expected  = DerObject::from_obj(DerObjectContent::Integer(b"\x01\x00\x01"));
/// assert_eq!(
///     parse_der_integer(&bytes),
///     Ok((empty, expected))
/// );
/// ```
#[inline]
pub fn parse_der_integer(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::Integer)
}

/// Read an bitstring value
///
/// To access the content as plain bytes, you will have to
/// interprete the resulting tuple which will contain in
/// its first item the number of padding bits left at
/// the end of the bit string, and in its second item
/// a `BitStringObject` structure which will, in its sole
/// structure field called `data`, contain a byte slice
/// representing the value of the bit string which can
/// be interpreted as a big-endian value with the padding
/// bits on the right (as in ASN.1 raw BER or DER encoding).
///
/// To access the content as an integer, use the [`as_u64`](struct.BerObject.html#method.as_u64)
/// or [`as_u32`](struct.BerObject.html#method.as_u32) methods.
/// Remember that a BER bit string has unlimited size, so these methods return `Result` or `Option`
/// objects.
#[inline]
pub fn parse_der_bitstring(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::BitString)
}

/// Read an octetstring value
#[inline]
pub fn parse_der_octetstring(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::OctetString)
}

/// Read a null value
#[inline]
pub fn parse_der_null(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::Null)
}

/// Read an object identifier value
#[inline]
pub fn parse_der_oid(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::Oid)
}

/// Read an enumerated value
#[inline]
pub fn parse_der_enum(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::Enumerated)
}

/// Read a UTF-8 string value. The encoding is checked.
#[inline]
pub fn parse_der_utf8string(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::Utf8String)
}

/// Read a relative object identifier value
#[inline]
pub fn parse_der_relative_oid(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::RelativeOid)
}

/// Parse a sequence of DER elements
///
/// Read a sequence of DER objects, without any constraint on the types.
/// Sequence is parsed recursively, so if constructed elements are found, they are parsed using the
/// same function.
///
/// To read a specific sequence of objects (giving the expected types), use the
/// [`parse_der_sequence_defined`](macro.parse_der_sequence_defined.html) macro.
#[inline]
pub fn parse_der_sequence(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::Sequence)
}

/// Parse a set of DER elements
///
/// Read a set of DER objects, without any constraint on the types.
/// Set is parsed recursively, so if constructed elements are found, they are parsed using the
/// same function.
///
/// To read a specific set of objects (giving the expected types), use the
/// [`parse_der_set_defined`](macro.parse_der_set_defined.html) macro.
#[inline]
pub fn parse_der_set(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::Set)
}

/// Read a numeric string value. The content is verified to
/// contain only digits and spaces.
#[inline]
pub fn parse_der_numericstring(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::NumericString)
}

/// Read a printable string value. The content is verified to
/// contain only the allowed characters.
#[inline]
pub fn visiblestring(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::VisibleString)
}

/// Read a printable string value. The content is verified to
/// contain only the allowed characters.
#[inline]
pub fn parse_der_printablestring(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::PrintableString)
}

/// Read a T61 string value
#[inline]
pub fn parse_der_t61string(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::T61String)
}

/// Read a Videotex string value
#[inline]
pub fn parse_der_videotexstring(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::VideotexString)
}

/// Read an IA5 string value. The content is verified to be ASCII.
#[inline]
pub fn parse_der_ia5string(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::Ia5String)
}

/// Read an UTC time value
#[inline]
pub fn parse_der_utctime(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::UtcTime)
}

/// Read a Generalized time value
#[inline]
pub fn parse_der_generalizedtime(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::GeneralizedTime)
}

/// Read a ObjectDescriptor value
#[inline]
pub fn parse_der_objectdescriptor(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::ObjectDescriptor)
}

/// Read a GraphicString value
#[inline]
pub fn parse_der_graphicstring(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::GraphicString)
}

/// Read a GeneralString value
#[inline]
pub fn parse_der_generalstring(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::GeneralString)
}

/// Read a BmpString value
#[inline]
pub fn parse_der_bmpstring(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::BmpString)
}

/// Read a UniversalString value
#[inline]
pub fn parse_der_universalstring(i: &[u8]) -> DerResult {
    parse_der_with_tag(i, Tag::UniversalString)
}

/// Parse an optional tagged object, applying function to get content
///
/// This function returns a `DerObject`, trying to read content as generic DER objects.
/// If parsing failed, return an optional object containing `None`.
///
/// To support other return or error types, use
/// [parse_der_tagged_explicit_g](fn.parse_der_tagged_explicit_g.html)
///
/// This function will never fail: if parsing content failed, the BER value `Optional(None)` is
/// returned.
#[inline]
pub fn parse_der_explicit_optional<F>(i: &[u8], tag: Tag, f: F) -> DerResult
where
    F: Fn(&[u8]) -> DerResult,
{
    parse_ber_explicit_optional(i, tag, f)
}

/// Parse an implicit tagged object, applying function to read content
///
/// Note: unlike explicit tagged functions, the callback must be a *content* parsing function,
/// often based on the [`parse_der_content`](fn.parse_der_content.html) combinator.
///
/// The built object will use the original header (and tag), so the content may not match the tag
/// value.
///
/// For a combinator version, see [parse_der_tagged_implicit](../ber/fn.parse_der_tagged_implicit.html).
///
/// For a generic version (different output and error types), see
/// [parse_der_tagged_implicit_g](../ber/fn.parse_der_tagged_implicit_g.html).
///
/// # Examples
///
/// The following parses `[3] IMPLICIT INTEGER` into a `DerObject`:
///
/// ```rust
/// # use der_parser::der::*;
/// # use der_parser::error::DerResult;
/// #
/// fn parse_int_implicit(i:&[u8]) -> DerResult {
///     parse_der_implicit(
///         i,
///         3,
///         parse_der_content(Tag::Integer),
///     )
/// }
///
/// # let bytes = &[0x83, 0x03, 0x01, 0x00, 0x01];
/// let res = parse_int_implicit(bytes);
/// # match res {
/// #     Ok((rem, content)) => {
/// #         assert!(rem.is_empty());
/// #         assert_eq!(content.as_u32(), Ok(0x10001));
/// #     },
/// #     _ => assert!(false)
/// # }
/// ```
#[inline]
pub fn parse_der_implicit<'a, T, F>(i: &'a [u8], tag: T, f: F) -> DerResult<'a>
where
    F: Fn(&'a [u8], &'_ Header, usize) -> BerResult<'a, DerObjectContent<'a>>,
    T: Into<Tag>,
{
    parse_ber_implicit(i, tag, f)
}

/// Parse DER object and try to decode it as a 32-bits signed integer
///
/// Return `IntegerTooLarge` if object is an integer, but can not be represented in the target
/// integer type.
#[inline]
pub fn parse_der_i32(i: &[u8]) -> BerResult<i32> {
    <i32>::from_der(i)
}

/// Parse DER object and try to decode it as a 64-bits signed integer
///
/// Return `IntegerTooLarge` if object is an integer, but can not be represented in the target
/// integer type.
#[inline]
pub fn parse_der_i64(i: &[u8]) -> BerResult<i64> {
    <i64>::from_der(i)
}

/// Parse DER object and try to decode it as a 32-bits unsigned integer
///
/// Return `IntegerTooLarge` if object is an integer, but can not be represented in the target
/// integer type.
pub fn parse_der_u32(i: &[u8]) -> BerResult<u32> {
    <u32>::from_der(i)
}

/// Parse DER object and try to decode it as a 64-bits unsigned integer
///
/// Return `IntegerTooLarge` if object is an integer, but can not be represented in the target
/// integer type.
pub fn parse_der_u64(i: &[u8]) -> BerResult<u64> {
    <u64>::from_der(i)
}

/// Parse DER object and get content as slice
#[inline]
pub fn parse_der_slice<T: Into<Tag>>(i: &[u8], tag: T) -> BerResult<&[u8]> {
    let tag = tag.into();
    parse_der_container(move |content, hdr| {
        hdr.assert_tag(tag)?;
        Ok((&b""[..], content))
    })(i)
}

/// Parse the next bytes as the content of a DER object (combinator, header reference)
///
/// Content type is *not* checked to match tag, caller is responsible of providing the correct tag
///
/// Caller is also responsible to check if parsing function consumed the expected number of
/// bytes (`header.len`).
///
/// This function differs from [`parse_der_content2`](fn.parse_der_content2.html) because it passes
/// the BER object header by reference (required for ex. by `parse_der_implicit`).
///
/// The arguments of the parse function are: `(input, ber_object_header, max_recursion)`.
///
/// Example: manually parsing header and content
///
/// ```
/// # use der_parser::ber::MAX_RECURSION;
/// # use der_parser::der::*;
/// #
/// # let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (i, header) = der_read_element_header(bytes).expect("parsing failed");
/// let (rem, content) = parse_der_content(header.tag())(i, &header, MAX_RECURSION)
///     .expect("parsing failed");
/// #
/// # assert_eq!(header.tag(), Tag::Integer);
/// ```
pub fn parse_der_content<'a>(
    tag: Tag,
) -> impl Fn(&'a [u8], &'_ Header, usize) -> BerResult<'a, DerObjectContent<'a>> {
    move |i: &[u8], hdr: &Header, max_recursion: usize| {
        der_read_element_content_as(i, tag, hdr.length(), hdr.is_constructed(), max_recursion)
    }
}

/// Parse the next bytes as the content of a DER object (combinator, owned header)
///
/// Content type is *not* checked to match tag, caller is responsible of providing the correct tag
///
/// Caller is also responsible to check if parsing function consumed the expected number of
/// bytes (`header.len`).
///
/// The arguments of the parse function are: `(input, ber_object_header, max_recursion)`.
///
/// This function differs from [`parse_der_content`](fn.parse_der_content.html) because it passes
/// an owned BER object header (required for ex. by `parse_der_tagged_implicit_g`).
///
/// Example: manually parsing header and content
///
/// ```
/// # use der_parser::ber::MAX_RECURSION;
/// # use der_parser::der::*;
/// #
/// # let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (i, header) = der_read_element_header(bytes).expect("parsing failed");
/// # assert_eq!(header.tag(), Tag::Integer);
/// let (rem, content) = parse_der_content2(header.tag())(i, header, MAX_RECURSION)
///     .expect("parsing failed");
/// ```
pub fn parse_der_content2<'a>(
    tag: Tag,
) -> impl Fn(&'a [u8], Header<'a>, usize) -> BerResult<'a, DerObjectContent<'a>> {
    move |i: &[u8], hdr: Header, max_recursion: usize| {
        der_read_element_content_as(i, tag, hdr.length(), hdr.is_constructed(), max_recursion)
    }
}

// --------- end of parse_der_xxx functions ----------

/// Parse the next bytes as the content of a DER object.
///
/// Content type is *not* checked, caller is responsible of providing the correct tag
pub fn der_read_element_content_as(
    i: &[u8],
    tag: Tag,
    length: Length,
    constructed: bool,
    max_depth: usize,
) -> BerResult<DerObjectContent> {
    // Indefinite lengths are not allowed in DER (X.690 section 10.1)
    let l = length.definite().map_err(BerError::from)?;
    if i.len() < l {
        return Err(Err::Incomplete(Needed::new(l)));
    }
    match tag {
        Tag::Boolean => {
            custom_check!(i, l != 1, BerError::InvalidLength)?;
            der_constraint_fail_if!(i, i[0] != 0 && i[0] != 0xff, DerConstraint::InvalidBoolean);
        }
        Tag::BitString => {
            der_constraint_fail_if!(i, constructed, DerConstraint::Constructed);
            // exception: read and verify padding bits
            return der_read_content_bitstring(i, l);
        }
        Tag::Integer => {
            // verify leading zeros
            match i[..l] {
                [] => {
                    return Err(nom::Err::Error(BerError::DerConstraintFailed(
                        DerConstraint::IntegerEmpty,
                    )))
                }
                [0, 0, ..] => {
                    return Err(nom::Err::Error(BerError::DerConstraintFailed(
                        DerConstraint::IntegerLeadingZeroes,
                    )))
                }
                [0, byte, ..] if byte < 0x80 => {
                    return Err(nom::Err::Error(BerError::DerConstraintFailed(
                        DerConstraint::IntegerLeadingZeroes,
                    )));
                }
                _ => (),
            }
        }
        Tag::NumericString
        | Tag::VisibleString
        | Tag::PrintableString
        | Tag::Ia5String
        | Tag::Utf8String
        | Tag::T61String
        | Tag::VideotexString
        | Tag::BmpString
        | Tag::UniversalString
        | Tag::ObjectDescriptor
        | Tag::GraphicString
        | Tag::GeneralString => {
            der_constraint_fail_if!(i, constructed, DerConstraint::Constructed);
        }
        Tag::UtcTime | Tag::GeneralizedTime => {
            if l == 0 || i.get(l - 1).cloned() != Some(b'Z') {
                return Err(Err::Error(BerError::DerConstraintFailed(
                    DerConstraint::MissingTimeZone,
                )));
            }
        }
        _ => (),
    }
    ber_read_element_content_as(i, tag, length, constructed, max_depth)
}

/// Parse DER object content recursively
///
/// *Note: an error is raised if recursion depth exceeds `MAX_RECURSION`.
pub fn der_read_element_content<'a>(i: &'a [u8], hdr: Header<'a>) -> DerResult<'a> {
    der_read_element_content_recursive(i, hdr, MAX_RECURSION)
}

fn der_read_element_content_recursive<'a>(
    i: &'a [u8],
    hdr: Header<'a>,
    max_depth: usize,
) -> DerResult<'a> {
    match hdr.class() {
        Class::Universal => (),
        _ => {
            let (i, data) = ber_get_object_content(i, &hdr, max_depth)?;
            let any = Any::new(hdr.clone(), data);
            let content = DerObjectContent::Unknown(any);
            let obj = DerObject::from_header_and_content(hdr, content);
            return Ok((i, obj));
        }
    }
    match der_read_element_content_as(i, hdr.tag(), hdr.length(), hdr.is_constructed(), max_depth) {
        Ok((rem, content)) => Ok((rem, DerObject::from_header_and_content(hdr, content))),
        Err(Err::Error(BerError::UnknownTag(_))) => {
            let (rem, data) = ber_get_object_content(i, &hdr, max_depth)?;
            let any = Any::new(hdr.clone(), data);
            let content = DerObjectContent::Unknown(any);
            let obj = DerObject::from_header_and_content(hdr, content);
            Ok((rem, obj))
        }
        Err(e) => Err(e),
    }
}

fn der_read_content_bitstring(i: &[u8], len: usize) -> BerResult<DerObjectContent> {
    let (i, ignored_bits) = be_u8(i)?;
    if ignored_bits > 7 {
        return Err(Err::Error(BerError::invalid_value(
            Tag::BitString,
            "More than 7 unused bits".to_owned(),
        )));
    }
    if len == 0 {
        return Err(Err::Error(BerError::InvalidLength));
    }
    let (i, data) = take(len - 1)(i)?;
    if len > 1 {
        let mut last_byte = data[len - 2];
        for _ in 0..ignored_bits as usize {
            der_constraint_fail_if!(i, last_byte & 1 != 0, DerConstraint::UnusedBitsNotZero);
            last_byte >>= 1;
        }
    }
    Ok((
        i,
        DerObjectContent::BitString(ignored_bits, BitStringObject { data }),
    ))
    // do_parse! {
    //     i,
    //     ignored_bits: be_u8 >>
    //                   custom_check!(ignored_bits > 7, BerError::DerConstraintFailed) >>
    //                   custom_check!(len == 0, BerError::InvalidLength) >>
    //     s:            take!(len - 1) >>
    //                   call!(|input| {
    //                       if len > 1 {
    //                           let mut last_byte = s[len-2];
    //                           for _ in 0..ignored_bits as usize {
    //                               der_constraint_fail_if!(i, last_byte & 1 != 0);
    //                               last_byte >>= 1;
    //                           }
    //                       }
    //                       Ok((input,()))
    //                   }) >>
    //     ( DerObjectContent::BitString(ignored_bits,BitStringObject{ data:s }) )
    // }
}

/// Read an object header (DER)
#[inline]
pub fn der_read_element_header(i: &[u8]) -> BerResult<Header> {
    Header::from_der(i)
}
