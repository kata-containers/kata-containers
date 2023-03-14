//! PKCS#8 private key support.
//!
//! Implements Ed25519 PKCS#8 private keys as described in RFC8410 Section 7:
//! <https://datatracker.ietf.org/doc/html/rfc8410#section-7>
//!
//! ## SemVer Notes
//!
//! The `pkcs8` module of this crate is exempted from SemVer as it uses a
//! pre-1.0 dependency (the `pkcs8` crate).
//!
//! However, breaking changes to this module will be accompanied by a minor
//! version bump.
//!
//! Please lock to a specific minor version of the `ed25519` crate to avoid
//! breaking changes when using this module.

pub use pkcs8::{DecodePrivateKey, DecodePublicKey};

#[cfg(feature = "alloc")]
pub use pkcs8::{spki::EncodePublicKey, EncodePrivateKey};

use core::fmt;
use pkcs8::ObjectIdentifier;

#[cfg(feature = "alloc")]
use pkcs8::der::{Document, SecretDocument};

#[cfg(feature = "pem")]
use {
    alloc::string::{String, ToString},
    core::str,
};

#[cfg(feature = "zeroize")]
use zeroize::Zeroize;

/// Algorithm [`ObjectIdentifier`] for the Ed25519 digital signature algorithm
/// (`id-Ed25519`).
///
/// <http://oid-info.com/get/1.3.101.112>
pub const ALGORITHM_OID: ObjectIdentifier = ObjectIdentifier::new_unwrap("1.3.101.112");

/// Ed25519 Algorithm Identifier.
pub const ALGORITHM_ID: pkcs8::AlgorithmIdentifier<'static> = pkcs8::AlgorithmIdentifier {
    oid: ALGORITHM_OID,
    parameters: None,
};

/// Ed25519 keypair serialized as bytes.
///
/// This type is primarily useful for decoding/encoding PKCS#8 private key
/// files (either DER or PEM) encoded using the following traits:
///
/// - [`DecodePrivateKey`]: decode DER or PEM encoded PKCS#8 private key.
/// - [`EncodePrivateKey`]: encode DER or PEM encoded PKCS#8 private key.
///
/// PKCS#8 private key files encoded with PEM begin with:
///
/// ```text
/// -----BEGIN PRIVATE KEY-----
/// ```
///
/// Note that this type operates on raw bytes and performs no validation that
/// keys represent valid Ed25519 field elements.
pub struct KeypairBytes {
    /// Ed25519 secret key.
    ///
    /// Little endian serialization of an element of the Curve25519 scalar
    /// field, prior to "clamping" (i.e. setting/clearing bits to ensure the
    /// scalar is actually a valid field element)
    pub secret_key: [u8; Self::BYTE_SIZE / 2],

    /// Ed25519 public key (if available).
    ///
    /// Compressed Edwards-y encoded curve point.
    pub public_key: Option<[u8; Self::BYTE_SIZE / 2]>,
}

impl KeypairBytes {
    /// Size of an Ed25519 keypair when serialized as bytes.
    const BYTE_SIZE: usize = 64;

    /// Parse raw keypair from a 64-byte input.
    pub fn from_bytes(bytes: &[u8; Self::BYTE_SIZE]) -> Self {
        let (sk, pk) = bytes.split_at(Self::BYTE_SIZE / 2);
        Self {
            secret_key: sk.try_into().expect("secret key size error"),
            public_key: Some(pk.try_into().expect("public key size error")),
        }
    }

    /// Serialize as a 64-byte keypair.
    ///
    /// # Returns
    ///
    /// - `Some(bytes)` if the `public_key` is present.
    /// - `None` if the `public_key` is absent (i.e. `None`).
    pub fn to_bytes(&self) -> Option<[u8; Self::BYTE_SIZE]> {
        if let Some(public_key) = &self.public_key {
            let mut result = [0u8; Self::BYTE_SIZE];
            let (sk, pk) = result.split_at_mut(Self::BYTE_SIZE / 2);
            sk.copy_from_slice(&self.secret_key);
            pk.copy_from_slice(public_key);
            Some(result)
        } else {
            None
        }
    }
}

impl DecodePrivateKey for KeypairBytes {}

impl Drop for KeypairBytes {
    fn drop(&mut self) {
        #[cfg(feature = "zeroize")]
        self.secret_key.zeroize()
    }
}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl EncodePrivateKey for KeypairBytes {
    fn to_pkcs8_der(&self) -> pkcs8::Result<SecretDocument> {
        // Serialize private key as nested OCTET STRING
        let mut private_key = [0u8; 2 + (Self::BYTE_SIZE / 2)];
        private_key[0] = 0x04;
        private_key[1] = 0x20;
        private_key[2..].copy_from_slice(&self.secret_key);

        let private_key_info = pkcs8::PrivateKeyInfo {
            algorithm: ALGORITHM_ID,
            private_key: &private_key,
            public_key: self.public_key.as_ref().map(AsRef::as_ref),
        };

        let result = SecretDocument::encode_msg(&private_key_info)?;

        #[cfg(feature = "zeroize")]
        private_key.zeroize();

        Ok(result)
    }
}

impl TryFrom<pkcs8::PrivateKeyInfo<'_>> for KeypairBytes {
    type Error = pkcs8::Error;

    fn try_from(private_key: pkcs8::PrivateKeyInfo<'_>) -> pkcs8::Result<Self> {
        private_key.algorithm.assert_algorithm_oid(ALGORITHM_OID)?;

        if private_key.algorithm.parameters.is_some() {
            return Err(pkcs8::Error::ParametersMalformed);
        }

        // Ed25519 PKCS#8 keys are represented as a nested OCTET STRING
        // (i.e. an OCTET STRING within an OCTET STRING).
        //
        // This match statement checks and removes the inner OCTET STRING
        // header value:
        //
        // - 0x04: OCTET STRING tag
        // - 0x20: 32-byte length
        let secret_key = match private_key.private_key {
            [0x04, 0x20, rest @ ..] => rest.try_into().map_err(|_| pkcs8::Error::KeyMalformed),
            _ => Err(pkcs8::Error::KeyMalformed),
        }?;

        // TODO(tarcieri): parse public key
        let public_key = private_key
            .public_key
            .map(|bytes| bytes.try_into().map_err(|_| pkcs8::Error::KeyMalformed))
            .transpose()?;

        Ok(Self {
            secret_key,
            public_key,
        })
    }
}

impl TryFrom<&[u8]> for KeypairBytes {
    type Error = pkcs8::Error;

    fn try_from(der_bytes: &[u8]) -> pkcs8::Result<Self> {
        Self::from_pkcs8_der(der_bytes)
    }
}

impl fmt::Debug for KeypairBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeypairBytes")
            .field("public_key", &self.public_key)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl str::FromStr for KeypairBytes {
    type Err = pkcs8::Error;

    fn from_str(pem: &str) -> pkcs8::Result<Self> {
        Self::from_pkcs8_pem(pem)
    }
}

/// Ed25519 public key serialized as bytes.
///
/// This type is primarily useful for decoding/encoding SPKI public key
/// files (either DER or PEM) encoded using the following traits:
///
/// - [`DecodePublicKey`]: decode DER or PEM encoded PKCS#8 private key.
/// - [`EncodePublicKey`]: encode DER or PEM encoded PKCS#8 private key.
///
/// SPKI public key files encoded with PEM begin with:
///
/// ```text
/// -----BEGIN PUBLIC KEY-----
/// ```
///
/// Note that this type operates on raw bytes and performs no validation that
/// public keys represent valid compressed Ed25519 y-coordinates.
pub struct PublicKeyBytes(pub [u8; Self::BYTE_SIZE]);

impl PublicKeyBytes {
    /// Size of an Ed25519 public key when serialized as bytes.
    const BYTE_SIZE: usize = 32;

    /// Returns the raw bytes of the public key.
    pub fn to_bytes(&self) -> [u8; Self::BYTE_SIZE] {
        self.0
    }
}

impl AsRef<[u8; Self::BYTE_SIZE]> for PublicKeyBytes {
    fn as_ref(&self) -> &[u8; Self::BYTE_SIZE] {
        &self.0
    }
}

impl DecodePublicKey for PublicKeyBytes {}

#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
impl EncodePublicKey for PublicKeyBytes {
    fn to_public_key_der(&self) -> pkcs8::spki::Result<Document> {
        pkcs8::SubjectPublicKeyInfo {
            algorithm: ALGORITHM_ID,
            subject_public_key: &self.0,
        }
        .try_into()
    }
}

impl TryFrom<pkcs8::spki::SubjectPublicKeyInfo<'_>> for PublicKeyBytes {
    type Error = pkcs8::spki::Error;

    fn try_from(spki: pkcs8::spki::SubjectPublicKeyInfo<'_>) -> pkcs8::spki::Result<Self> {
        spki.algorithm.assert_algorithm_oid(ALGORITHM_OID)?;

        if spki.algorithm.parameters.is_some() {
            return Err(pkcs8::spki::Error::KeyMalformed);
        }

        spki.subject_public_key
            .try_into()
            .map(Self)
            .map_err(|_| pkcs8::spki::Error::KeyMalformed)
    }
}

impl TryFrom<&[u8]> for PublicKeyBytes {
    type Error = pkcs8::spki::Error;

    fn try_from(der_bytes: &[u8]) -> pkcs8::spki::Result<Self> {
        Self::from_public_key_der(der_bytes)
    }
}

impl TryFrom<KeypairBytes> for PublicKeyBytes {
    type Error = pkcs8::spki::Error;

    fn try_from(keypair: KeypairBytes) -> pkcs8::spki::Result<PublicKeyBytes> {
        PublicKeyBytes::try_from(&keypair)
    }
}

impl TryFrom<&KeypairBytes> for PublicKeyBytes {
    type Error = pkcs8::spki::Error;

    fn try_from(keypair: &KeypairBytes) -> pkcs8::spki::Result<PublicKeyBytes> {
        keypair
            .public_key
            .map(PublicKeyBytes)
            .ok_or(pkcs8::spki::Error::KeyMalformed)
    }
}

impl fmt::Debug for PublicKeyBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PublicKeyBytes(")?;

        for &byte in self.as_ref() {
            write!(f, "{:02X}", byte)?;
        }

        f.write_str(")")
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl str::FromStr for PublicKeyBytes {
    type Err = pkcs8::spki::Error;

    fn from_str(pem: &str) -> pkcs8::spki::Result<Self> {
        Self::from_public_key_pem(pem)
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl ToString for PublicKeyBytes {
    fn to_string(&self) -> String {
        self.to_public_key_pem(Default::default())
            .expect("PEM serialization error")
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
#[cfg(test)]
mod tests {
    use super::KeypairBytes;
    use hex_literal::hex;

    const SECRET_KEY_BYTES: [u8; 32] =
        hex!("D4EE72DBF913584AD5B6D8F1F769F8AD3AFE7C28CBF1D4FBE097A88F44755842");

    const PUBLIC_KEY_BYTES: [u8; 32] =
        hex!("19BF44096984CDFE8541BAC167DC3B96C85086AA30B6B6CB0C5C38AD703166E1");

    #[test]
    fn to_bytes() {
        let valid_keypair = KeypairBytes {
            secret_key: SECRET_KEY_BYTES,
            public_key: Some(PUBLIC_KEY_BYTES),
        };

        assert_eq!(
            valid_keypair.to_bytes().unwrap(),
            hex!("D4EE72DBF913584AD5B6D8F1F769F8AD3AFE7C28CBF1D4FBE097A88F4475584219BF44096984CDFE8541BAC167DC3B96C85086AA30B6B6CB0C5C38AD703166E1")
        );

        let invalid_keypair = KeypairBytes {
            secret_key: SECRET_KEY_BYTES,
            public_key: None,
        };

        assert_eq!(invalid_keypair.to_bytes(), None);
    }
}
