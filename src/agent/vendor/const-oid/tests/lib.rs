//! `const-oid` crate tests

// TODO(tarcieri): test full set of OID encoding constraints specified here:
// <https://misc.daniel-marschall.de/asn.1/oid_facts.html>

use const_oid::ObjectIdentifier;
use hex_literal::hex;
use std::string::ToString;

/// Example OID value with a root arc of `0` (and large arc).
const EXAMPLE_OID_0_STR: &str = "0.9.2342.19200300.100.1.1";
const EXAMPLE_OID_0_BER: &[u8] = &hex!("0992268993F22C640101");
const EXAMPLE_OID_0: ObjectIdentifier = ObjectIdentifier::new(EXAMPLE_OID_0_STR);

/// Example OID value with a root arc of `1`.
const EXAMPLE_OID_1_STR: &str = "1.2.840.10045.2.1";
const EXAMPLE_OID_1_BER: &[u8] = &hex!("2A8648CE3D0201");
const EXAMPLE_OID_1: ObjectIdentifier = ObjectIdentifier::new(EXAMPLE_OID_1_STR);

/// Example OID value with a root arc of `2`.
const EXAMPLE_OID_2_STR: &str = "2.16.840.1.101.3.4.1.42";
const EXAMPLE_OID_2_BER: &[u8] = &hex!("60864801650304012A");
const EXAMPLE_OID_2: ObjectIdentifier = ObjectIdentifier::new(EXAMPLE_OID_2_STR);

/// Example OID value with a large arc
const EXAMPLE_OID_LARGE_ARC_STR: &str = "0.9.2342.19200300.100.1.1";
const EXAMPLE_OID_LARGE_ARC_BER: &[u8] = &hex!("0992268993F22C640101");
const EXAMPLE_OID_LARGE_ARC: ObjectIdentifier = ObjectIdentifier::new("0.9.2342.19200300.100.1.1");

#[test]
fn from_bytes() {
    let oid0 = ObjectIdentifier::from_bytes(EXAMPLE_OID_0_BER).unwrap();
    assert_eq!(oid0.arc(0).unwrap(), 0);
    assert_eq!(oid0.arc(1).unwrap(), 9);
    assert_eq!(oid0, EXAMPLE_OID_0);

    let oid1 = ObjectIdentifier::from_bytes(EXAMPLE_OID_1_BER).unwrap();
    assert_eq!(oid1.arc(0).unwrap(), 1);
    assert_eq!(oid1.arc(1).unwrap(), 2);
    assert_eq!(oid1, EXAMPLE_OID_1);

    let oid2 = ObjectIdentifier::from_bytes(EXAMPLE_OID_2_BER).unwrap();
    assert_eq!(oid2.arc(0).unwrap(), 2);
    assert_eq!(oid2.arc(1).unwrap(), 16);
    assert_eq!(oid2, EXAMPLE_OID_2);

    let oid3 = ObjectIdentifier::from_bytes(EXAMPLE_OID_LARGE_ARC_BER).unwrap();
    assert_eq!(oid3.arc(0).unwrap(), 0);
    assert_eq!(oid3.arc(1).unwrap(), 9);
    assert_eq!(oid3.arc(2).unwrap(), 2342);
    assert_eq!(oid3.arc(3).unwrap(), 19200300);
    assert_eq!(oid3.arc(4).unwrap(), 100);
    assert_eq!(oid3.arc(5).unwrap(), 1);
    assert_eq!(oid3.arc(6).unwrap(), 1);
    assert_eq!(oid3, EXAMPLE_OID_LARGE_ARC);

    // Empty
    assert!(ObjectIdentifier::from_bytes(&[]).is_err());

    // Truncated
    assert!(ObjectIdentifier::from_bytes(&[42]).is_err());
    assert!(ObjectIdentifier::from_bytes(&[42, 134]).is_err());
}

#[test]
fn from_str() {
    let oid0 = EXAMPLE_OID_0_STR.parse::<ObjectIdentifier>().unwrap();
    assert_eq!(oid0.arc(0).unwrap(), 0);
    assert_eq!(oid0.arc(1).unwrap(), 9);
    assert_eq!(oid0, EXAMPLE_OID_0);

    let oid1 = EXAMPLE_OID_1_STR.parse::<ObjectIdentifier>().unwrap();
    assert_eq!(oid1.arc(0).unwrap(), 1);
    assert_eq!(oid1.arc(1).unwrap(), 2);
    assert_eq!(oid1, EXAMPLE_OID_1);

    let oid2 = EXAMPLE_OID_2_STR.parse::<ObjectIdentifier>().unwrap();
    assert_eq!(oid2.arc(0).unwrap(), 2);
    assert_eq!(oid2.arc(1).unwrap(), 16);
    assert_eq!(oid2, EXAMPLE_OID_2);

    let oid3 = EXAMPLE_OID_LARGE_ARC_STR
        .parse::<ObjectIdentifier>()
        .unwrap();
    assert_eq!(oid3.arc(0).unwrap(), 0);
    assert_eq!(oid3.arc(1).unwrap(), 9);
    assert_eq!(oid3.arc(2).unwrap(), 2342);
    assert_eq!(oid3.arc(3).unwrap(), 19200300);
    assert_eq!(oid3.arc(4).unwrap(), 100);
    assert_eq!(oid3.arc(5).unwrap(), 1);
    assert_eq!(oid3.arc(6).unwrap(), 1);
    assert_eq!(oid3, EXAMPLE_OID_LARGE_ARC);

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
fn display() {
    assert_eq!(EXAMPLE_OID_0.to_string(), EXAMPLE_OID_0_STR);
    assert_eq!(EXAMPLE_OID_1.to_string(), EXAMPLE_OID_1_STR);
    assert_eq!(EXAMPLE_OID_2.to_string(), EXAMPLE_OID_2_STR);
    assert_eq!(EXAMPLE_OID_LARGE_ARC.to_string(), EXAMPLE_OID_LARGE_ARC_STR);
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
