use asn1_rs::*;
use hex_literal::hex;
use nom::sequence::pair;
use nom::Needed;
use std::collections::BTreeSet;
use std::convert::TryInto;

#[test]
fn from_der_any() {
    let input = &hex!("02 01 02 ff ff");
    let (rem, result) = Any::from_der(input).expect("parsing failed");
    // dbg!(&result);
    assert_eq!(rem, &[0xff, 0xff]);
    assert_eq!(result.header.tag(), Tag::Integer);
}

#[test]
fn from_der_any_into() {
    let input = &hex!("02 01 02 ff ff");
    let (rem, result) = Any::from_der(input).expect("parsing failed");
    assert_eq!(rem, &[0xff, 0xff]);
    assert_eq!(result.header.tag(), Tag::Integer);
    let i: u32 = result.try_into().unwrap();
    assert_eq!(i, 2);
    //
    let (_, result) = Any::from_der(input).expect("parsing failed");
    let i = result.u32().unwrap();
    assert_eq!(i, 2);
}

#[test]
fn from_der_bitstring() {
    //
    // correct DER encoding
    //
    let input = &hex!("03 04 06 6e 5d c0");
    let (rem, result) = BitString::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.unused_bits, 6);
    assert_eq!(&result.data[..], &input[3..]);
    //
    // correct encoding, but wrong padding bits (not all set to 0)
    //
    let input = &hex!("03 04 06 6e 5d e0");
    let res = BitString::from_der(input);
    assert_eq!(
        res,
        Err(Err::Error(Error::DerConstraintFailed(
            DerConstraint::UnusedBitsNotZero
        )))
    );
    //
    // long form of length (invalid, < 127)
    //
    // let input = &hex!("03 81 04 06 6e 5d c0");
    // let res = BitString::from_der(input);
    // assert_eq!(res, Err(Err::Error(Error::DerConstraintFailed)));
}

#[test]
fn from_der_bitstring_constructed() {
    let bytes: &[u8] = &hex!("23 81 0c 03 03 00 0a 3b 03 05 04 5f 29 1c d0");
    assert_eq!(
        BitString::from_der(bytes),
        Err(Err::Error(Error::ConstructUnexpected))
    );
}

#[test]
fn from_der_bmpstring() {
    // taken from https://docs.microsoft.com/en-us/windows/win32/seccertenroll/about-bmpstring
    let input = &hex!("1e 08 00 55 00 73 00 65 00 72");
    let (rem, result) = BmpString::from_der(input).expect("parsing failed");
    assert_eq!(result.as_ref(), "User");
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_bool() {
    let input = &hex!("01 01 00");
    let (rem, result) = Boolean::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result, Boolean::FALSE);
    //
    let input = &hex!("01 01 ff");
    let (rem, result) = Boolean::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result, Boolean::TRUE);
    assert!(result.bool());
    //
    let input = &hex!("01 01 7f");
    let res = Boolean::from_der(input);
    assert_eq!(
        res,
        Err(Err::Error(Error::DerConstraintFailed(
            DerConstraint::InvalidBoolean
        )))
    );
    // bool type
    let input = &hex!("01 01 00");
    let (rem, result) = <bool>::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert!(!result);
}

#[test]
fn from_der_embedded_pdv() {
    let input = &hex!("2b 0d a0 07 81 05 2a 03 04 05 06 82 02 aa a0");
    let (rem, result) = EmbeddedPdv::from_der(input).expect("parsing failed");
    assert_eq!(rem, &[]);
    assert_eq!(
        result.identification,
        PdvIdentification::Syntax(Oid::from(&[1, 2, 3, 4, 5, 6]).unwrap())
    );
    assert_eq!(result.data_value, &[0xaa, 0xa0]);
}

#[test]
fn from_der_enumerated() {
    let input = &hex!("0a 01 02");
    let (rem, result) = Enumerated::from_der(input).expect("parsing failed");
    assert_eq!(rem, &[]);
    assert_eq!(result.0, 2);
}

#[test]
fn from_der_generalizedtime() {
    let input = &hex!("18 0F 32 30 30 32 31 32 31 33 31 34 32 39 32 33 5A FF");
    let (rem, result) = GeneralizedTime::from_der(input).expect("parsing failed");
    assert_eq!(rem, &[0xff]);
    #[cfg(feature = "datetime")]
    {
        use time::macros::datetime;
        let datetime = datetime! {2002-12-13 14:29:23 UTC};
        assert_eq!(result.utc_datetime(), Ok(datetime));
    }
    let _ = result;
    // local time with fractional seconds (should fail: no 'Z' at end)
    let input = b"\x18\x1019851106210627.3";
    let result = GeneralizedTime::from_der(input).expect_err("should not parse");
    assert_eq!(
        result,
        nom::Err::Error(Error::DerConstraintFailed(DerConstraint::MissingTimeZone))
    );
    // coordinated universal time with fractional seconds
    let input = b"\x18\x1119851106210627.3Z";
    let (rem, result) = GeneralizedTime::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.0.millisecond, Some(300));
    assert_eq!(result.0.tz, ASN1TimeZone::Z);
    #[cfg(feature = "datetime")]
    {
        use time::macros::datetime;
        let datetime = datetime! {1985-11-06 21:06:27.3 UTC};
        assert_eq!(result.utc_datetime(), Ok(datetime));
    }
    let _ = result.to_string();
    // local time with fractional seconds, and with local time 5 hours retarded in relation to coordinated universal time.
    // (should fail: no 'Z' at end)
    let input = b"\x18\x1519851106210627.3-0500";
    let result = GeneralizedTime::from_der(input).expect_err("should not parse");
    assert_eq!(
        result,
        nom::Err::Error(Error::DerConstraintFailed(DerConstraint::MissingTimeZone))
    );
}

#[test]
fn from_der_indefinite_length() {
    let bytes: &[u8] = &hex!("23 80 03 03 00 0a 3b 03 05 04 5f 29 1c d0 00 00");
    assert_eq!(
        BitString::from_der(bytes),
        Err(Err::Error(Error::DerConstraintFailed(
            DerConstraint::IndefiniteLength
        )))
    );
    let bytes: &[u8] = &hex!("02 80 01 00 00");
    assert!(Integer::from_der(bytes).is_err());
}

#[test]
fn from_der_int() {
    let input = &hex!("02 01 02 ff ff");
    let (rem, result) = u8::from_der(input).expect("parsing failed");
    assert_eq!(result, 2);
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
fn from_der_null() {
    let input = &hex!("05 00 ff ff");
    let (rem, result) = Null::from_der(input).expect("parsing failed");
    assert_eq!(result, Null {});
    assert_eq!(rem, &[0xff, 0xff]);
    // unit
    let (rem, _unit) = <()>::from_der(input).expect("parsing failed");
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
fn from_der_octetstring() {
    // coverage
    use std::borrow::Cow;
    let s = OctetString::new(b"1234");
    assert_eq!(s.as_cow().len(), 4);
    assert_eq!(s.into_cow(), Cow::Borrowed(b"1234"));
    //
    let input = &hex!("04 05 41 41 41 41 41");
    let (rem, result) = OctetString::from_der(input).expect("parsing failed");
    assert_eq!(result.as_ref(), b"AAAAA");
    assert_eq!(rem, &[]);
    //
    let (rem, result) = <&[u8]>::from_der(input).expect("parsing failed");
    assert_eq!(result, b"AAAAA");
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_octetstring_as_slice() {
    let input = &hex!("04 05 41 41 41 41 41");
    let (rem, result) = <&[u8]>::from_der(input).expect("parsing failed");
    assert_eq!(result, b"AAAAA");
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_oid() {
    let input = &hex!("06 09 2a 86 48 86 f7 0d 01 01 05");
    let (rem, result) = Oid::from_der(input).expect("parsing failed");
    let expected = Oid::from(&[1, 2, 840, 113_549, 1, 1, 5]).unwrap();
    assert_eq!(result, expected);
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_optional() {
    let input = &hex!("30 0a 0a 03 00 00 01 02 03 01 00 01");
    let (rem, result) = Sequence::from_der_and_then(input, |input| {
        let (i, obj0) = <Option<Enumerated>>::from_der(input)?;
        let (i, obj1) = u32::from_der(i)?;
        Ok((i, (obj0, obj1)))
    })
    .expect("parsing failed");
    let expected = (Some(Enumerated::new(1)), 65537);
    assert_eq!(result, expected);
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_real_f32() {
    const EPSILON: f32 = 0.00001;
    // binary, base = 2
    let input = &hex!("09 03 80 ff 01 ff ff");
    let (rem, result) = <f32>::from_der(input).expect("parsing failed");
    assert!((result - 0.5).abs() < EPSILON);
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
fn from_der_real_f64() {
    const EPSILON: f64 = 0.00001;
    // binary, base = 2
    let input = &hex!("09 03 80 ff 01 ff ff");
    let (rem, result) = <f64>::from_der(input).expect("parsing failed");
    assert!((result - 0.5).abs() < EPSILON);
    assert_eq!(rem, &[0xff, 0xff]);
}

#[test]
fn from_der_relative_oid() {
    let input = &hex!("0d 04 c2 7b 03 02");
    let (rem, result) = Oid::from_der_relative(input).expect("parsing failed");
    let expected = Oid::from_relative(&[8571, 3, 2]).unwrap();
    assert_eq!(result, expected);
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_sequence() {
    let input = &hex!("30 05 02 03 01 00 01");
    let (rem, result) = Sequence::from_der(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_sequence_vec() {
    let input = &hex!("30 05 02 03 01 00 01");
    let (rem, result) = <Vec<u32>>::from_der(input).expect("parsing failed");
    assert_eq!(&result, &[65537]);
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_iter_sequence_parse() {
    let input = &hex!("30 0a 02 03 01 00 01 02 03 01 00 01");
    let (rem, result) = Sequence::from_der(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
    let (rem, v) = result
        .parse(pair(u32::from_der, u32::from_der))
        .expect("parse sequence data");
    assert_eq!(v, (65537, 65537));
    assert!(rem.is_empty());
}
#[test]
fn from_der_iter_sequence() {
    let input = &hex!("30 0a 02 03 01 00 01 02 03 01 00 01");
    let (rem, result) = Sequence::from_der(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
    let v = result
        .der_iter()
        .collect::<Result<Vec<u32>>>()
        .expect("could not iterate sequence");
    assert_eq!(&v, &[65537, 65537]);
}

#[test]
fn from_der_iter_sequence_incomplete() {
    let input = &hex!("30 09 02 03 01 00 01 02 03 01 00");
    let (rem, result) = Sequence::from_der(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
    let mut iter = result.der_iter::<u32, Error>();
    assert_eq!(iter.next(), Some(Ok(65537)));
    assert_eq!(iter.next(), Some(Err(Error::Incomplete(Needed::new(1)))));
    assert_eq!(iter.next(), None);
}

#[test]
fn from_der_set() {
    let input = &hex!("31 05 02 03 01 00 01");
    let (rem, result) = Set::from_der(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
    //
    let (_, i) = Set::from_der_and_then(input, Integer::from_der).expect("parsing failed");
    assert_eq!(i.as_u32(), Ok(0x10001));
}

#[test]
fn from_der_set_btreeset() {
    let input = &hex!("31 05 02 03 01 00 01");
    let (rem, result) = <BTreeSet<u32>>::from_der(input).expect("parsing failed");
    assert!(result.contains(&65537));
    assert_eq!(result.len(), 1);
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_set_of_vec() {
    let input = &hex!("31 05 02 03 01 00 01");
    let (rem, result) = <Set>::from_der(input).expect("parsing failed");
    let v = result.der_set_of::<u32, _>().expect("ber_set_of failed");
    assert_eq!(rem, &[]);
    assert_eq!(&v, &[65537]);
}

#[test]
fn from_der_iter_set() {
    let input = &hex!("31 0a 02 03 01 00 01 02 03 01 00 01");
    let (rem, result) = Set::from_der(input).expect("parsing failed");
    assert_eq!(result.as_ref(), &input[2..]);
    assert_eq!(rem, &[]);
    let v = result
        .der_iter()
        .collect::<Result<Vec<u32>>>()
        .expect("could not iterate set");
    assert_eq!(&v, &[65537, 65537]);
}

#[test]
fn from_der_utctime() {
    let input = &hex!("17 0D 30 32 31 32 31 33 31 34 32 39 32 33 5A FF");
    let (rem, result) = UtcTime::from_der(input).expect("parsing failed");
    assert_eq!(rem, &[0xff]);
    #[cfg(feature = "datetime")]
    {
        use time::macros::datetime;
        let datetime = datetime! {2-12-13 14:29:23 UTC};

        assert_eq!(result.utc_datetime(), Ok(datetime));
    }
    let _ = result.to_string();
    //
    let input = &hex!("17 11 30 32 31 32 31 33 31 34 32 39 32 33 2b 30 33 30 30 FF");
    let (rem, result) = UtcTime::from_der(input).expect("parsing failed");
    assert_eq!(rem, &[0xff]);
    #[cfg(feature = "datetime")]
    {
        use time::macros::datetime;
        let datetime = datetime! {2-12-13 14:29:23 +03:00};

        assert_eq!(result.utc_datetime(), Ok(datetime));
    }
    let _ = result.to_string();
    //
    let input = &hex!("17 11 30 32 31 32 31 33 31 34 32 39 32 33 2d 30 33 30 30 FF");
    let (rem, result) = UtcTime::from_der(input).expect("parsing failed");
    assert_eq!(rem, &[0xff]);
    #[cfg(feature = "datetime")]
    {
        use time::macros::datetime;
        let datetime = datetime! {2-12-13 14:29:23 -03:00};

        assert_eq!(result.utc_datetime(), Ok(datetime));
    }
    let _ = result.to_string();
}

#[cfg(feature = "datetime")]
#[test]
fn utctime_adjusted_datetime() {
    use time::macros::datetime;

    let input = &hex!("17 0D 30 32 31 32 31 33 31 34 32 39 32 33 5A FF");
    let (_, result) = UtcTime::from_der(input).expect("parsing failed");

    assert_eq!(
        result.utc_adjusted_datetime(),
        Ok(datetime! {2002-12-13 14:29:23 UTC})
    );

    let input = &hex!("17 0D 35 30 31 32 31 33 31 34 32 39 32 33 5A FF");
    let (_, result) = UtcTime::from_der(input).expect("parsing failed");

    assert_eq!(
        result.utc_adjusted_datetime(),
        Ok(datetime! {1950-12-13 14:29:23 UTC})
    );
    let _ = result.to_string();
}

#[test]
fn from_der_utf8string() {
    let input = &hex!("0c 0a 53 6f 6d 65 2d 53 74 61 74 65");
    let (rem, result) = Utf8String::from_der(input).expect("parsing failed");
    assert_eq!(result.as_ref(), "Some-State");
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_utf8string_as_str() {
    let input = &hex!("0c 0a 53 6f 6d 65 2d 53 74 61 74 65");
    let (rem, result) = <&str>::from_der(input).expect("parsing failed");
    assert_eq!(result, "Some-State");
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_utf8string_as_string() {
    let input = &hex!("0c 0a 53 6f 6d 65 2d 53 74 61 74 65");
    let (rem, result) = String::from_der(input).expect("parsing failed");
    assert_eq!(&result, "Some-State");
    assert_eq!(rem, &[]);
}

#[test]
fn from_der_opt_int() {
    let input = &hex!("02 01 02 ff ff");
    let (rem, result) = <Option<u8>>::from_der(input).expect("parsing failed");
    assert_eq!(result, Some(2));
    assert_eq!(rem, &[0xff, 0xff]);
    // non-fatal error
    let (rem, result) = <Option<Ia5String>>::from_der(input).expect("parsing failed");
    assert!(result.is_none());
    assert_eq!(rem, input);
    // fatal error (correct tag, but incomplete)
    let input = &hex!("02 03 02 01");
    let res = <Option<u8>>::from_der(input);
    assert_eq!(res, Err(nom::Err::Incomplete(Needed::new(1))));
}

#[test]
fn from_der_tagged_explicit() {
    let input = &hex!("a0 03 02 01 02");
    let (rem, result) = TaggedExplicit::<u32, Error, 0>::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.tag(), Tag(0));
    assert_eq!(result.as_ref(), &2);
}

#[test]
fn from_der_tagged_explicit_with_class() {
    let input = &hex!("a0 03 02 01 02");
    // Note: the strange notation (using braces) is required by the compiler to use
    // a constant instead of the numeric value.
    let (rem, result) =
        TaggedValue::<u32, Error, Explicit, { Class::CONTEXT_SPECIFIC }, 0>::from_der(input)
            .expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.tag(), Tag(0));
    assert_eq!(result.as_ref(), &2);
}

#[test]
fn from_der_tagged_explicit_any_tag() {
    let input = &hex!("a0 03 02 01 02");
    let (rem, result) = TaggedParser::<Explicit, u32>::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.tag(), Tag(0));
    assert_eq!(result.as_ref(), &2);
}

#[test]
fn from_der_tagged_explicit_optional() {
    let input = &hex!("a0 03 02 01 02");
    let (rem, result) =
        Option::<TaggedExplicit<u32, Error, 0>>::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert!(result.is_some());
    let tagged = result.unwrap();
    assert_eq!(tagged.tag(), Tag(0));
    assert_eq!(tagged.as_ref(), &2);
    let (rem, result) =
        Option::<TaggedExplicit<u32, Error, 1>>::from_der(input).expect("parsing failed");
    assert!(result.is_none());
    assert_eq!(rem, input);

    // using OptTaggedExplicit
    let (rem, result) =
        OptTaggedExplicit::<u32, Error, 0>::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert!(result.is_some());
    let tagged = result.unwrap();
    assert_eq!(tagged.tag(), Tag(0));
    assert_eq!(tagged.as_ref(), &2);

    // using OptTaggedParser
    let (rem, result) = OptTaggedParser::from(0)
        .parse_der(input, |_, data| Integer::from_der(data))
        .expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result, Some(Integer::from(2)));
}

#[test]
fn from_der_tagged_implicit() {
    let input = &hex!("81 04 70 61 73 73");
    let (rem, result) = TaggedImplicit::<&str, Error, 1>::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.tag(), Tag(1));
    assert_eq!(result.as_ref(), &"pass");
}

#[test]
fn from_der_tagged_implicit_with_class() {
    let input = &hex!("81 04 70 61 73 73");
    // Note: the strange notation (using braces) is required by the compiler to use
    // a constant instead of the numeric value.
    let (rem, result) =
        TaggedValue::<&str, Error, Implicit, { Class::CONTEXT_SPECIFIC }, 1>::from_der(input)
            .expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.tag(), Tag(1));
    assert_eq!(result.as_ref(), &"pass");
}

#[test]
fn from_der_tagged_implicit_any_tag() {
    let input = &hex!("81 04 70 61 73 73");
    let (rem, result) = TaggedParser::<Implicit, &str>::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.tag(), Tag(1));
    assert_eq!(result.as_ref(), &"pass");
}

#[test]
fn from_der_tagged_implicit_optional() {
    let input = &hex!("81 04 70 61 73 73");
    let (rem, result) =
        Option::<TaggedImplicit<&str, Error, 1>>::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert!(result.is_some());
    let tagged = result.unwrap();
    assert_eq!(tagged.tag(), Tag(1));
    assert_eq!(tagged.as_ref(), &"pass");
    let (rem, result) =
        Option::<TaggedImplicit<&str, Error, 0>>::from_der(input).expect("parsing failed");
    assert!(result.is_none());
    assert_eq!(rem, input);

    // using OptTaggedExplicit
    let (rem, result) =
        OptTaggedImplicit::<&str, Error, 1>::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert!(result.is_some());
    let tagged = result.unwrap();
    assert_eq!(tagged.tag(), Tag(1));
    assert_eq!(tagged.as_ref(), &"pass");
}

#[test]
fn from_der_tagged_implicit_all() {
    let input = &hex!("81 04 70 61 73 73");
    let (rem, result) =
        TaggedParser::<Implicit, Ia5String>::from_der(input).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(result.tag(), Tag(1));
    assert_eq!(result.as_ref().as_ref(), "pass");

    // try the API verifying class and tag
    let _ = TaggedParser::<Implicit, Ia5String>::parse_der(Class::ContextSpecific, Tag(1), input)
        .expect("parsing failed");

    // test TagParser API
    let parser = TaggedParserBuilder::implicit()
        .with_class(Class::ContextSpecific)
        .with_tag(Tag(1))
        .der_parser::<Ia5String>();
    let _ = parser(input).expect("parsing failed");

    // try specifying the expected tag (correct tag)
    let _ = parse_der_tagged_implicit::<_, Ia5String, _>(1)(input).expect("parsing failed");
    // try specifying the expected tag (incorrect tag)
    let _ = parse_der_tagged_implicit::<_, Ia5String, _>(2)(input)
        .expect_err("parsing should have failed");
}

/// Generic tests on methods, and coverage tests
#[test]
fn from_der_tagged_optional_cov() {
    let p =
        |input| OptTaggedParser::from(1).parse_der::<_, Error, _>(input, |_, data| Ok((data, ())));
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

    let p = OptTaggedParser::from(Tag(1));
    let _ = format!("{:?}", p);
}
