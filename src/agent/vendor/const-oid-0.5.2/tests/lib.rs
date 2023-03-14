//! `const-oid` crate tests

// TODO(tarcieri): test full set of OID encoding constraints specified here:
// <https://misc.daniel-marschall.de/asn.1/oid_facts.html>

use const_oid::ObjectIdentifier;
use hex_literal::hex;
use std::string::ToString;

/// Example OID value with a root arc of `1`
const EXAMPLE_OID_1: ObjectIdentifier = ObjectIdentifier::new("1.2.840.10045.2.1");

/// Example OID value with a root arc of `2`
const EXAMPLE_OID_2: ObjectIdentifier = ObjectIdentifier::new("2.16.840.1.101.3.4.1.42");

/// Example OID 1 encoded as ASN.1 BER/DER
const EXAMPLE_OID_1_BER: &[u8] = &hex!("2A8648CE3D0201");

/// Example OID 2 encoded as ASN.1 BER/DER
const EXAMPLE_OID_2_BER: &[u8] = &hex!("60864801650304012A");

/// Example OID 1 as a string
const EXAMPLE_OID_1_STRING: &str = "1.2.840.10045.2.1";

/// Example OID 2 as a string
const EXAMPLE_OID_2_STRING: &str = "2.16.840.1.101.3.4.1.42";

#[test]
fn display() {
    assert_eq!(EXAMPLE_OID_1.to_string(), EXAMPLE_OID_1_STRING);
    assert_eq!(EXAMPLE_OID_2.to_string(), EXAMPLE_OID_2_STRING);
}

#[test]
fn from_bytes() {
    let oid1 = ObjectIdentifier::from_bytes(EXAMPLE_OID_1_BER).unwrap();
    assert_eq!(oid1.arc(0).unwrap(), 1);
    assert_eq!(oid1.arc(1).unwrap(), 2);
    assert_eq!(oid1, EXAMPLE_OID_1);

    let oid2 = ObjectIdentifier::from_bytes(EXAMPLE_OID_2_BER).unwrap();
    assert_eq!(oid2.arc(0).unwrap(), 2);
    assert_eq!(oid2.arc(1).unwrap(), 16);
    assert_eq!(oid2, EXAMPLE_OID_2);

    // Empty
    assert!(ObjectIdentifier::from_bytes(&[]).is_err());

    // Truncated
    assert!(ObjectIdentifier::from_bytes(&[42]).is_err());
    assert!(ObjectIdentifier::from_bytes(&[42, 134]).is_err());
}

#[test]
fn from_str() {
    let oid1 = EXAMPLE_OID_1_STRING.parse::<ObjectIdentifier>().unwrap();
    assert_eq!(oid1.arc(0).unwrap(), 1);
    assert_eq!(oid1.arc(1).unwrap(), 2);
    assert_eq!(oid1, EXAMPLE_OID_1);

    let oid2 = EXAMPLE_OID_2_STRING.parse::<ObjectIdentifier>().unwrap();
    assert_eq!(oid2.arc(0).unwrap(), 2);
    assert_eq!(oid2.arc(1).unwrap(), 16);
    assert_eq!(oid2, EXAMPLE_OID_2);

    // Too short
    assert!("1.2".parse::<ObjectIdentifier>().is_err());

    // Truncated
    assert!("1.2.840.10045.2.".parse::<ObjectIdentifier>().is_err());

    // Invalid first arc
    assert!("3.2.840.10045.2.1".parse::<ObjectIdentifier>().is_err());

    // Invalid second arc
    assert!("1.40.840.10045.2.1".parse::<ObjectIdentifier>().is_err());
}

#[test]
fn try_from_u32_slice() {
    let oid1 = ObjectIdentifier::from_arcs(&[1, 2, 840, 10045, 2, 1]).unwrap();
    assert_eq!(oid1.arc(0).unwrap(), 1);
    assert_eq!(oid1.arc(1).unwrap(), 2);
    assert_eq!(EXAMPLE_OID_1, oid1);

    let oid2 = ObjectIdentifier::from_arcs(&[2, 16, 840, 1, 101, 3, 4, 1, 42]).unwrap();
    assert_eq!(oid2.arc(0).unwrap(), 2);
    assert_eq!(oid2.arc(1).unwrap(), 16);
    assert_eq!(EXAMPLE_OID_2, oid2);

    // Too short
    assert!(ObjectIdentifier::from_arcs(&[1, 2]).is_err());

    // Invalid first arc
    assert!(ObjectIdentifier::from_arcs(&[3, 2, 840, 10045, 3, 1, 7]).is_err());

    // Invalid second arc
    assert!(ObjectIdentifier::from_arcs(&[1, 40, 840, 10045, 3, 1, 7]).is_err());
}

#[test]
fn as_bytes() {
    assert_eq!(EXAMPLE_OID_1.as_bytes(), EXAMPLE_OID_1_BER);
    assert_eq!(EXAMPLE_OID_2.as_bytes(), EXAMPLE_OID_2_BER);
}

#[test]
#[should_panic]
fn parse_empty() {
    ObjectIdentifier::new("");
}

#[test]
#[should_panic]
fn parse_too_short() {
    ObjectIdentifier::new("1.2");
}

#[test]
#[should_panic]
fn parse_invalid_first_arc() {
    ObjectIdentifier::new("3.2.840.10045.3.1.7");
}

#[test]
#[should_panic]
fn parse_invalid_second_arc() {
    ObjectIdentifier::new("1.40.840.10045.3.1.7");
}
