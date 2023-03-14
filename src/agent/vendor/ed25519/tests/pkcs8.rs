//! PKCS#8 private key tests

#![cfg(feature = "pkcs8")]

use ed25519::pkcs8::{DecodePrivateKey, DecodePublicKey, KeypairBytes, PublicKeyBytes};
use hex_literal::hex;

#[cfg(feature = "alloc")]
use ed25519::pkcs8::{EncodePrivateKey, EncodePublicKey};

/// Ed25519 PKCS#8 v1 private key encoded as ASN.1 DER.
const PKCS8_V1_DER: &[u8] = include_bytes!("examples/pkcs8-v1.der");

/// Ed25519 PKCS#8 v2 private key + public key encoded as ASN.1 DER.
const PKCS8_V2_DER: &[u8] = include_bytes!("examples/pkcs8-v2.der");

/// Ed25519 SubjectPublicKeyInfo encoded as ASN.1 DER.
const PUBLIC_KEY_DER: &[u8] = include_bytes!("examples/pubkey.der");

#[test]
fn decode_pkcs8_v1() {
    let keypair = KeypairBytes::from_pkcs8_der(PKCS8_V1_DER).unwrap();

    // Extracted with:
    // $ openssl asn1parse -inform der -in tests/examples/p256-priv.der
    assert_eq!(
        keypair.secret_key,
        &hex!("D4EE72DBF913584AD5B6D8F1F769F8AD3AFE7C28CBF1D4FBE097A88F44755842")[..]
    );

    assert_eq!(keypair.public_key, None);
}

#[test]
fn decode_pkcs8_v2() {
    let keypair = KeypairBytes::from_pkcs8_der(PKCS8_V2_DER).unwrap();

    // Extracted with:
    // $ openssl asn1parse -inform der -in tests/examples/p256-priv.der
    assert_eq!(
        keypair.secret_key,
        &hex!("D4EE72DBF913584AD5B6D8F1F769F8AD3AFE7C28CBF1D4FBE097A88F44755842")[..]
    );

    assert_eq!(
        keypair.public_key.unwrap(),
        hex!("19BF44096984CDFE8541BAC167DC3B96C85086AA30B6B6CB0C5C38AD703166E1")
    );
}

#[test]
fn decode_public_key() {
    let public_key = PublicKeyBytes::from_public_key_der(PUBLIC_KEY_DER).unwrap();

    // Extracted with:
    // $ openssl pkey -inform der -in pkcs8-v1.der -pubout -text
    assert_eq!(
        public_key.as_ref(),
        &hex!("19BF44096984CDFE8541BAC167DC3B96C85086AA30B6B6CB0C5C38AD703166E1")
    );
}

#[cfg(feature = "alloc")]
#[test]
fn encode_pkcs8_v1() {
    let pk = KeypairBytes::from_pkcs8_der(PKCS8_V1_DER).unwrap();
    let pk_der = pk.to_pkcs8_der().unwrap();
    assert_eq!(pk_der.as_bytes(), PKCS8_V1_DER);
}

#[cfg(feature = "alloc")]
#[test]
fn encode_pkcs8_v2() {
    let pk = KeypairBytes::from_pkcs8_der(PKCS8_V2_DER).unwrap();
    let pk2 = KeypairBytes::from_pkcs8_der(pk.to_pkcs8_der().unwrap().as_bytes()).unwrap();
    assert_eq!(pk.secret_key, pk2.secret_key);
    assert_eq!(pk.public_key, pk2.public_key);
}

#[cfg(feature = "alloc")]
#[test]
fn encode_public_key() {
    let pk = PublicKeyBytes::from_public_key_der(PUBLIC_KEY_DER).unwrap();
    let pk_der = pk.to_public_key_der().unwrap();
    assert_eq!(pk_der.as_ref(), PUBLIC_KEY_DER);
}
