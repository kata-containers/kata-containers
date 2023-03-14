#![allow(clippy::use_self)]

//! Handles cryptographic keys and their serialization in TUF metadata files.

use crate::schema::decoded::{Decoded, EcdsaFlex, Hex, RsaPem};
use crate::schema::error::{self, Result};
use olpc_cjson::CanonicalFormatter;
use ring::digest::{digest, SHA256};
use ring::signature::VerificationAlgorithm;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snafu::ResultExt;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// Serializes signing keys as defined by the TUF specification. All keys have the format
/// ```json
///  { "keytype" : "KEYTYPE",
///     "scheme" : "SCHEME",
///     "keyval" : "KEYVAL"
///  }
/// ```
/// where:
/// KEYTYPE is a string denoting a public key signature system, such as RSA or ECDSA.
///
/// SCHEME is a string denoting a corresponding signature scheme.  For example: "rsassa-pss-sha256"
/// and "ecdsa-sha2-nistp256".
///
/// KEYVAL is a dictionary containing the public portion of the key:
/// `"keyval" : {"public" : PUBLIC}`
/// where:
///  * `Rsa`: PUBLIC is in PEM format and a string. All RSA keys must be at least 2048 bits.
///  * `Ed25519`: PUBLIC is a 64-byte hex encoded string.
///  * `Ecdsa`: PUBLIC is in PEM format and a string.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[serde(tag = "keytype")]
pub enum Key {
    /// An RSA key.
    Rsa {
        /// The RSA key.
        keyval: RsaKey,
        /// Denotes the key's signature scheme.
        scheme: RsaScheme,
        /// Any additional fields read during deserialization; will not be used.
        #[serde(flatten)]
        _extra: HashMap<String, Value>,
    },
    /// An Ed25519 key.
    Ed25519 {
        /// The Ed25519 key.
        keyval: Ed25519Key,
        /// Denotes the key's signature scheme.
        scheme: Ed25519Scheme,
        /// Any additional fields read during deserialization; will not be used.
        #[serde(flatten)]
        _extra: HashMap<String, Value>,
    },
    /// An EcdsaKey
    #[serde(rename = "ecdsa-sha2-nistp256")]
    Ecdsa {
        /// The Ecdsa key.
        keyval: EcdsaKey,
        /// Denotes the key's signature scheme.
        scheme: EcdsaScheme,
        /// Any additional fields read during deserialization; will not be used.
        #[serde(flatten)]
        _extra: HashMap<String, Value>,
    },
}

/// Used to identify the RSA signature scheme in use.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RsaScheme {
    /// `rsassa-pss-sha256`: RSA Probabilistic signature scheme with appendix.
    RsassaPssSha256,
}

/// Represents a deserialized (decoded) RSA public key.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct RsaKey {
    /// The public key.
    pub public: Decoded<RsaPem>,

    /// Any additional fields read during deserialization; will not be used.
    #[serde(flatten)]
    pub _extra: HashMap<String, Value>,
}

/// Used to identify the `EdDSA` signature scheme in use.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Ed25519Scheme {
    /// 'ed25519': Elliptic curve digital signature algorithm based on Twisted Edwards curves.
    Ed25519,
}

/// Represents a deserialized (decoded) Ed25519 public key.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct Ed25519Key {
    /// The public key.
    pub public: Decoded<Hex>,

    /// Any additional fields read during deserialization; will not be used.
    #[serde(flatten)]
    pub _extra: HashMap<String, Value>,
}

/// Used to identify the ECDSA signature scheme in use.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum EcdsaScheme {
    /// `ecdsa-sha2-nistp256`: Elliptic Curve Digital Signature Algorithm with NIST P-256 curve
    /// signing and SHA-256 hashing.
    EcdsaSha2Nistp256,
}

/// Represents a deserialized (decoded)  Ecdsa public key.
#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct EcdsaKey {
    /// The public key.
    pub public: Decoded<EcdsaFlex>,

    /// Any additional fields read during deserialization; will not be used.
    #[serde(flatten)]
    pub _extra: HashMap<String, Value>,
}

impl Key {
    /// Calculate the key ID for this key.
    pub fn key_id(&self) -> Result<Decoded<Hex>> {
        let mut buf = Vec::new();
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, CanonicalFormatter::new());
        self.serialize(&mut ser)
            .context(error::JsonSerializationSnafu {
                what: "key".to_owned(),
            })?;
        Ok(digest(&SHA256, &buf).as_ref().to_vec().into())
    }

    /// Verify a signature of an object made with this key.
    pub(super) fn verify(&self, msg: &[u8], signature: &[u8]) -> bool {
        let (alg, public_key): (&dyn VerificationAlgorithm, untrusted::Input<'_>) = match self {
            Key::Ecdsa {
                scheme: EcdsaScheme::EcdsaSha2Nistp256,
                keyval,
                ..
            } => (
                &ring::signature::ECDSA_P256_SHA256_ASN1,
                untrusted::Input::from(&keyval.public),
            ),
            Key::Ed25519 {
                scheme: Ed25519Scheme::Ed25519,
                keyval,
                ..
            } => (
                &ring::signature::ED25519,
                untrusted::Input::from(&keyval.public),
            ),
            Key::Rsa {
                scheme: RsaScheme::RsassaPssSha256,
                keyval,
                ..
            } => (
                &ring::signature::RSA_PSS_2048_8192_SHA256,
                untrusted::Input::from(&keyval.public),
            ),
        };

        alg.verify(
            public_key,
            untrusted::Input::from(msg),
            untrusted::Input::from(signature),
        )
        .is_ok()
    }
}

impl FromStr for Key {
    type Err = KeyParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if let Ok(public) = serde_plain::from_str::<Decoded<RsaPem>>(s) {
            Ok(Key::Rsa {
                keyval: RsaKey {
                    public,
                    _extra: HashMap::new(),
                },
                scheme: RsaScheme::RsassaPssSha256,
                _extra: HashMap::new(),
            })
        } else if let Ok(public) = serde_plain::from_str::<Decoded<Hex>>(s) {
            if public.len() == ring::signature::ED25519_PUBLIC_KEY_LEN {
                Ok(Key::Ed25519 {
                    keyval: Ed25519Key {
                        public,
                        _extra: HashMap::new(),
                    },
                    scheme: Ed25519Scheme::Ed25519,
                    _extra: HashMap::new(),
                })
            } else {
                Err(KeyParseError(()))
            }
        } else if let Ok(public) = serde_plain::from_str::<Decoded<EcdsaFlex>>(s) {
            Ok(Key::Ecdsa {
                keyval: EcdsaKey {
                    public,
                    _extra: HashMap::new(),
                },
                scheme: EcdsaScheme::EcdsaSha2Nistp256,
                _extra: HashMap::new(),
            })
        } else {
            Err(KeyParseError(()))
        }
    }
}

/// An error object to be used when a key cannot be parsed.
#[derive(Debug, Clone, Copy)]
pub struct KeyParseError(());

impl fmt::Display for KeyParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unrecognized or invalid public key")
    }
}

impl std::error::Error for KeyParseError {}
