use crate::jwk::alg::ec::EcCurve;
use crate::jwk::alg::ecx::EcxCurve;
use crate::jwk::alg::ed::EdCurve;
use crate::jwk::Jwk;
use crate::util;
use crate::util::der::{DerClass, DerError, DerReader, DerType};
use crate::util::oid::{
    OID_ED25519, OID_ED448, OID_ID_EC_PUBLIC_KEY, OID_MGF1, OID_PRIME256V1, OID_RSASSA_PSS,
    OID_RSA_ENCRYPTION, OID_SECP256K1, OID_SECP384R1, OID_SECP521R1, OID_SHA1, OID_SHA256,
    OID_SHA384, OID_SHA512, OID_X25519, OID_X448,
};
use crate::util::HashAlgorithm;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum KeyAlg {
    Rsa,
    RsaPss {
        hash: Option<HashAlgorithm>,
        mgf1_hash: Option<HashAlgorithm>,
        salt_len: Option<u8>,
    },
    Ec {
        curve: Option<EcCurve>,
    },
    Ed {
        curve: Option<EdCurve>,
    },
    Ecx {
        curve: Option<EcxCurve>,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum KeyFormat {
    Der { raw: bool },
    Pem { traditional: bool },
    Jwk,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KeyInfo {
    format: KeyFormat,
    alg: Option<KeyAlg>,
    is_public_key: bool,
}

impl KeyInfo {
    pub fn format(&self) -> KeyFormat {
        self.format
    }

    pub fn alg(&self) -> Option<KeyAlg> {
        self.alg
    }

    pub fn is_public_key(&self) -> bool {
        self.is_public_key
    }

    pub fn detect(input: &impl AsRef<[u8]>) -> Option<KeyInfo> {
        let input = input.as_ref();
        if input.len() == 0 {
            return None;
        }

        let key_info = match input[0] {
            // DER
            b'\x30' => Self::detect_from_der(input)?,
            // PEM
            b'-' => {
                let (alg, data) = util::parse_pem(input.as_ref()).ok()?;
                match alg.as_str() {
                    "PRIVATE KEY" => {
                        let key_info = Self::detect_from_der(&data)?;
                        if key_info.is_public_key() {
                            return None;
                        }

                        KeyInfo {
                            format: KeyFormat::Pem { traditional: false },
                            alg: key_info.alg(),
                            is_public_key: key_info.is_public_key(),
                        }
                    }
                    "RSA PRIVATE KEY" => {
                        let key_info = Self::detect_from_der(&data)?;
                        if key_info.is_public_key() || !matches!(key_info.alg(), Some(KeyAlg::Rsa))
                        {
                            return None;
                        }

                        KeyInfo {
                            format: KeyFormat::Pem { traditional: true },
                            alg: key_info.alg(),
                            is_public_key: key_info.is_public_key(),
                        }
                    }
                    "RSA-PSS PRIVATE KEY" => {
                        let key_info = Self::detect_from_der(&data)?;
                        if key_info.is_public_key()
                            || !matches!(
                                key_info.alg(),
                                Some(KeyAlg::RsaPss {
                                    hash: _,
                                    mgf1_hash: _,
                                    salt_len: _,
                                })
                            )
                        {
                            return None;
                        }

                        KeyInfo {
                            format: KeyFormat::Pem { traditional: true },
                            alg: key_info.alg(),
                            is_public_key: key_info.is_public_key(),
                        }
                    }
                    "EC PRIVATE KEY" => {
                        let key_info = Self::detect_from_der(&data)?;
                        if key_info.is_public_key()
                            || !matches!(key_info.alg(), Some(KeyAlg::Ec { curve: _ }))
                        {
                            return None;
                        }

                        KeyInfo {
                            format: KeyFormat::Pem { traditional: true },
                            alg: key_info.alg(),
                            is_public_key: key_info.is_public_key(),
                        }
                    }
                    "ED25519 PRIVATE KEY" => {
                        let key_info = Self::detect_from_der(&data)?;
                        if key_info.is_public_key()
                            || !matches!(
                                key_info.alg(),
                                Some(KeyAlg::Ed {
                                    curve: Some(EdCurve::Ed25519)
                                })
                            )
                        {
                            return None;
                        }

                        KeyInfo {
                            format: KeyFormat::Pem { traditional: true },
                            alg: key_info.alg(),
                            is_public_key: key_info.is_public_key(),
                        }
                    }
                    "ED448 PRIVATE KEY" => {
                        let key_info = Self::detect_from_der(&data)?;
                        if key_info.is_public_key()
                            || !matches!(
                                key_info.alg(),
                                Some(KeyAlg::Ed {
                                    curve: Some(EdCurve::Ed448)
                                })
                            )
                        {
                            return None;
                        }

                        KeyInfo {
                            format: KeyFormat::Pem { traditional: true },
                            alg: key_info.alg(),
                            is_public_key: key_info.is_public_key(),
                        }
                    }
                    "X25519 PRIVATE KEY" => {
                        let key_info = Self::detect_from_der(&data)?;
                        if key_info.is_public_key()
                            || !matches!(
                                key_info.alg(),
                                Some(KeyAlg::Ecx {
                                    curve: Some(EcxCurve::X25519)
                                })
                            )
                        {
                            return None;
                        }

                        KeyInfo {
                            format: KeyFormat::Pem { traditional: true },
                            alg: key_info.alg(),
                            is_public_key: key_info.is_public_key(),
                        }
                    }
                    "X448 PRIVATE KEY" => {
                        let key_info = Self::detect_from_der(&data)?;
                        if key_info.is_public_key()
                            || !matches!(
                                key_info.alg(),
                                Some(KeyAlg::Ecx {
                                    curve: Some(EcxCurve::X448)
                                })
                            )
                        {
                            return None;
                        }

                        KeyInfo {
                            format: KeyFormat::Pem { traditional: true },
                            alg: key_info.alg(),
                            is_public_key: key_info.is_public_key(),
                        }
                    }
                    "PUBLIC KEY" => {
                        let key_info = Self::detect_from_der(&data)?;
                        if !key_info.is_public_key() {
                            return None;
                        }

                        KeyInfo {
                            format: KeyFormat::Pem { traditional: false },
                            alg: key_info.alg(),
                            is_public_key: key_info.is_public_key(),
                        }
                    }
                    "RSA PUBLIC KEY" => {
                        let key_info = Self::detect_from_der(&data)?;
                        if !key_info.is_public_key() || !matches!(key_info.alg(), Some(KeyAlg::Rsa))
                        {
                            return None;
                        }

                        KeyInfo {
                            format: KeyFormat::Pem { traditional: true },
                            alg: key_info.alg(),
                            is_public_key: key_info.is_public_key(),
                        }
                    }
                    _ => return None,
                }
            }
            // JWK
            _ => {
                let jwk = Jwk::from_bytes(input).ok()?;
                match jwk.key_type() {
                    "oct" => KeyInfo {
                        format: KeyFormat::Jwk,
                        alg: None,
                        is_public_key: false,
                    },
                    "RSA" => {
                        let is_public_key = matches!(jwk.parameter("d"), None);

                        KeyInfo {
                            format: KeyFormat::Jwk,
                            alg: Some(KeyAlg::Rsa),
                            is_public_key: is_public_key,
                        }
                    }
                    "EC" => {
                        let alg = match jwk.curve() {
                            Some("P-256") => Some(KeyAlg::Ec {
                                curve: Some(EcCurve::P256),
                            }),
                            Some("P-384") => Some(KeyAlg::Ec {
                                curve: Some(EcCurve::P384),
                            }),
                            Some("P-521") => Some(KeyAlg::Ec {
                                curve: Some(EcCurve::P521),
                            }),
                            Some("secp256k1") => Some(KeyAlg::Ec {
                                curve: Some(EcCurve::Secp256k1),
                            }),
                            Some(_) => Some(KeyAlg::Ec { curve: None }),
                            None => return None,
                        };
                        let is_public_key = matches!(jwk.parameter("d"), None);

                        KeyInfo {
                            format: KeyFormat::Jwk,
                            alg,
                            is_public_key,
                        }
                    }
                    "OKP" => {
                        let alg = match jwk.curve() {
                            Some("Ed25519") => Some(KeyAlg::Ed {
                                curve: Some(EdCurve::Ed25519),
                            }),
                            Some("Ed448") => Some(KeyAlg::Ed {
                                curve: Some(EdCurve::Ed448),
                            }),
                            Some("X25519") => Some(KeyAlg::Ecx {
                                curve: Some(EcxCurve::X25519),
                            }),
                            Some("X448") => Some(KeyAlg::Ecx {
                                curve: Some(EcxCurve::X448),
                            }),
                            Some(_) => None,
                            None => return None,
                        };
                        let is_public_key = matches!(jwk.parameter("d"), None);

                        KeyInfo {
                            format: KeyFormat::Jwk,
                            alg,
                            is_public_key,
                        }
                    }
                    _ => KeyInfo {
                        format: KeyFormat::Jwk,
                        alg: None,
                        is_public_key: false,
                    },
                }
            }
        };

        Some(key_info)
    }

    fn detect_from_der(input: &[u8]) -> Option<KeyInfo> {
        let mut reader = DerReader::from_reader(input);

        match reader.next().ok()? {
            Some(DerType::Sequence) => {}
            _ => return None,
        }

        let key_info = match reader.next().ok()? {
            Some(DerType::Sequence) => match reader.next().ok()? {
                Some(DerType::ObjectIdentifier) => match reader.to_object_identifier().ok()? {
                    val if val == *OID_RSA_ENCRYPTION => KeyInfo {
                        format: KeyFormat::Der { raw: false },
                        alg: Some(KeyAlg::Rsa),
                        is_public_key: true,
                    },
                    val if val == *OID_RSASSA_PSS => {
                        let (hash, mgf1_hash, salt_len) =
                            Self::parse_rsa_pss_params(&mut reader).ok()?;

                        KeyInfo {
                            format: KeyFormat::Der { raw: false },
                            alg: Some(KeyAlg::RsaPss {
                                hash,
                                mgf1_hash,
                                salt_len,
                            }),
                            is_public_key: true,
                        }
                    }
                    val if val == *OID_ID_EC_PUBLIC_KEY => {
                        let curve = match reader.next().ok()? {
                            Some(DerType::ObjectIdentifier) => {
                                match reader.to_object_identifier().ok()? {
                                    val if val == *OID_PRIME256V1 => Some(EcCurve::P256),
                                    val if val == *OID_SECP384R1 => Some(EcCurve::P384),
                                    val if val == *OID_SECP521R1 => Some(EcCurve::P521),
                                    val if val == *OID_SECP256K1 => Some(EcCurve::Secp256k1),
                                    _ => None,
                                }
                            }
                            _ => None,
                        };

                        KeyInfo {
                            format: KeyFormat::Der { raw: false },
                            alg: Some(KeyAlg::Ec { curve }),
                            is_public_key: true,
                        }
                    }
                    val if val == *OID_ED25519 => KeyInfo {
                        format: KeyFormat::Der { raw: false },
                        alg: Some(KeyAlg::Ed {
                            curve: Some(EdCurve::Ed25519),
                        }),
                        is_public_key: true,
                    },
                    val if val == *OID_ED448 => KeyInfo {
                        format: KeyFormat::Der { raw: false },
                        alg: Some(KeyAlg::Ed {
                            curve: Some(EdCurve::Ed448),
                        }),
                        is_public_key: true,
                    },
                    val if val == *OID_X25519 => KeyInfo {
                        format: KeyFormat::Der { raw: false },
                        alg: Some(KeyAlg::Ecx {
                            curve: Some(EcxCurve::X25519),
                        }),
                        is_public_key: true,
                    },
                    val if val == *OID_X448 => KeyInfo {
                        format: KeyFormat::Der { raw: false },
                        alg: Some(KeyAlg::Ecx {
                            curve: Some(EcxCurve::X448),
                        }),
                        is_public_key: true,
                    },
                    _ => KeyInfo {
                        format: KeyFormat::Der { raw: false },
                        alg: None,
                        is_public_key: true,
                    },
                },
                _ => return None,
            },
            Some(DerType::Integer) => match reader.next().ok()? {
                Some(DerType::Sequence) => match reader.next().ok()? {
                    Some(DerType::ObjectIdentifier) => match reader.to_object_identifier().ok()? {
                        val if val == *OID_RSA_ENCRYPTION => KeyInfo {
                            format: KeyFormat::Der { raw: false },
                            alg: Some(KeyAlg::Rsa),
                            is_public_key: false,
                        },
                        val if val == *OID_RSASSA_PSS => {
                            let (hash, mgf1_hash, salt_len) =
                                Self::parse_rsa_pss_params(&mut reader).ok()?;

                            KeyInfo {
                                format: KeyFormat::Der { raw: false },
                                alg: Some(KeyAlg::RsaPss {
                                    hash,
                                    mgf1_hash,
                                    salt_len,
                                }),
                                is_public_key: false,
                            }
                        }
                        val if val == *OID_ID_EC_PUBLIC_KEY => {
                            let curve = match reader.next().ok()? {
                                Some(DerType::ObjectIdentifier) => {
                                    match reader.to_object_identifier().ok()? {
                                        val if val == *OID_PRIME256V1 => Some(EcCurve::P256),
                                        val if val == *OID_SECP384R1 => Some(EcCurve::P384),
                                        val if val == *OID_SECP521R1 => Some(EcCurve::P521),
                                        val if val == *OID_SECP256K1 => Some(EcCurve::Secp256k1),
                                        _ => None,
                                    }
                                }
                                _ => None,
                            };

                            KeyInfo {
                                format: KeyFormat::Der { raw: false },
                                alg: Some(KeyAlg::Ec { curve }),
                                is_public_key: false,
                            }
                        }
                        val if val == *OID_ED25519 => KeyInfo {
                            format: KeyFormat::Der { raw: false },
                            alg: Some(KeyAlg::Ed {
                                curve: Some(EdCurve::Ed25519),
                            }),
                            is_public_key: false,
                        },
                        val if val == *OID_ED448 => KeyInfo {
                            format: KeyFormat::Der { raw: false },
                            alg: Some(KeyAlg::Ed {
                                curve: Some(EdCurve::Ed448),
                            }),
                            is_public_key: false,
                        },
                        val if val == *OID_X25519 => KeyInfo {
                            format: KeyFormat::Der { raw: false },
                            alg: Some(KeyAlg::Ecx {
                                curve: Some(EcxCurve::X25519),
                            }),
                            is_public_key: false,
                        },
                        val if val == *OID_X448 => KeyInfo {
                            format: KeyFormat::Der { raw: false },
                            alg: Some(KeyAlg::Ecx {
                                curve: Some(EcxCurve::X448),
                            }),
                            is_public_key: false,
                        },
                        _ => return None,
                    },
                    _ => return None,
                },
                Some(DerType::Integer) => {
                    if let Some(DerType::EndOfContents) = reader.next().ok()? {
                        KeyInfo {
                            format: KeyFormat::Der { raw: true },
                            alg: Some(KeyAlg::Rsa),
                            is_public_key: true,
                        }
                    } else {
                        KeyInfo {
                            format: KeyFormat::Der { raw: true },
                            alg: Some(KeyAlg::Rsa),
                            is_public_key: false,
                        }
                    }
                }
                Some(DerType::OctetString) => {
                    let curve = match reader.next().ok()? {
                        Some(DerType::Other(DerClass::ContextSpecific, 0)) => {
                            match reader.next().ok()? {
                                Some(DerType::ObjectIdentifier) => {
                                    match reader.to_object_identifier().ok()? {
                                        val if val == *OID_PRIME256V1 => Some(EcCurve::P256),
                                        val if val == *OID_SECP384R1 => Some(EcCurve::P384),
                                        val if val == *OID_SECP521R1 => Some(EcCurve::P521),
                                        val if val == *OID_SECP256K1 => Some(EcCurve::Secp256k1),
                                        _ => None,
                                    }
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    };

                    KeyInfo {
                        format: KeyFormat::Der { raw: true },
                        alg: Some(KeyAlg::Ec { curve }),
                        is_public_key: false,
                    }
                }
                _ => return None,
            },
            _ => return None,
        };

        Some(key_info)
    }

    fn parse_rsa_pss_params(
        reader: &mut DerReader<&[u8]>,
    ) -> Result<(Option<HashAlgorithm>, Option<HashAlgorithm>, Option<u8>), DerError> {
        let mut hash = Some(HashAlgorithm::Sha1);
        let mut mgf1_hash = Some(HashAlgorithm::Sha1);
        let mut salt_len = Some(20);

        if let Some(DerType::Sequence) = reader.next()? {
            while let Some(DerType::Other(DerClass::ContextSpecific, i)) = reader.next()? {
                if i == 0 {
                    match reader.next()? {
                        Some(DerType::Sequence) => {}
                        _ => break,
                    }

                    match reader.next()? {
                        Some(DerType::ObjectIdentifier) => match reader.to_object_identifier()? {
                            val if val == *OID_SHA1 => hash = Some(HashAlgorithm::Sha1),
                            val if val == *OID_SHA256 => hash = Some(HashAlgorithm::Sha256),
                            val if val == *OID_SHA384 => hash = Some(HashAlgorithm::Sha384),
                            val if val == *OID_SHA512 => hash = Some(HashAlgorithm::Sha512),
                            _ => hash = None,
                        },
                        _ => break,
                    }

                    match reader.next()? {
                        Some(DerType::EndOfContents) => {}
                        _ => break,
                    }

                    match reader.next()? {
                        Some(DerType::EndOfContents) => {}
                        _ => break,
                    }
                } else if i == 1 {
                    match reader.next()? {
                        Some(DerType::Sequence) => {}
                        _ => break,
                    }

                    match reader.next()? {
                        Some(DerType::ObjectIdentifier) => match reader.to_object_identifier()? {
                            val if val == *OID_MGF1 => {}
                            _ => break,
                        },
                        _ => break,
                    }

                    match reader.next()? {
                        Some(DerType::Sequence) => {}
                        _ => break,
                    }

                    match reader.next()? {
                        Some(DerType::ObjectIdentifier) => match reader.to_object_identifier()? {
                            val if val == *OID_SHA1 => mgf1_hash = Some(HashAlgorithm::Sha1),
                            val if val == *OID_SHA256 => mgf1_hash = Some(HashAlgorithm::Sha256),
                            val if val == *OID_SHA384 => mgf1_hash = Some(HashAlgorithm::Sha384),
                            val if val == *OID_SHA512 => mgf1_hash = Some(HashAlgorithm::Sha512),
                            _ => mgf1_hash = None,
                        },
                        _ => break,
                    }

                    match reader.next()? {
                        Some(DerType::EndOfContents) => {}
                        _ => break,
                    }

                    match reader.next()? {
                        Some(DerType::EndOfContents) => {}
                        _ => break,
                    }

                    match reader.next()? {
                        Some(DerType::EndOfContents) => {}
                        _ => break,
                    }
                } else if i == 2 {
                    match reader.next()? {
                        Some(DerType::Integer) => match reader.to_u8()? {
                            val => salt_len = Some(val),
                        },
                        _ => break,
                    }

                    match reader.next()? {
                        Some(DerType::EndOfContents) => {}
                        _ => break,
                    }
                } else {
                    reader.skip_contents()?;
                }
            }
        }

        Ok((hash, mgf1_hash, salt_len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_detect_rsa_der_private_key() -> Result<()> {
        let input = load_file("der/RSA_2048bit_pkcs8_private.der")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Der { raw: false });
        assert_eq!(key_info.alg(), Some(KeyAlg::Rsa));
        assert_eq!(key_info.is_public_key(), false);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_der_public_key() -> Result<()> {
        let input = load_file("der/RSA_2048bit_spki_public.der")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Der { raw: false });
        assert_eq!(key_info.alg(), Some(KeyAlg::Rsa));
        assert_eq!(key_info.is_public_key(), true);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_raw_private_key() -> Result<()> {
        let input = load_file("der/RSA_2048bit_raw_private.der")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Der { raw: true });
        assert_eq!(key_info.alg(), Some(KeyAlg::Rsa));
        assert_eq!(key_info.is_public_key(), false);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_raw_public_key() -> Result<()> {
        let input = load_file("der/RSA_2048bit_raw_public.der")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Der { raw: true });
        assert_eq!(key_info.alg(), Some(KeyAlg::Rsa));
        assert_eq!(key_info.is_public_key(), true);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_pem_private_key() -> Result<()> {
        let input = load_file("pem/RSA_2048bit_private.pem")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Pem { traditional: false });
        assert_eq!(key_info.alg(), Some(KeyAlg::Rsa));
        assert_eq!(key_info.is_public_key(), false);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_pem_public_key() -> Result<()> {
        let input = load_file("pem/RSA_2048bit_public.pem")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Pem { traditional: false });
        assert_eq!(key_info.alg(), Some(KeyAlg::Rsa));
        assert_eq!(key_info.is_public_key(), true);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_traditional_pem_private_key() -> Result<()> {
        let input = load_file("pem/RSA_2048bit_traditional_private.pem")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Pem { traditional: true });
        assert_eq!(key_info.alg(), Some(KeyAlg::Rsa));
        assert_eq!(key_info.is_public_key(), false);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_traditional_pem_public_key() -> Result<()> {
        let input = load_file("pem/RSA_2048bit_traditional_public.pem")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Pem { traditional: true });
        assert_eq!(key_info.alg(), Some(KeyAlg::Rsa));
        assert_eq!(key_info.is_public_key(), true);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_jwk_private_key() -> Result<()> {
        let input = load_file("jwk/RSA_private.jwk")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Jwk);
        assert_eq!(key_info.alg(), Some(KeyAlg::Rsa));
        assert_eq!(key_info.is_public_key(), false);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_jwk_public_key() -> Result<()> {
        let input = load_file("jwk/RSA_public.jwk")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Jwk);
        assert_eq!(key_info.alg(), Some(KeyAlg::Rsa));
        assert_eq!(key_info.is_public_key(), true);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_pss_der_private_key() -> Result<()> {
        let input = load_file("der/RSA-PSS_2048bit_SHA-256_pkcs8_private.der")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Der { raw: false });
        assert_eq!(
            key_info.alg(),
            Some(KeyAlg::RsaPss {
                hash: Some(HashAlgorithm::Sha256),
                mgf1_hash: Some(HashAlgorithm::Sha256),
                salt_len: Some(32)
            })
        );
        assert_eq!(key_info.is_public_key(), false);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_pss_public_key() -> Result<()> {
        let input = load_file("der/RSA-PSS_2048bit_SHA-256_spki_public.der")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Der { raw: false });
        assert_eq!(
            key_info.alg(),
            Some(KeyAlg::RsaPss {
                hash: Some(HashAlgorithm::Sha256),
                mgf1_hash: Some(HashAlgorithm::Sha256),
                salt_len: Some(32)
            })
        );
        assert_eq!(key_info.is_public_key(), true);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_pss_pem_private_key() -> Result<()> {
        let input = load_file("pem/RSA-PSS_2048bit_SHA-256_private.pem")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Pem { traditional: false });
        assert_eq!(
            key_info.alg(),
            Some(KeyAlg::RsaPss {
                hash: Some(HashAlgorithm::Sha256),
                mgf1_hash: Some(HashAlgorithm::Sha256),
                salt_len: Some(32)
            })
        );
        assert_eq!(key_info.is_public_key(), false);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_pss_pem_public_key() -> Result<()> {
        let input = load_file("pem/RSA-PSS_2048bit_SHA-256_public.pem")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Pem { traditional: false });
        assert_eq!(
            key_info.alg(),
            Some(KeyAlg::RsaPss {
                hash: Some(HashAlgorithm::Sha256),
                mgf1_hash: Some(HashAlgorithm::Sha256),
                salt_len: Some(32)
            })
        );
        assert_eq!(key_info.is_public_key(), true);

        Ok(())
    }

    #[test]
    fn test_detect_rsa_pss_traditional_pem_private_key() -> Result<()> {
        let input = load_file("pem/RSA-PSS_2048bit_SHA-256_traditional_private.pem")?;

        let key_info = KeyInfo::detect(&input).unwrap();
        assert_eq!(key_info.format(), KeyFormat::Pem { traditional: true });
        assert_eq!(
            key_info.alg(),
            Some(KeyAlg::RsaPss {
                hash: Some(HashAlgorithm::Sha256),
                mgf1_hash: Some(HashAlgorithm::Sha256),
                salt_len: Some(32)
            })
        );
        assert_eq!(key_info.is_public_key(), false);

        Ok(())
    }

    #[test]
    fn test_detect_ec_der_private_key() -> Result<()> {
        for curve in &[
            EcCurve::P256,
            EcCurve::P384,
            EcCurve::P521,
            EcCurve::Secp256k1,
        ] {
            let input = load_file(&format!("der/EC_{}_pkcs8_private.der", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Der { raw: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ec {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ec_der_public_key() -> Result<()> {
        for curve in &[
            EcCurve::P256,
            EcCurve::P384,
            EcCurve::P521,
            EcCurve::Secp256k1,
        ] {
            let input = load_file(&format!("der/EC_{}_spki_public.der", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Der { raw: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ec {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), true);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ec_raw_private_key() -> Result<()> {
        for curve in &[
            EcCurve::P256,
            EcCurve::P384,
            EcCurve::P521,
            EcCurve::Secp256k1,
        ] {
            let input = load_file(&format!("der/EC_{}_raw_private.der", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Der { raw: true });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ec {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ec_pem_private_key() -> Result<()> {
        for curve in &[
            EcCurve::P256,
            EcCurve::P384,
            EcCurve::P521,
            EcCurve::Secp256k1,
        ] {
            let input = load_file(&format!("pem/EC_{}_private.pem", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Pem { traditional: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ec {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ec_pem_public_key() -> Result<()> {
        for curve in &[
            EcCurve::P256,
            EcCurve::P384,
            EcCurve::P521,
            EcCurve::Secp256k1,
        ] {
            let input = load_file(&format!("pem/EC_{}_public.pem", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Pem { traditional: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ec {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), true);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ec_traditional_pem_private_key() -> Result<()> {
        for curve in &[
            EcCurve::P256,
            EcCurve::P384,
            EcCurve::P521,
            EcCurve::Secp256k1,
        ] {
            let input = load_file(&format!("pem/EC_{}_traditional_private.pem", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Pem { traditional: true });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ec {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ec_jwk_private_key() -> Result<()> {
        for curve in &[
            EcCurve::P256,
            EcCurve::P384,
            EcCurve::P521,
            EcCurve::Secp256k1,
        ] {
            let input = load_file(&format!("jwk/EC_{}_private.jwk", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Jwk);
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ec {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ec_jwk_public_key() -> Result<()> {
        for curve in &[
            EcCurve::P256,
            EcCurve::P384,
            EcCurve::P521,
            EcCurve::Secp256k1,
        ] {
            let input = load_file(&format!("jwk/EC_{}_public.jwk", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Jwk);
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ec {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), true);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ed_der_private_key() -> Result<()> {
        for curve in &[EdCurve::Ed25519, EdCurve::Ed448] {
            let input = load_file(&format!(
                "der/{}_pkcs8_private.der",
                curve.name().to_uppercase()
            ))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Der { raw: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ed {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ed_der_public_key() -> Result<()> {
        for curve in &[EdCurve::Ed25519, EdCurve::Ed448] {
            let input = load_file(&format!(
                "der/{}_spki_public.der",
                curve.name().to_uppercase()
            ))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Der { raw: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ed {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), true);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ed_pem_private_key() -> Result<()> {
        for curve in &[EdCurve::Ed25519, EdCurve::Ed448] {
            let input = load_file(&format!("pem/{}_private.pem", curve.name().to_uppercase()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Pem { traditional: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ed {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ed_pem_public_key() -> Result<()> {
        for curve in &[EdCurve::Ed25519, EdCurve::Ed448] {
            let input = load_file(&format!("pem/{}_public.pem", curve.name().to_uppercase()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Pem { traditional: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ed {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), true);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ed_traditional_pem_private_key() -> Result<()> {
        for curve in &[EdCurve::Ed25519, EdCurve::Ed448] {
            let input = load_file(&format!(
                "pem/{}_traditional_private.pem",
                curve.name().to_uppercase()
            ))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Pem { traditional: true });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ed {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ed_jwk_private_key() -> Result<()> {
        for curve in &[EdCurve::Ed25519, EdCurve::Ed448] {
            let input = load_file(&format!("jwk/OKP_{}_private.jwk", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Jwk);
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ed {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ed_jwk_public_key() -> Result<()> {
        for curve in &[EdCurve::Ed25519, EdCurve::Ed448] {
            let input = load_file(&format!("jwk/OKP_{}_public.jwk", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Jwk);
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ed {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), true);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ecx_der_private_key() -> Result<()> {
        for curve in &[EcxCurve::X25519, EcxCurve::X448] {
            let input = load_file(&format!(
                "der/{}_pkcs8_private.der",
                curve.name().to_uppercase()
            ))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Der { raw: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ecx {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ecx_der_public_key() -> Result<()> {
        for curve in &[EcxCurve::X25519, EcxCurve::X448] {
            let input = load_file(&format!(
                "der/{}_spki_public.der",
                curve.name().to_uppercase()
            ))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Der { raw: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ecx {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), true);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ecx_pem_private_key() -> Result<()> {
        for curve in &[EcxCurve::X25519, EcxCurve::X448] {
            let input = load_file(&format!("pem/{}_private.pem", curve.name().to_uppercase()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Pem { traditional: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ecx {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ecx_pem_public_key() -> Result<()> {
        for curve in &[EcxCurve::X25519, EcxCurve::X448] {
            let input = load_file(&format!("pem/{}_public.pem", curve.name().to_uppercase()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Pem { traditional: false });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ecx {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), true);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ecx_traditional_pem_private_key() -> Result<()> {
        for curve in &[EcxCurve::X25519, EcxCurve::X448] {
            let input = load_file(&format!(
                "pem/{}_traditional_private.pem",
                curve.name().to_uppercase()
            ))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Pem { traditional: true });
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ecx {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ecx_jwk_private_key() -> Result<()> {
        for curve in &[EcxCurve::X25519, EcxCurve::X448] {
            let input = load_file(&format!("jwk/OKP_{}_private.jwk", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Jwk);
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ecx {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), false);
        }

        Ok(())
    }

    #[test]
    fn test_detect_ecx_jwk_public_key() -> Result<()> {
        for curve in &[EcxCurve::X25519, EcxCurve::X448] {
            let input = load_file(&format!("jwk/OKP_{}_public.jwk", curve.name()))?;

            let key_info = KeyInfo::detect(&input).unwrap();
            assert_eq!(key_info.format(), KeyFormat::Jwk);
            assert_eq!(
                key_info.alg(),
                Some(KeyAlg::Ecx {
                    curve: Some(*curve)
                })
            );
            assert_eq!(key_info.is_public_key(), true);
        }

        Ok(())
    }

    fn load_file(path: &str) -> Result<Vec<u8>> {
        let mut pb = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pb.push("data");
        pb.push(path);

        let data = fs::read(&pb)?;
        Ok(data)
    }
}
