use crate::ber::wrap_any::parse_ber_any_r;
use crate::ber::*;
use crate::error::*;
use asn1_rs::{FromBer, Tag};
use nom::bytes::streaming::take;
use nom::{Err, Offset};

/// Default maximum recursion limit
pub const MAX_RECURSION: usize = 50;

/// Default maximum object size (2^32)
pub const MAX_OBJECT_SIZE: usize = 4_294_967_295;

/// Skip object content, and return true if object was End-Of-Content
pub(crate) fn ber_skip_object_content<'a>(
    i: &'a [u8],
    hdr: &Header,
    max_depth: usize,
) -> BerResult<'a, bool> {
    if max_depth == 0 {
        return Err(Err::Error(BerError::BerMaxDepth));
    }
    match hdr.length() {
        Length::Definite(l) => {
            if l == 0 && hdr.tag() == Tag::EndOfContent {
                return Ok((i, true));
            }
            let (i, _) = take(l)(i)?;
            Ok((i, false))
        }
        Length::Indefinite => {
            if hdr.is_primitive() {
                return Err(Err::Error(BerError::ConstructExpected));
            }
            // read objects until EndOfContent (00 00)
            // this is recursive
            let mut i = i;
            loop {
                let (i2, header2) = ber_read_element_header(i)?;
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
) -> BerResult<'a, &'a [u8]> {
    let start_i = i;
    let (i, _) = ber_skip_object_content(i, hdr, max_depth)?;
    let len = start_i.offset(i);
    let (content, i) = start_i.split_at(len);
    // if len is indefinite, there are 2 extra bytes for EOC
    if hdr.length() == Length::Indefinite {
        let len = content.len();
        assert!(len >= 2);
        Ok((i, &content[..len - 2]))
    } else {
        Ok((i, content))
    }
}

/// Try to parse an input bit string as u64.
///
/// Note: this is for the primitive BER/DER encoding only, the
/// constructed BER encoding for BIT STRING does not seem to be
/// supported at all by the library currently.
#[inline]
pub(crate) fn bitstring_to_u64(
    padding_bits: usize,
    data: &BitStringObject,
) -> Result<u64, BerError> {
    let raw_bytes = data.data;
    let bit_size = (raw_bytes.len() * 8)
        .checked_sub(padding_bits)
        .ok_or(BerError::InvalidLength)?;
    if bit_size > 64 {
        return Err(BerError::IntegerTooLarge);
    }
    let padding_bits = padding_bits % 8;
    let num_bytes = if bit_size % 8 > 0 {
        (bit_size / 8) + 1
    } else {
        bit_size / 8
    };
    let mut resulting_integer: u64 = 0;
    for &c in &raw_bytes[..num_bytes] {
        resulting_integer <<= 8;
        resulting_integer |= c as u64;
    }
    Ok(resulting_integer >> padding_bits)
}

/// Read an object header
///
/// ### Example
///
/// ```
/// # use der_parser::ber::{ber_read_element_header, Class, Length, Tag};
/// #
/// let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (i, hdr) = ber_read_element_header(bytes).expect("could not read header");
///
/// assert_eq!(hdr.class(), Class::Universal);
/// assert_eq!(hdr.tag(), Tag::Integer);
/// assert_eq!(hdr.length(), Length::Definite(3));
/// ```
#[inline]
pub fn ber_read_element_header(i: &[u8]) -> BerResult<Header> {
    Header::from_ber(i)
}

/// Parse the next bytes as the *content* of a BER object.
///
/// Content type is *not* checked to match tag, caller is responsible of providing the correct tag
///
/// This function is mostly used when parsing implicit tagged objects, when reading primitive
/// types.
///
/// `max_depth` is the maximum allowed recursion for objects.
///
/// ### Example
///
/// ```
/// # use der_parser::ber::{ber_read_element_content_as, ber_read_element_header, Tag};
/// #
/// # let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (i, hdr) = ber_read_element_header(bytes).expect("could not read header");
/// let (_, content) = ber_read_element_content_as(
///     i, hdr.tag(), hdr.length(), hdr.is_constructed(), 5
/// ).expect("parsing failed");
/// #
/// # assert_eq!(hdr.tag(), Tag::Integer);
/// # assert_eq!(content.as_u32(), Ok(0x10001));
/// ```
#[inline]
pub fn ber_read_element_content_as(
    i: &[u8],
    tag: Tag,
    length: Length,
    constructed: bool,
    max_depth: usize,
) -> BerResult<BerObjectContent> {
    try_read_berobjectcontent_as(i, tag, length, constructed, max_depth)
}

/// Parse the next bytes as the content of a BER object (combinator, header reference)
///
/// Content type is *not* checked to match tag, caller is responsible of providing the correct tag
///
/// Caller is also responsible to check if parsing function consumed the expected number of
/// bytes (`header.len`).
///
/// The arguments of the parse function are: `(input, ber_object_header, max_recursion)`.
///
/// This function differs from [`parse_ber_content2`](fn.parse_ber_content2.html) because it passes
/// the BER object header by reference (required for ex. by `parse_ber_implicit`).
///
/// Example: manually parsing header and content
///
/// ```
/// # use der_parser::ber::*;
/// #
/// # let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (i, header) = ber_read_element_header(bytes).expect("parsing failed");
/// let (rem, content) = parse_ber_content(header.tag())(i, &header, MAX_RECURSION)
///     .expect("parsing failed");
/// #
/// # assert_eq!(header.tag(), Tag::Integer);
/// ```
pub fn parse_ber_content<'a>(
    tag: Tag,
) -> impl Fn(&'a [u8], &'_ Header, usize) -> BerResult<'a, BerObjectContent<'a>> {
    move |i: &[u8], hdr: &Header, max_recursion: usize| {
        ber_read_element_content_as(i, tag, hdr.length(), hdr.is_constructed(), max_recursion)
    }
}

/// Parse the next bytes as the content of a BER object (combinator, owned header)
///
/// Content type is *not* checked to match tag, caller is responsible of providing the correct tag
///
/// Caller is also responsible to check if parsing function consumed the expected number of
/// bytes (`header.len`).
///
/// The arguments of the parse function are: `(input, ber_object_header, max_recursion)`.
///
/// This function differs from [`parse_ber_content`](fn.parse_ber_content.html) because it passes
/// an owned BER object header (required for ex. by `parse_ber_tagged_implicit_g`).
///
/// Example: manually parsing header and content
///
/// ```
/// # use der_parser::ber::*;
/// #
/// # let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (i, header) = ber_read_element_header(bytes).expect("parsing failed");
/// let (rem, content) = parse_ber_content(header.tag())(i, &header, MAX_RECURSION)
///     .expect("parsing failed");
/// #
/// # assert_eq!(header.tag(), Tag::Integer);
/// ```
pub fn parse_ber_content2<'a>(
    tag: Tag,
) -> impl Fn(&'a [u8], Header<'a>, usize) -> BerResult<'a, BerObjectContent<'a>> {
    move |i: &[u8], hdr: Header, max_recursion: usize| {
        ber_read_element_content_as(i, tag, hdr.length(), hdr.is_constructed(), max_recursion)
    }
}

/// Parse a BER object, expecting a value with specified tag
///
/// The object is parsed recursively, with a maximum depth of `MAX_RECURSION`.
///
/// ### Example
///
/// ```
/// use der_parser::ber::Tag;
/// use der_parser::ber::parse_ber_with_tag;
///
/// let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (_, obj) = parse_ber_with_tag(bytes, Tag::Integer).expect("parsing failed");
///
/// assert_eq!(obj.header.tag(), Tag::Integer);
/// ```
pub fn parse_ber_with_tag<T: Into<Tag>>(i: &[u8], tag: T) -> BerResult {
    let tag = tag.into();
    let (i, hdr) = ber_read_element_header(i)?;
    hdr.assert_tag(tag)?;
    let (i, content) = ber_read_element_content_as(
        i,
        hdr.tag(),
        hdr.length(),
        hdr.is_constructed(),
        MAX_RECURSION,
    )?;
    Ok((i, BerObject::from_header_and_content(hdr, content)))
}

/// Read end of content marker
#[inline]
pub fn parse_ber_endofcontent(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::EndOfContent)
}

/// Read a boolean value
///
/// The encoding of a boolean value shall be primitive. The contents octets shall consist of a
/// single octet.
///
/// If the boolean value is FALSE, the octet shall be zero.
/// If the boolean value is TRUE, the octet shall be one byte, and have all bits set to one (0xff).
#[inline]
pub fn parse_ber_bool(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::Boolean)
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
/// # extern crate nom;
/// # use der_parser::ber::parse_ber_integer;
/// # use der_parser::ber::{BerObject,BerObjectContent};
/// let empty = &b""[..];
/// let bytes = [0x02, 0x03, 0x01, 0x00, 0x01];
/// let expected  = BerObject::from_obj(BerObjectContent::Integer(b"\x01\x00\x01"));
/// assert_eq!(
///     parse_ber_integer(&bytes),
///     Ok((empty, expected))
/// );
/// ```
#[inline]
pub fn parse_ber_integer(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::Integer)
}

/// Read an bitstring value
#[inline]
pub fn parse_ber_bitstring(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::BitString)
}

/// Read an octetstring value
#[inline]
pub fn parse_ber_octetstring(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::OctetString)
}

/// Read a null value
#[inline]
pub fn parse_ber_null(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::Null)
}

/// Read an object identifier value
#[inline]
pub fn parse_ber_oid(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::Oid)
}

/// Read an enumerated value
#[inline]
pub fn parse_ber_enum(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::Enumerated)
}

/// Read a UTF-8 string value. The encoding is checked.
#[inline]
pub fn parse_ber_utf8string(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::Utf8String)
}

/// Read a relative object identifier value
#[inline]
pub fn parse_ber_relative_oid(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::RelativeOid)
}

/// Parse a sequence of BER elements
///
/// Read a sequence of BER objects, without any constraint on the types.
/// Sequence is parsed recursively, so if constructed elements are found, they are parsed using the
/// same function.
///
/// To read a specific sequence of objects (giving the expected types), use the
/// [`parse_ber_sequence_defined`](macro.parse_ber_sequence_defined.html) macro.
#[inline]
pub fn parse_ber_sequence(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::Sequence)
}

/// Parse a set of BER elements
///
/// Read a set of BER objects, without any constraint on the types.
/// Set is parsed recursively, so if constructed elements are found, they are parsed using the
/// same function.
///
/// To read a specific set of objects (giving the expected types), use the
/// [`parse_ber_set_defined`](macro.parse_ber_set_defined.html) macro.
#[inline]
pub fn parse_ber_set(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::Set)
}

/// Read a numeric string value. The content is verified to
/// contain only digits and spaces.
#[inline]
pub fn parse_ber_numericstring(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::NumericString)
}

/// Read a visible string value. The content is verified to
/// contain only the allowed characters.
#[inline]
pub fn parse_ber_visiblestring(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::VisibleString)
}

/// Read a printable string value. The content is verified to
/// contain only the allowed characters.
#[inline]
pub fn parse_ber_printablestring(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::PrintableString)
}

/// Read a T61 string value
#[inline]
pub fn parse_ber_t61string(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::T61String)
}

/// Read a Videotex string value
#[inline]
pub fn parse_ber_videotexstring(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::VideotexString)
}

/// Read an IA5 string value. The content is verified to be ASCII.
#[inline]
pub fn parse_ber_ia5string(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::Ia5String)
}

/// Read an UTC time value
#[inline]
pub fn parse_ber_utctime(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::UtcTime)
}

/// Read a Generalized time value
#[inline]
pub fn parse_ber_generalizedtime(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::GeneralizedTime)
}

/// Read an ObjectDescriptor value
#[inline]
pub fn parse_ber_objectdescriptor(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::ObjectDescriptor)
}

/// Read a GraphicString value
#[inline]
pub fn parse_ber_graphicstring(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::GraphicString)
}

/// Read a GeneralString value
#[inline]
pub fn parse_ber_generalstring(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::GeneralString)
}

/// Read a BmpString value
#[inline]
pub fn parse_ber_bmpstring(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::BmpString)
}

/// Read a UniversalString value
#[inline]
pub fn parse_ber_universalstring(i: &[u8]) -> BerResult {
    parse_ber_with_tag(i, Tag::UniversalString)
}

/// Parse an optional tagged object, applying function to get content
///
/// This function returns a `BerObject`, trying to read content as generic BER objects.
/// If parsing failed, return an optional object containing `None`.
///
/// To support other return or error types, use
/// [parse_ber_tagged_explicit_g](fn.parse_ber_tagged_explicit_g.html)
///
/// This function will never fail: if parsing content failed, the BER value `Optional(None)` is
/// returned.
pub fn parse_ber_explicit_optional<F>(i: &[u8], tag: Tag, f: F) -> BerResult
where
    F: Fn(&[u8]) -> BerResult,
{
    parse_ber_optional(parse_ber_tagged_explicit_g(tag, |content, hdr| {
        let (rem, obj) = f(content)?;
        let content = BerObjectContent::Tagged(hdr.class(), hdr.tag(), Box::new(obj));
        let tagged = BerObject::from_header_and_content(hdr, content);
        Ok((rem, tagged))
    }))(i)
}

/// Parse an implicit tagged object, applying function to read content
///
/// Note: unlike explicit tagged functions, the callback must be a *content* parsing function,
/// often based on the [`parse_ber_content`](fn.parse_ber_content.html) combinator.
///
/// The built object will use the original header (and tag), so the content may not match the tag
/// value.
///
/// For a combinator version, see [parse_ber_tagged_implicit](fn.parse_ber_tagged_implicit.html).
///
/// For a generic version (different output and error types), see
/// [parse_ber_tagged_implicit_g](fn.parse_ber_tagged_implicit_g.html).
///
/// # Examples
///
/// The following parses `[3] IMPLICIT INTEGER` into a `BerObject`:
///
/// ```rust
/// # use der_parser::ber::*;
/// # use der_parser::error::BerResult;
/// #
/// fn parse_int_implicit(i:&[u8]) -> BerResult<BerObject> {
///     parse_ber_implicit(
///         i,
///         3,
///         parse_ber_content(Tag::Integer),
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
pub fn parse_ber_implicit<'a, T, F>(i: &'a [u8], tag: T, f: F) -> BerResult<'a>
where
    F: Fn(&'a [u8], &'_ Header, usize) -> BerResult<'a, BerObjectContent<'a>>,
    T: Into<Tag>,
{
    parse_ber_tagged_implicit(tag, f)(i)
}

/// Combinator for building optional BER values
///
/// To read optional BER values, it is to use the nom `opt()` combinator. However, this results in
/// a `Option<BerObject>` and prevents using some functions from this crate (the generic functions
/// can still be used).
///
/// This combinator is used when parsing BER values, while keeping `BerObject` output only.
///
/// This function will never fail: if parsing content failed, the BER value `Optional(None)` is
/// returned.
///
/// ### Example
///
/// ```
/// # use der_parser::ber::*;
/// #
/// let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let mut parser = parse_ber_optional(parse_ber_integer);
/// let (_, obj) = parser(bytes).expect("parsing failed");
///
/// assert_eq!(obj.header.tag(), Tag::Integer);
/// assert!(obj.as_optional().is_ok());
/// ```
pub fn parse_ber_optional<'a, F>(mut f: F) -> impl FnMut(&'a [u8]) -> BerResult<'a>
where
    F: FnMut(&'a [u8]) -> BerResult<'a>,
{
    move |i: &[u8]| {
        let res = f(i);
        match res {
            Ok((rem, inner)) => {
                let opt = BerObject::from_header_and_content(
                    inner.header.clone(),
                    BerObjectContent::Optional(Some(Box::new(inner))),
                );
                Ok((rem, opt))
            }
            Err(_) => Ok((i, BerObject::from_obj(BerObjectContent::Optional(None)))),
        }
    }
}

/// Parse BER object and try to decode it as a 32-bits signed integer
///
/// Return `IntegerTooLarge` if object is an integer, but can not be represented in the target
/// integer type.
#[inline]
pub fn parse_ber_i32(i: &[u8]) -> BerResult<i32> {
    <i32>::from_ber(i)
}

/// Parse BER object and try to decode it as a 64-bits signed integer
///
/// Return `IntegerTooLarge` if object is an integer, but can not be represented in the target
/// integer type.
#[inline]
pub fn parse_ber_i64(i: &[u8]) -> BerResult<i64> {
    <i64>::from_ber(i)
}

/// Parse BER object and try to decode it as a 32-bits unsigned integer
///
/// Return `IntegerTooLarge` if object is an integer, but can not be represented in the target
/// integer type.
#[inline]
pub fn parse_ber_u32(i: &[u8]) -> BerResult<u32> {
    <u32>::from_ber(i)
}

/// Parse BER object and try to decode it as a 64-bits unsigned integer
///
/// Return `IntegerTooLarge` if object is an integer, but can not be represented in the target
/// integer type.
#[inline]
pub fn parse_ber_u64(i: &[u8]) -> BerResult<u64> {
    <u64>::from_ber(i)
}

/// Parse BER object and get content as slice
#[inline]
pub fn parse_ber_slice<T: Into<Tag>>(i: &[u8], tag: T) -> BerResult<&[u8]> {
    let tag = tag.into();
    parse_ber_container(move |content, hdr| {
        hdr.assert_tag(tag)?;
        Ok((&b""[..], content))
    })(i)
}

/// Parse BER object recursively, specifying the maximum recursion depth
///
/// Return a tuple containing the remaining (unparsed) bytes and the BER Object, or an error.
///
/// ### Example
///
/// ```
/// use der_parser::ber::{parse_ber_recursive, Tag};
///
/// let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (_, obj) = parse_ber_recursive(bytes, 1).expect("parsing failed");
///
/// assert_eq!(obj.header.tag(), Tag::Integer);
/// ```
#[inline]
pub fn parse_ber_recursive(i: &[u8], max_depth: usize) -> BerResult {
    parse_ber_any_r(i, max_depth)
}

/// Parse BER object recursively
///
/// Return a tuple containing the remaining (unparsed) bytes and the BER Object, or an error.
///
/// *Note*: this is the same as calling `parse_ber_recursive` with `MAX_RECURSION`.
///
/// ### Example
///
/// ```
/// use der_parser::ber::{parse_ber, Tag};
///
/// let bytes = &[0x02, 0x03, 0x01, 0x00, 0x01];
/// let (_, obj) = parse_ber(bytes).expect("parsing failed");
///
/// assert_eq!(obj.header.tag(), Tag::Integer);
/// ```
#[inline]
pub fn parse_ber(i: &[u8]) -> BerResult {
    parse_ber_recursive(i, MAX_RECURSION)
}

#[test]
fn test_bitstring_to_u64() {
    // ignored bits modulo 8 to 0
    let data = &hex_literal::hex!("0d 71 82");
    let r = bitstring_to_u64(8, &BitStringObject { data });
    assert_eq!(r, Ok(0x0d71));

    // input too large to fit a 64-bits integer
    let data = &hex_literal::hex!("0d 71 82 0e 73 72 76 6e 67 6e 62 6c 6e 2d 65 78 30 31");
    let r = bitstring_to_u64(0, &BitStringObject { data });
    assert!(r.is_err());

    // test large number but with many ignored bits
    let data = &hex_literal::hex!("0d 71 82 0e 73 72 76 6e 67 6e 62 6c 6e 2d 65 78 30 31");
    let r = bitstring_to_u64(130, &BitStringObject { data });
    // 2 = 130 % 8
    assert_eq!(r, Ok(0x0d71 >> 2));
}
