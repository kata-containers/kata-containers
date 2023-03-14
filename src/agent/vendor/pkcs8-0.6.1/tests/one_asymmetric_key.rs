use std::convert::TryFrom;

use der::Encodable;
use hex_literal::hex;
use pkcs8::{OneAsymmetricKey, Version};

/// Ed25519 PKCS#8 v2 private key + public key encoded as ASN.1 DER
const ED25519_DER_V2_EXAMPLE: &[u8] = include_bytes!("examples/ed25519-pkcs8-v2.der");

#[test]
fn roundtrip_ed25519_oak_der() {
    const PRIV_KEY: [u8; 34] =
        hex!("04203A133DABADA2AA9CE54B0961CC3F1576B0943DC86EBF72A56E052C43F30FA3A5");
    const PUB_KEY: [u8; 32] =
        hex!("A3A7EAE3A8373830BC47E1167BC50E1DB551999651E0E2DC587623438EAC3F31");

    let oak = OneAsymmetricKey::try_from(ED25519_DER_V2_EXAMPLE).unwrap();

    assert_eq!(oak.algorithm.oid, "1.3.101.112".parse().unwrap());
    assert_eq!(oak.algorithm.parameters, None);

    // A3A7EAE3A8373830BC47E1167BC50E1DB551999651E0E2DC587623438EAC3F31

    assert_eq!(oak.private_key, PRIV_KEY);

    assert_eq!(oak.public_key, Some(&PUB_KEY[..]));

    assert_eq!(oak.version(), Version::V2);

    let mut slice = [0u8; ED25519_DER_V2_EXAMPLE.len()];

    oak.encode_to_slice(&mut slice[..]).unwrap();

    assert_eq!(slice, ED25519_DER_V2_EXAMPLE);
}
