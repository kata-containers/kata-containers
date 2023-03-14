use std::fmt::Display;
use std::ops::Deref;

use anyhow::bail;
use openssl::bn::{BigNum, BigNumContext};
use openssl::ec::{EcGroup, EcKey};
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};

use crate::jwk::{Jwk, KeyPair};
use crate::util;
use crate::util::der::{DerBuilder, DerClass, DerReader, DerType};
use crate::util::oid::{
    ObjectIdentifier, OID_ID_EC_PUBLIC_KEY, OID_PRIME256V1, OID_SECP256K1, OID_SECP384R1,
    OID_SECP521R1,
};
use crate::{JoseError, Value};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum EcCurve {
    P256,
    P384,
    P521,
    Secp256k1,
}

impl EcCurve {
    pub fn name(&self) -> &str {
        match self {
            Self::P256 => "P-256",
            Self::P384 => "P-384",
            Self::P521 => "P-521",
            Self::Secp256k1 => "secp256k1",
        }
    }

    pub fn oid(&self) -> &ObjectIdentifier {
        match self {
            Self::P256 => &OID_PRIME256V1,
            Self::P384 => &OID_SECP384R1,
            Self::P521 => &OID_SECP521R1,
            Self::Secp256k1 => &OID_SECP256K1,
        }
    }

    fn nid(&self) -> Nid {
        match self {
            Self::P256 => Nid::X9_62_PRIME256V1,
            Self::P384 => Nid::SECP384R1,
            Self::P521 => Nid::SECP521R1,
            Self::Secp256k1 => Nid::SECP256K1,
        }
    }

    fn coordinate_size(&self) -> usize {
        match self {
            Self::P256 | Self::Secp256k1 => 32,
            Self::P384 => 48,
            Self::P521 => 66,
        }
    }
}

impl Display for EcCurve {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str(self.name())
    }
}

#[derive(Debug, Clone)]
pub struct EcKeyPair {
    private_key: PKey<Private>,
    curve: EcCurve,
    algorithm: Option<String>,
    key_id: Option<String>,
}

impl EcKeyPair {
    pub fn curve(&self) -> EcCurve {
        self.curve
    }

    pub fn set_algorithm(&mut self, value: Option<&str>) {
        self.algorithm = value.map(|val| val.to_string());
    }

    pub fn set_key_id(&mut self, key_id: Option<impl Into<String>>) {
        match key_id {
            Some(val) => {
                self.key_id = Some(val.into());
            }
            None => {
                self.key_id = None;
            }
        }
    }

    pub(crate) fn into_private_key(self) -> PKey<Private> {
        self.private_key
    }

    /// Generate EC key pair.
    pub fn generate(curve: EcCurve) -> Result<EcKeyPair, JoseError> {
        (|| -> anyhow::Result<EcKeyPair> {
            let ec_group = EcGroup::from_curve_name(curve.nid())?;
            let ec_key = EcKey::generate(&ec_group)?;
            let private_key = PKey::from_ec_key(ec_key)?;

            Ok(EcKeyPair {
                curve,
                private_key,
                algorithm: None,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    /// Create a EC key pair from a private key that is a DER encoded PKCS#8 PrivateKeyInfo or ECPrivateKey.
    ///
    /// # Arguments
    ///
    /// * `input` - A private key that is a DER encoded PKCS#8 PrivateKeyInfo or ECPrivateKey.
    /// * `curve` - EC curve
    pub fn from_der(input: impl AsRef<[u8]>, curve: Option<EcCurve>) -> Result<Self, JoseError> {
        (|| -> anyhow::Result<Self> {
            let input = input.as_ref();
            let pkcs8_der_vec;
            let (pkcs8_der, curve) = match Self::detect_pkcs8(input, false) {
                Some(val) => match curve {
                    Some(val2) if val2 == val => (input, val),
                    Some(val2) => bail!("The curve is mismatched: {}", val2),
                    None => (input, val),
                },
                None => match curve {
                    Some(val) => {
                        pkcs8_der_vec = Self::to_pkcs8(input.as_ref(), false, val);
                        (pkcs8_der_vec.as_slice(), val)
                    }
                    None => bail!("A curve is required for raw format."),
                },
            };

            let private_key = PKey::private_key_from_der(pkcs8_der)?;

            Ok(EcKeyPair {
                private_key,
                curve,
                algorithm: None,
                key_id: None,
            })
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidKeyFormat(err),
        })
    }

    /// Return a signer from a private key that is formatted by a JWK of EC type.
    ///
    /// # Arguments
    ///
    /// * `jwk` - A private key that is formatted by a JWK of EC type.
    pub fn from_jwk(jwk: &Jwk) -> Result<Self, JoseError> {
        (|| -> anyhow::Result<Self> {
            match jwk.key_type() {
                val if val == "EC" => {}
                val => bail!("A parameter kty must be EC: {}", val),
            }
            let curve = match jwk.parameter("crv") {
                Some(Value::String(val)) => match val.as_str() {
                    "P-256" => EcCurve::P256,
                    "P-384" => EcCurve::P384,
                    "P-521" => EcCurve::P521,
                    "secp256k1" => EcCurve::Secp256k1,
                    _ => bail!("A Unknown curve: {}", val),
                },
                Some(_) => bail!("A parameter crv must be a string."),
                None => bail!("A parameter crv is required."),
            };
            let d = match jwk.parameter("d") {
                Some(Value::String(val)) => base64::decode_config(val, base64::URL_SAFE_NO_PAD)?,
                Some(_) => bail!("A parameter d must be a string."),
                None => bail!("A parameter d is required."),
            };
            let x = match jwk.parameter("x") {
                Some(Value::String(val)) => {
                    let x = base64::decode_config(val, base64::URL_SAFE_NO_PAD)?;
                    Some(x)
                }
                Some(_) => bail!("A parameter x must be a string."),
                None => None,
            };
            let y = match jwk.parameter("y") {
                Some(Value::String(val)) => {
                    let y = base64::decode_config(val, base64::URL_SAFE_NO_PAD)?;
                    Some(y)
                }
                Some(_) => bail!("A parameter y must be a string."),
                None => None,
            };

            let public_key = if let (Some(x), Some(y)) = (x, y) {
                let mut public_key = Vec::with_capacity(1 + x.len() + y.len());
                public_key.push(0x04);
                public_key.extend_from_slice(&x);
                public_key.extend_from_slice(&y);
                Some(public_key)
            } else {
                None
            };

            let mut builder = DerBuilder::new();
            builder.begin(DerType::Sequence);
            {
                builder.append_integer_from_u8(1);
                builder.append_octed_string_from_bytes(&d);
                builder.begin(DerType::Other(DerClass::ContextSpecific, 0));
                {
                    builder.append_object_identifier(curve.oid());
                }
                builder.end();

                if let Some(public_key) = public_key {
                    builder.begin(DerType::Other(DerClass::ContextSpecific, 1));
                    {
                        builder.append_bit_string_from_bytes(&public_key, 0);
                    }
                    builder.end();
                }
            }
            builder.end();

            let pkcs8 = EcKeyPair::to_pkcs8(&builder.build(), false, curve);
            let private_key = PKey::private_key_from_der(&pkcs8)?;
            let algorithm = jwk.algorithm().map(|val| val.to_string());
            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(EcKeyPair {
                private_key,
                curve,
                algorithm,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    /// Create a Ec key pair from a private key of common or traditinal PEM format.
    ///
    /// Common PEM format is a DER and base64 encoded PKCS#8 PrivateKeyInfo
    /// that surrounded by "-----BEGIN/END PRIVATE KEY----".
    ///
    /// Traditional PEM format is a DER and base64 encoded ECPrivateKey
    /// that surrounded by "-----BEGIN/END EC PRIVATE KEY----".
    ///
    /// # Arguments
    ///
    /// * `input` - A private key of common or traditinal PEM format.
    /// * `curve` - EC curve
    pub fn from_pem(input: impl AsRef<[u8]>, curve: Option<EcCurve>) -> Result<Self, JoseError> {
        (|| -> anyhow::Result<Self> {
            let (alg, data) = util::parse_pem(input.as_ref())?;

            let pkcs8_der_vec;
            let (pkcs8_der, curve) = match alg.as_str() {
                "PRIVATE KEY" => {
                    let curve = match Self::detect_pkcs8(&data, false) {
                        Some(val) => match curve {
                            Some(val2) if val2 == val => val2,
                            Some(val2) => bail!("The curve is mismatched: {}", val2),
                            None => val,
                        },
                        None => bail!("PEM contents is expected PKCS#8 wrapped key."),
                    };
                    (data.as_slice(), curve)
                }
                "EC PRIVATE KEY" => {
                    let curve = match Self::detect_ec_curve(data.as_slice()) {
                        Some(val) => match curve {
                            Some(val2) if val2 == val => val,
                            Some(val2) => bail!("The curve is mismatched: {}", val2),
                            None => val,
                        },
                        None => match curve {
                            Some(val) => val,
                            None => bail!("A curve name cannot be determined."),
                        },
                    };
                    pkcs8_der_vec = Self::to_pkcs8(&data, false, curve);
                    (pkcs8_der_vec.as_slice(), curve)
                }
                alg => bail!("Inappropriate algorithm: {}", alg),
            };

            let private_key = PKey::private_key_from_der(pkcs8_der)?;

            Ok(EcKeyPair {
                private_key,
                curve,
                algorithm: None,
                key_id: None,
            })
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidKeyFormat(err),
        })
    }

    pub fn to_raw_private_key(&self) -> Vec<u8> {
        let ec_key = self.private_key.ec_key().unwrap();
        ec_key.private_key_to_der().unwrap()
    }

    pub fn to_traditional_pem_private_key(&self) -> Vec<u8> {
        let ec_key = self.private_key.ec_key().unwrap();
        ec_key.private_key_to_pem().unwrap()
    }

    fn to_jwk(&self, private: bool, public: bool) -> Jwk {
        let ec_key = self.private_key.ec_key().unwrap();

        let mut jwk = Jwk::new("EC");
        if let Some(val) = &self.algorithm {
            jwk.set_algorithm(val);
        }
        if let Some(val) = &self.key_id {
            jwk.set_key_id(val);
        }
        jwk.set_parameter("crv", Some(Value::String(self.curve.to_string())))
            .unwrap();
        if private {
            let d = ec_key.private_key();
            let d = util::num_to_vec(&d, self.curve.coordinate_size());
            let d = base64::encode_config(&d, base64::URL_SAFE_NO_PAD);

            jwk.set_parameter("d", Some(Value::String(d))).unwrap();
        }
        if public {
            let public_key = ec_key.public_key();
            let mut x = BigNum::new().unwrap();
            let mut y = BigNum::new().unwrap();
            let mut ctx = BigNumContext::new().unwrap();
            public_key
                .affine_coordinates_gfp(ec_key.group(), &mut x, &mut y, &mut ctx)
                .unwrap();

            let x = util::num_to_vec(&x, self.curve.coordinate_size());
            let x = base64::encode_config(&x, base64::URL_SAFE_NO_PAD);

            let y = util::num_to_vec(&y, self.curve.coordinate_size());
            let y = base64::encode_config(&y, base64::URL_SAFE_NO_PAD);

            jwk.set_parameter("x", Some(Value::String(x))).unwrap();
            jwk.set_parameter("y", Some(Value::String(y))).unwrap();
        }
        jwk
    }

    pub(crate) fn detect_pkcs8(input: impl AsRef<[u8]>, is_public: bool) -> Option<EcCurve> {
        let curve;
        let mut reader = DerReader::from_reader(input.as_ref());

        match reader.next() {
            Ok(Some(DerType::Sequence)) => {}
            _ => return None,
        }

        {
            if !is_public {
                // Version
                match reader.next() {
                    Ok(Some(DerType::Integer)) => match reader.to_u8() {
                        Ok(val) if val == 0 => {}
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
                            if val != *OID_ID_EC_PUBLIC_KEY {
                                return None;
                            }
                        }
                        _ => return None,
                    },
                    _ => return None,
                }

                curve = match reader.next() {
                    Ok(Some(DerType::ObjectIdentifier)) => match reader.to_object_identifier() {
                        Ok(val) if val == *OID_PRIME256V1 => EcCurve::P256,
                        Ok(val) if val == *OID_SECP384R1 => EcCurve::P384,
                        Ok(val) if val == *OID_SECP521R1 => EcCurve::P521,
                        Ok(val) if val == *OID_SECP256K1 => EcCurve::Secp256k1,
                        _ => return None,
                    },
                    _ => return None,
                }
            }
        }

        Some(curve)
    }

    pub(crate) fn detect_ec_curve(input: impl AsRef<[u8]>) -> Option<EcCurve> {
        let curve;
        let mut reader = DerReader::from_reader(input.as_ref());

        match reader.next() {
            Ok(Some(DerType::Sequence)) => {}
            _ => return None,
        }

        {
            // Version
            match reader.next() {
                Ok(Some(DerType::Integer)) => match reader.to_u8() {
                    Ok(val) if val == 1 => {}
                    _ => return None,
                },
                _ => return None,
            }

            // Private Key
            match reader.next() {
                Ok(Some(DerType::OctetString)) => {}
                _ => return None,
            }

            // ECParameters
            match reader.next() {
                Ok(Some(DerType::Other(DerClass::ContextSpecific, 0))) => {}
                _ => return None,
            }

            {
                // NamedCurve
                curve = match reader.next() {
                    Ok(Some(DerType::ObjectIdentifier)) => match reader.to_object_identifier() {
                        Ok(val) if val == *OID_PRIME256V1 => EcCurve::P256,
                        Ok(val) if val == *OID_SECP384R1 => EcCurve::P384,
                        Ok(val) if val == *OID_SECP521R1 => EcCurve::P521,
                        Ok(val) if val == *OID_SECP256K1 => EcCurve::Secp256k1,
                        _ => return None,
                    },
                    _ => return None,
                }
            }
        }

        Some(curve)
    }

    pub(crate) fn to_pkcs8(input: &[u8], is_public: bool, curve: EcCurve) -> Vec<u8> {
        let mut builder = DerBuilder::new();
        builder.begin(DerType::Sequence);
        {
            if !is_public {
                builder.append_integer_from_u8(0);
            }

            builder.begin(DerType::Sequence);
            {
                builder.append_object_identifier(&OID_ID_EC_PUBLIC_KEY);
                builder.append_object_identifier(curve.oid());
            }
            builder.end();

            if is_public {
                builder.append_bit_string_from_bytes(input, 0);
            } else {
                builder.append_octed_string_from_bytes(input);
            }
        }
        builder.end();

        builder.build()
    }
}

impl KeyPair for EcKeyPair {
    fn algorithm(&self) -> Option<&str> {
        match &self.algorithm {
            Some(val) => Some(val.as_str()),
            None => None,
        }
    }

    fn key_id(&self) -> Option<&str> {
        match &self.key_id {
            Some(val) => Some(val.as_str()),
            None => None,
        }
    }

    fn to_der_private_key(&self) -> Vec<u8> {
        self.private_key.private_key_to_der().unwrap()
    }

    fn to_der_public_key(&self) -> Vec<u8> {
        self.private_key.public_key_to_der().unwrap()
    }

    fn to_pem_private_key(&self) -> Vec<u8> {
        self.private_key.private_key_to_pem_pkcs8().unwrap()
    }

    fn to_pem_public_key(&self) -> Vec<u8> {
        self.private_key.public_key_to_pem().unwrap()
    }

    fn to_jwk_private_key(&self) -> Jwk {
        self.to_jwk(true, false)
    }

    fn to_jwk_public_key(&self) -> Jwk {
        self.to_jwk(false, true)
    }

    fn to_jwk_key_pair(&self) -> Jwk {
        self.to_jwk(true, true)
    }

    fn box_clone(&self) -> Box<dyn KeyPair> {
        Box::new(self.clone())
    }
}

impl Deref for EcKeyPair {
    type Target = dyn KeyPair;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{EcCurve, EcKeyPair};

    #[test]
    fn test_ec_jwt() -> Result<()> {
        for curve in vec![
            EcCurve::P256,
            EcCurve::P384,
            EcCurve::P521,
            EcCurve::Secp256k1,
        ] {
            let key_pair_1 = EcKeyPair::generate(curve)?;
            let der_private1 = key_pair_1.to_der_private_key();
            let der_public1 = key_pair_1.to_der_public_key();

            let jwk_key_pair_1 = key_pair_1.to_jwk_key_pair();

            let key_pair_2 = EcKeyPair::from_jwk(&jwk_key_pair_1)?;
            let der_private2 = key_pair_2.to_der_private_key();
            let der_public2 = key_pair_2.to_der_public_key();

            assert_eq!(der_private1, der_private2);
            assert_eq!(der_public1, der_public2);
        }

        Ok(())
    }
}
