use std::borrow::Cow;
use std::fmt::Display;
use std::ops::Deref;

use anyhow::bail;
use openssl::aes::{self, AesKey};
use openssl::derive::Deriver;
use openssl::hash::{Hasher, MessageDigest};
use openssl::pkey::{PKey, Private, Public};

use crate::jwe::{JweAlgorithm, JweContentEncryption, JweDecrypter, JweEncrypter, JweHeader};
use crate::jwk::alg::{
    ec::{EcCurve, EcKeyPair},
    ecx::{EcxCurve, EcxKeyPair},
};
use crate::jwk::Jwk;
use crate::util;
use crate::util::der::{DerReader, DerType};
use crate::util::oid::{
    OID_ID_EC_PUBLIC_KEY, OID_PRIME256V1, OID_SECP256K1, OID_SECP384R1, OID_SECP521R1, OID_X25519,
    OID_X448,
};
use crate::{JoseError, JoseHeader, Map, Value};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum EcdhEsKeyType {
    Ec(EcCurve),
    Ecx(EcxCurve),
}

impl EcdhEsKeyType {
    fn key_type(&self) -> &str {
        match self {
            Self::Ec(_) => "EC",
            Self::Ecx(_) => "OKP",
        }
    }

    fn curve_name(&self) -> &str {
        match self {
            Self::Ec(val) => val.name(),
            Self::Ecx(val) => val.name(),
        }
    }
}

impl Display for EcdhEsKeyType {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str(self.key_type())?;
        fmt.write_str("(")?;
        fmt.write_str(self.curve_name())?;
        fmt.write_str(")")?;
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum EcdhEsJweAlgorithm {
    /// Elliptic Curve Diffie-Hellman Ephemeral Static key agreement using Concat KDF
    EcdhEs,
    /// ECDH-ES using Concat KDF and CEK wrapped with "A128KW"
    EcdhEsA128kw,
    /// ECDH-ES using Concat KDF and CEK wrapped with "A192KW"
    EcdhEsA192kw,
    /// ECDH-ES using Concat KDF and CEK wrapped with "A256KW"
    EcdhEsA256kw,
}

impl EcdhEsJweAlgorithm {
    /// Generate EC key pair for ECDH.
    pub fn generate_ec_key_pair(&self, curve: EcCurve) -> Result<EcKeyPair, JoseError> {
        let mut key_pair = EcKeyPair::generate(curve)?;
        key_pair.set_algorithm(Some(self.name()));
        Ok(key_pair)
    }

    /// Generate ECx key pair for ECDH.
    pub fn generate_ecx_key_pair(&self, curve: EcxCurve) -> Result<EcxKeyPair, JoseError> {
        let mut key_pair = EcxKeyPair::generate(curve)?;
        key_pair.set_algorithm(Some(self.name()));
        Ok(key_pair)
    }

    /// Create a EC key pair for ECDH from a private key that is a DER encoded PKCS#8 PrivateKeyInfo or ECPrivateKey.
    ///
    /// # Arguments
    /// * `input` - A private key that is a DER encoded PKCS#8 PrivateKeyInfo or ECPrivateKey.
    pub fn key_pair_from_ec_der(&self, input: impl AsRef<[u8]>) -> Result<EcKeyPair, JoseError> {
        let mut key_pair = EcKeyPair::from_der(input, None)?;
        key_pair.set_algorithm(Some(self.name()));
        Ok(key_pair)
    }

    /// Create a ECx key pair for ECDH from a private key that is a DER encoded PKCS#8 PrivateKeyInfo.
    ///
    /// # Arguments
    /// * `input` - A private key that is a DER encoded PKCS#8 PrivateKeyInfo.
    pub fn key_pair_from_ecx_der(&self, input: impl AsRef<[u8]>) -> Result<EcxKeyPair, JoseError> {
        let mut key_pair = EcxKeyPair::from_der(input)?;
        key_pair.set_algorithm(Some(self.name()));
        Ok(key_pair)
    }

    /// Create a EC key pair for ECDH from a private key of common or traditinal PEM format.
    ///
    /// Common PEM format is a DER and base64 encoded PKCS#8 PrivateKeyInfo
    /// that surrounded by "-----BEGIN/END PRIVATE KEY----".
    ///
    /// Traditional PEM format is a DER and base64 encoded ECPrivateKey
    /// that surrounded by "-----BEGIN/END EC PRIVATE KEY----".
    ///
    /// # Arguments
    /// * `input` - A private key of common or traditinal PEM format.
    pub fn key_pair_from_ec_pem(&self, input: impl AsRef<[u8]>) -> Result<EcKeyPair, JoseError> {
        let mut key_pair = EcKeyPair::from_pem(input.as_ref(), None)?;
        key_pair.set_algorithm(Some(self.name()));
        Ok(key_pair)
    }

    /// Create a ECx key pair for ECDH from a private key of common or traditinal PEM format.
    ///
    /// Common PEM format is a DER and base64 encoded PKCS#8 PrivateKeyInfo
    /// that surrounded by "-----BEGIN/END PRIVATE KEY----".
    ///
    /// Traditional PEM format is a DER and base64 encoded ECPrivateKey
    /// that surrounded by "-----BEGIN/END X25519/X448 PRIVATE KEY----".
    ///
    /// # Arguments
    /// * `input` - A private key of common or traditinal PEM format.
    pub fn key_pair_from_ecx_pem(&self, input: impl AsRef<[u8]>) -> Result<EcxKeyPair, JoseError> {
        let mut key_pair = EcxKeyPair::from_pem(input.as_ref())?;
        key_pair.set_algorithm(Some(self.name()));
        Ok(key_pair)
    }

    pub fn encrypter_from_der(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<EcdhEsJweEncrypter, JoseError> {
        (|| -> anyhow::Result<EcdhEsJweEncrypter> {
            let (spki, key_type) = match Self::detect_pkcs8(input.as_ref(), true) {
                Some(val) => (input.as_ref(), val),
                None => bail!("The public key must be wrapped by SubjectPublicKeyInfo."),
            };

            let public_key = PKey::public_key_from_der(spki)?;

            Ok(EcdhEsJweEncrypter {
                algorithm: self.clone(),
                public_key,
                key_type,
                key_id: None,
                agreement_partyuinfo: None,
                agreement_partyvinfo: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn encrypter_from_pem(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<EcdhEsJweEncrypter, JoseError> {
        (|| -> anyhow::Result<EcdhEsJweEncrypter> {
            let (alg, data) = util::parse_pem(input.as_ref())?;

            let (spki, key_type) = match alg.as_str() {
                "PUBLIC KEY" => match Self::detect_pkcs8(&data, true) {
                    Some(val) => (data.as_slice(), val),
                    None => bail!("PEM contents is expected SubjectPublicKeyInfo wrapped key."),
                },
                alg => bail!("Inappropriate algorithm: {}", alg),
            };

            let public_key = PKey::public_key_from_der(spki)?;

            Ok(EcdhEsJweEncrypter {
                algorithm: self.clone(),
                public_key,
                key_type,
                key_id: None,
                agreement_partyuinfo: None,
                agreement_partyvinfo: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn encrypter_from_jwk(&self, jwk: &Jwk) -> Result<EcdhEsJweEncrypter, JoseError> {
        (|| -> anyhow::Result<EcdhEsJweEncrypter> {
            let key_type = match jwk.key_type() {
                val if val == "EC" || val == "OKP" => val,
                val => bail!("A parameter kty must be EC or OKP: {}", val),
            };
            match jwk.key_use() {
                Some(val) if val == "enc" => {}
                None => {}
                Some(val) => bail!("A parameter use must be enc: {}", val),
            }
            if !jwk.is_for_key_operation("deriveKey") {
                bail!("A parameter key_ops must contains deriveKey.");
            }
            match jwk.algorithm() {
                Some(val) if val == self.name() => {}
                None => {}
                Some(val) => bail!("A parameter alg must be {} but {}", self.name(), val),
            }
            let (public_key, key_type) = match jwk.parameter("crv") {
                Some(Value::String(val)) => match key_type {
                    "EC" => {
                        let curve = match val.as_str() {
                            "P-256" => EcCurve::P256,
                            "P-384" => EcCurve::P384,
                            "P-521" => EcCurve::P521,
                            "secp256k1" => EcCurve::Secp256k1,
                            val => bail!("EC key doesn't support the curve algorithm: {}", val),
                        };
                        let x = match jwk.parameter("x") {
                            Some(Value::String(val)) => {
                                base64::decode_config(val, base64::URL_SAFE_NO_PAD)?
                            }
                            Some(_) => bail!("A parameter x must be a string."),
                            None => bail!("A parameter x is required."),
                        };
                        let y = match jwk.parameter("y") {
                            Some(Value::String(val)) => {
                                base64::decode_config(val, base64::URL_SAFE_NO_PAD)?
                            }
                            Some(_) => bail!("A parameter y must be a string."),
                            None => bail!("A parameter y is required."),
                        };

                        let mut vec = Vec::with_capacity(1 + x.len() + y.len());
                        vec.push(0x04);
                        vec.extend_from_slice(&x);
                        vec.extend_from_slice(&y);

                        let pkcs8 = EcKeyPair::to_pkcs8(&vec, true, curve);
                        let public_key = PKey::public_key_from_der(&pkcs8)?;

                        (public_key, EcdhEsKeyType::Ec(curve))
                    }
                    "OKP" => {
                        let curve = match val.as_str() {
                            "X25519" => EcxCurve::X25519,
                            "X448" => EcxCurve::X448,
                            val => bail!("OKP key doesn't support the curve algorithm: {}", val),
                        };
                        let x = match jwk.parameter("x") {
                            Some(Value::String(val)) => {
                                base64::decode_config(val, base64::URL_SAFE_NO_PAD)?
                            }
                            Some(_) => bail!("A parameter x must be a string."),
                            None => bail!("A parameter x is required."),
                        };

                        let pkcs8 = EcxKeyPair::to_pkcs8(&x, true, curve);
                        let public_key = PKey::public_key_from_der(&pkcs8)?;

                        (public_key, EcdhEsKeyType::Ecx(curve))
                    }
                    _ => unreachable!(),
                },
                Some(_) => bail!("A parameter crv must be a string."),
                None => bail!("A parameter crv is required."),
            };
            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(EcdhEsJweEncrypter {
                algorithm: self.clone(),
                key_type,
                public_key,
                key_id,
                agreement_partyuinfo: None,
                agreement_partyvinfo: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn decrypter_from_der(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<EcdhEsJweDecrypter, JoseError> {
        (|| -> anyhow::Result<EcdhEsJweDecrypter> {
            let pkcs8_der_vec;
            let (pkcs8_der, key_type) = match Self::detect_pkcs8(input.as_ref(), false) {
                Some(val) => (input.as_ref(), val),
                None => match EcKeyPair::detect_ec_curve(input.as_ref()) {
                    Some(val) => {
                        pkcs8_der_vec = EcKeyPair::to_pkcs8(input.as_ref(), false, val);
                        (pkcs8_der_vec.as_slice(), EcdhEsKeyType::Ec(val))
                    }
                    None => bail!("A curve name cannot be determined."),
                },
            };

            let private_key = PKey::private_key_from_der(pkcs8_der)?;

            Ok(EcdhEsJweDecrypter {
                algorithm: self.clone(),
                private_key,
                key_type,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn decrypter_from_pem(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<EcdhEsJweDecrypter, JoseError> {
        (|| -> anyhow::Result<EcdhEsJweDecrypter> {
            let (alg, data) = util::parse_pem(input.as_ref())?;

            let pkcs8_der_vec;
            let (pkcs8_der, key_type) = match alg.as_str() {
                "PRIVATE KEY" => match Self::detect_pkcs8(data.as_slice(), false) {
                    Some(val) => (data.as_slice(), val),
                    None => bail!("PEM contents is expected PKCS#8 wrapped key."),
                },
                "EC PRIVATE KEY" => match EcKeyPair::detect_ec_curve(data.as_slice()) {
                    Some(val) => {
                        pkcs8_der_vec = EcKeyPair::to_pkcs8(data.as_slice(), false, val);
                        (pkcs8_der_vec.as_slice(), EcdhEsKeyType::Ec(val))
                    }
                    None => bail!("A curve name cannot be determined."),
                },
                "X25519 PRIVATE KEY" => match Self::detect_pkcs8(data.as_slice(), false) {
                    Some(val @ EcdhEsKeyType::Ecx(EcxCurve::X25519)) => (data.as_slice(), val),
                    Some(val) => bail!("The curve name is mismatched: {}", val),
                    None => bail!("PEM contents is expected PKCS#8 wrapped key."),
                },
                "X448 PRIVATE KEY" => match Self::detect_pkcs8(data.as_slice(), false) {
                    Some(val @ EcdhEsKeyType::Ecx(EcxCurve::X448)) => (data.as_slice(), val),
                    Some(val) => bail!("The curve name is mismatched: {}", val),
                    None => bail!("PEM contents is expected PKCS#8 wrapped key."),
                },
                alg => bail!("Inappropriate algorithm: {}", alg),
            };

            let private_key = PKey::private_key_from_der(pkcs8_der)?;

            Ok(EcdhEsJweDecrypter {
                algorithm: self.clone(),
                private_key,
                key_type,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn decrypter_from_jwk(&self, jwk: &Jwk) -> Result<EcdhEsJweDecrypter, JoseError> {
        (|| -> anyhow::Result<EcdhEsJweDecrypter> {
            let key_type = match jwk.key_type() {
                val if val == "EC" || val == "OKP" => val,
                val => bail!("A parameter kty must be EC or OKP: {}", val),
            };
            match jwk.key_use() {
                Some(val) if val == "enc" => {}
                None => {}
                Some(val) => bail!("A parameter use must be enc: {}", val),
            }
            if !jwk.is_for_key_operation("deriveKey") {
                bail!("A parameter key_ops must contains deriveKey.");
            }
            match jwk.algorithm() {
                Some(val) if val == self.name() => {}
                None => {}
                Some(val) => bail!("A parameter alg must be {} but {}", self.name(), val),
            }
            let (private_key, key_type) = match jwk.parameter("crv") {
                Some(Value::String(val)) => match key_type {
                    "EC" => {
                        let curve = match val.as_str() {
                            "P-256" => EcCurve::P256,
                            "P-384" => EcCurve::P384,
                            "P-521" => EcCurve::P521,
                            "secp256k1" => EcCurve::Secp256k1,
                            val => bail!("EC key doesn't support the curve algorithm: {}", val),
                        };
                        match jwk.curve() {
                            Some(val) if val == curve.name() => {}
                            Some(val) => {
                                bail!("A parameter crv must be {} but {}", self.name(), val)
                            }
                            None => bail!("A parameter crv is required."),
                        }
                        let key_pair = EcKeyPair::from_jwk(&jwk)?;
                        let private_key = key_pair.into_private_key();

                        (private_key, EcdhEsKeyType::Ec(curve))
                    }
                    "OKP" => {
                        let curve = match val.as_str() {
                            "X25519" => EcxCurve::X25519,
                            "X448" => EcxCurve::X448,
                            val => bail!("OKP key doesn't support the curve algorithm: {}", val),
                        };
                        match jwk.curve() {
                            Some(val) if val == curve.name() => {}
                            Some(val) => {
                                bail!("A parameter crv must be {} but {}", self.name(), val)
                            }
                            None => bail!("A parameter crv is required."),
                        }
                        let key_pair = EcxKeyPair::from_jwk(&jwk)?;
                        let private_key = key_pair.into_private_key();

                        (private_key, EcdhEsKeyType::Ecx(curve))
                    }
                    _ => unreachable!(),
                },
                Some(_) => bail!("A parameter crv must be a string."),
                None => bail!("A parameter crv is required."),
            };
            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(EcdhEsJweDecrypter {
                algorithm: self.clone(),
                private_key,
                key_type,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    fn key_len(&self) -> usize {
        match self {
            Self::EcdhEsA128kw => 16,
            Self::EcdhEsA192kw => 24,
            Self::EcdhEsA256kw => 32,
            _ => unreachable!(),
        }
    }

    fn detect_pkcs8(input: &[u8], is_public: bool) -> Option<EcdhEsKeyType> {
        let key_type;
        let mut reader = DerReader::from_reader(input);

        match reader.next() {
            Ok(Some(DerType::Sequence)) => {}
            _ => return None,
        }

        {
            if !is_public {
                // Version
                match reader.next() {
                    Ok(Some(DerType::Integer)) => match reader.to_u8() {
                        Ok(val) => {
                            if val != 0 {
                                return None;
                            }
                        }
                        _ => return None,
                    },
                    _ => return None,
                }
            }

            match reader.next() {
                Ok(Some(DerType::Sequence)) => {}
                _ => return None,
            }

            {
                match reader.next() {
                    Ok(Some(DerType::ObjectIdentifier)) => match reader.to_object_identifier() {
                        Ok(val) => {
                            if val == *OID_X25519 {
                                return Some(EcdhEsKeyType::Ecx(EcxCurve::X25519));
                            } else if val == *OID_X448 {
                                return Some(EcdhEsKeyType::Ecx(EcxCurve::X448));
                            } else if val != *OID_ID_EC_PUBLIC_KEY {
                                return None;
                            }
                        }
                        _ => return None,
                    },
                    _ => return None,
                }

                key_type = match reader.next() {
                    Ok(Some(DerType::ObjectIdentifier)) => match reader.to_object_identifier() {
                        Ok(val) if val == *OID_PRIME256V1 => EcdhEsKeyType::Ec(EcCurve::P256),
                        Ok(val) if val == *OID_SECP384R1 => EcdhEsKeyType::Ec(EcCurve::P384),
                        Ok(val) if val == *OID_SECP521R1 => EcdhEsKeyType::Ec(EcCurve::P521),
                        Ok(val) if val == *OID_SECP256K1 => EcdhEsKeyType::Ec(EcCurve::Secp256k1),
                        _ => return None,
                    },
                    _ => return None,
                }
            }
        }

        Some(key_type)
    }

    fn concat_kdf(
        &self,
        alg: &str,
        shared_key_len: usize,
        derived_key: &[u8],
        apu: Option<&[u8]>,
        apv: Option<&[u8]>,
    ) -> anyhow::Result<Vec<u8>> {
        let shared_key_len_bytes = ((shared_key_len * 8) as u32).to_be_bytes();
        let alg_len_bytes = (alg.len() as u32).to_be_bytes();
        let apu_len_bytes = (match apu {
            Some(val) => val.len(),
            None => 0,
        } as u32)
            .to_be_bytes();
        let apv_len_bytes = (match apv {
            Some(val) => val.len(),
            None => 0,
        } as u32)
            .to_be_bytes();

        let mut shared_key = Vec::new();
        let md = MessageDigest::sha256();
        let count = util::ceiling(shared_key_len, md.size());
        for i in 0..count {
            let mut hasher = Hasher::new(md)?;
            hasher.update(&((i + 1) as u32).to_be_bytes())?;
            hasher.update(&derived_key)?;
            hasher.update(&alg_len_bytes)?;
            hasher.update(alg.as_bytes())?;
            hasher.update(&apu_len_bytes)?;
            if let Some(val) = apu {
                hasher.update(val)?;
            }
            hasher.update(&apv_len_bytes)?;
            if let Some(val) = apv {
                hasher.update(val)?;
            }
            hasher.update(&shared_key_len_bytes)?;

            let digest = hasher.finish()?;
            shared_key.extend(digest.to_vec());
        }

        if shared_key.len() > shared_key_len {
            shared_key.truncate(shared_key_len);
        } else if shared_key.len() < shared_key_len {
            unreachable!();
        }

        Ok(shared_key)
    }
}

impl JweAlgorithm for EcdhEsJweAlgorithm {
    fn name(&self) -> &str {
        match self {
            Self::EcdhEs => "ECDH-ES",
            Self::EcdhEsA128kw => "ECDH-ES+A128KW",
            Self::EcdhEsA192kw => "ECDH-ES+A192KW",
            Self::EcdhEsA256kw => "ECDH-ES+A256KW",
        }
    }

    fn box_clone(&self) -> Box<dyn JweAlgorithm> {
        Box::new(self.clone())
    }
}

impl Display for EcdhEsJweAlgorithm {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str(self.name())
    }
}

impl Deref for EcdhEsJweAlgorithm {
    type Target = dyn JweAlgorithm;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct EcdhEsJweEncrypter {
    algorithm: EcdhEsJweAlgorithm,
    key_type: EcdhEsKeyType,
    public_key: PKey<Public>,
    agreement_partyuinfo: Option<Vec<u8>>,
    agreement_partyvinfo: Option<Vec<u8>>,
    key_id: Option<String>,
}

impl EcdhEsJweEncrypter {
    pub fn set_agreement_partyuinfo(&mut self, value: impl Into<Vec<u8>>) {
        self.agreement_partyuinfo = Some(value.into());
    }

    pub fn remove_agreement_partyuinfo(&mut self) {
        self.agreement_partyuinfo = None;
    }

    pub fn set_agreement_partyvinfo(&mut self, value: impl Into<Vec<u8>>) {
        self.agreement_partyvinfo = Some(value.into());
    }

    pub fn remove_agreement_partyvinfo(&mut self) {
        self.agreement_partyvinfo = None;
    }

    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }

    fn compute_shared_key(
        &self,
        header: &mut JweHeader,
        alg: &str,
        key_len: usize,
    ) -> Result<Vec<u8>, JoseError> {
        (|| -> anyhow::Result<Vec<u8>> {
            let apu_vec;
            let apu = match header.claim("apu") {
                Some(Value::String(val)) => {
                    apu_vec = base64::decode_config(val, base64::URL_SAFE_NO_PAD)?;
                    Some(apu_vec.as_slice())
                }
                Some(_) => bail!("The apu header claim must be string."),
                None => match &self.agreement_partyuinfo {
                    Some(val) => {
                        let apu_b64 = base64::encode_config(val, base64::URL_SAFE_NO_PAD);
                        header.set_claim("apu", Some(Value::String(apu_b64)))?;
                        Some(val.as_slice())
                    }
                    None => None,
                },
            };
            let apv_vec;
            let apv = match header.claim("apv") {
                Some(Value::String(val)) => {
                    apv_vec = base64::decode_config(val, base64::URL_SAFE_NO_PAD)?;
                    Some(apv_vec.as_slice())
                }
                Some(_) => bail!("The apv header claim must be string."),
                None => match &self.agreement_partyvinfo {
                    Some(val) => {
                        let apv_b64 = base64::encode_config(val, base64::URL_SAFE_NO_PAD);
                        header.set_claim("apv", Some(Value::String(apv_b64)))?;
                        Some(val.as_slice())
                    }
                    None => None,
                },
            };

            let mut map = Map::new();
            map.insert(
                "kty".to_string(),
                Value::String(self.key_type.key_type().to_string()),
            );
            map.insert(
                "crv".to_string(),
                Value::String(self.key_type.curve_name().to_string()),
            );
            let private_key = match self.key_type {
                EcdhEsKeyType::Ec(curve) => {
                    let key_pair = EcKeyPair::generate(curve)?;
                    let mut jwk: Map<String, Value> = key_pair.to_jwk_public_key().into();

                    match jwk.remove("x") {
                        Some(val) => {
                            map.insert("x".to_string(), val);
                        }
                        None => unreachable!(),
                    }
                    match jwk.remove("y") {
                        Some(val) => {
                            map.insert("y".to_string(), val);
                        }
                        None => unreachable!(),
                    }

                    key_pair.into_private_key()
                }
                EcdhEsKeyType::Ecx(curve) => {
                    let key_pair = EcxKeyPair::generate(curve)?;
                    let mut jwk: Map<String, Value> = key_pair.to_jwk_public_key().into();

                    match jwk.remove("x") {
                        Some(val) => {
                            map.insert("x".to_string(), val);
                        }
                        None => unreachable!(),
                    }

                    key_pair.into_private_key()
                }
            };

            header.set_claim("epk", Some(Value::Object(map)))?;

            let mut deriver = Deriver::new(&private_key)?;
            deriver.set_peer(&self.public_key)?;
            let derived_key = deriver.derive_to_vec()?;

            let shared_key = self.algorithm.concat_kdf(
                alg,
                key_len,
                &derived_key,
                apu.as_deref(),
                apv.as_deref(),
            )?;

            Ok(shared_key)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidKeyFormat(err),
        })
    }
}

impl JweEncrypter for EcdhEsJweEncrypter {
    fn algorithm(&self) -> &dyn JweAlgorithm {
        &self.algorithm
    }

    fn key_id(&self) -> Option<&str> {
        match &self.key_id {
            Some(val) => Some(val.as_ref()),
            None => None,
        }
    }

    fn compute_content_encryption_key(
        &self,
        cencryption: &dyn JweContentEncryption,
        _merged: &JweHeader,
        header: &mut JweHeader,
    ) -> Result<Option<Cow<[u8]>>, JoseError> {
        if let EcdhEsJweAlgorithm::EcdhEs = self.algorithm {
            let shared_key =
                self.compute_shared_key(header, cencryption.name(), cencryption.key_len())?;
            Ok(Some(Cow::Owned(shared_key)))
        } else {
            Ok(None)
        }
    }

    fn encrypt(
        &self,
        key: &[u8],
        _merged: &JweHeader,
        header: &mut JweHeader,
    ) -> Result<Option<Vec<u8>>, JoseError> {
        (|| -> anyhow::Result<Option<Vec<u8>>> {
            if let EcdhEsJweAlgorithm::EcdhEs = self.algorithm {
                Ok(None)
            } else {
                let shared_key = self.compute_shared_key(
                    header,
                    self.algorithm().name(),
                    self.algorithm.key_len(),
                )?;
                let aes = match AesKey::new_encrypt(&shared_key) {
                    Ok(val) => val,
                    Err(_) => bail!("Failed to set encrypt key."),
                };

                let mut encrypted_key = vec![0; key.len() + 8];
                match aes::wrap_key(&aes, None, &mut encrypted_key, &key) {
                    Ok(len) => {
                        if len < encrypted_key.len() {
                            encrypted_key.truncate(len);
                        }
                    }
                    Err(_) => bail!("Failed to wrap key."),
                }

                Ok(Some(encrypted_key))
            }
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidKeyFormat(err),
        })
    }

    fn box_clone(&self) -> Box<dyn JweEncrypter> {
        Box::new(self.clone())
    }
}

impl Deref for EcdhEsJweEncrypter {
    type Target = dyn JweEncrypter;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct EcdhEsJweDecrypter {
    algorithm: EcdhEsJweAlgorithm,
    private_key: PKey<Private>,
    key_type: EcdhEsKeyType,
    key_id: Option<String>,
}

impl EcdhEsJweDecrypter {
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JweDecrypter for EcdhEsJweDecrypter {
    fn algorithm(&self) -> &dyn JweAlgorithm {
        &self.algorithm
    }

    fn key_id(&self) -> Option<&str> {
        match &self.key_id {
            Some(val) => Some(val.as_ref()),
            None => None,
        }
    }

    fn decrypt(
        &self,
        encrypted_key: Option<&[u8]>,
        cencryption: &dyn JweContentEncryption,
        header: &JweHeader,
    ) -> Result<Cow<[u8]>, JoseError> {
        (|| -> anyhow::Result<Cow<[u8]>> {
            match &self.algorithm {
                EcdhEsJweAlgorithm::EcdhEs => {
                    if encrypted_key.is_some() {
                        bail!("The encrypted_key must be empty.");
                    }
                }
                _ => {
                    if encrypted_key.is_none() {
                        bail!("A encrypted_key is required.");
                    }
                }
            }

            let apu = match header.claim("apu") {
                Some(Value::String(val)) => {
                    let apu = base64::decode_config(val, base64::URL_SAFE_NO_PAD)?;
                    Some(apu)
                }
                Some(_) => bail!("The apu header claim must be string."),
                None => None,
            };
            let apv = match header.claim("apv") {
                Some(Value::String(val)) => {
                    let apv = base64::decode_config(val, base64::URL_SAFE_NO_PAD)?;
                    Some(apv)
                }
                Some(_) => bail!("The apv header claim must be string."),
                None => None,
            };

            let public_key = match header.claim("epk") {
                Some(Value::Object(map)) => {
                    match map.get("kty") {
                        Some(Value::String(val)) => {
                            if val != self.key_type.key_type() {
                                bail!("The kty parameter in epk header claim is invalid: {}", val);
                            }
                        }
                        Some(_) => bail!("The kty parameter in epk header claim must be a string."),
                        None => bail!("The kty parameter in epk header claim is required."),
                    }

                    match map.get("crv") {
                        Some(Value::String(val)) => {
                            if val != self.key_type.curve_name() {
                                bail!("The crv parameter in epk header claim is invalid: {}", val);
                            }
                        }
                        Some(_) => bail!("The crv parameter in epk header claim must be a string."),
                        None => bail!("The crv parameter in epk header claim is required."),
                    }

                    match &self.key_type {
                        EcdhEsKeyType::Ec(curve) => {
                            let x = match map.get("x") {
                                Some(Value::String(val)) => {
                                    base64::decode_config(val, base64::URL_SAFE_NO_PAD)?
                                }
                                Some(_) => {
                                    bail!("The x parameter in epk header claim must be a string.")
                                }
                                None => bail!("The x parameter in epk header claim is required."),
                            };
                            let y = match map.get("y") {
                                Some(Value::String(val)) => {
                                    base64::decode_config(val, base64::URL_SAFE_NO_PAD)?
                                }
                                Some(_) => {
                                    bail!("The x parameter in epk header claim must be a string.")
                                }
                                None => bail!("The x parameter in epk header claim is required."),
                            };

                            let mut vec = Vec::with_capacity(1 + x.len() + y.len());
                            vec.push(0x04);
                            vec.extend_from_slice(&x);
                            vec.extend_from_slice(&y);

                            let pkcs8 = EcKeyPair::to_pkcs8(&vec, true, *curve);
                            PKey::public_key_from_der(&pkcs8)?
                        }
                        EcdhEsKeyType::Ecx(curve) => {
                            let x = match map.get("x") {
                                Some(Value::String(val)) => {
                                    base64::decode_config(val, base64::URL_SAFE_NO_PAD)?
                                }
                                Some(_) => {
                                    bail!("The x parameter in epk header claim must be a string.")
                                }
                                None => bail!("The x parameter in epk header claim is required."),
                            };

                            let pkcs8 = EcxKeyPair::to_pkcs8(&x, true, *curve);
                            PKey::public_key_from_der(&pkcs8)?
                        }
                    }
                }
                Some(_) => bail!("The epk header claim must be object."),
                None => bail!("This algorithm must have epk header claim."),
            };

            let mut deriver = Deriver::new(&self.private_key)?;
            deriver.set_peer(&public_key)?;
            let derived_key = deriver.derive_to_vec()?;

            // concat KDF
            if let EcdhEsJweAlgorithm::EcdhEs = self.algorithm {
                let shared_key = self.algorithm.concat_kdf(
                    cencryption.name(),
                    cencryption.key_len(),
                    &derived_key,
                    apu.as_deref(),
                    apv.as_deref(),
                )?;
                Ok(Cow::Owned(shared_key))
            } else {
                let shared_key = self.algorithm.concat_kdf(
                    self.algorithm.name(),
                    self.algorithm.key_len(),
                    &derived_key,
                    apu.as_deref(),
                    apv.as_deref(),
                )?;

                let aes = match AesKey::new_decrypt(&shared_key) {
                    Ok(val) => val,
                    Err(_) => bail!("Failed to set encrypt key."),
                };

                let encrypted_key = match encrypted_key {
                    Some(val) => val,
                    None => unreachable!(),
                };

                let mut key = vec![0; encrypted_key.len() - 8];
                match aes::unwrap_key(&aes, None, &mut key, &encrypted_key) {
                    Ok(len) => {
                        if len < key.len() {
                            key.truncate(len);
                        }
                    }
                    Err(_) => bail!("Failed to unwrap key."),
                };

                Ok(Cow::Owned(key))
            }
        })()
        .map_err(|err| JoseError::InvalidJweFormat(err))
    }

    fn box_clone(&self) -> Box<dyn JweDecrypter> {
        Box::new(self.clone())
    }
}

impl Deref for EcdhEsJweDecrypter {
    type Target = dyn JweDecrypter;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::borrow::Cow;
    use std::fs;
    use std::path::PathBuf;

    use super::{EcdhEsJweAlgorithm, EcdhEsKeyType};
    use crate::jwe::enc::aescbc_hmac::AescbcHmacJweEncryption;
    use crate::jwe::enc::aesgcm::AesgcmJweEncryption;
    use crate::jwe::JweHeader;
    use crate::jwk::alg::{ec::EcCurve, ecx::EcxCurve};
    use crate::jwk::Jwk;
    use crate::util;

    #[test]
    fn encrypt_and_decrypt_ecdh_es_with_pkcs8_der() -> Result<()> {
        let enc = AescbcHmacJweEncryption::A128cbcHs256;

        for alg in vec![
            EcdhEsJweAlgorithm::EcdhEs,
            EcdhEsJweAlgorithm::EcdhEsA128kw,
            EcdhEsJweAlgorithm::EcdhEsA192kw,
            EcdhEsJweAlgorithm::EcdhEsA256kw,
        ] {
            for key in vec![
                EcdhEsKeyType::Ec(EcCurve::P256),
                EcdhEsKeyType::Ec(EcCurve::P384),
                EcdhEsKeyType::Ec(EcCurve::P521),
                EcdhEsKeyType::Ec(EcCurve::Secp256k1),
                EcdhEsKeyType::Ecx(EcxCurve::X25519),
                EcdhEsKeyType::Ecx(EcxCurve::X448),
            ] {
                let private_key = load_file(match key {
                    EcdhEsKeyType::Ec(EcCurve::P256) => "der/EC_P-256_pkcs8_private.der",
                    EcdhEsKeyType::Ec(EcCurve::P384) => "der/EC_P-384_pkcs8_private.der",
                    EcdhEsKeyType::Ec(EcCurve::P521) => "der/EC_P-521_pkcs8_private.der",
                    EcdhEsKeyType::Ec(EcCurve::Secp256k1) => "der/EC_secp256k1_pkcs8_private.der",
                    EcdhEsKeyType::Ecx(EcxCurve::X25519) => "der/X25519_pkcs8_private.der",
                    EcdhEsKeyType::Ecx(EcxCurve::X448) => "der/X448_pkcs8_private.der",
                })?;

                let public_key = load_file(match key {
                    EcdhEsKeyType::Ec(EcCurve::P256) => "der/EC_P-256_spki_public.der",
                    EcdhEsKeyType::Ec(EcCurve::P384) => "der/EC_P-384_spki_public.der",
                    EcdhEsKeyType::Ec(EcCurve::P521) => "der/EC_P-521_spki_public.der",
                    EcdhEsKeyType::Ec(EcCurve::Secp256k1) => "der/EC_secp256k1_spki_public.der",
                    EcdhEsKeyType::Ecx(EcxCurve::X25519) => "der/X25519_spki_public.der",
                    EcdhEsKeyType::Ecx(EcxCurve::X448) => "der/X448_spki_public.der",
                })?;

                let mut header = JweHeader::new();
                header.set_content_encryption(enc.name());

                let encrypter = alg.encrypter_from_der(&public_key)?;
                let mut out_header = header.clone();
                let src_key = match encrypter.compute_content_encryption_key(
                    &enc,
                    &header,
                    &mut out_header,
                )? {
                    Some(val) => val,
                    None => Cow::Owned(util::random_bytes(enc.key_len())),
                };
                let encrypted_key = encrypter.encrypt(&src_key, &header, &mut out_header)?;

                out_header.set_algorithm(alg.name());
                let decrypter = alg.decrypter_from_der(&private_key)?;
                let dst_key = decrypter.decrypt(encrypted_key.as_deref(), &enc, &out_header)?;

                assert_eq!(&src_key, &dst_key);
            }
        }

        Ok(())
    }

    #[test]
    fn encrypt_and_decrypt_ecdh_es_with_pem() -> Result<()> {
        let enc = AescbcHmacJweEncryption::A128cbcHs256;

        for alg in vec![
            EcdhEsJweAlgorithm::EcdhEs,
            EcdhEsJweAlgorithm::EcdhEsA128kw,
            EcdhEsJweAlgorithm::EcdhEsA192kw,
            EcdhEsJweAlgorithm::EcdhEsA256kw,
        ] {
            for key in vec![
                EcdhEsKeyType::Ec(EcCurve::P256),
                EcdhEsKeyType::Ec(EcCurve::P384),
                EcdhEsKeyType::Ec(EcCurve::P521),
                EcdhEsKeyType::Ec(EcCurve::Secp256k1),
                EcdhEsKeyType::Ecx(EcxCurve::X25519),
                EcdhEsKeyType::Ecx(EcxCurve::X448),
            ] {
                let private_key = load_file(match key {
                    EcdhEsKeyType::Ec(EcCurve::P256) => "pem/EC_P-256_private.pem",
                    EcdhEsKeyType::Ec(EcCurve::P384) => "pem/EC_P-384_private.pem",
                    EcdhEsKeyType::Ec(EcCurve::P521) => "pem/EC_P-521_private.pem",
                    EcdhEsKeyType::Ec(EcCurve::Secp256k1) => "pem/EC_secp256k1_private.pem",
                    EcdhEsKeyType::Ecx(EcxCurve::X25519) => "pem/X25519_private.pem",
                    EcdhEsKeyType::Ecx(EcxCurve::X448) => "pem/X448_private.pem",
                })?;

                let public_key = load_file(match key {
                    EcdhEsKeyType::Ec(EcCurve::P256) => "pem/EC_P-256_public.pem",
                    EcdhEsKeyType::Ec(EcCurve::P384) => "pem/EC_P-384_public.pem",
                    EcdhEsKeyType::Ec(EcCurve::P521) => "pem/EC_P-521_public.pem",
                    EcdhEsKeyType::Ec(EcCurve::Secp256k1) => "pem/EC_secp256k1_public.pem",
                    EcdhEsKeyType::Ecx(EcxCurve::X25519) => "pem/X25519_public.pem",
                    EcdhEsKeyType::Ecx(EcxCurve::X448) => "pem/X448_public.pem",
                })?;

                let mut header = JweHeader::new();
                header.set_content_encryption(enc.name());

                let encrypter = alg.encrypter_from_pem(&public_key)?;
                let mut out_header = header.clone();
                let src_key = match encrypter.compute_content_encryption_key(
                    &enc,
                    &header,
                    &mut out_header,
                )? {
                    Some(val) => val,
                    None => Cow::Owned(util::random_bytes(enc.key_len())),
                };
                let encrypted_key = encrypter.encrypt(&src_key, &header, &mut out_header)?;

                out_header.set_algorithm(alg.name());
                let decrypter = alg.decrypter_from_pem(&private_key)?;
                let dst_key = decrypter.decrypt(encrypted_key.as_deref(), &enc, &out_header)?;

                assert_eq!(&src_key, &dst_key);
            }
        }

        Ok(())
    }

    #[test]
    fn encrypt_and_decrypt_ecdh_es_with_traditional_pem() -> Result<()> {
        let enc = AesgcmJweEncryption::A128gcm;

        for alg in vec![
            EcdhEsJweAlgorithm::EcdhEs,
            EcdhEsJweAlgorithm::EcdhEsA128kw,
            EcdhEsJweAlgorithm::EcdhEsA192kw,
            EcdhEsJweAlgorithm::EcdhEsA256kw,
        ] {
            for key in vec![
                EcdhEsKeyType::Ec(EcCurve::P256),
                EcdhEsKeyType::Ec(EcCurve::P384),
                EcdhEsKeyType::Ec(EcCurve::P521),
                EcdhEsKeyType::Ec(EcCurve::Secp256k1),
                EcdhEsKeyType::Ecx(EcxCurve::X25519),
                EcdhEsKeyType::Ecx(EcxCurve::X448),
            ] {
                let private_key = load_file(match key {
                    EcdhEsKeyType::Ec(EcCurve::P256) => "pem/EC_P-256_traditional_private.pem",
                    EcdhEsKeyType::Ec(EcCurve::P384) => "pem/EC_P-384_traditional_private.pem",
                    EcdhEsKeyType::Ec(EcCurve::P521) => "pem/EC_P-521_traditional_private.pem",
                    EcdhEsKeyType::Ec(EcCurve::Secp256k1) => {
                        "pem/EC_secp256k1_traditional_private.pem"
                    }
                    EcdhEsKeyType::Ecx(EcxCurve::X25519) => "pem/X25519_traditional_private.pem",
                    EcdhEsKeyType::Ecx(EcxCurve::X448) => "pem/X448_traditional_private.pem",
                })?;

                let public_key = load_file(match key {
                    EcdhEsKeyType::Ec(EcCurve::P256) => "pem/EC_P-256_public.pem",
                    EcdhEsKeyType::Ec(EcCurve::P384) => "pem/EC_P-384_public.pem",
                    EcdhEsKeyType::Ec(EcCurve::P521) => "pem/EC_P-521_public.pem",
                    EcdhEsKeyType::Ec(EcCurve::Secp256k1) => "pem/EC_secp256k1_public.pem",
                    EcdhEsKeyType::Ecx(EcxCurve::X25519) => "pem/X25519_public.pem",
                    EcdhEsKeyType::Ecx(EcxCurve::X448) => "pem/X448_public.pem",
                })?;

                let mut header = JweHeader::new();
                header.set_content_encryption(enc.name());

                let encrypter = alg.encrypter_from_pem(&public_key)?;
                let mut out_header = header.clone();
                let src_key = match encrypter.compute_content_encryption_key(
                    &enc,
                    &header,
                    &mut out_header,
                )? {
                    Some(val) => val,
                    None => Cow::Owned(util::random_bytes(enc.key_len())),
                };
                let encrypted_key = encrypter.encrypt(&src_key, &header, &mut out_header)?;

                out_header.set_algorithm(alg.name());
                let decrypter = alg.decrypter_from_pem(&private_key)?;
                let dst_key = decrypter.decrypt(encrypted_key.as_deref(), &enc, &out_header)?;

                assert_eq!(&src_key, &dst_key);
            }
        }

        Ok(())
    }

    #[test]
    fn encrypt_and_decrypt_ecdh_es_with_jwk() -> Result<()> {
        let enc = AescbcHmacJweEncryption::A128cbcHs256;

        for alg in vec![
            EcdhEsJweAlgorithm::EcdhEs,
            EcdhEsJweAlgorithm::EcdhEsA128kw,
            EcdhEsJweAlgorithm::EcdhEsA192kw,
            EcdhEsJweAlgorithm::EcdhEsA256kw,
        ] {
            for key in vec![
                EcdhEsKeyType::Ec(EcCurve::P256),
                EcdhEsKeyType::Ec(EcCurve::P384),
                EcdhEsKeyType::Ec(EcCurve::P521),
                EcdhEsKeyType::Ec(EcCurve::Secp256k1),
                EcdhEsKeyType::Ecx(EcxCurve::X25519),
                EcdhEsKeyType::Ecx(EcxCurve::X448),
            ] {
                let private_key = load_file(match key {
                    EcdhEsKeyType::Ec(EcCurve::P256) => "jwk/EC_P-256_private.jwk",
                    EcdhEsKeyType::Ec(EcCurve::P384) => "jwk/EC_P-384_private.jwk",
                    EcdhEsKeyType::Ec(EcCurve::P521) => "jwk/EC_P-521_private.jwk",
                    EcdhEsKeyType::Ec(EcCurve::Secp256k1) => "jwk/EC_secp256k1_private.jwk",
                    EcdhEsKeyType::Ecx(EcxCurve::X25519) => "jwk/OKP_X25519_private.jwk",
                    EcdhEsKeyType::Ecx(EcxCurve::X448) => "jwk/OKP_X448_private.jwk",
                })?;

                let public_key = load_file(match key {
                    EcdhEsKeyType::Ec(EcCurve::P256) => "jwk/EC_P-256_public.jwk",
                    EcdhEsKeyType::Ec(EcCurve::P384) => "jwk/EC_P-384_public.jwk",
                    EcdhEsKeyType::Ec(EcCurve::P521) => "jwk/EC_P-521_public.jwk",
                    EcdhEsKeyType::Ec(EcCurve::Secp256k1) => "jwk/EC_secp256k1_public.jwk",
                    EcdhEsKeyType::Ecx(EcxCurve::X25519) => "jwk/OKP_X25519_public.jwk",
                    EcdhEsKeyType::Ecx(EcxCurve::X448) => "jwk/OKP_X448_public.jwk",
                })?;

                let mut header = JweHeader::new();
                header.set_content_encryption(enc.name());

                let public_key = Jwk::from_bytes(&public_key)?;
                let encrypter = alg.encrypter_from_jwk(&public_key)?;
                let mut out_header = header.clone();
                let src_key = match encrypter.compute_content_encryption_key(
                    &enc,
                    &header,
                    &mut out_header,
                )? {
                    Some(val) => val,
                    None => Cow::Owned(util::random_bytes(enc.key_len())),
                };
                let encrypted_key = encrypter.encrypt(&src_key, &header, &mut out_header)?;

                out_header.set_algorithm(alg.name());
                let private_key = Jwk::from_bytes(&private_key)?;
                let decrypter = alg.decrypter_from_jwk(&private_key)?;
                let dst_key = decrypter.decrypt(encrypted_key.as_deref(), &enc, &out_header)?;

                assert_eq!(&src_key, &dst_key);
            }
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
