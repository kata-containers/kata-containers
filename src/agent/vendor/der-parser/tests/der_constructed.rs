// test_case seem to generate this warning - ignore it
#![allow(clippy::unused_unit)]

use der_parser::ber::Tag;
use der_parser::der::*;
use der_parser::error::*;
use hex_literal::hex;
use nom::combinator::map;
use nom::error::ErrorKind;
use nom::sequence::tuple;
use nom::{Err, Needed};
use test_case::test_case;

#[test_case(&hex!("a2 05 02 03 01 00 01"), Ok(0x10001) ; "tag ok")]
#[test_case(&hex!("a2 80 02 03 01 00 01 00 00"), Err(BerError::DerConstraintFailed(DerConstraint::IndefiniteLength)) ; "indefinite tag ok")]
#[test_case(&hex!("a3 05 02 03 01 00 01"), Err(BerError::unexpected_tag(Some(Tag(2)), Tag(3))) ; "invalid tag")]
#[test_case(&hex!("22 05 02 03 01 00 01"), Err(BerError::unexpected_class(None, Class::Universal)) ; "invalid class")]
#[test_case(&hex!("82 05 02 03 01 00 01"), Err(BerError::ConstructExpected) ; "construct expected")]
fn tc_der_tagged_explicit_g(i: &[u8], out: Result<u32, BerError>) {
    fn parse_int_explicit(i: &[u8]) -> BerResult<u32> {
        parse_der_tagged_explicit_g(2, move |content, _hdr| {
            let (rem, obj) = parse_der_integer(content)?;
            let value = obj.as_u32()?;
            Ok((rem, value))
        })(i)
    }
    let res = parse_int_explicit(i);
    match out {
        Ok(expected) => {
            pretty_assertions::assert_eq!(res, Ok((&b""[..], expected)));
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(Err::Error(e)));
        }
    }
}

#[test_case(&hex!("82 03 01 00 01"), Ok(0x10001) ; "tag ok")]
#[test_case(&hex!("83 03 01 00 01"), Err(BerError::unexpected_tag(Some(Tag(2)), Tag(3))) ; "invalid tag")]
fn tc_der_tagged_implicit_g(i: &[u8], out: Result<u32, BerError>) {
    fn parse_int_implicit(i: &[u8]) -> BerResult<u32> {
        parse_der_tagged_implicit_g(2, |content, hdr, depth| {
            let (rem, obj) = parse_der_content(Tag::Integer)(content, &hdr, depth)?;
            let value = obj.as_u32()?;
            Ok((rem, value))
        })(i)
    }
    let res = parse_int_implicit(i);
    match out {
        Ok(expected) => {
            pretty_assertions::assert_eq!(res, Ok((&b""[..], expected)));
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(Err::Error(e)));
        }
    }
}

#[test_case(&hex!("30 00"), Ok(&[]) ; "empty seq")]
#[test_case(&hex!("30 0a 02 03 01 00 01 02 03 01 00 00"), Ok(&[0x10001, 0x10000]) ; "seq ok")]
#[test_case(&hex!("30 07 02 03 01 00 01 02 03 01"), Err(BerError::NomError(ErrorKind::Eof)) ; "incomplete")]
#[test_case(&hex!("31 0a 02 03 01 00 01 02 03 01 00 00"), Err(BerError::unexpected_tag(Some(Tag::Sequence), Tag::Set)) ; "invalid tag")]
#[test_case(&hex!("30 80 02 03 01 00 01 00 00"), Err(BerError::DerConstraintFailed(DerConstraint::IndefiniteLength)) ; "indefinite seq ok")]
fn tc_der_seq_of(i: &[u8], out: Result<&[u32], BerError>) {
    fn parser(i: &[u8]) -> BerResult {
        parse_der_sequence_of(parse_der_integer)(i)
    }
    let res = parser(i);
    match out {
        Ok(l) => {
            let (rem, res) = res.expect("could not parse sequence of");
            assert!(rem.is_empty());
            if let DerObjectContent::Sequence(res) = res.content {
                pretty_assertions::assert_eq!(res.len(), l.len());
                for (a, b) in res.iter().zip(l.iter()) {
                    pretty_assertions::assert_eq!(a.as_u32().unwrap(), *b);
                }
            } else {
                panic!("wrong type for parsed object");
            }
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(Err::Error(e)));
        }
    }
}

#[test_case(&hex!("30 0a 02 03 01 00 01 02 03 01 00 00"), Ok(&[0x10001, 0x10000]) ; "seq ok")]
#[test_case(&hex!("30 07 02 03 01 00 01 02 01"), Err(Err::Incomplete(Needed::new(1))) ; "incomplete")]
#[test_case(&hex!("31 0a 02 03 01 00 01 02 03 01 00 00"), Err(Err::Error(BerError::unexpected_tag(Some(Tag::Sequence), Tag::Set))) ; "invalid tag")]
#[test_case(&hex!("30 80 02 03 01 00 01 00 00"), Err(Err::Error(BerError::DerConstraintFailed(DerConstraint::IndefiniteLength))) ; "indefinite seq ok")]
fn tc_der_seq_defined(i: &[u8], out: Result<&[u32], Err<BerError>>) {
    fn parser(i: &[u8]) -> BerResult<DerObject> {
        parse_der_sequence_defined(map(
            tuple((parse_der_integer, parse_der_integer)),
            |(a, b)| vec![a, b],
        ))(i)
    }
    let res = parser(i);
    match out {
        Ok(l) => {
            let (rem, res) = res.expect("could not parse sequence");
            assert!(rem.is_empty());
            if let DerObjectContent::Sequence(res) = res.content {
                pretty_assertions::assert_eq!(res.len(), l.len());
                for (a, b) in res.iter().zip(l.iter()) {
                    pretty_assertions::assert_eq!(a.as_u32().unwrap(), *b);
                }
            } else {
                panic!("wrong type for parsed object");
            }
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(e));
        }
    }
}

#[test_case(&hex!("31 00"), Ok(&[]) ; "empty set")]
#[test_case(&hex!("31 0a 02 03 01 00 01 02 03 01 00 00"), Ok(&[0x10001, 0x10000]) ; "set ok")]
#[test_case(&hex!("31 07 02 03 01 00 01 02 03 01"), Err(BerError::NomError(ErrorKind::Eof)) ; "incomplete")]
#[test_case(&hex!("30 0a 02 03 01 00 01 02 03 01 00 00"), Err(BerError::unexpected_tag(Some(Tag::Set), Tag::Sequence)) ; "invalid tag")]
#[test_case(&hex!("31 80 02 03 01 00 01 00 00"), Err(BerError::DerConstraintFailed(DerConstraint::IndefiniteLength)) ; "indefinite set ok")]
fn tc_der_set_of(i: &[u8], out: Result<&[u32], BerError>) {
    fn parser(i: &[u8]) -> BerResult {
        parse_der_set_of(parse_der_integer)(i)
    }
    let res = parser(i);
    match out {
        Ok(l) => {
            let (rem, res) = res.expect("could not parse set of");
            assert!(rem.is_empty());
            if let DerObjectContent::Set(res) = res.content {
                pretty_assertions::assert_eq!(res.len(), l.len());
                for (a, b) in res.iter().zip(l.iter()) {
                    pretty_assertions::assert_eq!(a.as_u32().unwrap(), *b);
                }
            } else {
                panic!("wrong type for parsed object");
            }
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(Err::Error(e)));
        }
    }
}

#[test_case(&hex!("31 0a 02 03 01 00 01 02 03 01 00 00"), Ok(&[0x10001, 0x10000]) ; "set ok")]
#[test_case(&hex!("31 07 02 03 01 00 01 02 01"), Err(Err::Incomplete(Needed::new(1))) ; "incomplete")]
#[test_case(&hex!("30 0a 02 03 01 00 01 02 03 01 00 00"), Err(Err::Error(BerError::unexpected_tag(Some(Tag::Set), Tag::Sequence))) ; "invalid tag")]
#[test_case(&hex!("31 80 02 03 01 00 01 00 00"), Err(Err::Error(BerError::DerConstraintFailed(DerConstraint::IndefiniteLength))) ; "indefinite set ok")]
fn tc_der_set_defined(i: &[u8], out: Result<&[u32], Err<BerError>>) {
    fn parser(i: &[u8]) -> BerResult<DerObject> {
        parse_der_set_defined(map(
            tuple((parse_der_integer, parse_der_integer)),
            |(a, b)| vec![a, b],
        ))(i)
    }
    let res = parser(i);
    match out {
        Ok(l) => {
            let (rem, res) = res.expect("could not parse set");
            assert!(rem.is_empty());
            if let DerObjectContent::Set(res) = res.content {
                pretty_assertions::assert_eq!(res.len(), l.len());
                for (a, b) in res.iter().zip(l.iter()) {
                    pretty_assertions::assert_eq!(a.as_u32().unwrap(), *b);
                }
            } else {
                panic!("wrong type for parsed object");
            }
        }
        Err(e) => {
            pretty_assertions::assert_eq!(res, Err(e));
        }
    }
}
