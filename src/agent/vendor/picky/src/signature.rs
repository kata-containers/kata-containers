//! Signature algorithms supported by picky

use crate::hash::HashAlgorithm;
use crate::key::{KeyError, PrivateKey, PublicKey};
use picky_asn1_x509::{oids, AlgorithmIdentifier};
use rsa::{PublicKey as _, RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SignatureError {
    /// Key error
    #[error("Key error: {source}")]
    Key { source: KeyError },

    /// RSA error
    #[error("RSA error: {context}")]
    Rsa { context: String },

    /// EC error
    #[error("EC error: {context}")]
    Ec { context: String },

    /// invalid signature
    #[error("invalid signature")]
    BadSignature,

    /// unsupported algorithm
    #[error("unsupported algorithm: {algorithm}")]
    UnsupportedAlgorithm { algorithm: String },
}

impl From<rsa::errors::Error> for SignatureError {
    fn from(e: rsa::errors::Error) -> Self {
        SignatureError::Rsa { context: e.to_string() }
    }
}

impl From<KeyError> for SignatureError {
    fn from(e: KeyError) -> Self {
        SignatureError::Key { source: e }
    }
}

/// Supported signature algorithms
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SignatureAlgorithm {
    RsaPkcs1v15(HashAlgorithm),
    Ecdsa(HashAlgorithm),
}

impl TryFrom<&'_ AlgorithmIdentifier> for SignatureAlgorithm {
    type Error = SignatureError;

    fn try_from(v: &AlgorithmIdentifier) -> Result<Self, Self::Error> {
        let oid_string: String = v.oid().into();
        match oid_string.as_str() {
            oids::MD5_WITH_RSA_ENCRYPTHION => Ok(Self::RsaPkcs1v15(HashAlgorithm::MD5)),
            oids::SHA1_WITH_RSA_ENCRYPTION => Ok(Self::RsaPkcs1v15(HashAlgorithm::SHA1)),
            oids::SHA224_WITH_RSA_ENCRYPTION => Ok(Self::RsaPkcs1v15(HashAlgorithm::SHA2_224)),
            oids::SHA256_WITH_RSA_ENCRYPTION => Ok(Self::RsaPkcs1v15(HashAlgorithm::SHA2_256)),
            oids::SHA384_WITH_RSA_ENCRYPTION => Ok(Self::RsaPkcs1v15(HashAlgorithm::SHA2_384)),
            oids::SHA512_WITH_RSA_ENCRYPTION => Ok(Self::RsaPkcs1v15(HashAlgorithm::SHA2_512)),
            oids::ID_RSASSA_PKCS1_V1_5_WITH_SHA3_384 => Ok(Self::RsaPkcs1v15(HashAlgorithm::SHA3_384)),
            oids::ID_RSASSA_PKCS1_V1_5_WITH_SHA3_512 => Ok(Self::RsaPkcs1v15(HashAlgorithm::SHA3_512)),
            oids::ECDSA_WITH_SHA256 => Ok(Self::Ecdsa(HashAlgorithm::SHA2_256)),
            oids::ECDSA_WITH_SHA384 => Ok(Self::Ecdsa(HashAlgorithm::SHA2_384)),
            _ => Err(SignatureError::UnsupportedAlgorithm { algorithm: oid_string }),
        }
    }
}

impl TryFrom<SignatureAlgorithm> for AlgorithmIdentifier {
    type Error = SignatureError;

    fn try_from(ty: SignatureAlgorithm) -> Result<Self, Self::Error> {
        match ty {
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::MD5) => {
                Ok(AlgorithmIdentifier::new_md5_with_rsa_encryption())
            }
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA1) => {
                Ok(AlgorithmIdentifier::new_sha1_with_rsa_encryption())
            }
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_224) => {
                Ok(AlgorithmIdentifier::new_sha224_with_rsa_encryption())
            }
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_256) => {
                Ok(AlgorithmIdentifier::new_sha256_with_rsa_encryption())
            }
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_384) => {
                Ok(AlgorithmIdentifier::new_sha384_with_rsa_encryption())
            }
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA2_512) => {
                Ok(AlgorithmIdentifier::new_sha512_with_rsa_encryption())
            }
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA3_384) => {
                Ok(AlgorithmIdentifier::new_sha3_384_with_rsa_encryption())
            }
            SignatureAlgorithm::RsaPkcs1v15(HashAlgorithm::SHA3_512) => {
                Ok(AlgorithmIdentifier::new_sha3_512_with_rsa_encryption())
            }
            SignatureAlgorithm::Ecdsa(HashAlgorithm::SHA2_256) => Ok(AlgorithmIdentifier::new_ecdsa_with_sha256()),
            SignatureAlgorithm::Ecdsa(HashAlgorithm::SHA2_384) => Ok(AlgorithmIdentifier::new_ecdsa_with_sha384()),
            SignatureAlgorithm::Ecdsa(HashAlgorithm::SHA2_512) => Ok(AlgorithmIdentifier::new_ecdsa_with_sha512()),
            SignatureAlgorithm::Ecdsa(hash) => {
                let msg = format!("ECDSA doesn't support {:?} hashing algorithm", hash);
                Err(SignatureError::Ec { context: msg })
            }
        }
    }
}

impl SignatureAlgorithm {
    pub fn from_algorithm_identifier(algorithm_identifier: &AlgorithmIdentifier) -> Result<Self, SignatureError> {
        Self::try_from(algorithm_identifier)
    }

    pub fn sign(self, msg: &[u8], private_key: &PrivateKey) -> Result<Vec<u8>, SignatureError> {
        let signature = match self {
            SignatureAlgorithm::RsaPkcs1v15(picky_hash_algo) => {
                let rsa_private_key = RsaPrivateKey::try_from(private_key)?;
                let digest = picky_hash_algo.digest(msg);
                let rsa_hash_algo = rsa::Hash::from(picky_hash_algo);
                let padding_scheme = rsa::PaddingScheme::new_pkcs1v15_sign(Some(rsa_hash_algo));
                rsa_private_key.sign_blinded(&mut rand::rngs::OsRng, padding_scheme, &digest)?
            }

            #[cfg(not(feature = "ec"))]
            SignatureAlgorithm::Ecdsa(_) => {
                return Err(SignatureError::UnsupportedAlgorithm {
                    algorithm: "ECDSA curves are not supported in this build (you need to enable the `ec` feature)"
                        .to_owned(),
                })
            }
            #[cfg(feature = "ec")]
            SignatureAlgorithm::Ecdsa(picky_hash_algo) => {
                use crate::key::ec::{EcdsaCurve, EcdsaKeypair};

                let ec_keypair = EcdsaKeypair::try_from(private_key)?;

                let signing_algorithm = match ec_keypair.curve {
                    EcdsaCurve::Nist256 => match picky_hash_algo {
                        HashAlgorithm::SHA2_256 => &ring::signature::ECDSA_P256_SHA256_ASN1_SIGNING,
                        _ => {
                            return Err(SignatureError::UnsupportedAlgorithm {
                                algorithm: format!(
                                    "ECDSA P-256 curve with {:?} hash algorithm is not supported",
                                    picky_hash_algo
                                ),
                            })
                        }
                    },
                    EcdsaCurve::Nist384 => match picky_hash_algo {
                        HashAlgorithm::SHA2_384 => &ring::signature::ECDSA_P384_SHA384_ASN1_SIGNING,
                        _ => {
                            return Err(SignatureError::UnsupportedAlgorithm {
                                algorithm: format!(
                                    "ECDSA P-384 curve with {:?} hash algorithm is not supported",
                                    picky_hash_algo
                                ),
                            })
                        }
                    },
                    EcdsaCurve::Nist512 => {
                        return Err(SignatureError::UnsupportedAlgorithm {
                            algorithm: "ECDSA P-512 curve is not yet supported".to_string(),
                        })
                    }
                };

                let keypair = ring::signature::EcdsaKeyPair::from_private_key_and_public_key(
                    signing_algorithm,
                    &ec_keypair.private_key,
                    ec_keypair.public_key,
                )
                .map_err(|e| SignatureError::Ec {
                    context: format!("Cannot decode EC keypair: {}", e),
                })?;

                let rng = ring::rand::SystemRandom::new();
                let signature = keypair.sign(&rng, msg).map_err(|e| SignatureError::Ec {
                    context: format!("Cannot produce signature: {}", e),
                })?;
                signature.as_ref().to_vec()
            }
        };

        Ok(signature)
    }

    pub fn verify(self, public_key: &PublicKey, msg: &[u8], signature: &[u8]) -> Result<(), SignatureError> {
        match self {
            SignatureAlgorithm::RsaPkcs1v15(picky_hash_algo) => {
                let rsa_public_key = RsaPublicKey::try_from(public_key)?;
                let digest = picky_hash_algo.digest(msg);
                let rsa_hash_algo = rsa::Hash::from(picky_hash_algo);
                let padding_scheme = rsa::PaddingScheme::new_pkcs1v15_sign(Some(rsa_hash_algo));
                rsa_public_key
                    .verify(padding_scheme, &digest, signature)
                    .map_err(|_| SignatureError::BadSignature)?;
            }

            #[cfg(not(feature = "ec"))]
            SignatureAlgorithm::Ecdsa(_) => {
                return Err(SignatureError::UnsupportedAlgorithm {
                    algorithm: "ECDSA curves are not supported in this build (you need to enable the `ec` feature)"
                        .to_owned(),
                })
            }
            #[cfg(feature = "ec")]
            SignatureAlgorithm::Ecdsa(picky_hash_algo) => {
                use crate::key::ec::EcdsaPublicKey;

                let verification_algorithm = match picky_hash_algo {
                    HashAlgorithm::SHA2_256 => &ring::signature::ECDSA_P256_SHA256_ASN1,
                    HashAlgorithm::SHA2_384 => &ring::signature::ECDSA_P384_SHA384_ASN1,
                    _ => {
                        return Err(SignatureError::UnsupportedAlgorithm {
                            algorithm: format!("ECDSA with {:?} hash algorithm is not supported", picky_hash_algo),
                        })
                    }
                };

                let ec_pub_key = EcdsaPublicKey::try_from(public_key)?;
                let verification_key = ring::signature::UnparsedPublicKey::new(verification_algorithm, ec_pub_key.data);

                verification_key
                    .verify(msg, signature)
                    .map_err(|_| SignatureError::BadSignature)?
            }
        }

        Ok(())
    }

    pub fn hash_algorithm(&self) -> HashAlgorithm {
        match &self {
            SignatureAlgorithm::RsaPkcs1v15(hash_algo) => *hash_algo,
            SignatureAlgorithm::Ecdsa(hash_algo) => *hash_algo,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    const EC_PRIVATE_KEY_NIST256_PEM: &str = r#"-----BEGIN EC PRIVATE KEY-----
MHcCAQEEICHio5XUa+RbeFfGtGHfbPWehTFJJtCB4/izKHJ9Vm+goAoGCCqGSM49
AwEHoUQDQgAEh7ZqcI6f0tgqq7nqdcxWM6P4GGCfkWc4q11uXFjtXOKHKCV3LzMY
g8/V1PD/YOh0HodRJAjkjXub8AmYxiTcXw==
-----END EC PRIVATE KEY-----"#;

    const EC_PRIVATE_KEY_NIST384_PEM: &str = r#"-----BEGIN EC PRIVATE KEY-----
MIGkAgEBBDDT8VOfdzHbIRaWOO1F0vgotY2qM2FfYS3zpdKE7Vqbh26hFsUw+iaG
GmGnT+29kg+gBwYFK4EEACKhZANiAAQFvVVUKRdN3/bqaEpDA1aHu8FEd3ujuyS0
AadG6QAiZxH37BGumBcyTTeGHyArqb+GTpsHTUXASbP+P+p5JgkfF9wBMF1SVTvu
ACZOYcqzGbsAXXdMYqewckhc42ye0u0=
-----END EC PRIVATE KEY-----"#;

    const EC_PRIVATE_KEY_NIST512_PEM: &str = r#"-----BEGIN EC PRIVATE KEY-----
MIHcAgEBBEIBhqphIGu2PmlcEb6xADhhSCpgPUulB0s4L2qOgolRgaBx4fNgINFE
mBsSyHJncsWG8WFEuUzAYy/YKz2lP0Qx6Z2gBwYFK4EEACOhgYkDgYYABABwBevJ
w/+Xh6I98ruzoTX3MNTsbgnc+glenJRCbEJkjbJrObFhbfgqP52r1lAy2RxuShGi
NYJJzNPT6vR1abS32QFtvTH7YbYa6OWk9dtGNY/cYxgx1nQyhUuofdW7qbbfu/Ww
TP2oFsPXRAavZCh4AbWUn8bAHmzNRyuJonQBKlQlVQ==
-----END EC PRIVATE KEY-----"#;

    #[rstest]
    #[case(HashAlgorithm::MD5, false)]
    #[case(HashAlgorithm::SHA1, false)]
    #[case(HashAlgorithm::SHA2_224, false)]
    #[case(HashAlgorithm::SHA2_256, true)]
    #[case(HashAlgorithm::SHA2_384, true)]
    #[case(HashAlgorithm::SHA2_512, true)]
    #[case(HashAlgorithm::SHA3_384, false)]
    #[case(HashAlgorithm::SHA3_512, false)]
    fn ec_algorithm_identifier_conversions(#[case] hash: HashAlgorithm, #[case] success: bool) {
        let signature_algorithm = SignatureAlgorithm::Ecdsa(hash);
        let algorithm_identifier = AlgorithmIdentifier::try_from(signature_algorithm);
        if success {
            assert!(algorithm_identifier.is_ok());
        } else {
            assert!(matches!(algorithm_identifier, Err(SignatureError::Ec { context: _ })));
        }
    }

    #[test]
    fn ec_verify_bad_signature() {
        let private_key_signature = PrivateKey::from_pem_str(EC_PRIVATE_KEY_NIST256_PEM).unwrap();
        let signature_algorithm = SignatureAlgorithm::Ecdsa(HashAlgorithm::SHA2_256);

        let msg = b"hello world";
        let signature = signature_algorithm.sign(msg, &private_key_signature).unwrap();

        let another_ec_private_key_nist256_pem = r#"-----BEGIN EC PRIVATE KEY-----
MHcCAQEEIBVYtZ17YMj89Kuu47TOxJlLVlk7MDUuAlFrVXxexgkSoAoGCCqGSM49
AwEHoUQDQgAE/irzdOJk28zjVv3sov15/NLIOxoIwL9kM2p/RfQAslATwHpD/T79
csaQwO9jFvbQFIpCvcMRjaunLfhIWiYDdg==
-----END EC PRIVATE KEY-----"#;

        let another_private_key = PrivateKey::from_pem_str(another_ec_private_key_nist256_pem).unwrap();
        let wrong_public_key = PublicKey::from(another_private_key);
        assert!(matches!(
            signature_algorithm.verify(&wrong_public_key, msg, &signature),
            Err(SignatureError::BadSignature)
        ));
    }

    #[rstest]
    #[case(EC_PRIVATE_KEY_NIST256_PEM, HashAlgorithm::SHA2_256, true)]
    #[case(EC_PRIVATE_KEY_NIST384_PEM, HashAlgorithm::SHA2_384, true)]
    #[case(EC_PRIVATE_KEY_NIST512_PEM, HashAlgorithm::SHA2_512, false)] // EC Nist 512 is not supported by ring yet
    fn ec_sign_and_verify(#[case] key_pem: &str, #[case] hash: HashAlgorithm, #[case] sign_successful: bool) {
        let private_key = PrivateKey::from_pem_str(key_pem).unwrap();
        let signature_algorithm = SignatureAlgorithm::Ecdsa(hash);

        let msg = b"hello world";
        let signature = signature_algorithm.sign(msg, &private_key);
        assert_eq!(signature.is_ok(), sign_successful);

        if !sign_successful {
            return;
        }

        let public_key = PublicKey::from(private_key);
        signature_algorithm
            .verify(&public_key, msg, &signature.unwrap())
            .unwrap();
    }
}
