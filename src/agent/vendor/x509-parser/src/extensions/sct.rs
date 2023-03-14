//! Certificate transparency [RFC6962](https://datatracker.ietf.org/doc/html/rfc6962)
//!
//! Code borrowed from tls-parser crate (file <https://github.com/rusticata/tls-parser/blob/tls-parser-0.11.0/src/certificate_transparency.rs>)

use std::convert::TryInto;

use asn1_rs::FromDer;
use der_parser::error::BerError;
use nom::bytes::streaming::take;
use nom::combinator::{complete, map_parser};
use nom::multi::{length_data, many1};
use nom::number::streaming::{be_u16, be_u64, be_u8};
use nom::IResult;

#[derive(Clone, Debug, PartialEq)]
pub struct SignedCertificateTimestamp<'a> {
    pub version: CtVersion,
    pub id: CtLogID<'a>,
    pub timestamp: u64,
    pub extensions: CtExtensions<'a>,
    pub signature: DigitallySigned<'a>,
}

/// Certificate Transparency Version as defined in
/// [RFC6962 Section 3.2](https://datatracker.ietf.org/doc/html/rfc6962#section-3.2)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CtVersion(pub u8);

impl CtVersion {
    pub const V1: CtVersion = CtVersion(0);
}

/// LogID as defined in
/// [RFC6962 Section 3.2](https://datatracker.ietf.org/doc/html/rfc6962#section-3.2)
#[derive(Clone, Debug, PartialEq)]
pub struct CtLogID<'a> {
    pub key_id: &'a [u8; 32],
}

/// CtExtensions as defined in
/// [RFC6962 Section 3.2](https://datatracker.ietf.org/doc/html/rfc6962#section-3.2)
#[derive(Clone, Debug, PartialEq)]
pub struct CtExtensions<'a>(pub &'a [u8]);

#[derive(Clone, Debug, PartialEq)]
pub struct DigitallySigned<'a> {
    pub hash_alg_id: u8,
    pub sign_alg_id: u8,
    pub data: &'a [u8],
}

/// Parses a list of Signed Certificate Timestamp entries
pub fn parse_ct_signed_certificate_timestamp_list(
    i: &[u8],
) -> IResult<&[u8], Vec<SignedCertificateTimestamp>, BerError> {
    // use nom::HexDisplay;
    // eprintln!("{}", i.to_hex(16));
    let (rem, b) = <&[u8]>::from_der(i)?;
    let (b, sct_len) = be_u16(b)?;
    let (_, sct_list) = map_parser(
        take(sct_len as usize),
        many1(complete(parse_ct_signed_certificate_timestamp)),
    )(b)?;
    Ok((rem, sct_list))
}

/// Parses as single Signed Certificate Timestamp entry
pub fn parse_ct_signed_certificate_timestamp(
    i: &[u8],
) -> IResult<&[u8], SignedCertificateTimestamp, BerError> {
    map_parser(
        length_data(be_u16),
        parse_ct_signed_certificate_timestamp_content,
    )(i)
}

pub(crate) fn parse_ct_signed_certificate_timestamp_content<'a>(
    i: &'a [u8],
) -> IResult<&'a [u8], SignedCertificateTimestamp, BerError> {
    let (i, version) = be_u8(i)?;
    let (i, id) = parse_log_id(i)?;
    let (i, timestamp) = be_u64(i)?;
    let (i, extensions) = parse_ct_extensions(i)?;
    let (i, signature) = parse_digitally_signed(i)?;
    let sct = SignedCertificateTimestamp {
        version: CtVersion(version),
        id,
        timestamp,
        extensions,
        signature,
    };
    Ok((i, sct))
}

// Safety: cannot fail, take() returns exactly 32 bytes
fn parse_log_id(i: &[u8]) -> IResult<&[u8], CtLogID, BerError> {
    let (i, key_id) = take(32usize)(i)?;
    Ok((
        i,
        CtLogID {
            key_id: key_id
                .try_into()
                .expect("take(32) is in sync with key_id size"),
        },
    ))
}

fn parse_ct_extensions(i: &[u8]) -> IResult<&[u8], CtExtensions, BerError> {
    let (i, ext_len) = be_u16(i)?;
    let (i, ext_data) = take(ext_len as usize)(i)?;
    Ok((i, CtExtensions(ext_data)))
}

fn parse_digitally_signed(i: &[u8]) -> IResult<&[u8], DigitallySigned, BerError> {
    let (i, hash_alg_id) = be_u8(i)?;
    let (i, sign_alg_id) = be_u8(i)?;
    let (i, data) = length_data(be_u16)(i)?;
    let signed = DigitallySigned {
        hash_alg_id,
        sign_alg_id,
        data,
    };
    Ok((i, signed))
}
