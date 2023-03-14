//! JSON Web Encryption (JWE) represents encrypted content using JSON-based data structures.
//!
//! See [RFC7516](https://tools.ietf.org/html/rfc7516).

use crate::jose::jwk::Jwk;
use crate::key::{PrivateKey, PublicKey};
use aes_gcm::aead::generic_array::typenum::Unsigned;
use aes_gcm::{AeadInPlace, Aes128Gcm, Aes256Gcm, NewAead};
use base64::DecodeError;
use digest::generic_array::GenericArray;
use rand::RngCore;
use rsa::{PaddingScheme, PublicKey as RsaPublicKeyInterface, RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use thiserror::Error;

type Aes192Gcm = aes_gcm::AesGcm<aes_gcm::aes::Aes192, aes_gcm::aead::generic_array::typenum::U12>;

// === error type === //

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum JweError {
    /// RSA error
    #[error("RSA error: {context}")]
    Rsa { context: String },

    /// AES-GCM error (opaque)
    #[error("AES-GCM error (opaque)")]
    AesGcm,

    /// Json error
    #[error("JSON error: {source}")]
    Json { source: serde_json::Error },

    /// Key error
    #[error("Key error: {source}")]
    Key { source: crate::key::KeyError },

    /// Invalid token encoding
    #[error("input isn't a valid token string: {input}")]
    InvalidEncoding { input: String },

    /// Couldn't decode base64
    #[error("couldn't decode base64: {source}")]
    Base64Decoding { source: DecodeError },

    /// Input isn't valid utf8
    #[error("input isn't valid utf8: {source}, input: {input:?}")]
    InvalidUtf8 {
        source: std::string::FromUtf8Error,
        input: Vec<u8>,
    },

    /// Unsupported algorithm
    #[error("unsupported algorithm: {algorithm}")]
    UnsupportedAlgorithm { algorithm: String },

    /// Invalid size
    #[error("invalid size for {ty}: expected {expected}, got {got}")]
    InvalidSize {
        ty: &'static str,
        expected: usize,
        got: usize,
    },
}

impl From<rsa::errors::Error> for JweError {
    fn from(e: rsa::errors::Error) -> Self {
        Self::Rsa { context: e.to_string() }
    }
}

impl From<aes_gcm::Error> for JweError {
    fn from(_: aes_gcm::Error) -> Self {
        Self::AesGcm
    }
}

impl From<serde_json::Error> for JweError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json { source: e }
    }
}

impl From<crate::key::KeyError> for JweError {
    fn from(e: crate::key::KeyError) -> Self {
        Self::Key { source: e }
    }
}

impl From<DecodeError> for JweError {
    fn from(e: DecodeError) -> Self {
        Self::Base64Decoding { source: e }
    }
}

// === JWE algorithms === //

/// `alg` header parameter values for JWE used to determine the Content Encryption Key (CEK)
///
/// [JSON Web Algorithms (JWA) draft-ietf-jose-json-web-algorithms-40 #4](https://tools.ietf.org/html/draft-ietf-jose-json-web-algorithms-40#section-4.1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JweAlg {
    /// RSAES-PKCS1-V1_5
    ///
    /// Recommended- by RFC
    #[serde(rename = "RSA1_5")]
    RsaPkcs1v15,

    /// RSAES OAEP using default parameters
    ///
    /// Recommended+ by RFC
    #[serde(rename = "RSA-OAEP")]
    RsaOaep,

    /// RSAES OAEP using SHA-256 and MGF1 with SHA-256
    #[serde(rename = "RSA-OAEP-256")]
    RsaOaep256,

    /// AES Key Wrap with default initial value using 128 bit key (unsupported)
    ///
    /// Recommended by RFC
    #[serde(rename = "A128KW")]
    AesKeyWrap128,

    /// AES Key Wrap with default initial value using 192 bit key (unsupported)
    #[serde(rename = "A192KW")]
    AesKeyWrap192,

    /// AES Key Wrap with default initial value using 256 bit key (unsupported)
    ///
    /// Recommended by RFC
    #[serde(rename = "A256KW")]
    AesKeyWrap256,

    /// Direct use of a shared symmetric key as the CEK
    #[serde(rename = "dir")]
    Direct,

    /// Elliptic Curve Diffie-Hellman Ephemeral Static key agreement using Concat KDF (unsupported)
    ///
    /// Recommended+ by RFC
    #[serde(rename = "ECDH-ES")]
    EcdhEs,

    /// ECDH-ES using Concat KDF and CEK wrapped with "A128KW" (unsupported)
    ///
    /// Recommended by RFC
    ///
    /// Additional header used: "epk", "apu", "apv"
    #[serde(rename = "ECDH-ES+A128KW")]
    EcdhEsAesKeyWrap128,

    /// ECDH-ES using Concat KDF and CEK wrapped with "A192KW" (unsupported)
    ///
    /// Additional header used: "epk", "apu", "apv"
    #[serde(rename = "ECDH-ES+A192KW")]
    EcdhEsAesKeyWrap192,

    /// ECDH-ES using Concat KDF and CEK wrapped with "A256KW" (unsupported)
    ///
    /// Recommended by RFC
    ///
    /// Additional header used: "epk", "apu", "apv"
    #[serde(rename = "ECDH-ES+A256KW")]
    EcdhEsAesKeyWrap256,
}

// === JWE header === //

/// `enc` header parameter values for JWE to encrypt content
///
/// [JSON Web Algorithms (JWA) draft-ietf-jose-json-web-algorithms-40 #5](https://www.rfc-editor.org/rfc/rfc7518.html#section-5.1)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JweEnc {
    /// AES_128_CBC_HMAC_SHA_256 authenticated encryption algorithm. (unsupported)
    ///
    /// Required by RFC
    #[serde(rename = "A128CBC-HS256")]
    Aes128CbcHmacSha256,

    /// AES_192_CBC_HMAC_SHA_384 authenticated encryption algorithm. (unsupported)
    #[serde(rename = "A192CBC-HS384")]
    Aes192CbcHmacSha384,

    /// AES_256_CBC_HMAC_SHA_512 authenticated encryption algorithm. (unsupported)
    ///
    /// Required by RFC
    #[serde(rename = "A256CBC-HS512")]
    Aes256CbcHmacSha512,

    /// AES GCM using 128-bit key.
    ///
    /// Recommended by RFC
    #[serde(rename = "A128GCM")]
    Aes128Gcm,

    /// AES GCM using 192-bit key.
    #[serde(rename = "A192GCM")]
    Aes192Gcm,

    /// AES GCM using 256-bit key.
    ///
    /// Recommended by RFC
    #[serde(rename = "A256GCM")]
    Aes256Gcm,
}

impl JweEnc {
    pub fn key_size(self) -> usize {
        match self {
            Self::Aes128CbcHmacSha256 | Self::Aes128Gcm => <Aes128Gcm as NewAead>::KeySize::to_usize(),
            Self::Aes192CbcHmacSha384 | Self::Aes192Gcm => <Aes192Gcm as NewAead>::KeySize::to_usize(),
            Self::Aes256CbcHmacSha512 | Self::Aes256Gcm => <Aes256Gcm as NewAead>::KeySize::to_usize(),
        }
    }

    pub fn nonce_size(self) -> usize {
        match self {
            Self::Aes128Gcm | Self::Aes192Gcm | Self::Aes256Gcm => 12usize,
            Self::Aes128CbcHmacSha256 | Self::Aes192CbcHmacSha384 | Self::Aes256CbcHmacSha512 => 16usize,
        }
    }

    pub fn tag_size(self) -> usize {
        match self {
            Self::Aes128Gcm | Self::Aes192Gcm | Self::Aes256Gcm => 16usize,
            Self::Aes128CbcHmacSha256 => 32usize,
            Self::Aes192CbcHmacSha384 => 48usize,
            Self::Aes256CbcHmacSha512 => 64usize,
        }
    }
}

// === JWE header === //

/// JWE specific part of JOSE header
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JweHeader {
    // -- specific to JWE -- //
    /// Algorithm used to encrypt or determine the Content Encryption Key (CEK) (key wrapping...)
    pub alg: JweAlg,

    /// Content encryption algorithm to use
    ///
    /// This must be a *symmetric* Authenticated Encryption with Associated Data (AEAD) algorithm.
    pub enc: JweEnc,

    // -- common with JWS -- //
    /// JWK Set URL
    ///
    /// URI that refers to a resource for a set of JSON-encoded public keys,
    /// one of which corresponds to the key used to digitally sign the JWK.
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
    /// Used by JWE applications to declare the media type [IANA.MediaTypes] of this complete JWE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub typ: Option<String>,

    /// Content Type header
    ///
    /// Used by JWE applications to declare the media type [IANA.MediaTypes] of the secured content (the payload).
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

impl JweHeader {
    pub fn new(alg: JweAlg, enc: JweEnc) -> Self {
        Self {
            alg,
            enc,
            jku: None,
            jwk: None,
            typ: None,
            cty: None,
            kid: None,
            x5u: None,
            x5c: None,
            x5t: None,
            x5t_s256: None,
            additional: HashMap::default(),
        }
    }

    pub fn new_with_cty(alg: JweAlg, enc: JweEnc, cty: impl Into<String>) -> Self {
        Self {
            cty: Some(cty.into()),
            ..Self::new(alg, enc)
        }
    }
}

// === json web encryption === //

/// Provides an API to encrypt any kind of data (binary). JSON claims are part of `Jwt` only.
#[derive(Debug, Clone)]
pub struct Jwe {
    pub header: JweHeader,
    pub payload: Vec<u8>,
}

impl Jwe {
    pub fn new(alg: JweAlg, enc: JweEnc, payload: Vec<u8>) -> Self {
        Self {
            header: JweHeader::new(alg, enc),
            payload,
        }
    }

    /// Encodes with CEK encrypted and included in the token using asymmetric cryptography.
    pub fn encode(self, asymmetric_key: &PublicKey) -> Result<String, JweError> {
        encode_impl(self, EncoderMode::Asymmetric(asymmetric_key))
    }

    /// Encodes with provided CEK (a symmetric key). This will ignore `alg` value and override it with "dir".
    pub fn encode_direct(self, cek: &[u8]) -> Result<String, JweError> {
        encode_impl(self, EncoderMode::Direct(cek))
    }

    /// Decodes with CEK encrypted and included in the token using asymmetric cryptography.
    pub fn decode(compact_repr: &str, key: &PrivateKey) -> Result<Jwe, JweError> {
        RawJwe::decode(compact_repr).and_then(|jwe| jwe.decrypt(key))
    }

    /// Decodes with provided CEK (a symmetric key).
    pub fn decode_direct(compact_repr: &str, cek: &[u8]) -> Result<Jwe, JweError> {
        RawJwe::decode(compact_repr).and_then(|jwe| jwe.decrypt_direct(cek))
    }
}

/// Raw low-level interface to the yet to be decoded JWE token.
///
/// This is useful to inspect the structure before performing further processing.
/// For most usecases, use `Jwe` directly.
#[derive(Debug, Clone)]
pub struct RawJwe<'repr> {
    pub compact_repr: Cow<'repr, str>,
    pub header: JweHeader,
    pub encrypted_key: Vec<u8>,
    pub initialization_vector: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub authentication_tag: Vec<u8>,
}

/// An owned `RawJws` for convenience.
pub type OwnedRawJwe = RawJwe<'static>;

impl<'repr> RawJwe<'repr> {
    /// Decodes a JWE in compact representation.
    pub fn decode(compact_repr: impl Into<Cow<'repr, str>>) -> Result<Self, JweError> {
        decode_impl(compact_repr.into())
    }

    /// Decrypts the ciphertext using asymmetric cryptography and returns a verified `Jwe` structure.
    pub fn decrypt(self, key: &PrivateKey) -> Result<Jwe, JweError> {
        decrypt_impl(self, DecoderMode::Normal(key))
    }

    /// Decrypts the ciphertext using the provided CEK (a symmetric key).
    pub fn decrypt_direct(self, cek: &[u8]) -> Result<Jwe, JweError> {
        decrypt_impl(self, DecoderMode::Direct(cek))
    }
}

fn decode_impl(compact_repr: Cow<'_, str>) -> Result<RawJwe<'_>, JweError> {
    fn parse_compact_repr(compact_repr: &str) -> Option<(&str, &str, &str, &str, &str)> {
        let mut split = compact_repr.splitn(5, '.');

        let protected_header = split.next()?;
        let encrypted_key = split.next()?;
        let initialization_vector = split.next()?;
        let ciphertext = split.next()?;
        let authentication_tag = split.next()?;

        Some((
            protected_header,
            encrypted_key,
            initialization_vector,
            ciphertext,
            authentication_tag,
        ))
    }

    let (protected_header, encrypted_key, initialization_vector, ciphertext, authentication_tag) =
        parse_compact_repr(&compact_repr).ok_or_else(|| JweError::InvalidEncoding {
            input: compact_repr.clone().into_owned(),
        })?;

    let protected_header = base64::decode_config(protected_header, base64::URL_SAFE_NO_PAD)?;
    let header = serde_json::from_slice::<JweHeader>(&protected_header)?;

    Ok(RawJwe {
        header,
        encrypted_key: base64::decode_config(encrypted_key, base64::URL_SAFE_NO_PAD)?,
        initialization_vector: base64::decode_config(initialization_vector, base64::URL_SAFE_NO_PAD)?,
        ciphertext: base64::decode_config(ciphertext, base64::URL_SAFE_NO_PAD)?,
        authentication_tag: base64::decode_config(authentication_tag, base64::URL_SAFE_NO_PAD)?,
        compact_repr,
    })
}

// encoder

#[derive(Debug, Clone)]
enum EncoderMode<'a> {
    Asymmetric(&'a PublicKey),
    Direct(&'a [u8]),
}

fn encode_impl(jwe: Jwe, mode: EncoderMode) -> Result<String, JweError> {
    let mut header = jwe.header;
    let protected_header_base64 = base64::encode_config(&serde_json::to_vec(&header)?, base64::URL_SAFE_NO_PAD);

    let (encrypted_key_base64, jwe_cek) = match mode {
        EncoderMode::Direct(symmetric_key) => {
            if symmetric_key.len() != header.enc.key_size() {
                return Err(JweError::InvalidSize {
                    ty: "symmetric key",
                    expected: header.enc.key_size(),
                    got: symmetric_key.len(),
                });
            }

            // Override `alg` header with "dir"
            header.alg = JweAlg::Direct;

            (String::new(), Cow::Borrowed(symmetric_key))
        }
        EncoderMode::Asymmetric(public_key) => {
            // Currently, only rsa is supported
            let rsa_public_key = RsaPublicKey::try_from(public_key)?;

            let mut rng = rand::rngs::OsRng;

            let mut symmetric_key = vec![0u8; header.enc.key_size()];
            rng.fill_bytes(&mut symmetric_key);

            let padding = match header.alg {
                JweAlg::RsaPkcs1v15 => PaddingScheme::new_pkcs1v15_encrypt(),
                JweAlg::RsaOaep => PaddingScheme::new_oaep::<sha1::Sha1>(),
                JweAlg::RsaOaep256 => PaddingScheme::new_oaep::<sha2::Sha256>(),
                unsupported => {
                    return Err(JweError::UnsupportedAlgorithm {
                        algorithm: format!("{:?}", unsupported),
                    })
                }
            };

            let encrypted_key = rsa_public_key.encrypt(&mut rng, padding, &symmetric_key)?;

            (
                base64::encode_config(&encrypted_key, base64::URL_SAFE_NO_PAD),
                Cow::Owned(symmetric_key),
            )
        }
    };

    let mut buffer = jwe.payload;
    let nonce = <aes_gcm::aead::Nonce<Aes128Gcm> as From<[u8; 12]>>::from(rand::random()); // 96-bits nonce for all AES-GCM variants
    let aad = protected_header_base64.as_bytes(); // The Additional Authenticated Data value used for AES-GCM.
    let authentication_tag = match header.enc {
        JweEnc::Aes128Gcm => {
            Aes128Gcm::new(GenericArray::from_slice(&jwe_cek)).encrypt_in_place_detached(&nonce, aad, &mut buffer)?
        }
        JweEnc::Aes192Gcm => {
            Aes192Gcm::new(GenericArray::from_slice(&jwe_cek)).encrypt_in_place_detached(&nonce, aad, &mut buffer)?
        }
        JweEnc::Aes256Gcm => {
            Aes256Gcm::new(GenericArray::from_slice(&jwe_cek)).encrypt_in_place_detached(&nonce, aad, &mut buffer)?
        }
        unsupported => {
            return Err(JweError::UnsupportedAlgorithm {
                algorithm: format!("{:?}", unsupported),
            })
        }
    };

    let initialization_vector_base64 = base64::encode_config(nonce.as_slice(), base64::URL_SAFE_NO_PAD);
    let ciphertext_base64 = base64::encode_config(&buffer, base64::URL_SAFE_NO_PAD);
    let authentication_tag_base64 = base64::encode_config(&authentication_tag, base64::URL_SAFE_NO_PAD);

    Ok([
        protected_header_base64,
        encrypted_key_base64,
        initialization_vector_base64,
        ciphertext_base64,
        authentication_tag_base64,
    ]
    .join("."))
}

// decoder

#[derive(Clone)]
enum DecoderMode<'a> {
    Normal(&'a PrivateKey),
    Direct(&'a [u8]),
}

fn decrypt_impl(raw: RawJwe<'_>, mode: DecoderMode<'_>) -> Result<Jwe, JweError> {
    let RawJwe {
        compact_repr,
        header,
        encrypted_key,
        initialization_vector,
        ciphertext,
        authentication_tag,
    } = raw;

    let protected_header_base64 = compact_repr
        .split('.')
        .next()
        .ok_or_else(|| JweError::InvalidEncoding {
            input: compact_repr.clone().into_owned(),
        })?;

    let jwe_cek = match mode {
        DecoderMode::Direct(symmetric_key) => Cow::Borrowed(symmetric_key),
        DecoderMode::Normal(private_key) => {
            let rsa_private_key = RsaPrivateKey::try_from(private_key)?;

            let padding = match header.alg {
                JweAlg::RsaPkcs1v15 => PaddingScheme::new_pkcs1v15_encrypt(),
                JweAlg::RsaOaep => PaddingScheme::new_oaep::<sha1::Sha1>(),
                JweAlg::RsaOaep256 => PaddingScheme::new_oaep::<sha2::Sha256>(),
                unsupported => {
                    return Err(JweError::UnsupportedAlgorithm {
                        algorithm: format!("{:?}", unsupported),
                    })
                }
            };

            let decrypted_key = rsa_private_key.decrypt(padding, &encrypted_key)?;

            Cow::Owned(decrypted_key)
        }
    };

    if jwe_cek.len() != header.enc.key_size() {
        return Err(JweError::InvalidSize {
            ty: "symmetric key",
            expected: header.enc.key_size(),
            got: jwe_cek.len(),
        });
    }

    if initialization_vector.len() != header.enc.nonce_size() {
        return Err(JweError::InvalidSize {
            ty: "initialization vector (nonce)",
            expected: header.enc.nonce_size(),
            got: initialization_vector.len(),
        });
    }

    if authentication_tag.len() != header.enc.tag_size() {
        return Err(JweError::InvalidSize {
            ty: "authentication tag",
            expected: header.enc.tag_size(),
            got: authentication_tag.len(),
        });
    }

    let mut buffer = ciphertext;
    let nonce = GenericArray::from_slice(&initialization_vector);
    let aad = protected_header_base64.as_bytes(); // The Additional Authenticated Data value used for AES-GCM.
    match header.enc {
        JweEnc::Aes128Gcm => Aes128Gcm::new(GenericArray::from_slice(&jwe_cek)).decrypt_in_place_detached(
            nonce,
            aad,
            &mut buffer,
            GenericArray::from_slice(&authentication_tag),
        )?,
        JweEnc::Aes192Gcm => Aes192Gcm::new(GenericArray::from_slice(&jwe_cek)).decrypt_in_place_detached(
            nonce,
            aad,
            &mut buffer,
            GenericArray::from_slice(&authentication_tag),
        )?,
        JweEnc::Aes256Gcm => Aes256Gcm::new(GenericArray::from_slice(&jwe_cek)).decrypt_in_place_detached(
            nonce,
            aad,
            &mut buffer,
            GenericArray::from_slice(&authentication_tag),
        )?,
        unsupported => {
            return Err(JweError::UnsupportedAlgorithm {
                algorithm: format!("{:?}", unsupported),
            })
        }
    };

    Ok(Jwe {
        header,
        payload: buffer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::PrivateKey;
    use crate::pem::Pem;

    fn get_private_key_1() -> PrivateKey {
        let pk_pem = crate::test_files::RSA_2048_PK_1.parse::<Pem>().unwrap();
        PrivateKey::from_pem(&pk_pem).expect("private_key 1")
    }

    fn get_private_key_2() -> PrivateKey {
        let pk_pem = crate::test_files::RSA_2048_PK_7.parse::<Pem>().unwrap();
        PrivateKey::from_pem(&pk_pem).expect("private_key 7")
    }

    #[test]
    fn rsa_oaep_aes_128_gcm() {
        let payload = "何だと？……無駄な努力だ？……百も承知だ！だがな、勝つ望みがある時ばかり、戦うのとは訳が違うぞ！"
            .as_bytes()
            .to_vec();

        let private_key = get_private_key_1();
        let public_key = private_key.to_public_key();

        let jwe = Jwe::new(JweAlg::RsaOaep, JweEnc::Aes128Gcm, payload);
        let encoded = jwe.clone().encode(&public_key).unwrap();

        let decoded = Jwe::decode(&encoded, &private_key).unwrap();

        assert_eq!(jwe.payload, decoded.payload);
        assert_eq!(jwe.header, decoded.header);
    }

    #[test]
    fn rsa_pkcs1v15_aes_128_gcm_bad_key() {
        let payload = "そうとも！ 負けると知って戦うのが、遙かに美しいのだ！"
            .as_bytes()
            .to_vec();

        let private_key = get_private_key_1();
        let public_key = get_private_key_2().to_public_key();

        let jwe = Jwe::new(JweAlg::RsaPkcs1v15, JweEnc::Aes128Gcm, payload);
        let encoded = jwe.clone().encode(&public_key).unwrap();

        let err = Jwe::decode(&encoded, &private_key).err().unwrap();
        assert_eq!(err.to_string(), "RSA error: decryption error");
    }

    #[test]
    fn direct_aes_256_gcm() {
        let payload = "さあ、取れ、取るがいい！だがな、貴様たちがいくら騒いでも、あの世へ、俺が持って行くものが一つある！それはな…".as_bytes().to_vec();

        let key = "わたしの……心意気だ!!";

        let jwe = Jwe::new(JweAlg::Direct, JweEnc::Aes256Gcm, payload);
        let encoded = jwe.clone().encode_direct(key.as_bytes()).unwrap();

        let decoded = Jwe::decode_direct(&encoded, key.as_bytes()).unwrap();

        assert_eq!(jwe.payload, decoded.payload);
        assert_eq!(jwe.header, decoded.header);
    }

    #[test]
    fn direct_aes_192_gcm_bad_key() {
        let payload = "和解をしよう？ 俺が？ 真っ平だ！ 真っ平御免だ！".as_bytes().to_vec();

        let jwe = Jwe::new(JweAlg::Direct, JweEnc::Aes192Gcm, payload);
        let encoded = jwe.clone().encode_direct(b"abcdefghabcdefghabcdefgh").unwrap();

        let err = Jwe::decode_direct(&encoded, b"zzzzzzzzabcdefghzzzzzzzz").err().unwrap();
        assert_eq!(err.to_string(), "AES-GCM error (opaque)");
    }

    #[test]
    #[ignore = "this is not directly using picky code"]
    fn rfc7516_example_using_rsaes_oaep_and_aes_gcm() {
        // See: https://tools.ietf.org/html/rfc7516#appendix-A.1

        let plaintext = b"The true sign of intelligence is not knowledge but imagination.";
        let jwe = Jwe::new(JweAlg::RsaOaep, JweEnc::Aes256Gcm, plaintext.to_vec());

        // 1: JOSE header

        let protected_header_base64 =
            base64::encode_config(&serde_json::to_vec(&jwe.header).unwrap(), base64::URL_SAFE_NO_PAD);
        assert_eq!(
            protected_header_base64,
            "eyJhbGciOiJSU0EtT0FFUCIsImVuYyI6IkEyNTZHQ00ifQ"
        );

        // 2: Content Encryption Key (CEK)

        let cek = [
            177, 161, 244, 128, 84, 143, 225, 115, 63, 180, 3, 255, 107, 154, 212, 246, 138, 7, 110, 91, 112, 46, 34,
            105, 47, 130, 203, 46, 122, 234, 64, 252,
        ];

        // 3: Key Encryption

        let encrypted_key_base64 = "OKOawDo13gRp2ojaHV7LFpZcgV7T6DVZKTyKOMTYUmKoTCVJRgckCL9kiMT03JGeipsEdY3mx_etLbbWSrFr05kLzcSr4qKAq7YN7e9jwQRb23nfa6c9d-StnImGyFDbSv04uVuxIp5Zms1gNxKKK2Da14B8S4rzVRltdYwam_lDp5XnZAYpQdb76FdIKLaVmqgfwX7XWRxv2322i-vDxRfqNzo_tETKzpVLzfiwQyeyPGLBIO56YJ7eObdv0je81860ppamavo35UgoRdbYaBcoh9QcfylQr66oc6vFWXRcZ_ZT2LawVCWTIy3brGPi6UklfCpIMfIjf7iGdXKHzg";

        // 4: Initialization Vector

        let iv_base64 = "48V1_ALb6US04U3b";
        let iv = base64::decode_config(iv_base64, base64::URL_SAFE_NO_PAD).unwrap();

        // 5: AAD

        let aad = protected_header_base64.as_bytes();

        // 6: Content Encryption

        let mut buffer = plaintext.to_vec();
        let tag = Aes256Gcm::new(GenericArray::from_slice(&cek))
            .encrypt_in_place_detached(GenericArray::from_slice(&iv), aad, &mut buffer)
            .unwrap();
        let ciphertext = buffer;

        assert_eq!(
            ciphertext,
            [
                229, 236, 166, 241, 53, 191, 115, 196, 174, 43, 73, 109, 39, 122, 233, 96, 140, 206, 120, 52, 51, 237,
                48, 11, 190, 219, 186, 80, 111, 104, 50, 142, 47, 167, 59, 61, 181, 127, 196, 21, 40, 82, 242, 32, 123,
                143, 168, 226, 73, 216, 176, 144, 138, 247, 106, 60, 16, 205, 160, 109, 64, 63, 192
            ]
            .to_vec()
        );
        assert_eq!(
            tag.as_slice(),
            &[92, 80, 104, 49, 133, 25, 161, 215, 173, 101, 219, 211, 136, 91, 210, 145]
        );

        // 7: Complete Representation

        let token = format!(
            "{}.{}.{}.{}.{}",
            protected_header_base64,
            encrypted_key_base64,
            iv_base64,
            base64::encode_config(&ciphertext, base64::URL_SAFE_NO_PAD),
            base64::encode_config(&tag, base64::URL_SAFE_NO_PAD),
        );

        assert_eq!(token, "eyJhbGciOiJSU0EtT0FFUCIsImVuYyI6IkEyNTZHQ00ifQ.OKOawDo13gRp2ojaHV7LFpZcgV7T6DVZKTyKOMTYUmKoTCVJRgckCL9kiMT03JGeipsEdY3mx_etLbbWSrFr05kLzcSr4qKAq7YN7e9jwQRb23nfa6c9d-StnImGyFDbSv04uVuxIp5Zms1gNxKKK2Da14B8S4rzVRltdYwam_lDp5XnZAYpQdb76FdIKLaVmqgfwX7XWRxv2322i-vDxRfqNzo_tETKzpVLzfiwQyeyPGLBIO56YJ7eObdv0je81860ppamavo35UgoRdbYaBcoh9QcfylQr66oc6vFWXRcZ_ZT2LawVCWTIy3brGPi6UklfCpIMfIjf7iGdXKHzg.48V1_ALb6US04U3b.5eym8TW_c8SuK0ltJ3rpYIzOeDQz7TALvtu6UG9oMo4vpzs9tX_EFShS8iB7j6jiSdiwkIr3ajwQzaBtQD_A.XFBoMYUZodetZdvTiFvSkQ");
    }
}
