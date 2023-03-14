// test_case seem to generate this warning - ignore it
#![allow(clippy::unused_unit)]

use der_parser::ber::*;
use der_parser::error::*;
use der_parser::oid::*;
use hex_literal::hex;
use nom::Err;
// use pretty_assertions::assert_eq;
use test_case::test_case;

#[cfg(feature = "bigint")]
use num_bigint::{BigInt, BigUint, Sign};

#[test_case(&hex!("01 01 00"), Some(false) ; "val false")]
#[test_case(&hex!("01 01 ff"), Some(true) ; "val true")]
#[test_case(&hex!("01 01 7f"), Some(true) ; "true not ff")]
#[test_case(&hex!("01 02 00 00"), None ; "invalid length")]
#[test_case(&hex!("01 01"), None ; "incomplete")]
fn tc_ber_bool(i: &[u8], out: Option<bool>) {
    let res = parse_ber_bool(i);
    if let Some(b) = out {
        let expected = BerObject::from_obj(BerObjectContent::Boolean(b));
        pretty_assertions::assert_eq!(res, Ok((&b""[..], expected)));
    } else {
        assert!(res.is_err());
    }
}

#[test]
fn test_ber_bool() {
    let empty = &b""[..];
    let b_true = BerObject::from_obj(BerObjectContent::Boolean(true));
    let b_false = BerObject::from_obj(BerObjectContent::Boolean(false));
    assert_eq!(parse_ber_bool(&[0x01, 0x01, 0x00]), Ok((empty, b_false)));
    assert_eq!(
        parse_ber_bool(&[0x01, 0x01, 0xff]),
        Ok((empty, b_true.clone()))
    );
    assert_eq!(parse_ber_bool(&[0x01, 0x01, 0x7f]), Ok((empty, b_true)));
    assert_eq!(
        parse_ber_bool(&[0x01, 0x02, 0x12, 0x34]),
        Err(Err::Error(BerError::InvalidLength))
    );
}

#[test]
fn test_seq_indefinite_length() {
    let data = hex!("30 80 04 03 56 78 90 00 00 02 01 01");
    let res = parse_ber(&data);
    assert_eq!(
        res,
        Ok((
            &data[9..],
            BerObject::from_seq(vec![BerObject::from_obj(BerObjectContent::OctetString(
                &data[4..=6]
            )),])
        ))
    );
    let res = parse_ber_sequence(&data);
    assert_eq!(
        res,
        Ok((
            &data[9..],
            BerObject::from_seq(vec![BerObject::from_obj(BerObjectContent::OctetString(
                &data[4..=6]
            )),])
        ))
    );
}

#[test]
fn test_ber_set_of() {
    let empty = &b""[..];
    let bytes = [
        0x31, 0x0a, 0x02, 0x03, 0x01, 0x00, 0x01, 0x02, 0x03, 0x01, 0x00, 0x00,
    ];
    let expected = BerObject::from_set(vec![
        BerObject::from_int_slice(b"\x01\x00\x01"),
        BerObject::from_int_slice(b"\x01\x00\x00"),
    ]);
    fn parser(i: &[u8]) -> BerResult {
        parse_ber_set_of(parse_ber_integer)(i)
    }
    assert_eq!(parser(&bytes), Ok((empty, expected)));
    // empty input should raise error (could not read set header)
    assert!(parser(&[]).is_err());
    // empty set is ok (returns empty vec)
    assert!(parser(&[0x31, 0x00]).is_ok());
}

#[test]
fn test_ber_set_of_v() {
    let empty = &b""[..];
    let bytes = [
        0x31, 0x0a, 0x02, 0x03, 0x01, 0x00, 0x01, 0x02, 0x03, 0x01, 0x00, 0x00,
    ];
    let expected = vec![
        BerObject::from_int_slice(b"\x01\x00\x01"),
        BerObject::from_int_slice(b"\x01\x00\x00"),
    ];
    fn parser(i: &[u8]) -> BerResult<Vec<BerObject>> {
        parse_ber_set_of_v(parse_ber_integer)(i)
    }
    assert_eq!(parser(&bytes), Ok((empty, expected)));
    // empty input should raise error (could not read set header)
    assert!(parser(&[]).is_err());
    // empty set is ok (returns empty vec)
    assert_eq!(parser(&[0x31, 0x00]), Ok((empty, vec![])));
}

#[test]
fn test_set_indefinite_length() {
    let data = hex!("31 80 04 03 56 78 90 00 00");
    let res = parse_ber(&data);
    assert_eq!(
        res,
        Ok((
            &data[9..],
            BerObject::from_set(vec![BerObject::from_obj(BerObjectContent::OctetString(
                &data[4..=6]
            )),])
        ))
    );
    let res = parse_ber_set(&data);
    assert_eq!(
        res,
        Ok((
            &data[9..],
            BerObject::from_set(vec![BerObject::from_obj(BerObjectContent::OctetString(
                &data[4..=6]
            )),])
        ))
    );
}

#[test]
fn test_ber_int() {
    let empty = &b""[..];
    let bytes = [0x02, 0x03, 0x01, 0x00, 0x01];
    let expected = BerObject::from_obj(BerObjectContent::Integer(b"\x01\x00\x01"));
    assert_eq!(parse_ber_integer(&bytes), Ok((empty, expected)));
}

#[test_case(&hex!("02 01 01"), Ok(1) ; "u32-1")]
#[test_case(&hex!("02 02 00 ff"), Ok(255) ; "u32-255")]
#[test_case(&hex!("02 02 01 23"), Ok(0x123) ; "u32-0x123")]
#[test_case(&hex!("02 04 01 23 45 67"), Ok(0x0123_4567) ; "u32-long-ok")]
#[test_case(&hex!("02 05 00 ff ff ff ff"), Ok(0xffff_ffff) ; "u32-long2-ok")]
#[test_case(&hex!("02 04 ff ff ff ff"), Err(BerError::IntegerNegative) ; "u32-long2-neg")]
#[test_case(&hex!("02 06 ff ff ff ff ff ff"), Err(BerError::IntegerNegative) ; "u32-long3-neg")]
#[test_case(&hex!("02 06 00 00 01 23 45 67"), Ok(0x0123_4567) ; "u32-long-leading-zeros-ok")]
#[test_case(&hex!("02 05 01 23 45 67 01"), Err(BerError::IntegerTooLarge) ; "u32 too large")]
#[test_case(&hex!("02 09 01 23 45 67 01 23 45 67 ab"), Err(BerError::IntegerTooLarge) ; "u32 too large 2")]
#[test_case(&hex!("03 03 01 00 01"), Err(BerError::unexpected_tag(Some(Tag(2)), Tag(3))) ; "invalid tag")]
fn tc_ber_u32(i: &[u8], out: Result<u32, BerError>) {
    let res = parse_ber_u32(i);
    match out {
        Ok(expected) => {
            pretty_assertions::assert_eq!(res, Ok((&b""[..], expected)));
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(Err::Error(e)));
        }
    }
}

#[test_case(&hex!("02 01 01"), Ok(1) ; "u64-1")]
#[test_case(&hex!("02 02 00 ff"), Ok(255) ; "u64-255")]
#[test_case(&hex!("02 02 01 23"), Ok(0x123) ; "u64-0x123")]
#[test_case(&hex!("02 08 01 23 45 67 01 23 45 67"), Ok(0x0123_4567_0123_4567) ; "u64-long-ok")]
#[test_case(&hex!("02 09 00 ff ff ff ff ff ff ff ff"), Ok(0xffff_ffff_ffff_ffff) ; "u64-long2-ok")]
#[test_case(&hex!("02 09 01 23 45 67 01 23 45 67 ab"), Err(BerError::IntegerTooLarge) ; "u64 too large")]
#[test_case(&hex!("03 03 01 00 01"), Err(BerError::unexpected_tag(Some(Tag(2)), Tag(3))) ; "invalid tag")]
fn tc_ber_u64(i: &[u8], out: Result<u64, BerError>) {
    let res = parse_ber_u64(i);
    match out {
        Ok(expected) => {
            pretty_assertions::assert_eq!(res, Ok((&b""[..], expected)));
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(Err::Error(e)));
        }
    }
}

#[test_case(&hex!("02 01 01"), Ok(1) ; "i64-1")]
#[test_case(&hex!("02 01 ff"), Ok(-1) ; "i64-neg1")]
#[test_case(&hex!("02 01 80"), Ok(-128) ; "i64-neg128")]
#[test_case(&hex!("02 02 ff 7f"), Ok(-129) ; "i64-neg129")]
#[test_case(&hex!("02 04 ff ff ff ff"), Ok(-1) ; "i64-long-neg")]
#[test_case(&hex!("03 03 01 00 01"), Err(BerError::unexpected_tag(Some(Tag(2)), Tag(3))) ; "invalid tag")]
fn tc_ber_i64(i: &[u8], out: Result<i64, BerError>) {
    let res = parse_ber_i64(i);
    match out {
        Ok(expected) => {
            pretty_assertions::assert_eq!(res, Ok((&b""[..], expected)));
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(Err::Error(e)));
        }
    }
}

#[cfg(feature = "bigint")]
#[test_case(&hex!("02 01 01"), Ok(BigInt::from(1)) ; "bigint-1")]
#[test_case(&hex!("02 02 00 ff"), Ok(BigInt::from(255)) ; "bigint-255")]
#[test_case(&hex!("02 01 ff"), Ok(BigInt::from(-1)) ; "bigint-neg1")]
#[test_case(&hex!("02 01 80"), Ok(BigInt::from(-128)) ; "bigint-neg128")]
#[test_case(&hex!("02 02 ff 7f"), Ok(BigInt::from(-129)) ; "bigint-neg129")]
#[test_case(&hex!("02 09 00 ff ff ff ff ff ff ff ff"), Ok(BigInt::from(0xffff_ffff_ffff_ffff_u64)) ; "bigint-long2-ok")]
#[test_case(&hex!("02 09 01 23 45 67 01 23 45 67 ab"), Ok(BigInt::from_bytes_be(Sign::Plus, &hex!("01 23 45 67 01 23 45 67 ab"))) ; "bigint-longer1")]
fn tc_ber_bigint(i: &[u8], out: Result<BigInt, BerError>) {
    let res = parse_ber_integer(i);
    match out {
        Ok(expected) => {
            let (rem, ber) = res.expect("parsing failed");
            assert!(rem.is_empty());
            let int = ber.as_bigint().expect("failed to convert to bigint");
            pretty_assertions::assert_eq!(int, expected);
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(Err::Error(e)));
        }
    }
}

#[cfg(feature = "bigint")]
#[test_case(&hex!("02 01 01"), Ok(BigUint::from(1_u8)) ; "biguint-1")]
#[test_case(&hex!("02 02 00 ff"), Ok(BigUint::from(255_u8)) ; "biguint-255")]
#[test_case(&hex!("02 01 ff"), Err(BerError::IntegerNegative) ; "biguint-neg1")]
#[test_case(&hex!("02 01 80"), Err(BerError::IntegerNegative) ; "biguint-neg128")]
#[test_case(&hex!("02 02 ff 7f"), Err(BerError::IntegerNegative) ; "biguint-neg129")]
#[test_case(&hex!("02 09 00 ff ff ff ff ff ff ff ff"), Ok(BigUint::from(0xffff_ffff_ffff_ffff_u64)) ; "biguint-long2-ok")]
#[test_case(&hex!("02 09 01 23 45 67 01 23 45 67 ab"), Ok(BigUint::from_bytes_be(&hex!("01 23 45 67 01 23 45 67 ab"))) ; "biguint-longer1")]
#[test_case(&hex!("03 03 01 00 01"), Err(BerError::unexpected_tag(Some(Tag(2)), Tag(3))) ; "invalid tag")]
fn tc_ber_biguint(i: &[u8], out: Result<BigUint, BerError>) {
    let res = parse_ber_integer(i).and_then(|(rem, ber)| Ok((rem, ber.as_biguint()?)));
    match out {
        Ok(expected) => {
            pretty_assertions::assert_eq!(res, Ok((&b""[..], expected)));
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(Err::Error(e)));
        }
    }
}

#[test_case(&hex!("02 01 01"), Ok(&[1]) ; "slice 1")]
#[test_case(&hex!("02 01 ff"), Ok(&[255]) ; "slice 2")]
#[test_case(&hex!("02 09 01 23 45 67 01 23 45 67 ab"), Ok(&hex!("01 23 45 67 01 23 45 67 ab")) ; "slice 3")]
#[test_case(&hex!("22 80 02 01 01 00 00"), Ok(&[2, 1, 1]) ; "constructed slice")]
#[test_case(&hex!("03 03 01 00 01"), Err(BerError::unexpected_tag(Some(Tag(2)), Tag(3))) ; "invalid tag")]
fn tc_ber_slice(i: &[u8], out: Result<&[u8], BerError>) {
    let res = parse_ber_slice(i, 2);
    match out {
        Ok(expected) => {
            pretty_assertions::assert_eq!(res, Ok((&b""[..], expected)));
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(Err::Error(e)));
        }
    }
}

#[test]
fn test_ber_bitstring_primitive() {
    let empty = &b""[..];
    let bytes = &[0x03, 0x07, 0x04, 0x0a, 0x3b, 0x5f, 0x29, 0x1c, 0xd0];
    let expected = BerObject::from_obj(BerObjectContent::BitString(
        4,
        BitStringObject { data: &bytes[3..] },
    ));
    assert_eq!(parse_ber_bitstring(bytes), Ok((empty, expected)));
    //
    // correct encoding, padding bits not all set to 0
    //
    let bytes = &[0x03, 0x04, 0x06, 0x6e, 0x5d, 0xe0];
    let expected = BerObject::from_obj(BerObjectContent::BitString(
        6,
        BitStringObject { data: &bytes[3..] },
    ));
    assert_eq!(parse_ber_bitstring(bytes), Ok((empty, expected)));
    //
    // long form of length
    //
    let bytes = &[0x03, 0x81, 0x04, 0x06, 0x6e, 0x5d, 0xc0];
    let expected = BerObject::from_obj(BerObjectContent::BitString(
        6,
        BitStringObject { data: &bytes[4..] },
    ));
    assert_eq!(parse_ber_bitstring(bytes), Ok((empty, expected)));
}

#[test]
fn test_ber_bitstring_constructed() {
    let bytes = &[
        0x23, 0x80, 0x03, 0x03, 0x00, 0x0a, 0x3b, 0x03, 0x05, 0x04, 0x5f, 0x29, 0x1c, 0xd0, 0x00,
        0x00,
    ];
    assert_eq!(
        parse_ber_bitstring(bytes),
        Err(Err::Error(BerError::Unsupported))
    ); // XXX valid encoding
}

#[test]
fn test_ber_octetstring_primitive() {
    let empty = &b""[..];
    let bytes = [0x04, 0x05, 0x41, 0x41, 0x41, 0x41, 0x41];
    let expected = BerObject::from_obj(BerObjectContent::OctetString(b"AAAAA"));
    assert_eq!(parse_ber_octetstring(&bytes), Ok((empty, expected)));
}

#[test]
fn test_ber_null() {
    let empty = &b""[..];
    let expected = BerObject::from_obj(BerObjectContent::Null);
    assert_eq!(parse_ber_null(&[0x05, 0x00]), Ok((empty, expected)));
}

#[test]
fn test_ber_oid() {
    let empty = &b""[..];
    let bytes = [
        0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x05,
    ];
    let expected = BerObject::from_obj(BerObjectContent::OID(
        Oid::from(&[1, 2, 840, 113_549, 1, 1, 5]).unwrap(),
    ));
    assert_eq!(parse_ber_oid(&bytes), Ok((empty, expected)));
}

#[test]
fn test_ber_enum() {
    let empty = &b""[..];
    let expected = BerObject::from_obj(BerObjectContent::Enum(2));
    assert_eq!(parse_ber_enum(&[0x0a, 0x01, 0x02]), Ok((empty, expected)));
}

#[test_case(&hex!("0c 04 31 32 33 34"), Ok("1234") ; "utf8: numeric")]
#[test_case(&hex!("0c 05 68 65 6c 6c 6f"), Ok("hello") ; "utf8: string")]
#[test_case(&hex!("0c 0b 68 65 6c 6c 6f 20 77 6f 72 6c 64"), Ok("hello world") ; "utf8: string with spaces")]
#[test_case(&hex!("0c 0b 68 65 6c 6c 6f 5c 77 6f 72 6c 64"), Ok("hello\\world") ; "utf8: string with backspace")]
#[test_case(&hex!("0c 0b 68 65 6c 6c 6f 2b 77 6f 72 6c 64"), Ok("hello+world") ; "utf8: string with plus")]
#[test_case(&hex!("0c 05 01 02 03 04 05"), Ok("\x01\x02\x03\x04\x05") ; "invalid chars")]
#[test_case(&hex!("0c 0e d0 bf d1 80 d0 b8 d0 b2 d0 b5 cc 81 d1 82"), Ok("приве́т") ; "utf8")]
#[test_case(&hex!("0c 04 00 9f 92 96"), Err(Err::Error(BerError::StringInvalidCharset)) ; "invalid utf8")]
fn tc_ber_utf8_string(i: &[u8], out: Result<&str, Err<BerError>>) {
    let res = parse_ber_utf8string(i);
    match out {
        Ok(b) => {
            let (rem, res) = res.expect("could not parse utf8string");
            assert!(rem.is_empty());
            let r = res.as_str().expect("could not convert to string");
            // let expected = BerObject::from_obj(BerObjectContent::Boolean(b));
            pretty_assertions::assert_eq!(r, b);
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(e));
        }
    }
}

#[test_case(&hex!("12 04 31 32 33 34"), Ok("1234") ; "numeric string")]
#[test_case(&hex!("12 05 68 65 6c 6c 6f"), Err(Err::Error(BerError::StringInvalidCharset)) ; "invalid chars")]
#[test_case(&hex!("12 05 01 02 03 04 05"), Err(Err::Error(BerError::StringInvalidCharset)) ; "invalid chars2")]
fn tc_ber_numeric_string(i: &[u8], out: Result<&str, Err<BerError>>) {
    let res = parse_ber_numericstring(i);
    match out {
        Ok(b) => {
            let (rem, res) = res.expect("could not parse numericstring");
            assert!(rem.is_empty());
            let r = res.as_str().expect("could not convert to string");
            // let expected = BerObject::from_obj(BerObjectContent::Boolean(b));
            pretty_assertions::assert_eq!(r, b);
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(e));
        }
    }
}

#[test_case(&hex!("13 04 31 32 33 34"), Ok("1234") ; "printable: numeric")]
#[test_case(&hex!("13 05 68 65 6c 6c 6f"), Ok("hello") ; "printable: string")]
#[test_case(&hex!("13 0b 68 65 6c 6c 6f 20 77 6f 72 6c 64"), Ok("hello world") ; "printable: string with spaces")]
#[test_case(&hex!("13 0b 68 65 6c 6c 6f 5c 77 6f 72 6c 64"), Err(Err::Error(BerError::StringInvalidCharset)) ; "printable: string with backspace")]
#[test_case(&hex!("13 0b 68 65 6c 6c 6f 2b 77 6f 72 6c 64"), Ok("hello+world") ; "printable: string with plus")]
#[test_case(&hex!("13 05 01 02 03 04 05"), Err(Err::Error(BerError::StringInvalidCharset)) ; "invalid chars")]
fn tc_ber_printable_string(i: &[u8], out: Result<&str, Err<BerError>>) {
    let res = parse_ber_printablestring(i);
    match out {
        Ok(b) => {
            let (rem, res) = res.expect("could not parse printablestring");
            assert!(rem.is_empty());
            let r = res.as_str().expect("could not convert to string");
            // let expected = BerObject::from_obj(BerObjectContent::Boolean(b));
            pretty_assertions::assert_eq!(r, b);
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(e));
        }
    }
}

#[test_case(&hex!("16 04 31 32 33 34"), Ok("1234") ; "ia5: numeric")]
#[test_case(&hex!("16 05 68 65 6c 6c 6f"), Ok("hello") ; "ia5: string")]
#[test_case(&hex!("16 0b 68 65 6c 6c 6f 20 77 6f 72 6c 64"), Ok("hello world") ; "ia5: string with spaces")]
#[test_case(&hex!("16 0b 68 65 6c 6c 6f 5c 77 6f 72 6c 64"), Ok("hello\\world") ; "ia5: string with backspace")]
#[test_case(&hex!("16 0b 68 65 6c 6c 6f 2b 77 6f 72 6c 64"), Ok("hello+world") ; "ia5: string with plus")]
#[test_case(&hex!("16 05 01 02 03 04 05"), Ok("\x01\x02\x03\x04\x05") ; "invalid chars")]
#[test_case(&hex!("16 0d d0 bf d1 80 d0 b8 d0 b2 d0 b5 cc 81 d1 82"), Err(Err::Error(BerError::StringInvalidCharset)) ; "utf8")]
fn tc_ber_ia5_string(i: &[u8], out: Result<&str, Err<BerError>>) {
    let res = parse_ber_ia5string(i);
    match out {
        Ok(b) => {
            let (rem, res) = res.expect("could not parse ia5string");
            assert!(rem.is_empty());
            let r = res.as_str().expect("could not convert to string");
            // let expected = BerObject::from_obj(BerObjectContent::Boolean(b));
            pretty_assertions::assert_eq!(r, b);
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(e));
        }
    }
}

#[test_case(&hex!("1a 04 31 32 33 34"), Ok("1234") ; "visible: numeric")]
#[test_case(&hex!("1a 05 68 65 6c 6c 6f"), Ok("hello") ; "visible: string")]
#[test_case(&hex!("1a 0b 68 65 6c 6c 6f 20 77 6f 72 6c 64"), Ok("hello world") ; "visible: string with spaces")]
#[test_case(&hex!("1a 0b 68 65 6c 6c 6f 5c 77 6f 72 6c 64"), Ok("hello\\world") ; "printable: string with backspace")]
#[test_case(&hex!("1a 0b 68 65 6c 6c 6f 2b 77 6f 72 6c 64"), Ok("hello+world") ; "printable: string with plus")]
#[test_case(&hex!("1a 05 01 02 03 04 05"), Err(Err::Error(BerError::StringInvalidCharset)) ; "invalid chars")]
fn tc_ber_visible_string(i: &[u8], out: Result<&str, Err<BerError>>) {
    let res = parse_ber_visiblestring(i);
    match out {
        Ok(b) => {
            let (rem, res) = res.expect("could not parse visiblestring");
            assert!(rem.is_empty());
            let r = res.as_str().expect("could not convert to string");
            // let expected = BerObject::from_obj(BerObjectContent::Boolean(b));
            pretty_assertions::assert_eq!(r, b);
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(e));
        }
    }
}

#[test]
fn test_ber_utf8string() {
    let empty = &b""[..];
    let bytes = [
        0x0c, 0x0a, 0x53, 0x6f, 0x6d, 0x65, 0x2d, 0x53, 0x74, 0x61, 0x74, 0x65,
    ];
    let expected = BerObject::from_obj(BerObjectContent::UTF8String("Some-State"));
    assert_eq!(parse_ber_utf8string(&bytes), Ok((empty, expected)));
}

#[test]
fn test_ber_relativeoid() {
    let empty = &b""[..];
    let bytes = hex!("0d 04 c2 7b 03 02");
    let expected = BerObject::from_obj(BerObjectContent::RelativeOID(
        Oid::from_relative(&[8571, 3, 2]).unwrap(),
    ));
    assert_eq!(parse_ber_relative_oid(&bytes), Ok((empty, expected)));
}

#[test]
fn test_ber_bmpstring() {
    let empty = &b""[..];
    let bytes = hex!("1e 08 00 55 00 73 00 65 00 72");
    let expected = BerObject::from_obj(BerObjectContent::BmpString("\x00U\x00s\x00e\x00r"));
    assert_eq!(parse_ber_bmpstring(&bytes), Ok((empty, expected)));
}

#[test]
fn test_ber_customtags() {
    let bytes = hex!("8f 02 12 34");
    let hdr = ber_read_element_header(&bytes)
        .expect("ber_read_element_header")
        .1;
    // println!("{:?}", hdr);
    let expected: &[u8] = &[0x8f];
    assert_eq!(hdr.raw_tag(), Some(expected));
    let bytes = hex!("9f 0f 02 12 34");
    let hdr = ber_read_element_header(&bytes)
        .expect("ber_read_element_header")
        .1;
    // println!("{:?}", hdr);
    let expected: &[u8] = &[0x9f, 0x0f];
    assert_eq!(hdr.raw_tag(), Some(expected));
}

#[test]
fn test_ber_indefinite() {
    let bytes = hex!("30 80 02 03 01 00 01 00 00");
    let (rem, val) = parse_ber_container::<_, _, BerError>(|i, _| {
        assert!(!i.is_empty());
        let (_, val) = parse_ber_u32(i)?;
        Ok((i, val))
    })(&bytes)
    .unwrap();
    assert!(rem.is_empty());
    assert_eq!(val, 0x10001);
}

#[test]
fn test_ber_indefinite_recursion() {
    let data = &hex!(
        "
        24 80 24 80 24 80 24 80 24 80 24 80 24 80 24 80
        24 80 24 80 24 80 24 80 24 80 24 80 24 80 24 80
        24 80 24 80 24 80 24 80 24 80 24 80 24 80 24 80
        24 80 24 80 24 80 24 80 24 80 24 80 24 80 24 80
        24 80 24 80 24 80 24 80 24 80 24 80 24 80 24 80
        24 80 24 80 24 80 24 80 24 80 24 80 24 80 24 80
        24 80 24 80 24 80 24 80 24 80 24 80 24 80 24 80
        24 80 24 80 24 80 24 80 24 80 24 80 24 80 24 80
        24 80 24 80 24 80 24 80 24 80 24 80 24 80 24 80 00 00"
    );
    let _ = parse_ber_container::<_, _, BerError>(|i, _| Ok((i, ())))(data)
        .expect_err("max parsing depth overflow");
}

#[test]
fn test_parse_ber_content() {
    let bytes = &hex!("02 03 01 00 01");
    let (i, header) = ber_read_element_header(bytes).expect("parsing failed");
    let (rem, content) =
        parse_ber_content(header.tag())(i, &header, MAX_RECURSION).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(header.tag(), Tag::Integer);
    assert_eq!(content.as_u32(), Ok(0x10001));
}

#[test]
fn test_parse_ber_content2() {
    let bytes = &hex!("02 03 01 00 01");
    let (i, header) = ber_read_element_header(bytes).expect("parsing failed");
    let tag = header.tag();
    let (rem, content) = parse_ber_content2(tag)(i, header, MAX_RECURSION).expect("parsing failed");
    assert!(rem.is_empty());
    assert_eq!(tag, Tag::Integer);
    assert_eq!(content.as_u32(), Ok(0x10001));
}

#[test]
fn parse_ber_private() {
    let bytes = &hex!("c0 03 01 00 01");
    let (rem, res) = parse_ber(bytes).expect("parsing failed");
    assert!(rem.is_empty());
    assert!(matches!(res.content, BerObjectContent::Unknown(_)));
}
