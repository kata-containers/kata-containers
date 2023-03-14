use asn1_rs::*;
use hex_literal::hex;
use nom::Needed;
#[cfg(feature = "datetime")]
use time::macros::datetime;

#[test]
fn from_ber_any() {
    let input = &hex!("02 01 02 ff ff");
    let (rem, result) = Any::from_ber(input).expect("parsing failed");
    // dbg!(&result);
    assert_eq!(rem, &[0xff, 0xff]);
    assert_eq!(result.header.tag(), Tag::Integer);
}

#[test]
fn from_ber_bitstring() {
    //
    // correct DER encoding
    //
    let input = &hex!("03 04 06 6e 5d c0");
    let (rem, result) = BitString::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.unused_bits, 6);
    assert_eq!(&result.data[..], &input[3..]);
    //
    // correct encoding, but wrong padding bits (not all set to 0)
    //
    let input = &hex!("03 04 06 6e 5d e0");
    let (rem, result) = BitString::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.unused_bits, 6);
    assert_eq!(&result.data[..], &input[3..]);
    //
    // long form of length (invalid, < 127)
    //
    let input = &hex!("03 81 04 06 6e 5d c0");
    let (rem, result) = BitString::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.unused_bits, 6);
    assert_eq!(&result.data[..], &input[4..]);
}

#[test]
fn from_ber_embedded_pdv() {
    let input = &hex!("2b 0d a0 07 81 05 2a 03 04 05 06 82 02 aa a0");
    let (rem, result) = EmbeddedPdv::from_ber(input).expect("parsing failed");
    assert_eq!(rem, &[]);
    assert_eq!(
        result.identification,
        PdvIdentification::Syntax(Oid::from(&[1, 2, 3, 4, 5, 6]).unwrap())
    );
    assert_eq!(result.data_value, &[0xaa, 0xa0]);
}

#[test]
fn embedded_pdv_variants() {
    // identification: syntaxes
    let input = &hex!("2b 11 a0 0c a0 0a 80 02  2a 03 81 04 2a 03 04 05   82 01 00");
    let (rem, res) = EmbeddedPdv::from_ber(input).expect("parsing EMBEDDED PDV failed");
    assert!(rem.is_empty());
    assert!(matches!(
        res.identification,
        PdvIdentification::Syntaxes { .. }
    ));
    // identification: syntax
    let input = &hex!("2b 09 a0 04 81 02 2a 03  82 01 00");
    let (rem, res) = EmbeddedPdv::from_ber(input).expect("parsing EMBEDDED PDV failed");
    assert!(rem.is_empty());
    assert!(matches!(res.identification, PdvIdentification::Syntax(_)));
    // identification: presentation-context-id
    let input = &hex!("2b 08 a0 03 82 01 02 82  01 00");
    let (rem, res) = EmbeddedPdv::from_ber(input).expect("parsing EMBEDDED PDV failed");
    assert!(rem.is_empty());
    assert!(matches!(
        res.identification,
        PdvIdentification::PresentationContextId(_)
    ));
    // identification: context-negotiation
    let input = &hex!("2b 10 a0 0b a3 09 80 01  2a 81 04 2a 03 04 05 82   01 00");
    let (rem, res) = EmbeddedPdv::from_ber(input).expect("parsing EMBEDDED PDV failed");
    assert!(rem.is_empty());
    assert!(matches!(
        res.identification,
        PdvIdentification::ContextNegotiation { .. }
    ));
    // identification: transfer-syntax
    let input = &hex!("2b 0b a0 06 84 04 2a 03  04 05 82 01 00");
    let (rem, res) = EmbeddedPdv::from_ber(input).expect("parsing EMBEDDED PDV failed");
    assert!(rem.is_empty());
    assert!(matches!(
        res.identification,
        PdvIdentification::TransferSyntax(_)
    ));
    // identification: fixed
    let input = &hex!("2b 07 a0 02 85 00 82 01  00");
    let (rem, res) = EmbeddedPdv::from_ber(input).expect("parsing EMBEDDED PDV failed");
    assert!(rem.is_empty());
    assert!(matches!(res.identification, PdvIdentification::Fixed));
    // identification: invalid
    let input = &hex!("2b 07 a0 02 86 00 82 01  00");
    let e = EmbeddedPdv::from_ber(input).expect_err("parsing should fail");
    assert!(matches!(e, Err::Error(Error::InvalidValue { .. })));
}

#[test]
fn from_ber_endofcontent() {
    let input = &hex!("00 00");
    let (rem, _result) = EndOfContent::from_ber(input).expect("parsing failed");
    assert_eq!(rem, &[]);
}

#[test]
fn from_ber_generalizedtime() {
    let input = &hex!("18 0F 32 30 30 32 31 32 31 33 31 34 32 39 32 33 5A FF");
    let (rem, result) = GeneralizedTime::from_ber(input).expect("parsing failed");
    assert_eq!(rem, &[0xff]);
    #[cfg(feature = "datetime")]
    {
        let datetime = datetime! {2002-12-13 14:29:23 UTC};

        assert_eq!(result.utc_datetime(), Ok(datetime));
    }
    let _ = result;
    // local time with fractional seconds
    let input = b"\x18\x1019851106210627.3";
    let (rem, result) = GeneralizedTime::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.0.millisecond, Some(300));
    assert_eq!(result.0.tz, ASN1TimeZone::Undefined);
    #[cfg(feature = "datetime")]
    {
        let datetime = datetime! {1985-11-06 21:06:27.300_000_000 UTC};
        assert_eq!(result.utc_datetime(), Ok(datetime));
    }
    let _ = result;
    // coordinated universal time with fractional seconds
    let input = b"\x18\x1119851106210627.3Z";
    let (rem, result) = GeneralizedTime::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.0.millisecond, Some(300));
    assert_eq!(result.0.tz, ASN1TimeZone::Z);
    #[cfg(feature = "datetime")]
    {
        let datetime = datetime! {1985-11-06 21:06:27.300_000_000 UTC};
        assert_eq!(result.utc_datetime(), Ok(datetime));
    }
    let _ = result;
    // coordinated universal time with fractional seconds
    let input = b"\x18\x1219851106210627.03Z";
    let (rem, result) = GeneralizedTime::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.0.millisecond, Some(30));
    assert_eq!(result.0.tz, ASN1TimeZone::Z);
    #[cfg(feature = "datetime")]
    {
        let datetime = datetime! {1985-11-06 21:06:27.03 UTC};
        assert_eq!(result.utc_datetime(), Ok(datetime));
    }
    let _ = result;
    // local time with fractional seconds, and with local time 5 hours retarded in relation to coordinated universal time.
    let input = b"\x18\x1519851106210627.3-0500";
    let (rem, result) = GeneralizedTime::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.0.millisecond, Some(300));
    assert_eq!(result.0.tz, ASN1TimeZone::Offset(-5, 0));
    #[cfg(feature = "datetime")]
    {
        let datetime = datetime! {1985-11-06 21:06:27.300_000_000 -05:00};
        assert_eq!(result.utc_datetime(), Ok(datetime));
    }
    let _ = result;
}

#[test]
fn from_ber_int() {
    let input = &hex!("02 01 02 ff ff");
    let (rem, result) = u8::from_ber(input).expect("parsing failed");
    assert_eq!(result, 2);
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
fn from_ber_relative_oid() {
    let input = &hex!("0d 04 c2 7b 03 02");
    let (rem, result) = Oid::from_ber_relative(input).expect("parsing failed");
    assert_eq!(result, Oid::from_relative(&[8571, 3, 2]).unwrap());
    assert_eq!(rem, &[]);
}

#[test]
fn from_ber_length_incomplete() {
    let input = &hex!("30");
    let res = u8::from_ber(input).expect_err("parsing should have failed");
    assert_eq!(res, nom::Err::Incomplete(Needed::new(1)));
    let input = &hex!("02");
    let res = u8::from_ber(input).expect_err("parsing should have failed");
    assert_eq!(res, nom::Err::Incomplete(Needed::new(1)));
    let input = &hex!("02 05");
    let res = u8::from_ber(input).expect_err("parsing should have failed");
    assert_eq!(res, nom::Err::Incomplete(Needed::new(5)));
    let input = &hex!("02 85");
    let res = u8::from_ber(input).expect_err("parsing should have failed");
    assert_eq!(res, nom::Err::Incomplete(Needed::new(5)));
    let input = &hex!("02 85 ff");
    let res = u8::from_ber(input).expect_err("parsing should have failed");
    assert_eq!(res, nom::Err::Incomplete(Needed::new(4)));
}

#[test]
fn from_ber_length_invalid() {
    let input = &hex!("02 ff 00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f 10");
    let res = u8::from_ber(input).expect_err("parsing should have failed");
    assert_eq!(res, nom::Err::Error(Error::InvalidLength));
    let input = &hex!("02 85 ff ff ff ff ff 00");
    let res = u8::from_ber(input).expect_err("parsing should have failed");
    assert!(res.is_incomplete());
}

#[test]
fn from_ber_octetstring() {
    let input = &hex!("04 05 41 41 41 41 41");
    let (rem, result) = OctetString::from_ber(input).expect("parsing failed");
    assert_eq!(result.as_ref(), b"AAAAA");
    assert_eq!(rem, &[]);
}

#[test]
fn from_ber_real_binary() {
    const EPSILON: f32 = 0.00001;
    // binary, base = 2
    let input = &hex!("09 03 80 ff 01 ff ff");
    let (rem, result) = Real::from_ber(input).expect("parsing failed");
    assert_eq!(result, Real::binary(1.0, 2, -1));
    assert!((result.f32() - 0.5).abs() < EPSILON);
    assert_eq!(rem, &[0xff, 0xff]);
    // binary, base = 2 and scale factor
    let input = &hex!("09 03 94 ff 0d ff ff");
    let (rem, result) = Real::from_ber(input).expect("parsing failed");
    assert_eq!(result, Real::binary(26.0, 2, -3).with_enc_base(8));
    assert!((result.f32() - 3.25).abs() < EPSILON);
    assert_eq!(rem, &[0xff, 0xff]);
    // binary, base = 16
    let input = &hex!("09 03 a0 fe 01 ff ff");
    let (rem, result) = Real::from_ber(input).expect("parsing failed");
    assert_eq!(result, Real::binary(1.0, 2, -8).with_enc_base(16));
    assert!((result.f32() - 0.00390625).abs() < EPSILON);
    assert_eq!(rem, &[0xff, 0xff]);
    // binary, exponent = 0
    let input = &hex!("09 03 80 00 01 ff ff");
    let (rem, result) = Real::from_ber(input).expect("parsing failed");
    assert_eq!(result, Real::binary(1.0, 2, 0));
    assert!((result.f32() - 1.0).abs() < EPSILON);
    assert_eq!(rem, &[0xff, 0xff]);
    // 2 octets for exponent and negative exponent
    let input = &hex!("09 04 a1 ff 01 03 ff ff");
    let (rem, result) = Real::from_ber(input).expect("parsing failed");
    assert_eq!(result, Real::binary(3.0, 2, -1020).with_enc_base(16));
    let epsilon = 1e-311_f64;
    assert!((result.f64() - 2.67e-307).abs() < epsilon);
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
fn from_ber_real_f32() {
    const EPSILON: f32 = 0.00001;
    // binary, base = 2
    let input = &hex!("09 03 80 ff 01 ff ff");
    let (rem, result) = <f32>::from_ber(input).expect("parsing failed");
    assert!((result - 0.5).abs() < EPSILON);
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
fn from_ber_real_f64() {
    const EPSILON: f64 = 0.00001;
    // binary, base = 2
    let input = &hex!("09 03 80 ff 01 ff ff");
    let (rem, result) = <f64>::from_ber(input).expect("parsing failed");
    assert!((result - 0.5).abs() < EPSILON);
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
fn from_ber_real_special() {
    // 0
    let input = &hex!("09 00 ff ff");
    let (rem, result) = Real::from_ber(input).expect("parsing failed");
    assert_eq!(result, Real::from(0.0));
    assert_eq!(rem, &[0xff, 0xff]);
    // infinity
    let input = &hex!("09 01 40 ff ff");
    let (rem, result) = Real::from_ber(input).expect("parsing failed");
    assert_eq!(result, Real::Infinity);
    assert_eq!(rem, &[0xff, 0xff]);
    // negative infinity
    let input = &hex!("09 01 41 ff ff");
    let (rem, result) = Real::from_ber(input).expect("parsing failed");
    assert_eq!(result, Real::NegInfinity);
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
#[allow(clippy::approx_constant)]
fn from_ber_real_string() {
    // text representation, NR3
    let input = &hex!("09 07 03 33 31 34 45 2D 32 ff ff");
    let (rem, result) = Real::from_ber(input).expect("parsing failed");
    assert_eq!(result, Real::from(3.14));
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
#[allow(clippy::approx_constant)]
fn from_ber_real_string_primitive() {
    // text representation, NR3
    let input = &hex!("09 07 03 33 31 34 45 2D 32 ff ff");
    let (rem, result) = f32::from_ber(input).expect("parsing failed");
    assert!((result - 3.14).abs() < 0.01);
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
fn from_ber_sequence() {
    let input = &hex!("30 05 02 03 01 00 01");
    let (rem, result) = Sequence::from_ber(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
    //
    let (_, i) = Sequence::from_ber_and_then(input, Integer::from_ber).expect("parsing failed");
    assert_eq!(i.as_u32(), Ok(0x10001));
}

#[test]
fn from_ber_sequence_vec() {
    let input = &hex!("30 05 02 03 01 00 01");
    let (rem, result) = <Vec<u32>>::from_ber(input).expect("parsing failed");
    assert_eq!(&result, &[65537]);
    assert_eq!(rem, &[]);
}

#[test]
fn from_ber_sequence_of_vec() {
    let input = &hex!("30 05 02 03 01 00 01");
    let (rem, result) = <Sequence>::from_ber(input).expect("parsing failed");
    let v = result
        .ber_sequence_of::<u32, _>()
        .expect("ber_sequence_of failed");
    assert_eq!(rem, &[]);
    assert_eq!(&v, &[65537]);
}

#[test]
fn from_ber_iter_sequence() {
    let input = &hex!("30 0a 02 03 01 00 01 02 03 01 00 01");
    let (rem, result) = Sequence::from_ber(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
    let v = result
        .ber_iter()
        .collect::<Result<Vec<u32>>>()
        .expect("could not iterate sequence");
    assert_eq!(&v, &[65537, 65537]);
}

#[test]
fn from_ber_set() {
    let input = &hex!("31 05 02 03 01 00 01");
    let (rem, result) = Set::from_ber(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
    //
    let (_, i) = Set::from_ber_and_then(input, Integer::from_ber).expect("parsing failed");
    assert_eq!(i.as_u32(), Ok(0x10001));
}

#[test]
fn from_ber_set_of_vec() {
    let input = &hex!("31 05 02 03 01 00 01");
    let (rem, result) = <Set>::from_ber(input).expect("parsing failed");
    let v = result.ber_set_of::<u32, _>().expect("ber_set_of failed");
    assert_eq!(rem, &[]);
    assert_eq!(&v, &[65537]);
}

#[test]
fn from_ber_iter_set() {
    let input = &hex!("31 0a 02 03 01 00 01 02 03 01 00 01");
    let (rem, result) = Set::from_ber(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
    let v = result
        .ber_iter()
        .collect::<Result<Vec<u32>>>()
        .expect("could not iterate set");
    assert_eq!(&v, &[65537, 65537]);
}

#[test]
fn from_ber_tag_custom() {
    // canonical tag encoding
    let input = &hex!("8f 02 12 34");
    let (rem, any) = Any::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(any.tag(), Tag(15));
    // non-canonical tag encoding
    let input = &hex!("9f 0f 02 12 34");
    let (rem, any) = Any::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(any.tag(), Tag(15));
    assert_eq!(any.header.raw_tag(), Some(&[0x9f, 0x0f][..]));
}

#[test]
fn from_ber_tag_incomplete() {
    let input = &hex!("9f a2 a2");
    let res = Any::from_ber(input).expect_err("parsing should have failed");
    assert_eq!(res, nom::Err::Error(Error::InvalidTag));
}

#[test]
fn from_ber_tag_overflow() {
    let input = &hex!("9f a2 a2 a2 a2 a2 a2 22 01 00");
    let res = Any::from_ber(input).expect_err("parsing should have failed");
    assert_eq!(res, nom::Err::Error(Error::InvalidTag));
}

#[test]
fn from_ber_tag_long() {
    let input = &hex!("9f a2 22 01 00");
    let (rem, any) = Any::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(any.tag(), Tag(0x1122));
    assert_eq!(any.header.raw_tag(), Some(&[0x9f, 0xa2, 0x22][..]));
}

#[test]
fn from_ber_iter_sequence_incomplete() {
    let input = &hex!("30 09 02 03 01 00 01 02 03 01 00");
    let (rem, result) = Sequence::from_ber(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
    let mut iter = result.ber_iter::<u32, Error>();
    assert_eq!(iter.next(), Some(Ok(65537)));
    assert_eq!(iter.next(), Some(Err(Error::Incomplete(Needed::new(1)))));
    assert_eq!(iter.next(), None);
}

#[test]
fn from_ber_set_of() {
    let input = &hex!("31 05 02 03 01 00 01");
    let (rem, result) = SetOf::<u32>::from_ber(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &[0x10001]);
    assert_eq!(rem, &[]);
    // not constructed
    let input = &hex!("11 05 02 03 01 00 01");
    let err = SetOf::<u32>::from_ber(input).expect_err("should have failed");
    assert_eq!(err, Err::Error(Error::ConstructExpected));
}

#[test]
fn from_ber_tagged_explicit_optional() {
    let input = &hex!("a0 03 02 01 02");
    let (rem, result) =
        Option::<TaggedExplicit<u32, Error, 0>>::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert!(result.is_some());
    let tagged = result.unwrap();
    assert_eq!(tagged.tag(), Tag(0));
    assert_eq!(tagged.as_ref(), &2);
    let (rem, result) =
        Option::<TaggedExplicit<u32, Error, 1>>::from_ber(input).expect("parsing failed");
    assert!(result.is_none());
    assert_eq!(rem, input);

    // using OptTaggedExplicit
    let (rem, result) =
        OptTaggedExplicit::<u32, Error, 0>::from_ber(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert!(result.is_some());
    let tagged = result.unwrap();
    assert_eq!(tagged.tag(), Tag(0));
    assert_eq!(tagged.as_ref(), &2);

    // using OptTaggedParser
    let (rem, result) = OptTaggedParser::from(0)
        .parse_ber(input, |_, data| Integer::from_ber(data))
        .expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result, Some(Integer::from(2)));
}

/// Generic tests on methods, and coverage tests
#[test]
fn from_ber_tagged_optional_cov() {
    let p =
        |input| OptTaggedParser::from(1).parse_ber::<_, Error, _>(input, |_, data| Ok((data, ())));
    // empty input
    let input = &[];
    let (_, r) = p(input).expect("parsing failed");
    assert!(r.is_none());
    // wrong tag
    let input = &hex!("a0 03 02 01 02");
    let (_, r) = p(input).expect("parsing failed");
    assert!(r.is_none());
    // wrong class
    let input = &hex!("e1 03 02 01 02");
    let r = p(input);
    assert!(r.is_err());
}

#[test]
fn from_ber_universalstring() {
    let input = &hex!("1C 10 00000061 00000062 00000063 00000064");
    let (rem, result) = UniversalString::from_ber(input).expect("parsing failed");
    assert_eq!(result.as_ref(), "abcd");
    assert_eq!(rem, &[]);
}
