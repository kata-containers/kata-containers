#![cfg(feature = "std")]
use crate::ber::*;
use crate::oid::Oid;
use asn1_rs::{ASN1DateTime, Tag};
use cookie_factory::bytes::be_u8;
use cookie_factory::combinator::slice;
use cookie_factory::gen_simple;
use cookie_factory::multi::many_ref;
use cookie_factory::sequence::tuple;
use cookie_factory::{GenError, SerializeFn};
use std::io::Write;

fn encode_length<'a, W: Write + 'a, Len: Into<Length>>(len: Len) -> impl SerializeFn<W> + 'a {
    let l = len.into();
    move |out| {
        match l {
            Length::Definite(sz) => {
                if sz <= 128 {
                    // definite, short form
                    be_u8(sz as u8)(out)
                } else {
                    // definite, long form
                    let v: Vec<u8> = sz
                        .to_be_bytes()
                        .iter()
                        .cloned()
                        .skip_while(|&b| b == 0)
                        .collect();
                    let b0 = 0b1000_0000 | (v.len() as u8);
                    tuple((be_u8(b0 as u8), slice(v)))(out)
                }
            }
            Length::Indefinite => be_u8(0b1000_0000)(out),
        }
    }
}

/// Encode header as object
///
/// The `len` field must be correct
#[cfg_attr(docsrs, doc(cfg(feature = "serialize")))]
pub fn ber_encode_header<'a, 'b: 'a, W: Write + 'a>(hdr: &'b Header) -> impl SerializeFn<W> + 'a {
    move |out| {
        // identifier octets (X.690 8.1.2)
        let class_u8 = (hdr.class() as u8) << 6;
        let pc_u8 = (if hdr.constructed() { 1 } else { 0 }) << 5;
        if hdr.tag().0 >= 30 {
            unimplemented!();
        }
        let byte_0 = class_u8 | pc_u8 | (hdr.tag().0 as u8);
        // length octets (X.690 8.1.3)
        tuple((be_u8(byte_0), encode_length(hdr.length())))(out)
    }
}

fn ber_encode_oid<'a, W: Write + 'a>(oid: &'a Oid) -> impl SerializeFn<W> + 'a {
    move |out| {
        // check oid.relative attribute ? this should not be necessary
        slice(oid.as_bytes())(out)
    }
}

fn ber_encode_datetime<'a, W: Write + 'a>(time: &'a ASN1DateTime) -> impl SerializeFn<W> + 'a {
    move |out| {
        let s = format!("{}", time);
        slice(s)(out)
    }
}

fn ber_encode_sequence<'a, W: Write + Default + AsRef<[u8]> + 'a>(
    v: &'a [BerObject],
) -> impl SerializeFn<W> + 'a {
    many_ref(v, ber_encode_object)
}

/// Encode the provided object in an EXPLICIT tagged value, using the provided tag ans class
///
/// Note: `obj` should be the object to be encapsulated, not the `ContextSpecific` variant.
#[cfg_attr(docsrs, doc(cfg(feature = "serialize")))]
pub fn ber_encode_tagged_explicit<'a, W: Write + Default + AsRef<[u8]> + 'a>(
    tag: Tag,
    class: Class,
    obj: &'a BerObject,
) -> impl SerializeFn<W> + 'a {
    move |out| {
        // encode inner object
        let v = gen_simple(ber_encode_object(obj), W::default())?;
        let len = v.as_ref().len();
        // encode the application header, using the tag
        let hdr = Header::new(class, true /* X.690 8.14.2 */, tag, len.into());
        let v_hdr = gen_simple(ber_encode_header(&hdr), W::default())?;
        tuple((slice(v_hdr), slice(v)))(out)
    }
}

/// Encode the provided object in an IMPLICIT tagged value, using the provided tag and class
///
/// Note: `obj` should be the object to be encapsulated, not the `ContextSpecific` variant.
#[cfg_attr(docsrs, doc(cfg(feature = "serialize")))]
pub fn ber_encode_tagged_implicit<'a, W: Write + Default + AsRef<[u8]> + 'a>(
    tag: Tag,
    class: Class,
    obj: &'a BerObject,
) -> impl SerializeFn<W> + 'a {
    move |out| {
        // encode inner object content
        let v = gen_simple(ber_encode_object_content(&obj.content), W::default())?;
        // but replace the tag (keep constructed attribute)
        let len = v.as_ref().len();
        let hdr = Header::new(class, obj.header.constructed(), tag, len.into());
        let v_hdr = gen_simple(ber_encode_header(&hdr), W::default())?;
        tuple((slice(v_hdr), slice(v)))(out)
    }
}

fn ber_encode_object_content<'a, W: Write + Default + AsRef<[u8]> + 'a>(
    c: &'a BerObjectContent,
) -> impl SerializeFn<W> + 'a {
    move |out| match c {
        BerObjectContent::EndOfContent => be_u8(0)(out),
        BerObjectContent::Boolean(b) => {
            let b0 = if *b { 0xff } else { 0x00 };
            be_u8(b0)(out)
        }
        BerObjectContent::Integer(s) => slice(s)(out),
        BerObjectContent::BitString(ignored_bits, s) => {
            tuple((be_u8(*ignored_bits), slice(s)))(out)
        }
        BerObjectContent::OctetString(s) => slice(s)(out),
        BerObjectContent::Null => Ok(out),
        BerObjectContent::Enum(i) => {
            let v: Vec<u8> = i
                .to_be_bytes()
                .iter()
                .cloned()
                .skip_while(|&b| b == 0)
                .collect();
            slice(v)(out)
        }
        BerObjectContent::OID(oid) | BerObjectContent::RelativeOID(oid) => ber_encode_oid(oid)(out),
        BerObjectContent::UTCTime(time) | BerObjectContent::GeneralizedTime(time) => {
            ber_encode_datetime(time)(out)
        }
        BerObjectContent::NumericString(s)
        | BerObjectContent::BmpString(s)
        | BerObjectContent::GeneralString(s)
        | BerObjectContent::ObjectDescriptor(s)
        | BerObjectContent::GraphicString(s)
        | BerObjectContent::VisibleString(s)
        | BerObjectContent::PrintableString(s)
        | BerObjectContent::IA5String(s)
        | BerObjectContent::T61String(s)
        | BerObjectContent::VideotexString(s)
        | BerObjectContent::UTF8String(s) => slice(s)(out),
        BerObjectContent::UniversalString(s) => slice(s)(out),
        BerObjectContent::Sequence(v) | BerObjectContent::Set(v) => ber_encode_sequence(v)(out),
        // best we can do is tagged-explicit, but we don't know
        BerObjectContent::Optional(inner) => {
            // directly encode inner object
            match inner {
                Some(obj) => ber_encode_object_content(&obj.content)(out),
                None => slice(&[])(out), // XXX encode NOP ?
            }
        }
        BerObjectContent::Tagged(_class, _tag, inner) => {
            // directly encode inner object
            // XXX wrong, we should wrap it!
            ber_encode_object(inner)(out)
        }
        BerObjectContent::Unknown(any) => slice(any.data)(out),
    }
}

/// Encode header and object content as BER, without any validation
///
/// Note that the encoding will not check *any* `field of the header (including length)
/// This can be used to craft invalid objects.
///
/// *This function is only available if the `serialize` feature is enabled.*
#[cfg_attr(docsrs, doc(cfg(feature = "serialize")))]
pub fn ber_encode_object_raw<'a, 'b: 'a, 'c: 'a, W: Write + Default + AsRef<[u8]> + 'a>(
    hdr: &'b Header,
    content: &'c BerObjectContent,
) -> impl SerializeFn<W> + 'a {
    tuple((ber_encode_header(hdr), ber_encode_object_content(content)))
}

/// Encode object as BER
///
/// Note that the encoding will not check that the values of the `BerObject` fields are correct.
/// The length is automatically calculated, and the field is ignored.
///
/// `Tagged` objects will be encoded as EXPLICIT.
///
/// *This function is only available if the `serialize` feature is enabled.*
#[cfg_attr(docsrs, doc(cfg(feature = "serialize")))]
pub fn ber_encode_object<'a, 'b: 'a, W: Write + Default + AsRef<[u8]> + 'a>(
    obj: &'b BerObject,
) -> impl SerializeFn<W> + 'a {
    move |out| {
        // XXX should we make an exception for tagged values here ?
        let v = gen_simple(ber_encode_object_content(&obj.content), W::default())?;
        let len = v.as_ref().len();
        let hdr = obj.header.clone().with_length(len.into());
        let v_hdr = gen_simple(ber_encode_header(&hdr), W::default())?;
        tuple((slice(v_hdr), slice(v)))(out)
    }
}

impl<'a> BerObject<'a> {
    /// Attempt to encode object as BER
    ///
    /// Note that the encoding will not check that the values of the `BerObject` fields are correct.
    /// The length is automatically calculated, and the field is ignored.
    ///
    /// `Tagged` objects will be encoded as EXPLICIT.
    ///
    /// *This function is only available if the `serialize` feature is enabled.*
    #[cfg_attr(docsrs, doc(cfg(feature = "serialize")))]
    pub fn to_vec(&self) -> Result<Vec<u8>, GenError> {
        gen_simple(ber_encode_object(self), Vec::new())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::error::BerResult;
    use cookie_factory::gen_simple;
    use hex_literal::hex;

    macro_rules! encode_and_parse {
        ($obj:ident, $encode:ident, $parse:ident) => {{
            let v = gen_simple($encode(&$obj), Vec::new()).expect("could not encode");
            let (_, obj2) = $parse(&v).expect("could not re-parse");
            assert_eq!($obj, obj2);
            v
        }};
    }

    #[test]
    fn test_encode_length() {
        let l = 38;
        let v = gen_simple(encode_length(l), Vec::new()).expect("could not serialize");
        assert_eq!(&v[..], &[38]);
        let l = 201;
        let v = gen_simple(encode_length(l), Vec::new()).expect("could not serialize");
        assert_eq!(&v[..], &[129, 201]);
        let l = 0x1234_5678;
        let v = gen_simple(encode_length(l), Vec::new()).expect("could not serialize");
        assert_eq!(&v[..], &[132, 0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn test_encode_header() {
        // simple header (integer)
        let bytes = hex!("02 03 01 00 01");
        let (_, hdr) = ber_read_element_header(&bytes).expect("could not parse");
        let v = encode_and_parse!(hdr, ber_encode_header, ber_read_element_header);
        assert_eq!(&v[..], &bytes[..2]);
    }

    #[test]
    fn test_encode_bool() {
        let b_true = BerObject::from_obj(BerObjectContent::Boolean(true));
        let b_false = BerObject::from_obj(BerObjectContent::Boolean(false));
        encode_and_parse!(b_true, ber_encode_object, parse_ber_bool);
        encode_and_parse!(b_false, ber_encode_object, parse_ber_bool);
    }

    #[test]
    fn test_encode_integer() {
        let i = BerObject::from_obj(BerObjectContent::Integer(b"\x01\x00\x01"));
        encode_and_parse!(i, ber_encode_object, parse_ber_integer);
    }

    #[test]
    fn test_encode_bitstring() {
        let bytes = hex!("03 04 06 6e 5d e0");
        let b = BerObject::from_obj(BerObjectContent::BitString(
            6,
            BitStringObject { data: &bytes[3..] },
        ));
        let v = encode_and_parse!(b, ber_encode_object, parse_ber_bitstring);
        assert_eq!(&v[..], bytes)
    }

    #[test]
    fn test_encode_octetstring() {
        let i = BerObject::from_obj(BerObjectContent::OctetString(b"AAAAA"));
        let v = encode_and_parse!(i, ber_encode_object, parse_ber_octetstring);
        assert_eq!(&v[..], hex!("04 05 41 41 41 41 41"))
    }

    #[test]
    fn test_encode_enum() {
        let i = BerObject::from_obj(BerObjectContent::Enum(2));
        let v = encode_and_parse!(i, ber_encode_object, parse_ber_enum);
        assert_eq!(&v[..], hex!("0a 01 02"))
    }

    #[test]
    fn test_encode_null() {
        let i = BerObject::from_obj(BerObjectContent::Null);
        encode_and_parse!(i, ber_encode_object, parse_ber_null);
    }

    #[test]
    fn test_encode_oid() {
        let bytes = hex!("06 09 2A 86 48 86 F7 0D 01 01 05");
        let obj = BerObject::from_obj(BerObjectContent::OID(
            Oid::from(&[1, 2, 840, 113_549, 1, 1, 5]).unwrap(),
        ));
        let v = encode_and_parse!(obj, ber_encode_object, parse_ber_oid);
        assert_eq!(&v[..], bytes);
    }

    #[test]
    fn test_encode_relative_oid() {
        let bytes = hex!("0d 04 c2 7b 03 02");
        let obj = BerObject::from_obj(BerObjectContent::RelativeOID(
            Oid::from_relative(&[8571, 3, 2]).unwrap(),
        ));
        let v = encode_and_parse!(obj, ber_encode_object, parse_ber_relative_oid);
        assert_eq!(&v[..], bytes);
    }

    #[test]
    fn test_encode_sequence() {
        let bytes = hex!("30 0a 02 03 01 00 01 02 03 01 00 00");
        let obj = BerObject::from_seq(vec![
            BerObject::from_int_slice(b"\x01\x00\x01"),
            BerObject::from_int_slice(b"\x01\x00\x00"),
        ]);
        let v = encode_and_parse!(obj, ber_encode_object, parse_ber_sequence);
        assert_eq!(&v[..], bytes);
    }

    #[test]
    fn test_encode_set() {
        let bytes = hex!("31 0a 02 03 01 00 01 02 03 01 00 00");
        let obj = BerObject::from_set(vec![
            BerObject::from_int_slice(b"\x01\x00\x01"),
            BerObject::from_int_slice(b"\x01\x00\x00"),
        ]);
        let v = encode_and_parse!(obj, ber_encode_object, parse_ber_set);
        assert_eq!(&v[..], bytes);
    }

    #[test]
    fn test_encode_tagged_explicit() {
        fn local_parse(i: &[u8]) -> BerResult {
            parse_ber_explicit_optional(i, Tag(0), parse_ber_integer)
        }
        let bytes = hex!("a0 03 02 01 02");
        let obj = BerObject::from_int_slice(b"\x02");
        let v = gen_simple(
            ber_encode_tagged_explicit(Tag(0), Class::ContextSpecific, &obj),
            Vec::new(),
        )
        .expect("could not encode");
        let (_, obj2) = local_parse(&v).expect("could not re-parse");
        let obj2 = obj2
            .as_optional()
            .expect("tagged object not found")
            .expect("optional object empty");
        let (_class, tag, inner) = obj2.as_tagged().expect("not a tagged object");
        assert_eq!(tag, Tag(0));
        assert_eq!(&obj, inner);
        assert_eq!(&v[..], bytes);
    }

    #[test]
    fn test_encode_tagged_implicit() {
        fn der_read_integer_content<'a>(
            i: &'a [u8],
            hdr: &Header,
            depth: usize,
        ) -> BerResult<'a, BerObjectContent<'a>> {
            ber_read_element_content_as(i, Tag::Integer, hdr.length(), false, depth)
        }
        fn local_parse(i: &[u8]) -> BerResult<BerObject> {
            parse_ber_implicit(i, Tag(3), der_read_integer_content)
        }
        let obj = BerObject::from_int_slice(b"\x02");
        let v = gen_simple(
            ber_encode_tagged_implicit(Tag(3), Class::ContextSpecific, &obj),
            Vec::new(),
        )
        .expect("could not encode");
        let (_, obj2) = local_parse(&v).expect("could not re-parse");
        assert_eq!(obj2.header.tag(), Tag(3));
        assert_eq!(&obj.content, &obj2.content);
        let bytes = hex!("83 01 02");
        assert_eq!(&v[..], bytes);
    }
    #[test]
    fn test_encode_tagged_application() {
        fn local_parse(i: &[u8]) -> BerResult {
            parse_ber_explicit_optional(i, Tag(2), parse_ber_integer)
        }
        let obj = BerObject::from_int_slice(b"\x02");
        let v = gen_simple(
            ber_encode_tagged_explicit(Tag(2), Class::Application, &obj),
            Vec::new(),
        )
        .expect("could not encode");
        let (_, obj2) = local_parse(&v).expect("could not re-parse");
        let obj2 = obj2
            .as_optional()
            .expect("tagged object not found")
            .expect("optional object empty");
        let (_class, tag, inner) = obj2.as_tagged().expect("not a tagged object");
        assert_eq!(tag, Tag(2));
        assert_eq!(&obj, inner);
        let bytes = hex!("62 03 02 01 02");
        assert_eq!(&v[..], bytes);
    }
}
