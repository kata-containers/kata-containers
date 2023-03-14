//! JSON Web Signature (JWS) represents content secured with digital signatures or Message Authentication Codes (MACs) using JSON-based data structures.
//!
//! See [RFC7515](https://tools.ietf.org/html/rfc7515).

use crate::hash::HashAlgorithm;
use crate::jose::jwk::Jwk;
use crate::key::{PrivateKey, PublicKey};
use crate::signature::{SignatureAlgorithm, SignatureError};
use base64::DecodeError;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use thiserror::Error;

// === error type === //

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum JwsError {
    /// RSA error
    #[error("RSA error: {context}")]
    Rsa { context: String },

    /// Json error
    #[error("JSON error: {source}")]
    Json { source: serde_json::Error },

    /// signature error
    #[error("signature error: {source}")]
    Signature { source: SignatureError },

    /// invalid token encoding
    #[error("input isn't a valid token string: {input}")]
    InvalidEncoding { input: String },

    /// couldn't decode base64
    #[error("couldn't decode base64: {source}")]
    Base64Decoding { source: DecodeError },

    /// input isn't valid utf8
    #[error("input isn't valid utf8: {source}, input: {input:?}")]
    InvalidUtf8 {
        source: std::string::FromUtf8Error,
        input: Vec<u8>,
    },
}

impl From<rsa::errors::Error> for JwsError {
    fn from(e: rsa::errors::Error) -> Self {
        Self::Rsa { context: e.to_string() }
    }
}

impl From<serde_json::Error> for JwsError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json { source: e }
    }
}

impl From<SignatureError> for JwsError {
    fn from(e: SignatureError) -> Self {
        Self::Signature { source: e }
    }
}

impl From<DecodeError> for JwsError {
    fn from(e: DecodeError) -> Self {
        Self::Base64Decoding { source: e }
    }
}

// === JWS algorithms === //

/// `alg` header parameter values for JWS
///
/// [JSON Web Algorithms (JWA) draft-ietf-jose-json-web-algorithms-40 #3](https://tools.ietf.org/html/draft-ietf-jose-json-web-algorithms-40#section-3.1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JwsAlg {
    /// HMAC using SHA-256 (unsupported)
    ///
    /// Required by RFC
    HS256,

    /// HMAC using SHA-384 (unsupported)
    HS384,

    /// HMAC using SHA-512 (unsupported)
    HS512,

    /// RSASSA-PKCS-v1_5 using SHA-256
    ///
    /// Recommended by RFC
    RS256,

    /// RSASSA-PKCS-v1_5 using SHA-384
    RS384,

    /// RSASSA-PKCS-v1_5 using SHA-512
    RS512,

    /// ECDSA using P-256 and SHA-256 (unsupported)
    ///
    /// Recommended+ by RFC
    ES256,

    /// ECDSA using P-384 and SHA-384 (unsupported)
    ES384,

    /// ECDSA using P-521 and SHA-512 (unsupported)
    ES512,

    /// RSASSA-PSS using SHA-256 and MGF1 with SHA-256 (unsupported)
    PS256,

    /// RSASSA-PSS using SHA-384 and MGF1 with SHA-384 (unsupported)
    PS384,

    /// RSASSA-PSS using SHA-512 and MGF1 with SHA-512 (unsupported)
    PS512,
}

impl TryFrom<SignatureAlgorithm> for JwsAlg {
    type Error = SignatureError;

    fn try_from(v: SignatureAlgorithm) -> Result<Self, Self::Error> {
        match v {
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_256) => Ok(Self::RS256),
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_384) => Ok(Self::RS384),
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_512) => Ok(Self::RS512),
            unsupported => Err(SignatureError::UnsupportedAlgorithm {
                algorithm: format!("{:?}", unsupported),
            }),
        }
    }
}

impl TryFrom<JwsAlg> for SignatureAlgorithm {
    type Error = SignatureError;

    fn try_from(v: JwsAlg) -> Result<Self, Self::Error> {
        match v {
            JwsAlg::RS256 => Ok(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_256)),
            JwsAlg::RS384 => Ok(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_384)),
            JwsAlg::RS512 => Ok(SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_512)),
            unsupported => Err(SignatureError::UnsupportedAlgorithm {
                algorithm: format!("{:?}", unsupported),
            }),
        }
    }
}

// === JWS header === //

/// JOSE header of a JWS
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JwsHeader {
    // -- specific to JWS -- //
    /// Algorithm Header
    ///
    /// identifies the cryptographic algorithm used to secure the JWS.
    pub alg: JwsAlg,

    // -- common with JWE -- //
    /// JWK Set URL
    ///
    /// URI that refers to a resource for a set of JSON-encoded public keys,
    /// one of which corresponds to the key used to digitally sign the JWS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jku: Option<String>,

    /// JSON Web Key
    ///
    /// The public key that corresponds to the key used to digitally sign the JWS.
    /// This key is represented as a JSON Web Key (JWK).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwk: Option<Jwk>,

    /// Type header
    ///
    /// Used by JWS applications to declare the media type [IANA.MediaTypes] of this complete JWS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typ: Option<String>,

    /// Content Type header
    ///
    /// Used by JWS applications to declare the media type [IANA.MediaTypes] of the secured content (the payload).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cty: Option<String>,

    // -- common with all -- //
    /// Key ID Header
    ///
    /// A hint indicating which key was used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kid: Option<String>,

    /// X.509 URL Header
    ///
    /// URI that refers to a resource for an X.509 public key certificate or certificate chain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x5u: Option<String>,

    /// X.509 Certificate Chain
    ///
    /// Chain of one or more PKIX certificates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x5c: Option<Vec<String>>,

    /// X.509 Certificate SHA-1 Thumbprint
    ///
    /// base64url-encoded SHA-1 thumbprint (a.k.a. digest) of the DER encoding of an X.509 certificate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x5t: Option<String>,

    /// X.509 Certificate SHA-256 Thumbprint
    ///
    /// base64url-encoded SHA-256 thumbprint (a.k.a. digest) of the DER encoding of an X.509 certificate.
    #[serde(rename = "x5t#S256", alias = "x5t#s256", skip_serializing_if = "Option::is_none")]
    pub x5t_s256: Option<String>,

    // -- extra parameters -- //
    /// Additional header parameters (both public and private)
    #[serde(flatten)]
    pub additional: HashMap<String, serde_json::Value>,
}

impl JwsHeader {
    pub fn new(alg: JwsAlg) -> Self {
        Self {
            alg,
            jku: None,
            jwk: None,
            typ: None,
            cty: None,
            kid: None,
            x5u: None,
            x5c: None,
            x5t: None,
            x5t_s256: None,
            additional: HashMap::new(),
        }
    }

    pub fn new_with_cty(alg: JwsAlg, cty: impl Into<String>) -> Self {
        Self {
            cty: Some(cty.into()),
            ..Self::new(alg)
        }
    }
}

// === json web signature === //

/// Provides an API to sign any kind of data (binary). JSON claims are part of `Jwt` only.
#[derive(Debug, Clone)]
pub struct Jws {
    pub header: JwsHeader,
    pub payload: Vec<u8>,
}

impl Jws {
    pub fn new(alg: JwsAlg, payload: Vec<u8>) -> Self {
        Self {
            header: JwsHeader::new(alg),
            payload,
        }
    }

    pub fn encode(&self, private_key: &PrivateKey) -> Result<String, JwsError> {
        let header_base64 = base64::encode_config(&serde_json::to_vec(&self.header)?, base64::URL_SAFE_NO_PAD);
        let payload_base64 = base64::encode_config(&self.payload, base64::URL_SAFE_NO_PAD);
        let header_and_payload = [header_base64, payload_base64].join(".");
        let signature_algo = SignatureAlgorithm::try_from(self.header.alg)?;
        let signature = signature_algo.sign(header_and_payload.as_bytes(), private_key)?;
        let signature_base64 = base64::encode_config(&signature, base64::URL_SAFE_NO_PAD);
        Ok([header_and_payload, signature_base64].join("."))
    }

    /// Verifies signature and returns decoded JWS payload.
    pub fn decode(encoded_token: &str, public_key: &PublicKey) -> Result<Self, JwsError> {
        RawJws::decode(encoded_token).and_then(|raw_jws| raw_jws.verify(public_key))
    }
}

/// Raw low-level interface to the yet to be verified JWS token.
///
/// This is useful to inspect the structure before performing further processing.
/// For most usecases, use `Jws` directly.
#[derive(Debug, Clone)]
pub struct RawJws<'repr> {
    pub compact_repr: Cow<'repr, str>,
    pub header: JwsHeader,
    payload: Vec<u8>,
    pub signature: Vec<u8>,
}

/// An owned `RawJws` for convenience.
pub type OwnedRawJws = RawJws<'static>;

impl<'repr> RawJws<'repr> {
    /// Decodes a JWS in compact representation.
    pub fn decode(compact_repr: impl Into<Cow<'repr, str>>) -> Result<Self, JwsError> {
        decode_impl(compact_repr.into())
    }

    /// Peeks the payload before signature verification.
    pub fn peek_payload(&self) -> &[u8] {
        &self.payload
    }

    /// Verifies signature and returns a verified `Jws` structure.
    pub fn verify(self, public_key: &PublicKey) -> Result<Jws, JwsError> {
        verify_signature(&self.compact_repr, public_key, self.header.alg)?;
        Ok(self.discard_signature())
    }

    /// Discards the signature without verifying it and hands a `Jws` structure.
    ///
    /// Generally, you should not do that.
    pub fn discard_signature(self) -> Jws {
        Jws {
            header: self.header,
            payload: self.payload,
        }
    }
}

fn decode_impl(compact_repr: Cow<'_, str>) -> Result<RawJws<'_>, JwsError> {
    let first_dot_idx = compact_repr.find('.').ok_or_else(|| JwsError::InvalidEncoding {
        input: compact_repr.clone().into_owned(),
    })?;

    let last_dot_idx = compact_repr.rfind('.').ok_or_else(|| JwsError::InvalidEncoding {
        input: compact_repr.clone().into_owned(),
    })?;

    if first_dot_idx == last_dot_idx || compact_repr.starts_with('.') || compact_repr.ends_with('.') {
        return Err(JwsError::InvalidEncoding {
            input: compact_repr.into_owned(),
        });
    }

    let header_json = base64::decode_config(&compact_repr[..first_dot_idx], base64::URL_SAFE_NO_PAD)?;
    let header = serde_json::from_slice::<JwsHeader>(&header_json)?;

    let signature = base64::decode_config(&compact_repr[last_dot_idx + 1..], base64::URL_SAFE_NO_PAD)?;

    let payload = base64::decode_config(&compact_repr[first_dot_idx + 1..last_dot_idx], base64::URL_SAFE_NO_PAD)?;

    Ok(RawJws {
        compact_repr,
        header,
        payload,
        signature,
    })
}

/// JWS verification primitive
pub fn verify_signature(encoded_token: &str, public_key: &PublicKey, algorithm: JwsAlg) -> Result<(), JwsError> {
    let last_dot_idx = encoded_token.rfind('.').ok_or_else(|| JwsError::InvalidEncoding {
        input: encoded_token.to_owned(),
    })?;

    if encoded_token.ends_with('.') {
        return Err(JwsError::InvalidEncoding {
            input: encoded_token.to_owned(),
        });
    }

    let signature = base64::decode_config(&encoded_token[last_dot_idx + 1..], base64::URL_SAFE_NO_PAD)?;
    let signature_algo = SignatureAlgorithm::try_from(algorithm)?;
    signature_algo.verify(public_key, encoded_token[..last_dot_idx].as_bytes(), &signature)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pem::Pem;

    const PAYLOAD: &str = r#"{"sub":"1234567890","name":"John Doe","admin":true,"iat":1516239022}"#;

    fn get_private_key_1() -> PrivateKey {
        let pk_pem = crate::test_files::RSA_2048_PK_1.parse::<Pem>().unwrap();
        PrivateKey::from_pem(&pk_pem).unwrap()
    }

    fn get_private_key_2() -> PrivateKey {
        let pk_pem = crate::test_files::RSA_2048_PK_7.parse::<Pem>().unwrap();
        PrivateKey::from_pem(&pk_pem).unwrap()
    }

    #[test]
    fn encode_rsa_sha256() {
        let jwt = Jws {
            header: JwsHeader {
                typ: Some(String::from("JWT")),
                ..JwsHeader::new(JwsAlg::RS256)
            },
            payload: PAYLOAD.as_bytes().to_vec(),
        };
        let encoded = jwt.encode(&get_private_key_1()).unwrap();
        assert_eq!(encoded, crate::test_files::JOSE_JWT_SIG_EXAMPLE);
    }

    #[test]
    fn decode_rsa_sha256() {
        let public_key = get_private_key_1().to_public_key();
        let jwt = Jws::decode(crate::test_files::JOSE_JWT_SIG_EXAMPLE, &public_key).unwrap();
        assert_eq!(jwt.payload.as_slice(), PAYLOAD.as_bytes());
    }

    #[test]
    fn decode_rsa_sha256_delayed_signature_check() {
        let jws = RawJws::decode(crate::test_files::JOSE_JWT_SIG_EXAMPLE).unwrap();
        println!("{}", String::from_utf8_lossy(&jws.payload));
        assert_eq!(jws.peek_payload(), PAYLOAD.as_bytes());

        let public_key = get_private_key_2().to_public_key();
        let err = jws.verify(&public_key).err().unwrap();
        assert_eq!(err.to_string(), "signature error: invalid signature");
    }

    #[test]
    fn decode_rsa_sha256_invalid_signature_err() {
        let public_key = get_private_key_2().to_public_key();
        let err = Jws::decode(crate::test_files::JOSE_JWT_SIG_EXAMPLE, &public_key)
            .err()
            .unwrap();
        assert_eq!(err.to_string(), "signature error: invalid signature");
    }

    #[test]
    fn decode_invalid_base64_err() {
        let public_key = get_private_key_1().to_public_key();
        let err = Jws::decode("aieoè~†.tésp.à", &public_key).err().unwrap();
        assert_eq!(err.to_string(), "couldn\'t decode base64: Invalid byte 195, offset 4.");
    }

    #[test]
    fn decode_invalid_json_err() {
        let public_key = get_private_key_1().to_public_key();

        let err = Jws::decode("abc.abc.abc", &public_key).err().unwrap();
        assert_eq!(err.to_string(), "JSON error: expected value at line 1 column 1");

        let err = Jws::decode("eyAiYWxnIjogIkhTMjU2IH0K.abc.abc", &public_key)
            .err()
            .unwrap();
        assert_eq!(
            err.to_string(),
            "JSON error: control character (\\u0000-\\u001F) \
             found while parsing a string at line 2 column 0"
        );
    }

    #[test]
    fn decode_invalid_encoding_err() {
        let public_key = get_private_key_1().to_public_key();

        let err = Jws::decode(".abc.abc", &public_key).err().unwrap();
        assert_eq!(err.to_string(), "input isn\'t a valid token string: .abc.abc");

        let err = Jws::decode("abc.abc.", &public_key).err().unwrap();
        assert_eq!(err.to_string(), "input isn\'t a valid token string: abc.abc.");

        let err = Jws::decode("abc.abc", &public_key).err().unwrap();
        assert_eq!(err.to_string(), "input isn\'t a valid token string: abc.abc");

        let err = Jws::decode("abc", &public_key).err().unwrap();
        assert_eq!(err.to_string(), "input isn\'t a valid token string: abc");
    }
}
