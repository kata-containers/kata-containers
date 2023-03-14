use crate::key::{KeyError, PrivateKey, PublicKey};
use picky_asn1::wrapper::{BitStringAsn1, OctetStringAsn1Container};
use picky_asn1_x509::{oids, private_key_info};

pub(crate) struct EcdsaKeypair<'a> {
    pub(crate) private_key: Vec<u8>,
    pub(crate) public_key: &'a [u8],
    pub(crate) curve: EcdsaCurve,
}

#[derive(Debug, PartialEq)]
pub(crate) enum EcdsaCurve {
    Nist256,
    Nist384,
    Nist512,
}

impl<'a> TryFrom<&'a PrivateKey> for EcdsaKeypair<'a> {
    type Error = KeyError;

    fn try_from(v: &'a PrivateKey) -> Result<Self, Self::Error> {
        let (private_key_data, public_key_data) = match &v.as_inner().private_key {
            private_key_info::PrivateKeyValue::RSA(_) => Err(KeyError::EC {
                context: "EC keypair cannot be built from RSA private key".to_string(),
            }),
            private_key_info::PrivateKeyValue::EC(OctetStringAsn1Container(private_key)) => {
                let private_key_data = private_key.private_key.to_vec();
                let public_key_data = private_key.public_key.payload_view();

                if public_key_data.is_empty() {
                    Err(KeyError::EC {
                        context:
                            "EC keypair cannot be built from EC private key that doesn't have a bundled private key"
                                .to_string(),
                    })
                } else {
                    Ok((private_key_data, public_key_data))
                }
            }
        }?;

        let curve = match &v.as_inner().private_key_algorithm.parameters() {
            picky_asn1_x509::AlgorithmIdentifierParameters::Ec(params) => match params {
                Some(ec_parameters) => match ec_parameters {
                    picky_asn1_x509::EcParameters::NamedCurve(curve) => {
                        let oid = Into::<String>::into(&curve.0);
                        match oid.as_str() {
                            oids::SECP256R1 => Ok(EcdsaCurve::Nist256),
                            oids::SECP384R1 => Ok(EcdsaCurve::Nist384),
                            oids::SECP521R1 => Ok(EcdsaCurve::Nist512),
                            unknown => Err(KeyError::EC {
                                context: format!("Unknown curve type: {}", unknown),
                            }),
                        }
                    }
                },
                None => Err(KeyError::EC {
                    context: "EC keypair cannot be built when curve type is not provided by private key".to_string(),
                }),
            },
            _ => Err(KeyError::EC {
                context: "No Ec parameters found in private_key_algorithm".to_string(),
            }),
        }?;

        Ok(EcdsaKeypair {
            private_key: private_key_data,
            public_key: public_key_data,
            curve,
        })
    }
}
pub(crate) struct EcdsaPublicKey<'a> {
    pub(crate) data: &'a [u8],
}

impl<'a> TryFrom<&'a PublicKey> for EcdsaPublicKey<'a> {
    type Error = KeyError;

    fn try_from(v: &'a PublicKey) -> Result<Self, Self::Error> {
        use picky_asn1_x509::PublicKey as InnerPublicKey;

        match &v.as_inner().subject_public_key {
            InnerPublicKey::Rsa(_) => Err(KeyError::EC {
                context: "EC public key cannot be constructed from RSA public key".to_string(),
            }),
            InnerPublicKey::Ec(BitStringAsn1(bitstring)) => {
                let data = bitstring.payload_view();
                Ok(EcdsaPublicKey { data })
            }
            InnerPublicKey::Ed(_) => Err(KeyError::EC {
                context: "EC public key cannot be constructed from ED25519 public key".to_string(),
            }),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    const PKCS8_EC_PRIVATE_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
                                            MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgKZqrmOg/cDZ4tPCn\n\
                                            4LROs145nxx+ssufvflL8cROxFmhRANCAARmU90fCSTsncefY7hVeKw1WIg/YQmT\n\
                                            4DGJ7nJPZ+WXAd/xxp4c0bHGlIOju/U95ITPN9dAmro7OUTDJpz+rzGW\n\
                                            -----END PRIVATE KEY-----";

    const RSA_PUBLIC_KEY_PEM: &str = "-----BEGIN RSA PUBLIC KEY-----\n\
                                      MIIBCgKCAQEA61BjmfXGEvWmegnBGSuS+rU9soUg2FnODva32D1AqhwdziwHINFa\n\
                                      D1MVlcrYG6XRKfkcxnaXGfFDWHLEvNBSEVCgJjtHAGZIm5GL/KA86KDp/CwDFMSw\n\
                                      luowcXwDwoyinmeOY9eKyh6aY72xJh7noLBBq1N0bWi1e2i+83txOCg4yV2oVXhB\n\
                                      o8pYEJ8LT3el6Smxol3C1oFMVdwPgc0vTl25XucMcG/ALE/KNY6pqC2AQ6R2ERlV\n\
                                      gPiUWOPatVkt7+Bs3h5Ramxh7XjBOXeulmCpGSynXNcpZ/06+vofGi/2MlpQZNhH\n\
                                      Ao8eayMp6FcvNucIpUndo1X8dKMv3Y26ZQIDAQAB\n\
                                      -----END RSA PUBLIC KEY-----";

    const EC_PUBLIC_KEY_PEM: &str = "-----BEGIN PUBLIC KEY-----\n\
                                    MFYwEAYHKoZIzj0CAQYFK4EEAAoDQgAE6grzTyQyJdYOaVVwZosUEv02AdwvYQOv\n\
                                    bJM105PImXUuqTMyqSmX96/m7zFfyh/DQQbyXIo3E07qifCPMw9/oQ==\n\
                                    -----END PUBLIC KEY-----";

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
    #[case(EC_PRIVATE_KEY_NIST256_PEM)]
    #[case(EC_PRIVATE_KEY_NIST384_PEM)]
    #[case(EC_PRIVATE_KEY_NIST512_PEM)]
    #[case(PKCS8_EC_PRIVATE_KEY_PEM)]
    fn private_key_from_ec_pem(#[case] key_pem: &str) {
        PrivateKey::from_pem_str(key_pem).unwrap();
    }

    #[test]
    fn public_key_from_ec_pem() {
        PublicKey::from_pem_str(EC_PUBLIC_KEY_PEM).unwrap();
    }

    #[test]
    fn ecdsa_public_key_conversions() {
        // ECDSA public key conversion works
        let pk: &PublicKey = &PublicKey::from_pem_str(EC_PUBLIC_KEY_PEM).unwrap();
        let epk: Result<EcdsaPublicKey, KeyError> = pk.try_into();
        assert!(epk.is_ok());

        // PEM public key conversion fails with an error
        let pk: &PublicKey = &PublicKey::from_pem_str(RSA_PUBLIC_KEY_PEM).unwrap();
        let epk: Result<EcdsaPublicKey, KeyError> = pk.try_into();
        assert!(epk.is_err());
        assert!(matches!(epk, Err(KeyError::EC { context: _ })));

        // TODO: add check for attempted conversion from ED keys - which are not supported yet
    }

    #[rstest]
    #[case(EC_PRIVATE_KEY_NIST256_PEM, EcdsaCurve::Nist256)]
    #[case(EC_PRIVATE_KEY_NIST384_PEM, EcdsaCurve::Nist384)]
    #[case(EC_PRIVATE_KEY_NIST512_PEM, EcdsaCurve::Nist512)]
    fn ecdsa_key_pair_from_ec_private_key(#[case] key: &str, #[case] curve: EcdsaCurve) {
        let pk = PrivateKey::from_pem_str(key).unwrap();
        let pair = EcdsaKeypair::try_from(&pk).unwrap();
        assert_eq!(curve, pair.curve);
    }
}
