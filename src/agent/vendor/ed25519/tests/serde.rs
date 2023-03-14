//! Tests for serde serializers/deserializers

#![cfg(feature = "serde")]

use ed25519::Signature;

#[cfg(feature = "serde_bytes")]
use serde_bytes_crate as serde_bytes;

const EXAMPLE_SIGNATURE: [u8; Signature::BYTE_SIZE] = [
    63, 62, 61, 60, 59, 58, 57, 56, 55, 54, 53, 52, 51, 50, 49, 48, 47, 46, 45, 44, 43, 42, 41, 40,
    39, 38, 37, 36, 35, 34, 33, 32, 31, 30, 29, 28, 27, 26, 25, 24, 23, 22, 21, 20, 19, 18, 17, 16,
    15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
];

#[test]
fn test_serialize() {
    let signature = Signature::try_from(&EXAMPLE_SIGNATURE[..]).unwrap();
    let encoded_signature: Vec<u8> = bincode::serialize(&signature).unwrap();
    assert_eq!(&EXAMPLE_SIGNATURE[..], &encoded_signature[..]);
}

#[test]
fn test_deserialize() {
    let signature = bincode::deserialize::<Signature>(&EXAMPLE_SIGNATURE).unwrap();
    assert_eq!(&EXAMPLE_SIGNATURE[..], signature.as_ref());
}

#[cfg(feature = "serde_bytes")]
#[test]
fn test_serialize_bytes() {
    use bincode::Options;

    let signature = Signature::try_from(&EXAMPLE_SIGNATURE[..]).unwrap();

    let mut encoded_signature = Vec::new();
    let options = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes();
    let mut serializer = bincode::Serializer::new(&mut encoded_signature, options);
    serde_bytes::serialize(&signature, &mut serializer).unwrap();

    let mut expected = Vec::from(Signature::BYTE_SIZE.to_le_bytes());
    expected.extend(&EXAMPLE_SIGNATURE[..]);
    assert_eq!(&expected[..], &encoded_signature[..]);
}

#[cfg(feature = "serde_bytes")]
#[test]
fn test_deserialize_bytes() {
    use bincode::Options;

    let mut encoded_signature = Vec::from(Signature::BYTE_SIZE.to_le_bytes());
    encoded_signature.extend(&EXAMPLE_SIGNATURE[..]);

    let options = bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes();
    let mut deserializer = bincode::de::Deserializer::from_slice(&encoded_signature[..], options);

    let signature: Signature = serde_bytes::deserialize(&mut deserializer).unwrap();

    assert_eq!(&EXAMPLE_SIGNATURE[..], signature.as_ref());
}
