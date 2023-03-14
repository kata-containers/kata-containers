//! Test implementation for X.509
//!
//! This is mostly used to verify that required types and functions are implemented,
//! and that provided API is convenient.

use asn1_rs::{
    nom, Any, CheckDerConstraints, Choice, Error, FromBer, FromDer, Oid, ParseResult, Sequence,
    SetOf, Tag, Tagged,
};
use hex_literal::hex;
use nom::sequence::pair;
use std::convert::{TryFrom, TryInto};

const DN: &[u8] = &hex!(
    "
30 45 31 0b 30 09 06 03 55 04 06 13 02 46 52
31 13 30 11 06 03 55 04 08 0c 0a 53 6f 6d 65
2d 53 74 61 74 65 31 21 30 1f 06 03 55 04 0a
0c 18 49 6e 74 65 72 6e 65 74 20 57 69 64 67
69 74 73 20 50 74 79 20 4c 74 64
"
);

// Name ::= CHOICE { -- only one possibility for now --
//     rdnSequence  RDNSequence }

// RDNSequence ::= SEQUENCE OF RelativeDistinguishedName
#[derive(Debug)]
pub struct Name<'a> {
    pub rdn_sequence: Vec<RelativeDistinguishedName<'a>>,
}

impl<'a> FromDer<'a> for Name<'a> {
    fn from_der(bytes: &'a [u8]) -> ParseResult<'a, Self> {
        let (rem, rdn_sequence) = <Vec<RelativeDistinguishedName>>::from_der(bytes)?;
        let dn = Name { rdn_sequence };
        Ok((rem, dn))
    }
}

// RelativeDistinguishedName ::=
//     SET SIZE (1..MAX) OF AttributeTypeAndValue
#[derive(Debug)]
pub struct RelativeDistinguishedName<'a> {
    pub v: Vec<AttributeTypeAndValue<'a>>,
}

impl<'a> FromDer<'a> for RelativeDistinguishedName<'a> {
    fn from_der(bytes: &'a [u8]) -> ParseResult<'a, Self> {
        let (rem, set) = SetOf::<AttributeTypeAndValue>::from_der(bytes)?;
        let v: Vec<_> = set.into();
        if v.is_empty() {
            return Err(nom::Err::Failure(Error::InvalidLength));
        }
        Ok((rem, RelativeDistinguishedName { v }))
    }
}

// AttributeTypeAndValue ::= SEQUENCE {
//     type     AttributeType,
//     value    AttributeValue }
#[derive(Debug)]
pub struct AttributeTypeAndValue<'a> {
    pub oid: Oid<'a>,
    pub value: AttributeValue<'a>,
}

impl<'a> FromBer<'a> for AttributeTypeAndValue<'a> {
    fn from_ber(bytes: &'a [u8]) -> ParseResult<'a, Self> {
        let (rem, seq) = Sequence::from_der(bytes)?;
        let (_, (oid, value)) =
            seq.parse_into(|i| pair(Oid::from_der, AttributeValue::from_der)(i))?;
        let attr = AttributeTypeAndValue { oid, value };
        Ok((rem, attr))
    }
}

impl<'a> FromDer<'a> for AttributeTypeAndValue<'a> {
    fn from_der(bytes: &'a [u8]) -> ParseResult<'a, Self> {
        let (rem, seq) = Sequence::from_der(bytes)?;
        let (_, (oid, value)) =
            seq.parse_into(|i| pair(Oid::from_der, AttributeValue::from_der)(i))?;
        let attr = AttributeTypeAndValue { oid, value };
        Ok((rem, attr))
    }
}

impl<'a> CheckDerConstraints for AttributeTypeAndValue<'a> {
    fn check_constraints(any: &Any) -> asn1_rs::Result<()> {
        any.tag().assert_eq(Sequence::TAG)?;
        Ok(())
    }
}

// AttributeType ::= OBJECT IDENTIFIER

// AttributeValue ::= ANY -- DEFINED BY AttributeType
#[derive(Debug)]
pub enum AttributeValue<'a> {
    DirectoryString(DirectoryString),
    Other(Any<'a>),
}

impl<'a> FromDer<'a> for AttributeValue<'a> {
    fn from_der(bytes: &'a [u8]) -> ParseResult<'a, Self> {
        let (rem, any) = Any::from_der(bytes)?;
        let ds = if DirectoryString::can_decode(any.tag()) {
            AttributeValue::DirectoryString(any.try_into()?)
        } else {
            AttributeValue::Other(any)
        };
        Ok((rem, ds))
    }
}

// DirectoryString ::= CHOICE {
//         teletexString           TeletexString (SIZE (1..MAX)),
//         printableString         PrintableString (SIZE (1..MAX)),
//         universalString         UniversalString (SIZE (1..MAX)),
//         utf8String              UTF8String (SIZE (1..MAX)),
//         bmpString               BMPString (SIZE (1..MAX)) }
#[derive(Debug)]
pub enum DirectoryString {
    Printable(String),
    Utf8(String),
}

impl Choice for DirectoryString {
    fn can_decode(tag: Tag) -> bool {
        matches!(tag, Tag::PrintableString | Tag::Utf8String)
    }
}

impl<'a> TryFrom<Any<'a>> for DirectoryString {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Self, Self::Error> {
        match any.tag() {
            Tag::PrintableString => {
                let s = any.printablestring()?;
                Ok(DirectoryString::Printable(s.string()))
            }
            Tag::Utf8String => {
                let s = any.string()?;
                Ok(DirectoryString::Utf8(s))
            }
            _ => Err(Error::InvalidTag),
        }
    }
}

#[test]
fn x509_decode_dn() {
    let (rem, dn) = Name::from_der(DN).expect("parsing failed");
    assert!(rem.is_empty());
    // dbg!(&dn);
    assert_eq!(dn.rdn_sequence.len(), 3);
}
