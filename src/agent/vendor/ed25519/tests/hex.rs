//! Hexadecimal display/serialization tests.

use ed25519::Signature;
use hex_literal::hex;
use std::str::FromStr;

/// Test 1 signature from RFC 8032 § 7.1
/// <https://datatracker.ietf.org/doc/html/rfc8032#section-7.1>
const TEST_1_SIGNATURE: [u8; Signature::BYTE_SIZE] = hex!(
    "e5564300c360ac729086e2cc806e828a
     84877f1eb8e5d974d873e06522490155
     5fb8821590a33bacc61e39701cf9b46b
     d25bf5f0595bbe24655141438e7a100b"
);

#[test]
fn display() {
    let sig = Signature::from_bytes(&TEST_1_SIGNATURE).unwrap();
    assert_eq!(sig.to_string(), "E5564300C360AC729086E2CC806E828A84877F1EB8E5D974D873E065224901555FB8821590A33BACC61E39701CF9B46BD25BF5F0595BBE24655141438E7A100B")
}

#[test]
fn lower_hex() {
    let sig = Signature::from_bytes(&TEST_1_SIGNATURE).unwrap();
    assert_eq!(format!("{:x}", sig), "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b")
}

#[test]
fn upper_hex() {
    let sig = Signature::from_bytes(&TEST_1_SIGNATURE).unwrap();
    assert_eq!(format!("{:X}", sig), "E5564300C360AC729086E2CC806E828A84877F1EB8E5D974D873E065224901555FB8821590A33BACC61E39701CF9B46BD25BF5F0595BBE24655141438E7A100B")
}

#[test]
fn from_str_lower() {
    let sig = Signature::from_str("e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b").unwrap();
    assert_eq!(sig.as_ref(), TEST_1_SIGNATURE);
}

#[test]
fn from_str_upper() {
    let sig = Signature::from_str("E5564300C360AC729086E2CC806E828A84877F1EB8E5D974D873E065224901555FB8821590A33BACC61E39701CF9B46BD25BF5F0595BBE24655141438E7A100B").unwrap();
    assert_eq!(sig.as_ref(), TEST_1_SIGNATURE);
}

#[test]
fn from_str_rejects_mixed_case() {
    let result = Signature::from_str("E5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b");
    assert!(result.is_err());
}

#[test]
fn from_str_rejects_invalid_signature() {
    let result = Signature::from_str("FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF");
    assert!(result.is_err());
}
