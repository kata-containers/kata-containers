//! A JSON Web Key (JWK) is a JavaScript Object Notation (JSON) data structure that represents a cryptographic key.
//!
//! See [RFC7517](https://tools.ietf.org/html/rfc7517).

use crate::jose::jwe::{JweAlg, JweEnc};
use crate::jose::jws::JwsAlg;
use crate::key::PublicKey;
use base64::DecodeError;
use picky_asn1::wrapper::IntegerAsn1;
use picky_asn1_x509::SubjectPublicKeyInfo;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// === error type === //

#[derive(Debug, Error)]
pub enum JwkError {
    /// Json error
    #[error("JSON error: {source}")]
    Json { source: serde_json::Error },

    /// couldn't decode base64
    #[error("couldn't decode base64: {source}")]
    Base64Decoding { source: DecodeError },

    /// unsupported algorithm
    #[error("unsupported algorithm: {algorithm}")]
    UnsupportedAlgorithm { algorithm: &'static str },
}

impl From<serde_json::Error> for JwkError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json { source: e }
    }
}

impl From<DecodeError> for JwkError {
    fn from(e: DecodeError) -> Self {
        Self::Base64Decoding { source: e }
    }
}

// === key type === //

/// Algorithm type for JWK
///
/// See [RFC7518 #6](https://tools.ietf.org/html/rfc7518#section-6.1)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kty")]
pub enum JwkKeyType {
    /// Elliptic Curve (unsupported)
    ///
    /// Recommended+ by RFC
    #[serde(rename = "EC")]
    Ec,
    /// Elliptic Curve
    ///
    /// Required by RFC
    #[serde(rename = "RSA")]
    Rsa(JwkPublicRsaKey),
    /// Octet sequence (used to represent symmetric keys) (unsupported)
    ///
    /// Required by RFC
    #[serde(rename = "oct")]
    Oct,
}

impl JwkKeyType {
    /// Build a JWK key from RSA components.
    ///
    /// Each argument is the unsigned big-endian representation as an octet sequence of the value.
    /// If a signed representation is provided, leading zero is removed for any number bigger than 0x7F.
    pub fn new_rsa_key(modulus: &[u8], public_exponent: &[u8]) -> Self {
        let modulus = Self::h_strip_unrequired_leading_zero(modulus);
        let public_exponent = Self::h_strip_unrequired_leading_zero(public_exponent);
        Self::Rsa(JwkPublicRsaKey {
            n: base64::encode_config(modulus, base64::URL_SAFE_NO_PAD),
            e: base64::encode_config(public_exponent, base64::URL_SAFE_NO_PAD),
        })
    }

    /// Build a JWK key from RSA components already encoded following base64 url format.
    ///
    /// Each argument is the unsigned big-endian representation as an octet sequence of the value.
    /// The octet sequence MUST utilize the minimum number of octets needed to represent the value.
    /// That is: **no leading zero** must be present.
    ///
    /// See definition for term `Base64urlUInt` in [RFC7518 section 2](https://datatracker.ietf.org/doc/html/rfc7518#section-2)
    pub fn new_rsa_key_from_base64_url(modulus: String, public_exponent: String) -> Self {
        Self::Rsa(JwkPublicRsaKey {
            n: modulus,
            e: public_exponent,
        })
    }

    pub fn as_rsa(&self) -> Option<&JwkPublicRsaKey> {
        match self {
            JwkKeyType::Rsa(rsa) => Some(rsa),
            _ => None,
        }
    }

    pub fn is_rsa(&self) -> bool {
        self.as_rsa().is_some()
    }

    /// Strips leading zero for any number bigger than 0x7F.
    fn h_strip_unrequired_leading_zero(value: &[u8]) -> &[u8] {
        if let [0x00, rest @ ..] = value {
            rest
        } else {
            value
        }
    }
}

// === public key use === //

/// Public Key Use, identifies the intended use of the public key.
///
/// See [RFC7517 #4](https://tools.ietf.org/html/rfc7517#section-4.2)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JwkPubKeyUse {
    #[serde(rename = "sig")]
    Signature,
    #[serde(rename = "enc")]
    Encryption,
}

// === key operations === //

/// Key Operations, identifies the operation(s) for which the key is intended to be used.
///
/// See [RFC7517 #4](https://tools.ietf.org/html/rfc7517#section-4.3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JwkKeyOps {
    #[serde(rename = "sign")]
    Sign,
    #[serde(rename = "verify")]
    Verify,
    #[serde(rename = "encrypt")]
    Encrypt,
    #[serde(rename = "decrypt")]
    Decrypt,
    #[serde(rename = "wrapKey")]
    WrapKey,
    #[serde(rename = "unwrapKey")]
    UnwrapKey,
    #[serde(rename = "deriveKey")]
    DeriveKey,
    #[serde(rename = "deriveBits")]
    DeriveBits,
}

// === algorithms === //

/// JOSE algorithms names as defined by [RFC7518](https://tools.ietf.org/html/rfc7518)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Jwa {
    Sig(JwsAlg),
    Enc(JweEnc),
    CEKAlg(JweAlg),
}

// === json web key === //

/// Represents a cryptographic key as defined by [RFC7517](https://tools.ietf.org/html/rfc7517).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Jwk {
    // -- specific to JWK -- //
    #[serde(flatten)]
    pub key: JwkKeyType,

    /// Identifies the algorithm intended for use with the key.
    pub alg: Option<Jwa>,

    /// Public Key Use
    ///
    /// Intended use of the public key.
    #[serde(rename = "use", skip_serializing_if = "Option::is_none")]
    pub key_use: Option<JwkPubKeyUse>,

    /// Key Operations
    ///
    /// identifies the operation(s) for which the key is intended to be used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_ops: Option<Vec<JwkKeyOps>>,

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
}

impl Jwk {
    pub fn new(key: JwkKeyType) -> Self {
        Jwk {
            key,
            alg: None,
            key_use: None,
            key_ops: None,
            kid: None,
            x5u: None,
            x5c: None,
            x5t: None,
            x5t_s256: None,
        }
    }

    pub fn from_json(json: &str) -> Result<Self, JwkError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn from_public_key(public_key: &PublicKey) -> Result<Self, JwkError> {
        use picky_asn1::wrapper::BitStringAsn1Container;
        use picky_asn1_x509::PublicKey as SerdePublicKey;

        match &public_key.as_inner().subject_public_key {
            SerdePublicKey::Rsa(BitStringAsn1Container(rsa)) => {
                let modulus = rsa.modulus.as_signed_bytes_be();
                let public_exponent = rsa.public_exponent.as_signed_bytes_be();
                Ok(Self::new(JwkKeyType::new_rsa_key(modulus, public_exponent)))
            }
            SerdePublicKey::Ec(_) => Err(JwkError::UnsupportedAlgorithm {
                algorithm: "elliptic curves",
            }),
            SerdePublicKey::Ed(_) => Err(JwkError::UnsupportedAlgorithm {
                algorithm: "edwards curves",
            }),
        }
    }

    pub fn to_json(&self) -> Result<String, JwkError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn to_json_pretty(&self) -> Result<String, JwkError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn to_public_key(&self) -> Result<PublicKey, JwkError> {
        match &self.key {
            JwkKeyType::Rsa(rsa) => {
                let modulus = IntegerAsn1::from_bytes_be_signed(rsa.modulus_signed_bytes_be()?);
                let public_exponent = IntegerAsn1::from_bytes_be_signed(rsa.public_exponent_signed_bytes_be()?);
                let spki = SubjectPublicKeyInfo::new_rsa_key(modulus, public_exponent);
                Ok(spki.into())
            }
            JwkKeyType::Ec => Err(JwkError::UnsupportedAlgorithm {
                algorithm: "elliptic curves",
            }),
            JwkKeyType::Oct => Err(JwkError::UnsupportedAlgorithm {
                algorithm: "octet sequence",
            }),
        }
    }
}

// === jwk set === //

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JwkSet {
    pub keys: Vec<Jwk>,
}

impl JwkSet {
    pub fn from_json(json: &str) -> Result<Self, JwkError> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn to_json(&self) -> Result<String, JwkError> {
        Ok(serde_json::to_string(self)?)
    }

    pub fn to_json_pretty(&self) -> Result<String, JwkError> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

// === public rsa key === //

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JwkPublicRsaKey {
    n: String,
    e: String,
}

impl JwkPublicRsaKey {
    pub fn modulus_signed_bytes_be(&self) -> Result<Vec<u8>, JwkError> {
        let mut buf = Self::h_allocate_signed_big_int_buffer(&self.n);
        base64::decode_config_buf(&self.n, base64::URL_SAFE_NO_PAD, &mut buf).map_err(JwkError::from)?;
        Ok(buf)
    }

    pub fn modulus_unsigned_bytes_be(&self) -> Result<Vec<u8>, JwkError> {
        base64::decode_config(&self.n, base64::URL_SAFE_NO_PAD).map_err(JwkError::from)
    }

    pub fn public_exponent_signed_bytes_be(&self) -> Result<Vec<u8>, JwkError> {
        let mut buf = Self::h_allocate_signed_big_int_buffer(&self.e);
        base64::decode_config_buf(&self.e, base64::URL_SAFE_NO_PAD, &mut buf).map_err(JwkError::from)?;
        Ok(buf)
    }

    pub fn public_exponent_unsigned_bytes_be(&self) -> Result<Vec<u8>, JwkError> {
        base64::decode_config(&self.e, base64::URL_SAFE_NO_PAD).map_err(JwkError::from)
    }

    fn h_allocate_signed_big_int_buffer(base64_url_encoding: &str) -> Vec<u8> {
        // Big integers from 0x00 to 0x7F are all base64-encoded using two ASCII characters ranging from "AA" to "fw".
        // We know the required capacity is _exactly_ of one byte.
        // The value 0 is valid and is represented as the array [0x00] ("AA").
        // For numbers greater than 0x7F, logic is a bit more complex.
        // There is no leading zero in JWK keys because _unsigned_ numbers are used.
        // As such, there is no need to disambiguate the high-order bit (0x80)
        // which is used as the sign bit for _signed_ numbers.
        // The high-order bit is set when base64 encoding's leading character matches [g-z0-9_-].
        match base64_url_encoding.chars().next() {
            // The leading zero is re-introduced for any number whose high-order bit is set
            Some('g'..='z' | '0'..='9' | '_' | '-') => vec![0],
            // Otherwise, there is nothing more to do
            _ => Vec::with_capacity(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jose::jws::JwsAlg;
    use crate::pem::Pem;

    const RSA_MODULUS: &str = "rpJjxW0nNZiq1mPC3ZAxqf9qNjmKurP7XuKrpWrfv3IOUldqChQVPNg8zCvDOMZIO-ZDuRmVH\
                               EZ5E1vz5auHNACnpl6AvDGJ-4qyX42vfUDMNZx8i86d7bQpwJkO_MVMLj8qMGmTVbQ8zqVw2z\
                               MyKUFfa2V83nvx2wz4FJh2Thw2uZX2P7h8nlDVSuXO0wJ_OY_2qtqRIAnNXMzL5BF5pEFh4hi\
                               JIFiMTNkhVtUjT1QSB9E8DtDme8g4u769Oc0My45fgqSNE7kKKyaDhTfqSovyhj-qWiD-X_Gw\
                               pWkW4ungpHzz_97-ZDB3yQ7AMwKAsw5EW2cMqseAp3f-kf159w";

    const RSA_PUBLIC_EXPONENT: &str = "AQAB";

    const X509_SHA1_THUMBPRINT: &str = "N3ORVnr9T6opxpS9iRbkKGwKiQI";

    const X509_CERT_0: &str = "MIIDWjCCAkKgAwIBAgIUWRsBqKmpXGP/OwrwLWicwxhuCFowDQYJKoZIhvc\
                               NAQELBQAwKjEoMCYGA1UEAwwfbG9naW4uZGV2b2x1dGlvbnMuY29tIEF1dG\
                               hvcml0eTAeFw0xOTAzMTMxMzE1MzVaFw0yMDAzMTIxMzE1MzVaMCYxJDAiB\
                               gNVBAMMG2xvZ2luLmRldm9sdXRpb25zLmNvbSBUb2tlbjCCASIwDQYJKoZI\
                               hvcNAQEBBQADggEPADCCAQoCggEBAK6SY8VtJzWYqtZjwt2QMan/ajY5irq\
                               z+17iq6Vq379yDlJXagoUFTzYPMwrwzjGSDvmQ7kZlRxGeRNb8+WrhzQAp6\
                               ZegLwxifuKsl+Nr31AzDWcfIvOne20KcCZDvzFTC4/KjBpk1W0PM6lcNszM\
                               ilBX2tlfN578dsM+BSYdk4cNrmV9j+4fJ5Q1UrlztMCfzmP9qrakSAJzVzM\
                               y+QReaRBYeIYiSBYjEzZIVbVI09UEgfRPA7Q5nvIOLu+vTnNDMuOX4KkjRO\
                               5Cismg4U36kqL8oY/qlog/l/xsKVpFuLp4KR88//e/mQwd8kOwDMCgLMORF\
                               tnDKrHgKd3/pH9efcCAwEAAaN8MHowCQYDVR0TBAIwADAOBgNVHQ8BAf8EB\
                               AMCBeAwHQYDVR0lBBYwFAYIKwYBBQUHAwEGCCsGAQUFBwMCMB0GA1UdDgQW\
                               BBQQW2Cx8HUpXfFM3B76WzBb/BhCBDAfBgNVHSMEGDAWgBRWAUlOiE4Z3ww\
                               aHgz284/sYB9NaDANBgkqhkiG9w0BAQsFAAOCAQEAkliCiJF9Z/Y57V6Rrn\
                               gHCBBWtqR+N/A+KHQqWxP2MmJiHVBBnZAueVPsvykO+EfbazNEkUoPVKhUd\
                               5NxEmTEMOBu9HUEzlmA5xDjl5xS7fejJIr7pgbxIup4m+DsNsPVnF1Snk56\
                               F6660RhRb9fsHQ0pgvWuG+tQXJ4J1Zi0cp+xi4yze6hJGAyAqj6wU46AUiL\
                               6kUr9GUVHqEsl5mNMIW18JT4KM/s5DWxFGO2soSTkaVHwGSkMBQSTgHMWs0\
                               L3bBfimjw9FwjwwHAbe1W5QU6uVXGApuKANRsXxgCn566QkE/BuV3WVR6uy\
                               n2P1J/vU9hxasgRIcjf3jHC4lGpew==";

    const X509_CERT_1: &str = "MIIDRjCCAi6gAwIBAgIUUqhc3/U6OhKtEk1b8JfX3GL0FPYwDQYJKoZIhvc\
                               NAQELBQAwKDEmMCQGA1UEAwwdbG9naW4uZGV2b2x1dGlvbnMuY29tIFJvb3\
                               QgQ0EwHhcNMTkwMzEzMTMxNTM1WhcNMjAwMzEyMTMxNTM1WjAqMSgwJgYDV\
                               QQDDB9sb2dpbi5kZXZvbHV0aW9ucy5jb20gQXV0aG9yaXR5MIIBIjANBgkq\
                               hkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAlbRwXVPc/WH4t/Yti5qv24pAu8Q\
                               m0eOVvbum23bYtfJDbCSDh7sY/vvQXgIkM8/0C3tFZ3XaXHbyDHAMn6OC+S\
                               Obzs6SjpfKk9s69Yo/aWFl9oRnAK/+dZ0Y6MTdZO1w+PpR81q5QOFMLpWX1\
                               YNdahaZec31sBmsHqlW04OrHUhGOTGdWNots9/PWvN//x++FL+Sqgh/jxF7\
                               khbgfAuz1QKa8P0ZlE4cOcRIs5bSnUFwtoytKH02/YZnCJD7I/iXFuCPV/+\
                               LZO6yobkTREE3npeXvAKr1OKF2F0JVORMhHiYyguh9t3bMwHTCFqmfQkIMD\
                               GjaTJD7bd8y2Au+eDzgwIDAQABo2YwZDAOBgNVHQ8BAf8EBAMCAQYwEgYDV\
                               R0TAQH/BAgwBgEB/wIBAjAdBgNVHQ4EFgQUVgFJTohOGd8MGh4M9vOP7GAf\
                               TWgwHwYDVR0jBBgwFoAU42BA1coGHUUPUSeacQfTzicjosgwDQYJKoZIhvc\
                               NAQELBQADggEBAKyyDs+uIughmloEmlf8s1cSP8cLtC1Di2TfYSG0bpEM3B\
                               EPTond/7ujDlv0eug9NRurvWd5v7bWvy9VlJo+x2rLBmkzaNcBSVHZ4UbFU\
                               90MSvHjxNZ7VbUfbWsJVeaYHtqf1m3z0fYT0tUor3chD+wbSqraWw4+t54h\
                               fJl22jExTWS9X0F5/Gf3LQOiOvtjHP+b3VkpXkEPIBbvIO/X6kgoGDLm/lA\
                               IPdZmpI956z5+acLHu3AQkxNXQPzCjSSdJphLVU1XeHXOMWldVtE9BqSMVI\
                               HZ6oCz/FtMA4F6R7WiVXXGR+ywRwFyeiFoRea2ImUK9TRWFsaXKeOBMm+TL\
                               bk=";

    const X509_CERT_2: &str = "MIIDRDCCAiygAwIBAgIUCAKwhsjTttdG4koEAV7zqlnI7wkwDQYJKoZIhvc\
                               NAQELBQAwKDEmMCQGA1UEAwwdbG9naW4uZGV2b2x1dGlvbnMuY29tIFJvb3\
                               QgQ0EwHhcNMTkwMzEzMTMxNTM1WhcNMjQwMzExMTMxNTM1WjAoMSYwJAYDV\
                               QQDDB1sb2dpbi5kZXZvbHV0aW9ucy5jb20gUm9vdCBDQTCCASIwDQYJKoZI\
                               hvcNAQEBBQADggEPADCCAQoCggEBANRZxxg9eTCMVr4DsIUcytQOLnlZ7tl\
                               uliP+jM76mjJEuWqizHzZ1ZoPcEbdW9sV8kgWdPHL3KOlXAr0DEobnhQsNx\
                               uzJ8B73TcV7AKp2HR+xCTKPEha1gVHgQMmzQyCIgLEsdcjhsFeFYqMflELZ\
                               rMy+7DBSZWWf3wCnxiKbzTL01wKqylVWeSiXsniTpsoUSSk8Fe2/Li8dBMY\
                               he1vTb57GI8ta24P4lfJv6CPTNTVsr+6ue3lRuY/UIMNTybhBSc00qbuo0K\
                               ahWHyzDgY+iNEaALbyWeNOoTBQIO8lp4mhHcO/Znh2PxdqCi/FSCB2+A1Xd\
                               uOArn+MKegU5aVJN0CAwEAAaNmMGQwEgYDVR0TAQH/BAgwBgEB/wIBAjAOB\
                               gNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFONgQNXKBh1FD1EnmnEH084nI6LI\
                               MB8GA1UdIwQYMBaAFONgQNXKBh1FD1EnmnEH084nI6LIMA0GCSqGSIb3DQE\
                               BCwUAA4IBAQB+v34Vk/+qQgA7eWlczWNVWM0J67om+QwtMEo+VgzE2OHNID\
                               2o5QXsxcck0j8dANutkoqsUXpos/RG+QPNng5RBWA/sWUYWdfwZgrE30rBK\
                               waP8Yi8gVsZpz3/RClbPcfkUXI12ANw3bRI1TscOK165p1TV6nmeEus5LZq\
                               CJV37/WRt47CccsDNZaqSN7T5lQ045jsZVYpfgx/I1l9Q/fICrTOFwqYbXJ\
                               9DTe1v8C+LFbtTNcEzRGwZefLTNH2yuZjGy1/t4+cnmFJUzmC4abOoZcpkr\
                               z6U68caCbQA+wdmFs4XaO2bFaiyM+m0LVMOQfLuX/0RZc2KB7fAbb7oHQl";

    fn get_jwk_set() -> JwkSet {
        JwkSet {
            keys: vec![Jwk {
                alg: Some(Jwa::Sig(JwsAlg::RS256)),
                key_ops: Some(vec![JwkKeyOps::Verify]),
                kid: Some("bG9naW4uZGV2b2x1dGlvbnMuY29tIFRva2VuLk1hciAxMyAxMzoxNTozNSAyMDE5IEdNVA".to_owned()),
                x5t: Some(X509_SHA1_THUMBPRINT.to_owned()),
                x5c: Some(vec![
                    X509_CERT_0.to_owned(),
                    X509_CERT_1.to_owned(),
                    X509_CERT_2.to_owned(),
                ]),
                ..Jwk::new(JwkKeyType::new_rsa_key_from_base64_url(
                    RSA_MODULUS.into(),
                    RSA_PUBLIC_EXPONENT.into(),
                ))
            }],
        }
    }

    #[test]
    fn rsa_key() {
        let expected = get_jwk_set();
        let decoded = JwkSet::from_json(crate::test_files::JOSE_JWK_SET).unwrap();
        pretty_assertions::assert_eq!(decoded, expected);

        let encoded = expected.to_json_pretty().unwrap();
        let decoded = JwkSet::from_json(&encoded).unwrap();
        pretty_assertions::assert_eq!(decoded, expected);
    }

    const PUBLIC_KEY_PEM: &str = r#"-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA61BjmfXGEvWmegnBGSuS
+rU9soUg2FnODva32D1AqhwdziwHINFaD1MVlcrYG6XRKfkcxnaXGfFDWHLEvNBS
EVCgJjtHAGZIm5GL/KA86KDp/CwDFMSwluowcXwDwoyinmeOY9eKyh6aY72xJh7n
oLBBq1N0bWi1e2i+83txOCg4yV2oVXhBo8pYEJ8LT3el6Smxol3C1oFMVdwPgc0v
Tl25XucMcG/ALE/KNY6pqC2AQ6R2ERlVgPiUWOPatVkt7+Bs3h5Ramxh7XjBOXeu
lmCpGSynXNcpZ/06+vofGi/2MlpQZNhHAo8eayMp6FcvNucIpUndo1X8dKMv3Y26
ZQIDAQAB
-----END PUBLIC KEY-----"#;

    #[test]
    fn x509_and_jwk_conversion() {
        let initial_key = PublicKey::from_pem(&PUBLIC_KEY_PEM.parse::<Pem>().expect("pem")).expect("public key");
        let jwk = Jwk::from_public_key(&initial_key).unwrap();
        if let JwkKeyType::Rsa(rsa_key) = &jwk.key {
            let modulus = base64::decode_config(&rsa_key.n, base64::URL_SAFE_NO_PAD).unwrap();
            assert_ne!(modulus[0], 0x00);
            let public_exponent = base64::decode_config(&rsa_key.e, base64::URL_SAFE_NO_PAD).unwrap();
            assert_ne!(public_exponent[0], 0x00);
        } else {
            panic!("Unexpected key type");
        }
        let from_jwk_key = jwk.to_public_key().unwrap();
        assert_eq!(from_jwk_key, initial_key);
    }
}
