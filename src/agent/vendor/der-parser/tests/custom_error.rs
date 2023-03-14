//! This test file ensures the functions to parse containers like sequences and sets
//! work correctly with custom errors.

use der_parser::ber::{parse_ber_sequence_of_v, parse_ber_u32};
use der_parser::error::BerError;
use nom::error::{ErrorKind, ParseError};
use nom::{Err, IResult};

#[derive(Debug)]
pub enum MyError<'a> {
    Variant1,
    Variant2,
    BerError(BerError),
    NomError(&'a [u8], ErrorKind),
}

impl<'a> ParseError<&'a [u8]> for MyError<'a> {
    fn from_error_kind(input: &'a [u8], kind: ErrorKind) -> Self {
        MyError::NomError(input, kind)
    }

    fn append(_input: &'a [u8], _kind: ErrorKind, other: Self) -> Self {
        other
    }
}

impl<'a> From<BerError> for MyError<'a> {
    fn from(e: BerError) -> Self {
        MyError::BerError(e)
    }
}

#[test]
fn parse_sequence_of_v_custom_errors() {
    fn parse_element(i: &[u8]) -> IResult<&[u8], u32, MyError> {
        // incomplete must *NOT* be mapped, or parse_ber_sequence_of_v cannot detect end of
        // sequence
        match parse_ber_u32(i) {
            Ok(x) => Ok(x),
            Err(Err::Incomplete(e)) => Err(Err::Incomplete(e)),
            _ => Err(Err::Error(MyError::Variant1)),
        }
    }

    let bytes = [
        0x30, 0x0a, 0x02, 0x03, 0x01, 0x00, 0x01, 0x02, 0x03, 0x01, 0x00, 0x00,
    ];

    let (rem, v) =
        parse_ber_sequence_of_v(parse_element)(&bytes).expect("Could not parse SEQUENCE OF");
    assert!(rem.is_empty());
    assert_eq!(&v, &[65537, 65536]);
}
