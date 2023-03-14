use crate::error::{X509Error, X509Result};
use asn1_rs::FromDer;
use der_parser::der::*;
use der_parser::error::BerError;
use der_parser::{oid, oid::Oid};
use nom::{Err, IResult};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyUsage {
    pub flags: u16,
}

impl KeyUsage {
    pub fn digital_signature(&self) -> bool {
        self.flags & 1 == 1
    }
    pub fn non_repudiation(&self) -> bool {
        (self.flags >> 1) & 1u16 == 1
    }
    pub fn key_encipherment(&self) -> bool {
        (self.flags >> 2) & 1u16 == 1
    }
    pub fn data_encipherment(&self) -> bool {
        (self.flags >> 3) & 1u16 == 1
    }
    pub fn key_agreement(&self) -> bool {
        (self.flags >> 4) & 1u16 == 1
    }
    pub fn key_cert_sign(&self) -> bool {
        (self.flags >> 5) & 1u16 == 1
    }
    pub fn crl_sign(&self) -> bool {
        (self.flags >> 6) & 1u16 == 1
    }
    pub fn encipher_only(&self) -> bool {
        (self.flags >> 7) & 1u16 == 1
    }
    pub fn decipher_only(&self) -> bool {
        (self.flags >> 8) & 1u16 == 1
    }
}

// This list must have the same order as KeyUsage flags declaration (4.2.1.3)
const KEY_USAGE_FLAGS: &[&str] = &[
    "Digital Signature",
    "Non Repudiation",
    "Key Encipherment",
    "Data Encipherment",
    "Key Agreement",
    "Key Cert Sign",
    "CRL Sign",
    "Encipher Only",
    "Decipher Only",
];

impl fmt::Display for KeyUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = KEY_USAGE_FLAGS
            .iter()
            .enumerate()
            .fold(String::new(), |acc, (idx, s)| {
                if self.flags >> idx & 1 != 0 {
                    acc + s + ", "
                } else {
                    acc
                }
            });
        s.pop();
        s.pop();
        f.write_str(&s)
    }
}

impl<'a> FromDer<'a, X509Error> for KeyUsage {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parse_keyusage(i).map_err(Err::convert)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExtendedKeyUsage<'a> {
    pub any: bool,
    pub server_auth: bool,
    pub client_auth: bool,
    pub code_signing: bool,
    pub email_protection: bool,
    pub time_stamping: bool,
    pub ocsp_signing: bool,
    pub other: Vec<Oid<'a>>,
}

impl<'a> FromDer<'a, X509Error> for ExtendedKeyUsage<'a> {
    fn from_der(i: &'a [u8]) -> X509Result<'a, Self> {
        parse_extendedkeyusage(i).map_err(Err::convert)
    }
}

pub(crate) fn parse_keyusage(i: &[u8]) -> IResult<&[u8], KeyUsage, BerError> {
    let (rest, obj) = parse_der_bitstring(i)?;
    let bitstring = obj
        .content
        .as_bitstring()
        .or(Err(Err::Error(BerError::BerTypeError)))?;
    let flags = bitstring
        .data
        .iter()
        .rev()
        .fold(0, |acc, x| acc << 8 | (x.reverse_bits() as u16));
    Ok((rest, KeyUsage { flags }))
}

pub(crate) fn parse_extendedkeyusage(i: &[u8]) -> IResult<&[u8], ExtendedKeyUsage, BerError> {
    let (ret, seq) = <Vec<Oid>>::from_der(i)?;
    let mut seen = std::collections::HashSet::new();
    let mut eku = ExtendedKeyUsage {
        any: false,
        server_auth: false,
        client_auth: false,
        code_signing: false,
        email_protection: false,
        time_stamping: false,
        ocsp_signing: false,
        other: Vec::new(),
    };
    for oid in &seq {
        if !seen.insert(oid.clone()) {
            continue;
        }
        let asn1 = oid.as_bytes();
        if asn1 == oid!(raw 2.5.29.37.0) {
            eku.any = true;
        } else if asn1 == oid!(raw 1.3.6.1.5.5.7.3.1) {
            eku.server_auth = true;
        } else if asn1 == oid!(raw 1.3.6.1.5.5.7.3.2) {
            eku.client_auth = true;
        } else if asn1 == oid!(raw 1.3.6.1.5.5.7.3.3) {
            eku.code_signing = true;
        } else if asn1 == oid!(raw 1.3.6.1.5.5.7.3.4) {
            eku.email_protection = true;
        } else if asn1 == oid!(raw 1.3.6.1.5.5.7.3.8) {
            eku.time_stamping = true;
        } else if asn1 == oid!(raw 1.3.6.1.5.5.7.3.9) {
            eku.ocsp_signing = true;
        } else {
            eku.other.push(oid.clone());
        }
    }
    Ok((ret, eku))
}
