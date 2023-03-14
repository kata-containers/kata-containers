use crate::ber::Length;
use crate::der::*;
use crate::error::*;
use nom::bytes::complete::take;
use nom::combinator::{all_consuming, complete, cut, map};
use nom::error::ParseError;
use nom::multi::many0;
use nom::{Err, IResult};

/// Parse a SEQUENCE OF object
///
/// Given a subparser for a DER type, parse a sequence of identical objects.
///
/// ```rust
/// # use der_parser::der::{parse_der_integer, parse_der_sequence_of, DerObject};
/// # use der_parser::error::BerResult;
/// #
/// /// Read a SEQUENCE OF INTEGER
/// fn parser(i:&[u8]) -> BerResult<DerObject> {
///     parse_der_sequence_of(parse_der_integer)(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x30, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x02, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = DerObject::from_seq(vec![
/// #     DerObject::from_int_slice(b"\x01\x00\x01"),
/// #     DerObject::from_int_slice(b"\x01\x00\x00"),
/// # ]);
/// # assert_eq!(parser(&bytes), Ok((empty, expected)));
/// let (rem, v) = parser(&bytes).expect("parsing failed");
/// ```
pub fn parse_der_sequence_of<'a, F>(f: F) -> impl FnMut(&'a [u8]) -> BerResult
where
    F: Fn(&'a [u8]) -> BerResult,
{
    map(parse_der_sequence_of_v(f), DerObject::from_seq)
}

/// Parse a SEQUENCE OF object (returning a vec)
///
/// Given a subparser for a DER type, parse a sequence of identical objects.
///
/// This differs from `parse_der_sequence_of` in the parse function and return type.
///
/// ```rust
/// # use der_parser::der::{parse_der_integer, parse_der_sequence_of_v, DerObject};
/// # use der_parser::error::BerResult;
/// #
/// /// Read a SEQUENCE OF INTEGER
/// fn parser(i:&[u8]) -> BerResult<Vec<DerObject>> {
///     parse_der_sequence_of_v(parse_der_integer)(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x30, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x02, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = vec![
/// #     DerObject::from_int_slice(b"\x01\x00\x01"),
/// #     DerObject::from_int_slice(b"\x01\x00\x00"),
/// # ];
/// let (rem, v) = parser(&bytes).expect("parsing failed");
/// # assert_eq!(v, expected);
/// ```
pub fn parse_der_sequence_of_v<'a, T, F, E>(
    f: F,
) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], Vec<T>, E>
where
    F: FnMut(&'a [u8]) -> IResult<&'a [u8], T, E>,
    E: ParseError<&'a [u8]> + From<BerError>,
{
    let mut subparser = all_consuming(many0(complete(cut(f))));
    parse_der_sequence_defined_g(move |data, _| subparser(data))
}

/// Parse a defined sequence of DER elements (function version)
///
/// Given a list of expected parsers, apply them to build a DER sequence and
/// return the remaining bytes and the built object.
///
/// The remaining bytes point *after* the sequence: any bytes that are part of the sequence but not
/// parsed are ignored.
///
/// The object header is not available to the parsing function, and the returned type is always a
/// `DerObject`.
/// For a generic version, see
/// [`parse_der_sequence_defined_g`](fn.parse_der_sequence_defined_g.html).
///
/// # Examples
///
/// Parsing a sequence of identical types (same as `parse_der_sequence_of`):
///
/// ```rust
/// # use der_parser::der::{parse_der_integer, parse_der_sequence_defined, DerObject};
/// # use der_parser::error::BerResult;
/// use nom::combinator::complete;
/// use nom::multi::many1;
///
/// fn localparse_seq(i:&[u8]) -> BerResult {
///     parse_der_sequence_defined(
///         many1(complete(parse_der_integer))
///     )(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x30, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x02, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = DerObject::from_seq(vec![
/// #     DerObject::from_int_slice(b"\x01\x00\x01"),
/// #     DerObject::from_int_slice(b"\x01\x00\x00"),
/// # ]);
/// # assert_eq!(localparse_seq(&bytes), Ok((empty, expected)));
/// let (rem, v) = localparse_seq(&bytes).expect("parsing failed");
/// ```
///
/// Parsing a defined sequence with different types:
///
/// ```rust
/// # use der_parser::der::*;
/// # use der_parser::error::BerResult;
/// use nom::combinator::map;
/// use nom::sequence::tuple;
///
/// /// Read a DER-encoded object:
/// /// SEQUENCE {
/// ///     a INTEGER,
/// ///     b OCTETSTRING
/// /// }
/// fn localparse_seq(i:&[u8]) -> BerResult {
///     parse_der_sequence_defined(
///         // the nom `tuple` combinator returns a tuple, so we have to map it
///         // to a list
///         map(
///             tuple((parse_der_integer, parse_der_octetstring)),
///             |(a, b)| vec![a, b]
///         )
///     )(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x30, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x04, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = DerObject::from_seq(vec![
/// #     DerObject::from_int_slice(b"\x01\x00\x01"),
/// #     DerObject::from_obj(DerObjectContent::OctetString(b"\x01\x00\x00")),
/// # ]);
/// # assert_eq!(localparse_seq(&bytes), Ok((empty, expected)));
/// let (rem, v) = localparse_seq(&bytes).expect("parsing failed");
/// ```
pub fn parse_der_sequence_defined<'a, F>(mut f: F) -> impl FnMut(&'a [u8]) -> BerResult
where
    F: FnMut(&'a [u8]) -> BerResult<Vec<DerObject>>,
{
    map(
        parse_der_sequence_defined_g(move |data, _| f(data)),
        DerObject::from_seq,
    )
}

/// Parse a defined SEQUENCE object (generic function)
///
/// Given a parser for sequence content, apply it to build a DER sequence and
/// return the remaining bytes and the built object.
///
/// The remaining bytes point *after* the sequence: any bytes that are part of the sequence but not
/// parsed are ignored.
///
/// Unlike `parse_der_sequence_defined`, this function allows returning any object or error type,
/// and also passes the object header to the callback.
///
/// # Examples
///
/// Parsing a defined sequence with different types:
///
/// ```rust
/// # use der_parser::der::*;
/// # use der_parser::error::BerResult;
/// #
/// # #[derive(Debug, PartialEq)]
/// pub struct MyObject<'a> {
///     a: u32,
///     b: &'a [u8],
/// }
///
/// /// Read a DER-encoded object:
/// /// SEQUENCE {
/// ///     a INTEGER (0..4294967295),
/// ///     b OCTETSTRING
/// /// }
/// fn parse_myobject(i: &[u8]) -> BerResult<MyObject> {
///     parse_der_sequence_defined_g(
///         |i:&[u8], _| {
///             let (i, a) = parse_der_u32(i)?;
///             let (i, obj) = parse_der_octetstring(i)?;
///             let b = obj.as_slice().unwrap();
///             Ok((i, MyObject{ a, b }))
///         }
///     )(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x30, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x04, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = MyObject {
/// #   a: 0x010001,
/// #   b: &[01, 00, 00]
/// # };
/// # assert_eq!(parse_myobject(&bytes), Ok((empty, expected)));
/// let (rem, v) = parse_myobject(&bytes).expect("parsing failed");
/// ```
pub fn parse_der_sequence_defined_g<'a, O, F, E>(
    mut f: F,
) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], O, E>
where
    F: FnMut(&'a [u8], Header<'a>) -> IResult<&'a [u8], O, E>,
    E: ParseError<&'a [u8]> + From<BerError>,
{
    parse_der_container(move |i, hdr| {
        hdr.assert_tag(Tag::Sequence)
            .map_err(|e| Err::Error(e.into()))?;
        f(i, hdr)
    })
}

/// Parse a SET OF object
///
/// Given a subparser for a DER type, parse a set of identical objects.
///
/// ```rust
/// # use der_parser::der::{parse_der_integer, parse_der_set_of, DerObject};
/// # use der_parser::error::BerResult;
/// #
/// /// Read a SET OF INTEGER
/// fn parser(i:&[u8]) -> BerResult<DerObject> {
///     parse_der_set_of(parse_der_integer)(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x31, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x02, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = DerObject::from_set(vec![
/// #     DerObject::from_int_slice(b"\x01\x00\x01"),
/// #     DerObject::from_int_slice(b"\x01\x00\x00"),
/// # ]);
/// # assert_eq!(parser(&bytes), Ok((empty, expected)));
/// let (rem, v) = parser(&bytes).expect("parsing failed");
/// ```
pub fn parse_der_set_of<'a, F>(f: F) -> impl FnMut(&'a [u8]) -> BerResult
where
    F: Fn(&'a [u8]) -> BerResult,
{
    map(parse_der_set_of_v(f), DerObject::from_set)
}

/// Parse a SET OF object (returning a vec)
///
/// Given a subparser for a DER type, parse a set of identical objects.
///
/// This differs from `parse_der_set_of` in the parse function and return type.
///
/// ```rust
/// # use der_parser::der::{parse_der_integer, parse_der_set_of_v, DerObject};
/// # use der_parser::error::BerResult;
/// #
/// /// Read a SET OF INTEGER
/// fn parser(i:&[u8]) -> BerResult<Vec<DerObject>> {
///     parse_der_set_of_v(parse_der_integer)(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x31, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x02, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = vec![
/// #     DerObject::from_int_slice(b"\x01\x00\x01"),
/// #     DerObject::from_int_slice(b"\x01\x00\x00"),
/// # ];
/// let (rem, v) = parser(&bytes).expect("parsing failed");
/// # assert_eq!(v, expected);
/// ```
pub fn parse_der_set_of_v<'a, T, F, E>(f: F) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], Vec<T>, E>
where
    F: FnMut(&'a [u8]) -> IResult<&'a [u8], T, E>,
    E: ParseError<&'a [u8]> + From<BerError>,
{
    let mut subparser = all_consuming(many0(complete(cut(f))));
    parse_der_set_defined_g(move |data, _| subparser(data))
}

/// Parse a defined set of DER elements (function version)
///
/// Given a list of expected parsers, apply them to build a DER set and
/// return the remaining bytes and the built object.
///
/// The remaining bytes point *after* the set: any bytes that are part of the sequence but not
/// parsed are ignored.
/// The nom combinator `all_consuming` can be used to ensure all the content is parsed.
///
/// The object header is not available to the parsing function, and the returned type is always a
/// `DerObject`.
/// For a generic version, see [`parse_der_set_defined_g`](fn.parse_der_set_defined_g.html).
///
/// # Examples
///
/// Parsing a set of identical types (same as `parse_der_set_of`):
///
/// ```rust
/// # use der_parser::der::{parse_der_integer, parse_der_set_defined, DerObject};
/// # use der_parser::error::BerResult;
/// use nom::combinator::complete;
/// use nom::multi::many1;
///
/// fn localparse_seq(i:&[u8]) -> BerResult {
///     parse_der_set_defined(
///         many1(complete(parse_der_integer))
///     )(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x31, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x02, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = DerObject::from_set(vec![
/// #     DerObject::from_int_slice(b"\x01\x00\x01"),
/// #     DerObject::from_int_slice(b"\x01\x00\x00"),
/// # ]);
/// # assert_eq!(localparse_seq(&bytes), Ok((empty, expected)));
/// let (rem, v) = localparse_seq(&bytes).expect("parsing failed");
/// ```
///
/// Parsing a defined set with different types:
///
/// ```rust
/// # use der_parser::der::*;
/// # use der_parser::error::BerResult;
/// use nom::combinator::map;
/// use nom::sequence::tuple;
///
/// /// Read a DER-encoded object:
/// /// SET {
/// ///     a INTEGER,
/// ///     b OCTETSTRING
/// /// }
/// fn localparse_set(i:&[u8]) -> BerResult {
///     parse_der_set_defined(
///         // the nom `tuple` combinator returns a tuple, so we have to map it
///         // to a list
///         map(
///             tuple((parse_der_integer, parse_der_octetstring)),
///             |(a, b)| vec![a, b]
///         )
///     )(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x31, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x04, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = DerObject::from_set(vec![
/// #     DerObject::from_int_slice(b"\x01\x00\x01"),
/// #     DerObject::from_obj(DerObjectContent::OctetString(b"\x01\x00\x00")),
/// # ]);
/// # assert_eq!(localparse_set(&bytes), Ok((empty, expected)));
/// let (rem, v) = localparse_set(&bytes).expect("parsing failed");
/// ```
pub fn parse_der_set_defined<'a, F>(mut f: F) -> impl FnMut(&'a [u8]) -> BerResult
where
    F: FnMut(&'a [u8]) -> BerResult<Vec<DerObject>>,
{
    map(
        parse_der_set_defined_g(move |data, _| f(data)),
        DerObject::from_set,
    )
}

/// Parse a defined SET object (generic version)
///
/// Given a parser for set content, apply it to build a DER set and
/// return the remaining bytes and the built object.
///
/// The remaining bytes point *after* the set: any bytes that are part of the sequence but not
/// parsed are ignored.
/// The nom combinator `all_consuming` can be used to ensure all the content is parsed.
///
/// Unlike `parse_der_set_defined`, this function allows returning any object or error type,
/// and also passes the object header to the callback.
///
/// # Examples
///
/// Parsing a defined set with different types:
///
/// ```rust
/// # use der_parser::der::*;
/// # use der_parser::error::BerResult;
/// #
/// # #[derive(Debug, PartialEq)]
/// pub struct MyObject<'a> {
///     a: u32,
///     b: &'a [u8],
/// }
///
/// /// Read a DER-encoded object:
/// /// SET {
/// ///     a INTEGER (0..4294967295),
/// ///     b OCTETSTRING
/// /// }
/// fn parse_myobject(i: &[u8]) -> BerResult<MyObject> {
///     parse_der_set_defined_g(
///         |i:&[u8], _| {
///             let (i, a) = parse_der_u32(i)?;
///             let (i, obj) = parse_der_octetstring(i)?;
///             let b = obj.as_slice().unwrap();
///             Ok((i, MyObject{ a, b }))
///         }
///     )(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x31, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x04, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = MyObject {
/// #   a: 0x010001,
/// #   b: &[01, 00, 00]
/// # };
/// # assert_eq!(parse_myobject(&bytes), Ok((empty, expected)));
/// let (rem, v) = parse_myobject(&bytes).expect("parsing failed");
/// ```
pub fn parse_der_set_defined_g<'a, O, F, E>(
    mut f: F,
) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], O, E>
where
    F: FnMut(&'a [u8], Header<'a>) -> IResult<&'a [u8], O, E>,
    E: ParseError<&'a [u8]> + From<BerError>,
{
    parse_der_container(move |i, hdr| {
        hdr.assert_tag(Tag::Set).map_err(|e| Err::Error(e.into()))?;
        f(i, hdr)
    })
}

/// Parse a DER object and apply provided function to content
///
/// Given a parser for content, read DER object header and apply parser to
/// return the remaining bytes and the parser result.
///
/// The remaining bytes point *after* the content: any bytes that are part of the content but not
/// parsed are ignored.
/// The nom combinator `all_consuming` can be used to ensure all the content is parsed.
///
/// This function is mostly intended for constructed objects, but can be used for any valid DER
/// object.
///
/// # Examples
///
/// Parsing a defined sequence with different types:
///
/// ```rust
/// # use der_parser::der::*;
/// # use der_parser::error::{BerError, BerResult};
/// #
/// # #[derive(Debug, PartialEq)]
/// pub struct MyObject<'a> {
///     a: u32,
///     b: &'a [u8],
/// }
///
/// /// Read a DER-encoded object:
/// /// SEQUENCE {
/// ///     a INTEGER (0..4294967295),
/// ///     b OCTETSTRING
/// /// }
/// fn parse_myobject(i: &[u8]) -> BerResult<MyObject> {
///     parse_der_container(
///         |i: &[u8], hdr: Header| {
///             if hdr.tag() != Tag::Sequence {
///                 return Err(nom::Err::Error(BerError::BerTypeError.into()));
///             }
///             let (i, a) = parse_der_u32(i)?;
///             let (i, obj) = parse_der_octetstring(i)?;
///             let b = obj.as_slice().unwrap();
///             Ok((i, MyObject{ a, b }))
///         }
///     )(i)
/// }
///
/// # let empty = &b""[..];
/// # let bytes = [ 0x30, 0x0a,
/// #               0x02, 0x03, 0x01, 0x00, 0x01,
/// #               0x04, 0x03, 0x01, 0x00, 0x00,
/// # ];
/// # let expected  = MyObject {
/// #   a: 0x010001,
/// #   b: &[01, 00, 00]
/// # };
/// # assert_eq!(parse_myobject(&bytes), Ok((empty, expected)));
/// let (rem, v) = parse_myobject(&bytes).expect("parsing failed");
/// ```
pub fn parse_der_container<'a, O, F, E>(mut f: F) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], O, E>
where
    F: FnMut(&'a [u8], Header<'a>) -> IResult<&'a [u8], O, E>,
    E: ParseError<&'a [u8]> + From<BerError>,
{
    move |i: &[u8]| {
        let (i, hdr) = der_read_element_header(i).map_err(nom::Err::convert)?;
        // X.690 10.1: the definitive form of length encoding shall be used
        let (i, data) = match hdr.length() {
            Length::Definite(len) => take(len)(i)?,
            Length::Indefinite => {
                return Err(Err::Error(
                    BerError::DerConstraintFailed(DerConstraint::IndefiniteLength).into(),
                ));
            }
        };
        let (_rest, v) = f(data, hdr)?;
        Ok((i, v))
    }
}
