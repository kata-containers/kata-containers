//! `SubjectPublicKeyInfo` tests.

#[cfg(feature = "alloc")]
use spki::der::Encodable;

#[cfg(feature = "fingerprint")]
use {hex_literal::hex, spki::SubjectPublicKeyInfo};

#[cfg(feature = "pem")]
use spki::{der::Document, EncodePublicKey, PublicKeyDocument};

#[cfg(feature = "fingerprint")]
// Taken from pkcs8/tests/public_key.rs
/// Ed25519 `SubjectPublicKeyInfo` encoded as ASN.1 DER
const ED25519_DER_EXAMPLE: &[u8] = include_bytes!("examples/ed25519-pub.der");

/// Ed25519 public key encoded as PEM
#[cfg(feature = "pem")]
const ED25519_PEM_EXAMPLE: &str = include_str!("examples/ed25519-pub.pem");

/// The SPKI fingerprint for `ED25519_SPKI_FINGERPRINT` as a Base64 string
///
/// Generated using `cat ed25519-pub.der | openssl dgst -binary -sha256 | base64`
#[cfg(all(feature = "fingerprint", feature = "alloc"))]
const ED25519_SPKI_FINGERPRINT_BASE64: &str = "Vd1MdLDkhTTi9OFzzs61DfjyenrCqomRzHrpFOAwvO0=";

/// The SPKI fingerprint for `ED25519_SPKI_FINGERPRINT` as straight hash bytes
///
/// Generated using `cat ed25519-pub.der | openssl dgst -sha256`
#[cfg(all(feature = "fingerprint"))]
const ED25519_SPKI_FINGERPRINT: &[u8] =
    &hex!("55dd4c74b0e48534e2f4e173ceceb50df8f27a7ac2aa8991cc7ae914e030bced");

#[cfg(all(feature = "fingerprint", feature = "alloc"))]
#[test]
fn decode_and_base64fingerprint_spki() {
    // Repeat the decode test from the pkcs8 crate
    let spki = SubjectPublicKeyInfo::try_from(ED25519_DER_EXAMPLE).unwrap();

    assert_eq!(spki.algorithm.oid, "1.3.101.112".parse().unwrap());
    assert_eq!(spki.algorithm.parameters, None);
    assert_eq!(
        spki.subject_public_key,
        &hex!("4D29167F3F1912A6F7ADFA293A051A15C05EC67B8F17267B1C5550DCE853BD0D")[..]
    );

    // Check the fingerprint
    assert_eq!(
        spki.fingerprint_base64().unwrap(),
        ED25519_SPKI_FINGERPRINT_BASE64
    );
}

#[cfg(feature = "fingerprint")]
#[test]
fn decode_and_fingerprint_spki() {
    // Repeat the decode test from the pkcs8 crate
    let spki = SubjectPublicKeyInfo::try_from(ED25519_DER_EXAMPLE).unwrap();

    assert_eq!(spki.algorithm.oid, "1.3.101.112".parse().unwrap());
    assert_eq!(spki.algorithm.parameters, None);
    assert_eq!(
        spki.subject_public_key,
        &hex!("4D29167F3F1912A6F7ADFA293A051A15C05EC67B8F17267B1C5550DCE853BD0D")[..]
    );

    // Check the fingerprint
    assert_eq!(
        spki.fingerprint().unwrap().as_slice(),
        ED25519_SPKI_FINGERPRINT
    );
}

#[test]
#[cfg(feature = "pem")]
fn decode_ed25519_pem() {
    let doc: PublicKeyDocument = ED25519_PEM_EXAMPLE.parse().unwrap();
    assert_eq!(doc.as_ref(), ED25519_DER_EXAMPLE);

    // Ensure `PublicKeyDocument` parses successfully
    let spki = SubjectPublicKeyInfo::try_from(ED25519_DER_EXAMPLE).unwrap();
    assert_eq!(doc.decode(), spki);
}

#[test]
#[cfg(feature = "alloc")]
fn encode_ed25519_der() {
    let pk = SubjectPublicKeyInfo::try_from(ED25519_DER_EXAMPLE).unwrap();
    let pk_encoded = pk.to_vec().unwrap();
    assert_eq!(ED25519_DER_EXAMPLE, pk_encoded.as_slice());
}

#[test]
#[cfg(feature = "pem")]
fn encode_ed25519_pem() {
    let pk = SubjectPublicKeyInfo::try_from(ED25519_DER_EXAMPLE).unwrap();
    let pk_encoded = PublicKeyDocument::try_from(pk)
        .unwrap()
        .to_public_key_pem(Default::default())
        .unwrap();

    assert_eq!(ED25519_PEM_EXAMPLE, pk_encoded);
}
