//! Wrappers around public and private keys raw data providing an easy to use API

#[cfg(feature = "ec")]
pub(crate) mod ec;

use crate::pem::{parse_pem, Pem, PemError};
use num_bigint_dig::traits::ModInverse;
use num_bigint_dig::BigUint;
use picky_asn1::wrapper::{BitStringAsn1Container, IntegerAsn1, OctetStringAsn1Container};
use picky_asn1_der::Asn1DerError;
use picky_asn1_x509::{private_key_info, PrivateKeyInfo, PrivateKeyValue, SubjectPublicKeyInfo};
use rsa::{RsaPrivateKey, RsaPublicKey};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum KeyError {
    /// ASN1 serialization error
    #[error("(ASN1) couldn't serialize {element}: {source}")]
    Asn1Serialization {
        element: &'static str,
        source: Asn1DerError,
    },

    /// ASN1 deserialization error
    #[error("(ASN1) couldn't deserialize {element}: {source}")]
    Asn1Deserialization {
        element: &'static str,
        source: Asn1DerError,
    },

    /// RSA error
    #[error("RSA error: {context}")]
    Rsa { context: String },

    /// EC error
    #[error("EC error: {context}")]
    EC { context: String },

    /// invalid PEM label error
    #[error("invalid PEM label: {label}")]
    InvalidPemLabel { label: String },

    /// unsupported algorithm
    #[error("unsupported algorithm: {algorithm}")]
    UnsupportedAlgorithm { algorithm: &'static str },

    /// invalid PEM provided
    #[error("invalid PEM provided: {source}")]
    Pem { source: PemError },
}

impl From<rsa::errors::Error> for KeyError {
    fn from(e: rsa::errors::Error) -> Self {
        Self::Rsa { context: e.to_string() }
    }
}

impl From<PemError> for KeyError {
    fn from(e: PemError) -> Self {
        Self::Pem { source: e }
    }
}

// === private key === //

const PRIVATE_KEY_PEM_LABEL: &str = "PRIVATE KEY";
const RSA_PRIVATE_KEY_PEM_LABEL: &str = "RSA PRIVATE KEY";
const EC_PRIVATE_KEY_LABEL: &str = "EC PRIVATE KEY";

#[derive(Debug, Clone, PartialEq)]
pub struct PrivateKey(PrivateKeyInfo);

impl From<PrivateKeyInfo> for PrivateKey {
    fn from(key: PrivateKeyInfo) -> Self {
        Self(key)
    }
}

impl From<PrivateKey> for PrivateKeyInfo {
    fn from(key: PrivateKey) -> Self {
        key.0
    }
}

impl From<PrivateKey> for SubjectPublicKeyInfo {
    fn from(key: PrivateKey) -> Self {
        match key.0.private_key {
            PrivateKeyValue::RSA(OctetStringAsn1Container(mut key)) => SubjectPublicKeyInfo::new_rsa_key(
                std::mem::take(&mut key.modulus),
                std::mem::take(&mut key.public_exponent),
            ),
            PrivateKeyValue::EC(OctetStringAsn1Container(mut key)) => {
                SubjectPublicKeyInfo::new_ec_key(std::mem::take(&mut key.public_key.0 .0))
            }
        }
    }
}

impl TryFrom<&'_ PrivateKey> for RsaPrivateKey {
    type Error = KeyError;

    fn try_from(v: &PrivateKey) -> Result<Self, Self::Error> {
        match &v.as_inner().private_key {
            private_key_info::PrivateKeyValue::RSA(OctetStringAsn1Container(key)) => {
                let p1 = BigUint::from_bytes_be(key.prime_1.as_unsigned_bytes_be());
                let p2 = BigUint::from_bytes_be(key.prime_2.as_unsigned_bytes_be());
                Ok(RsaPrivateKey::from_components(
                    BigUint::from_bytes_be(key.modulus.as_unsigned_bytes_be()),
                    BigUint::from_bytes_be(key.public_exponent.as_unsigned_bytes_be()),
                    BigUint::from_bytes_be(key.private_exponent.as_unsigned_bytes_be()),
                    vec![p1, p2],
                ))
            }
            private_key_info::PrivateKeyValue::EC(_) => Err(KeyError::Rsa {
                context: "RSA private key cannot be constructed from an EC private key.".to_string(),
            }),
        }
    }
}

impl TryFrom<&'_ PrivateKey> for RsaPublicKey {
    type Error = KeyError;

    fn try_from(v: &PrivateKey) -> Result<Self, Self::Error> {
        match &v.as_inner().private_key {
            private_key_info::PrivateKeyValue::RSA(OctetStringAsn1Container(key)) => Ok(RsaPublicKey::new(
                BigUint::from_bytes_be(key.modulus.as_unsigned_bytes_be()),
                BigUint::from_bytes_be(key.public_exponent.as_unsigned_bytes_be()),
            )?),
            private_key_info::PrivateKeyValue::EC(_) => Err(KeyError::Rsa {
                context: "RSA public key cannot be constructed from an EC private key.".to_string(),
            }),
        }
    }
}

impl PrivateKey {
    pub fn from_rsa_components(
        modulus: &BigUint,
        public_exponent: &BigUint,
        private_exponent: &BigUint,
        primes: &[BigUint],
    ) -> Result<Self, KeyError> {
        let mut primes_it = primes.iter();
        let prime_1 = primes_it.next().ok_or_else(|| KeyError::Rsa {
            context: format!("invalid number of primes provided: expected 2, got: {}", primes.len()),
        })?;
        let prime_2 = primes_it.next().ok_or_else(|| KeyError::Rsa {
            context: format!("invalid number of primes provided: expected 2, got: {}", primes.len()),
        })?;

        let exponent_1 = private_exponent.clone() % (prime_1 - 1u8);
        let exponent_2 = private_exponent.clone() % (prime_2 - 1u8);

        let coefficient = prime_2
            .mod_inverse(prime_1)
            .ok_or_else(|| KeyError::Rsa {
                context: "no modular inverse for prime 1".to_string(),
            })?
            .to_biguint()
            .ok_or_else(|| KeyError::Rsa {
                context: "BigUint conversion failed".to_string(),
            })?;

        Ok(Self(PrivateKeyInfo::new_rsa_encryption(
            IntegerAsn1::from_bytes_be_unsigned(modulus.to_bytes_be()),
            IntegerAsn1::from_bytes_be_unsigned(public_exponent.to_bytes_be()),
            IntegerAsn1::from_bytes_be_unsigned(private_exponent.to_bytes_be()),
            (
                // primes
                IntegerAsn1::from_bytes_be_unsigned(prime_1.to_bytes_be()),
                IntegerAsn1::from_bytes_be_unsigned(prime_2.to_bytes_be()),
            ),
            (
                // exponents
                IntegerAsn1::from_bytes_be_unsigned(exponent_1.to_bytes_be()),
                IntegerAsn1::from_bytes_be_unsigned(exponent_2.to_bytes_be()),
            ),
            IntegerAsn1::from_bytes_be_unsigned(coefficient.to_bytes_be()),
        )))
    }

    pub fn from_pem(pem: &Pem) -> Result<Self, KeyError> {
        match pem.label() {
            PRIVATE_KEY_PEM_LABEL => Self::from_pkcs8(pem.data()),
            RSA_PRIVATE_KEY_PEM_LABEL => Self::from_rsa_der(pem.data()),
            EC_PRIVATE_KEY_LABEL => Self::from_ec_der(pem.data()),
            _ => Err(KeyError::InvalidPemLabel {
                label: pem.label().to_owned(),
            }),
        }
    }

    pub fn from_pem_str(pem_str: &str) -> Result<Self, KeyError> {
        let pem = parse_pem(pem_str)?;
        Self::from_pem(&pem)
    }

    pub fn from_pkcs8<T: ?Sized + AsRef<[u8]>>(pkcs8: &T) -> Result<Self, KeyError> {
        Ok(Self(picky_asn1_der::from_bytes(pkcs8.as_ref()).map_err(|e| {
            KeyError::Asn1Deserialization {
                source: e,
                element: "private key info (pkcs8)",
            }
        })?))
    }

    pub fn from_rsa_der<T: ?Sized + AsRef<[u8]>>(der: &T) -> Result<Self, KeyError> {
        use picky_asn1_x509::{AlgorithmIdentifier, RsaPrivateKey};

        let private_key =
            picky_asn1_der::from_bytes::<RsaPrivateKey>(der.as_ref()).map_err(|e| KeyError::Asn1Deserialization {
                source: e,
                element: "rsa private key",
            })?;

        Ok(Self(PrivateKeyInfo {
            version: 0,
            private_key_algorithm: AlgorithmIdentifier::new_rsa_encryption(),
            private_key: PrivateKeyValue::RSA(private_key.into()),
        }))
    }

    pub fn from_ec_der<T: ?Sized + AsRef<[u8]>>(der: &T) -> Result<Self, KeyError> {
        use picky_asn1_x509::{AlgorithmIdentifier, ECPrivateKey};

        let private_key =
            picky_asn1_der::from_bytes::<ECPrivateKey>(der.as_ref()).map_err(|e| KeyError::Asn1Deserialization {
                source: e,
                element: "ec private key",
            })?;

        Ok(Self(PrivateKeyInfo {
            version: 0,
            private_key_algorithm: AlgorithmIdentifier::new_elliptic_curve(private_key.parameters.0 .0.clone()),
            private_key: PrivateKeyValue::EC(private_key.into()),
        }))
    }

    pub fn to_pkcs8(&self) -> Result<Vec<u8>, KeyError> {
        picky_asn1_der::to_vec(&self.0).map_err(|e| KeyError::Asn1Serialization {
            source: e,
            element: "private key info (pkcs8)",
        })
    }

    pub fn to_pem(&self) -> Result<Pem<'static>, KeyError> {
        let pkcs8 = self.to_pkcs8()?;
        Ok(Pem::new(PRIVATE_KEY_PEM_LABEL, pkcs8))
    }

    pub fn to_pem_str(&self) -> Result<String, KeyError> {
        self.to_pem().map(|pem| pem.to_string())
    }

    pub fn to_public_key(&self) -> PublicKey {
        match &self.0.private_key {
            PrivateKeyValue::RSA(OctetStringAsn1Container(key)) => {
                SubjectPublicKeyInfo::new_rsa_key(key.modulus.clone(), key.public_exponent.clone()).into()
            }
            PrivateKeyValue::EC(OctetStringAsn1Container(key)) => {
                SubjectPublicKeyInfo::new_ec_key(key.public_key.0 .0.clone()).into()
            }
        }
    }

    /// **Beware**: this is insanely slow in debug builds.
    pub fn generate_rsa(bits: usize) -> Result<Self, KeyError> {
        use rand::rngs::OsRng;
        use rsa::PublicKeyParts;

        let key = RsaPrivateKey::new(&mut OsRng, bits)?;

        let modulus = key.n();
        let public_exponent = key.e();
        let private_exponent = key.d();

        Self::from_rsa_components(modulus, public_exponent, private_exponent, key.primes())
    }

    pub(crate) fn as_inner(&self) -> &PrivateKeyInfo {
        &self.0
    }
}

// === public key === //

const PUBLIC_KEY_PEM_LABEL: &str = "PUBLIC KEY";
const RSA_PUBLIC_KEY_PEM_LABEL: &str = "RSA PUBLIC KEY";

#[derive(Clone, Debug, PartialEq)]
#[repr(transparent)]
pub struct PublicKey(SubjectPublicKeyInfo);

impl<'a> From<&'a SubjectPublicKeyInfo> for &'a PublicKey {
    #[inline]
    fn from(spki: &'a SubjectPublicKeyInfo) -> Self {
        unsafe { &*(spki as *const SubjectPublicKeyInfo as *const PublicKey) }
    }
}

impl<'a> From<&'a PublicKey> for &'a SubjectPublicKeyInfo {
    #[inline]
    fn from(key: &'a PublicKey) -> Self {
        unsafe { &*(key as *const PublicKey as *const SubjectPublicKeyInfo) }
    }
}

impl From<SubjectPublicKeyInfo> for PublicKey {
    #[inline]
    fn from(spki: SubjectPublicKeyInfo) -> Self {
        Self(spki)
    }
}

impl From<PublicKey> for SubjectPublicKeyInfo {
    #[inline]
    fn from(key: PublicKey) -> Self {
        key.0
    }
}

impl From<PrivateKey> for PublicKey {
    #[inline]
    fn from(key: PrivateKey) -> Self {
        Self(key.into())
    }
}

impl AsRef<SubjectPublicKeyInfo> for PublicKey {
    #[inline]
    fn as_ref(&self) -> &SubjectPublicKeyInfo {
        self.into()
    }
}

impl AsRef<PublicKey> for PublicKey {
    #[inline]
    fn as_ref(&self) -> &PublicKey {
        self
    }
}

impl TryFrom<&'_ PublicKey> for RsaPublicKey {
    type Error = KeyError;

    fn try_from(v: &PublicKey) -> Result<Self, Self::Error> {
        use picky_asn1_x509::PublicKey as InnerPublicKey;

        match &v.as_inner().subject_public_key {
            InnerPublicKey::Rsa(BitStringAsn1Container(key)) => Ok(RsaPublicKey::new(
                BigUint::from_bytes_be(key.modulus.as_unsigned_bytes_be()),
                BigUint::from_bytes_be(key.public_exponent.as_unsigned_bytes_be()),
            )?),
            InnerPublicKey::Ec(_) => Err(KeyError::UnsupportedAlgorithm {
                algorithm: "elliptic curves",
            }),
            InnerPublicKey::Ed(_) => Err(KeyError::UnsupportedAlgorithm {
                algorithm: "edwards curves",
            }),
        }
    }
}

impl PublicKey {
    pub fn from_rsa_components(modulus: &BigUint, public_exponent: &BigUint) -> Self {
        PublicKey(SubjectPublicKeyInfo::new_rsa_key(
            IntegerAsn1::from_bytes_be_unsigned(modulus.to_bytes_be()),
            IntegerAsn1::from_bytes_be_unsigned(public_exponent.to_bytes_be()),
        ))
    }

    pub fn to_der(&self) -> Result<Vec<u8>, KeyError> {
        picky_asn1_der::to_vec(&self.0).map_err(|e| KeyError::Asn1Serialization {
            source: e,
            element: "subject public key info",
        })
    }

    pub fn to_pem(&self) -> Result<Pem<'static>, KeyError> {
        let der = self.to_der()?;
        Ok(Pem::new(PUBLIC_KEY_PEM_LABEL, der))
    }

    pub fn to_pem_str(&self) -> Result<String, KeyError> {
        self.to_pem().map(|pem| pem.to_string())
    }

    pub fn from_pem(pem: &Pem) -> Result<Self, KeyError> {
        match pem.label() {
            PUBLIC_KEY_PEM_LABEL => Self::from_der(pem.data()),
            RSA_PUBLIC_KEY_PEM_LABEL => Self::from_rsa_der(pem.data()),
            _ => Err(KeyError::InvalidPemLabel {
                label: pem.label().to_owned(),
            }),
        }
    }

    pub fn from_pem_str(pem_str: &str) -> Result<Self, KeyError> {
        let pem = parse_pem(pem_str)?;
        Self::from_pem(&pem)
    }

    pub fn from_der<T: ?Sized + AsRef<[u8]>>(der: &T) -> Result<Self, KeyError> {
        Ok(Self(picky_asn1_der::from_bytes(der.as_ref()).map_err(|e| {
            KeyError::Asn1Deserialization {
                source: e,
                element: "subject public key info",
            }
        })?))
    }

    pub fn from_rsa_der<T: ?Sized + AsRef<[u8]>>(der: &T) -> Result<Self, KeyError> {
        use picky_asn1_x509::{AlgorithmIdentifier, PublicKey, RsaPublicKey};

        let public_key =
            picky_asn1_der::from_bytes::<RsaPublicKey>(der.as_ref()).map_err(|e| KeyError::Asn1Deserialization {
                source: e,
                element: "rsa public key",
            })?;

        Ok(Self(SubjectPublicKeyInfo {
            algorithm: AlgorithmIdentifier::new_rsa_encryption(),
            subject_public_key: PublicKey::Rsa(public_key.into()),
        }))
    }

    pub(crate) fn as_inner(&self) -> &SubjectPublicKeyInfo {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::HashAlgorithm;
    use crate::signature::SignatureAlgorithm;
    use rsa::PublicKeyParts;

    cfg_if::cfg_if! { if #[cfg(feature = "x509")] {
        use crate::x509::{certificate::CertificateBuilder, date::UtcDate, name::DirectoryName};

        fn generate_certificate_from_pk(private_key: PrivateKey) {
            // validity
            let valid_from = UtcDate::ymd(2019, 10, 10).unwrap();
            let valid_to = UtcDate::ymd(2019, 10, 11).unwrap();

            CertificateBuilder::new()
                .validity(valid_from, valid_to)
                .self_signed(DirectoryName::new_common_name("Test Root CA"), &private_key)
                .ca(true)
                .build()
                .expect("couldn't build root ca");
        }
    } else {
        fn generate_certificate_from_pk(_: PrivateKey) {}
    }}

    /// Generating RSA keys in debug is very slow. Therefore, this test is ignored in debug builds
    #[test]
    #[cfg_attr(debug_assertions, ignore)]
    fn generate_rsa_key() {
        let private_key = PrivateKey::generate_rsa(4096).expect("couldn't generate rsa key");
        generate_certificate_from_pk(private_key);
    }

    const RSA_PRIVATE_KEY_PEM: &str = "-----BEGIN RSA PRIVATE KEY-----\n\
                                       MIIEpAIBAAKCAQEA5Kz4i/+XZhiE+fyrgtx/4yI3i6C6HXbC4QJYpDuSUEKN2bO9\n\
                                       RsE+Fnds/FizHtJVWbvya9ktvKdDPBdy58+CIM46HEKJhYLnBVlkEcg9N2RNgR3x\n\
                                       HnpRbKfv+BmWjOpSmWrmJSDLY0dbw5X5YL8TU69ImoouCUfStyCgrpwkctR0GD3G\n\
                                       fcGjbZRucV7VvVH9bS1jyaT/9yORyzPOSTwb+K9vOr6XlJX0CGvzQeIOcOimejHx\n\
                                       ACFOCnhEKXiwMsmL8FMz0drkGeMuCODY/OHVmAdXDE5UhroL0oDhSmIrdZ8CxngO\n\
                                       xHr1WD2yC0X0jAVP/mrxjSSfBwmmqhSMmONlvQIDAQABAoIBAQCJrBl3L8nWjayB\n\
                                       VL1ta5MTC+alCX8DfhyVmvQC7FqKN4dvKecqUe0vWXcj9cLhK4B3JdAtXfNLQOgZ\n\
                                       pYRoS2XsmjwiB20EFGtBrS+yBPvV/W0r7vrbfojHAdRXahBZhjl0ZAdrEvNgMfXt\n\
                                       Kr2YoXDhUQZFBCvzKmqSFfKnLRpEhsCBOsp+Sx0ZbP3yVPASXnqiZmKblpY4qcE5\n\
                                       KfYUO0nUWBSzY8I5c/29IY5oBbOUGS1DTMkx3R7V0BzbH/xmskVACn+cMzf467vp\n\
                                       yupTKG9hIX8ff0QH4Ggx88uQTRTI9IvfrAMnICFtR6U7g70hLN6j9ujXkPNhmycw\n\
                                       E5nQCmuBAoGBAPVbYtGBvnlySN73UrlyJ1NItUmOGhBt/ezpRjMIdMkJ6dihq7i2\n\
                                       RpE76sRvwHY9Tmw8oxR/V1ITK3dM2jZP1SRcm1mn5Y1D3K38jwFS0C47AXzIN2N+\n\
                                       LExekI1J4YOPV9o378vUKQuWpbQrQOOvylQBkRJ0Cd8DI3xhiBT/AVGbAoGBAO6Y\n\
                                       WBP3GMloO2v6PHijhRqrNdaI0qht8tDhO5L1troFLst3sfpK9fUP/KTlhHOzNVBF\n\
                                       fIJnNdcYAe9BISBbfSat+/R9F+GoUvpoC4j8ygHTQkT6ZMcMDfR8RQ4BlqGHIDKZ\n\
                                       YaAJoPZVkg7hNRMcvIruYpzFrheDE/4xvnC51GeHAoGAHzCFyFIw72lKwCU6e956\n\
                                       B0lH2ljZEVuaGuKwjM43YlMDSgmLNcjeAZpXRq9aDO3QKUwwAuwJIqLTNLAtURgm\n\
                                       5R9slCIWuTV2ORvQ5f8r/aR8lOsyt1ATu4WN5JgOtdWj+laAAi4vJYz59YRGFGuF\n\
                                       UdZ9JZZgptvUR/xx+xFLjp8CgYBMRzghaeXqvgABTUb36o8rL4FOzP9MCZqPXPKG\n\
                                       0TdR0UZcli+4LS7k4e+LaDUoKCrrNsvPhN+ZnHtB2jiU96rTKtxaFYQFCKM+mvTV\n\
                                       HrwWSUvucX62hAwSFYieKbPWgDSy+IZVe76SAllnmGg3bAB7CitMo4Y8zhMeORkB\n\
                                       QOe/EQKBgQDgeNgRud7S9BvaT3iT7UtizOr0CnmMfoF05Ohd9+VE4ogvLdAoDTUF\n\
                                       JFtdOT/0naQk0yqIwLDjzCjhe8+Ji5Y/21pjau8bvblTnASq26FRRjv5+hV8lmcR\n\
                                       zzk3Y05KXvJL75ksJdomkzZZb0q+Omf3wyjMR8Xl5WueJH1fh4hpBw==\n\
                                       -----END RSA PRIVATE KEY-----";

    #[test]
    fn private_key_from_rsa_pem() {
        PrivateKey::from_pem(&RSA_PRIVATE_KEY_PEM.parse::<Pem>().expect("pem")).expect("private key");
    }

    const PUBLIC_KEY_PEM: &str = "-----BEGIN PUBLIC KEY-----\n\
                                  MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA61BjmfXGEvWmegnBGSuS\n\
                                  +rU9soUg2FnODva32D1AqhwdziwHINFaD1MVlcrYG6XRKfkcxnaXGfFDWHLEvNBS\n\
                                  EVCgJjtHAGZIm5GL/KA86KDp/CwDFMSwluowcXwDwoyinmeOY9eKyh6aY72xJh7n\n\
                                  oLBBq1N0bWi1e2i+83txOCg4yV2oVXhBo8pYEJ8LT3el6Smxol3C1oFMVdwPgc0v\n\
                                  Tl25XucMcG/ALE/KNY6pqC2AQ6R2ERlVgPiUWOPatVkt7+Bs3h5Ramxh7XjBOXeu\n\
                                  lmCpGSynXNcpZ/06+vofGi/2MlpQZNhHAo8eayMp6FcvNucIpUndo1X8dKMv3Y26\n\
                                  ZQIDAQAB\n\
                                  -----END PUBLIC KEY-----";

    #[test]
    fn public_key_from_pem() {
        PublicKey::from_pem(&PUBLIC_KEY_PEM.parse::<Pem>().expect("pem")).expect("public key");
    }

    const RSA_PUBLIC_KEY_PEM: &str = "-----BEGIN RSA PUBLIC KEY-----\n\
                                      MIIBCgKCAQEA61BjmfXGEvWmegnBGSuS+rU9soUg2FnODva32D1AqhwdziwHINFa\n\
                                      D1MVlcrYG6XRKfkcxnaXGfFDWHLEvNBSEVCgJjtHAGZIm5GL/KA86KDp/CwDFMSw\n\
                                      luowcXwDwoyinmeOY9eKyh6aY72xJh7noLBBq1N0bWi1e2i+83txOCg4yV2oVXhB\n\
                                      o8pYEJ8LT3el6Smxol3C1oFMVdwPgc0vTl25XucMcG/ALE/KNY6pqC2AQ6R2ERlV\n\
                                      gPiUWOPatVkt7+Bs3h5Ramxh7XjBOXeulmCpGSynXNcpZ/06+vofGi/2MlpQZNhH\n\
                                      Ao8eayMp6FcvNucIpUndo1X8dKMv3Y26ZQIDAQAB\n\
                                      -----END RSA PUBLIC KEY-----";

    #[test]
    fn public_key_from_rsa_pem() {
        PublicKey::from_pem(&RSA_PUBLIC_KEY_PEM.parse::<Pem>().expect("pem")).expect("public key");
    }

    const GARBAGE_PEM: &str = "-----BEGIN GARBAGE-----GARBAGE-----END GARBAGE-----";

    #[test]
    fn public_key_from_garbage_pem_err() {
        let err = PublicKey::from_pem(&GARBAGE_PEM.parse::<Pem>().expect("pem"))
            .err()
            .expect("key error");
        assert_eq!(err.to_string(), "invalid PEM label: GARBAGE");
    }

    fn check_pk(pem_str: &str) {
        const MSG: &'static [u8] = b"abcde";

        let pem = pem_str.parse::<Pem>().expect("pem");
        let pk = PrivateKey::from_pem(&pem).expect("private key");
        let algo = SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_256);
        let signed_rsa = algo.sign(MSG, &pk).expect("rsa sign");
        algo.verify(&pk.to_public_key(), MSG, &signed_rsa)
            .expect("rsa verify rsa");

        println!("Success!");
    }

    #[test]
    fn invalid_coeff_private_key_regression() {
        println!("2048 PK 7");
        check_pk(crate::test_files::RSA_2048_PK_7);
        println!("4096 PK 3");
        check_pk(crate::test_files::RSA_4096_PK_3);
    }

    #[test]
    fn rsa_crate_private_key_conversion() {
        use rsa::pkcs8::DecodePrivateKey;

        let pk_pem = crate::test_files::RSA_2048_PK_1.parse::<crate::pem::Pem>().unwrap();
        let pk = PrivateKey::from_pem(&pk_pem).unwrap();
        let converted_rsa_private_key = RsaPrivateKey::try_from(&pk).unwrap();
        let expected_rsa_private_key = RsaPrivateKey::from_pkcs8_der(pk_pem.data()).unwrap();

        assert_eq!(converted_rsa_private_key.n(), expected_rsa_private_key.n());
        assert_eq!(converted_rsa_private_key.e(), expected_rsa_private_key.e());
        assert_eq!(converted_rsa_private_key.d(), expected_rsa_private_key.d());

        let converted_primes = converted_rsa_private_key.primes();
        let expected_primes = expected_rsa_private_key.primes();
        assert_eq!(converted_primes.len(), expected_primes.len());
        for (converted_prime, expected_prime) in converted_primes.iter().zip(expected_primes.iter()) {
            assert_eq!(converted_prime, expected_prime);
        }
    }

    #[test]
    #[cfg_attr(debug_assertions, ignore)] // this test is slow in debug
    fn ring_understands_picky_pkcs8() {
        // Make sure we're generating pkcs8 understood by the `ring` crate
        let key = PrivateKey::generate_rsa(2048).unwrap();
        let pkcs8 = key.to_pkcs8().unwrap();
        ring::signature::RsaKeyPair::from_pkcs8(&pkcs8).unwrap();
    }
}
