use serde::{Deserialize, Serialize};

use std::fmt::Display;
use std::io::Read;
use std::string::ToString;

use anyhow::bail;

use crate::jwk::alg::ec::{EcCurve, EcKeyPair};
use crate::jwk::alg::ecx::{EcxCurve, EcxKeyPair};
use crate::jwk::alg::ed::{EdCurve, EdKeyPair};
use crate::jwk::alg::rsa::RsaKeyPair;
use crate::util;
use crate::{JoseError, Map, Value};

/// Represents JWK object.
#[derive(Debug, Eq, PartialEq, Clone, Deserialize, Serialize)]
pub struct Jwk {
    #[serde(flatten)]
    map: Map<String, Value>,
}

impl Jwk {
    pub fn new(key_type: &str) -> Self {
        Self {
            map: {
                let mut map = Map::new();
                map.insert("kty".to_string(), Value::String(key_type.to_string()));
                map
            },
        }
    }

    pub fn from_map(map: impl Into<Map<String, Value>>) -> Result<Self, JoseError> {
        let map: Map<String, Value> = map.into();
        Self::check_map(&map)?;

        Ok(Self { map })
    }

    pub fn from_reader(input: &mut dyn Read) -> Result<Self, JoseError> {
        (|| -> anyhow::Result<Self> {
            let map: Map<String, Value> = serde_json::from_reader(input)?;
            Ok(Self::from_map(map)?)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwkFormat(err),
        })
    }

    pub fn from_bytes(input: impl AsRef<[u8]>) -> Result<Self, JoseError> {
        (|| -> anyhow::Result<Self> {
            let map: Map<String, Value> = serde_json::from_slice(input.as_ref())?;
            Ok(Self::from_map(map)?)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwkFormat(err),
        })
    }

    /// Generate a new oct type JWK.
    ///
    /// # Arguments
    /// * `key_len` - A key byte length
    pub fn generate_oct_key(key_len: u8) -> Result<Self, JoseError> {
        let k = util::random_bytes(key_len as usize);

        let mut jwk = Self::new("oct");
        jwk.map.insert(
            "k".to_string(),
            Value::String(base64::encode_config(&k, base64::URL_SAFE_NO_PAD)),
        );
        Ok(jwk)
    }

    /// Generate a new RSA type JWK.
    ///
    /// # Arguments
    /// * `bits` - A key bits size
    pub fn generate_rsa_key(bits: u32) -> Result<Self, JoseError> {
        let key_pair = RsaKeyPair::generate(bits)?;
        Ok(key_pair.to_jwk_key_pair())
    }

    /// Generate a new EC type JWK.
    ///
    /// # Arguments
    /// * `curve` - A EC curve algorithm
    pub fn generate_ec_key(curve: EcCurve) -> Result<Self, JoseError> {
        let key_pair = EcKeyPair::generate(curve)?;
        Ok(key_pair.to_jwk_key_pair())
    }

    /// Generate a new Ed type JWK.
    ///
    /// # Arguments
    /// * `curve` - A Ed curve algorithm
    pub fn generate_ed_key(curve: EdCurve) -> Result<Self, JoseError> {
        let key_pair = EdKeyPair::generate(curve)?;
        Ok(key_pair.to_jwk_key_pair())
    }

    /// Generate a new Ecx type JWK.
    ///
    /// # Arguments
    /// * `curve` - A Ecx curve algorithm
    pub fn generate_ecx_key(curve: EcxCurve) -> Result<Self, JoseError> {
        let key_pair = EcxKeyPair::generate(curve)?;
        Ok(key_pair.to_jwk_key_pair())
    }

    /// Generate private key from private key.
    pub fn to_public_key(&self) -> Result<Self, JoseError> {
        (|| -> anyhow::Result<Jwk> {
            let jwk = match self.key_type() {
                "oct" => bail!("The key type 'oct' doesn't have public key."),
                "RSA" => {
                    let mut jwk = Jwk::new("RSA");
                    match self.map.get("use") {
                        Some(Value::String(val)) => {
                            jwk.map
                                .insert("use".to_string(), Value::String(val.clone()));
                        }
                        _ => {}
                    }
                    match self.map.get("e") {
                        Some(Value::String(val)) => {
                            jwk.map.insert("e".to_string(), Value::String(val.clone()));
                        }
                        Some(_) => bail!("The parameter 'x' must be a string."),
                        None => bail!("The key type 'RSA' must have parameter 'e'."),
                    }
                    match self.map.get("n") {
                        Some(Value::String(val)) => {
                            jwk.map.insert("n".to_string(), Value::String(val.clone()));
                        }
                        Some(_) => bail!("The parameter 'x' must be a string."),
                        None => bail!("The key type 'RSA' must have parameter 'n'."),
                    }
                    jwk
                }
                "EC" => {
                    let mut jwk = Jwk::new("EC");
                    match self.map.get("use") {
                        Some(Value::String(val)) => {
                            jwk.map
                                .insert("use".to_string(), Value::String(val.clone()));
                        }
                        _ => {}
                    }
                    match self.map.get("crv") {
                        Some(Value::String(val)) => match val.as_str() {
                            "P-256" | "P-384" | "P-521" | "secp256k1" => {
                                jwk.map
                                    .insert("crv".to_string(), Value::String(val.clone()));
                            }
                            val => bail!("Unknown curve: {}", val),
                        },
                        Some(_) => bail!("The parameter 'crv' must be a string."),
                        None => bail!("The key type 'EC' must have parameter 'crv'."),
                    }
                    match self.map.get("x") {
                        Some(Value::String(val)) => {
                            jwk.map.insert("x".to_string(), Value::String(val.clone()));
                        }
                        Some(_) => bail!("The parameter 'x' must be a string."),
                        None => bail!("The key type 'EC' must have parameter 'x'."),
                    }
                    match self.map.get("y") {
                        Some(Value::String(val)) => {
                            jwk.map.insert("y".to_string(), Value::String(val.clone()));
                        }
                        Some(_) => bail!("The parameter 'x' must be a string."),
                        None => bail!("The key type 'EC' must have parameter 'y'."),
                    }
                    jwk
                }
                "OKP" => {
                    let mut jwk = Jwk::new("OKP");
                    match self.map.get("use") {
                        Some(Value::String(val)) => {
                            jwk.map
                                .insert("use".to_string(), Value::String(val.clone()));
                        }
                        _ => {}
                    }
                    match self.map.get("crv") {
                        Some(Value::String(val)) => match val.as_str() {
                            "Ed25519" | "Ed448" | "X25519" | "X448" => {
                                jwk.map
                                    .insert("crv".to_string(), Value::String(val.clone()));
                            }
                            val => bail!("Unknown curve: {}", val),
                        },
                        Some(_) => bail!("The parameter 'crv' must be a string."),
                        None => bail!("The key type 'EC' must have parameter 'crv'."),
                    }
                    match self.map.get("x") {
                        Some(Value::String(val)) => {
                            jwk.map.insert("x".to_string(), Value::String(val.clone()));
                        }
                        Some(_) => bail!("The parameter 'x' must be a string."),
                        None => bail!("The key type 'OKP' must have parameter 'x'."),
                    }
                    jwk
                }
                val => bail!("Unknown key type: {}", val),
            };
            Ok(jwk)
        })()
        .map_err(|err| JoseError::InvalidJwkFormat(err))
    }

    /// Set a value for a key type parameter (kty).
    ///
    /// # Arguments
    /// * `value` - A key type
    pub fn set_key_type(&mut self, value: impl Into<String>) {
        let value: String = value.into();
        self.map.insert("kty".to_string(), Value::String(value));
    }

    /// Return a value for a key type parameter (kty).
    pub fn key_type(&self) -> &str {
        match self.map.get("kty") {
            Some(Value::String(val)) => val,
            _ => unreachable!("The JWS kty parameter is required."),
        }
    }

    /// Set a value for a key use parameter (use).
    ///
    /// # Arguments
    /// * `value` - A key use
    pub fn set_key_use(&mut self, value: impl Into<String>) {
        let value: String = value.into();
        self.map.insert("use".to_string(), Value::String(value));
    }

    /// Return a value for a key use parameter (use).
    pub fn key_use(&self) -> Option<&str> {
        match self.map.get("use") {
            Some(Value::String(val)) => Some(val),
            None => None,
            _ => unreachable!(),
        }
    }

    /// Set values for a key operations parameter (key_ops).
    ///
    /// # Arguments
    /// * `values` - key operations
    pub fn set_key_operations(&mut self, values: Vec<impl Into<String>>) {
        let mut vec = Vec::with_capacity(values.len());
        for val in values {
            let val: String = val.into();
            vec.push(Value::String(val.clone()));
        }
        self.map.insert("key_ops".to_string(), Value::Array(vec));
    }

    /// Return values for a key operations parameter (key_ops).
    pub fn key_operations(&self) -> Option<Vec<&str>> {
        match self.map.get("key_ops") {
            Some(Value::Array(vals)) => {
                let mut vec = Vec::with_capacity(vals.len());
                for val in vals {
                    match val {
                        Value::String(val2) => vec.push(val2.as_str()),
                        _ => return None,
                    }
                }
                Some(vec)
            }
            _ => None,
        }
    }

    pub fn is_for_key_operation(&self, key_operation: &str) -> bool {
        match self.map.get("key_ops") {
            Some(Value::Array(vals)) => vals.iter().any(|val| match val {
                Value::String(val2) if val2 == key_operation => true,
                _ => false,
            }),
            Some(_) => false,
            None => true,
        }
    }

    /// Set a value for a algorithm parameter (alg).
    ///
    /// # Arguments
    /// * `value` - A algorithm
    pub fn set_algorithm(&mut self, value: impl Into<String>) {
        let value: String = value.into();
        self.map.insert("alg".to_string(), Value::String(value));
    }

    /// Return a value for a algorithm parameter (alg).
    pub fn algorithm(&self) -> Option<&str> {
        match self.map.get("alg") {
            Some(Value::String(val)) => Some(val),
            None => None,
            _ => unreachable!(),
        }
    }

    /// Set a value for a key ID parameter (kid).
    ///
    /// # Arguments
    /// * `value` - A key ID
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        let value: String = value.into();
        self.map.insert("kid".to_string(), Value::String(value));
    }

    /// Return a value for a key ID parameter (kid).
    pub fn key_id(&self) -> Option<&str> {
        match self.map.get("kid") {
            Some(Value::String(val)) => Some(val),
            None => None,
            _ => unreachable!(),
        }
    }

    /// Set a value for a x509 url parameter (x5u).
    ///
    /// # Arguments
    /// * `value` - A x509 url
    pub fn set_x509_url(&mut self, value: impl Into<String>) {
        let value: String = value.into();
        self.map.insert("x5u".to_string(), Value::String(value));
    }

    /// Return a value for a x509 url parameter (x5u).
    pub fn x509_url(&self) -> Option<&str> {
        match self.map.get("x5u") {
            Some(Value::String(val)) => Some(val),
            None => None,
            _ => unreachable!(),
        }
    }

    /// Set a value for a x509 certificate SHA-1 thumbprint parameter (x5t).
    ///
    /// # Arguments
    /// * `value` - A x509 certificate SHA-1 thumbprint
    pub fn set_x509_certificate_sha1_thumbprint(&mut self, value: impl AsRef<[u8]>) {
        self.map.insert(
            "x5t".to_string(),
            Value::String(base64::encode_config(&value, base64::URL_SAFE_NO_PAD)),
        );
    }

    /// Return a value for a x509 certificate SHA-1 thumbprint parameter (x5t).
    pub fn x509_certificate_sha1_thumbprint(&self) -> Option<Vec<u8>> {
        match self.map.get("x5t") {
            Some(Value::String(val)) => match base64::decode_config(val, base64::URL_SAFE_NO_PAD) {
                Ok(val) => Some(val),
                Err(_) => None,
            },
            _ => None,
        }
    }

    /// Set a value for a x509 certificate SHA-256 thumbprint parameter (x5t#S256).
    ///
    /// # Arguments
    /// * `value` - A x509 certificate SHA-256 thumbprint
    pub fn set_x509_certificate_sha256_thumbprint(&mut self, value: impl AsRef<[u8]>) {
        self.map.insert(
            "x5t#S256".to_string(),
            Value::String(base64::encode_config(&value, base64::URL_SAFE_NO_PAD)),
        );
    }

    /// Return a value for a x509 certificate SHA-256 thumbprint parameter (x5t#S256).
    pub fn x509_certificate_sha256_thumbprint(&self) -> Option<Vec<u8>> {
        match self.map.get("x5t#S256") {
            Some(Value::String(val)) => match base64::decode_config(val, base64::URL_SAFE_NO_PAD) {
                Ok(val) => Some(val),
                Err(_) => None,
            },
            _ => None,
        }
    }

    /// Set values for a X.509 certificate chain parameter (x5c).
    ///
    /// # Arguments
    /// * `values` - X.509 certificate chain
    pub fn set_x509_certificate_chain(&mut self, values: &Vec<impl AsRef<[u8]>>) {
        let mut vec = Vec::with_capacity(values.len());
        for val in values {
            vec.push(Value::String(base64::encode_config(
                &val,
                base64::URL_SAFE_NO_PAD,
            )));
        }
        self.map.insert("x5c".to_string(), Value::Array(vec));
    }

    /// Return values for a X.509 certificate chain parameter (x5c).
    pub fn x509_certificate_chain(&self) -> Option<Vec<Vec<u8>>> {
        match self.map.get("x5c") {
            Some(Value::Array(vals)) => {
                let mut vec = Vec::with_capacity(vals.len());
                for val in vals {
                    match val {
                        Value::String(val2) => {
                            match base64::decode_config(val2, base64::URL_SAFE_NO_PAD) {
                                Ok(val3) => vec.push(val3),
                                Err(_) => return None,
                            }
                        }
                        _ => return None,
                    }
                }
                Some(vec)
            }
            _ => None,
        }
    }

    /// Set a value for a curve parameter (crv).
    ///
    /// # Arguments
    /// * `value` - A curve
    pub fn set_curve(&mut self, value: impl Into<String>) {
        let value: String = value.into();
        self.map.insert("crv".to_string(), Value::String(value));
    }

    /// Return a value for a curve parameter (crv).
    pub fn curve(&self) -> Option<&str> {
        match self.map.get("crv") {
            Some(Value::String(val)) => Some(val),
            None => None,
            _ => unreachable!(),
        }
    }

    /// Set a value for a key value parameter (k) of a oct type.
    ///
    /// # Arguments
    /// * `value` - A curve
    pub fn set_key_value(&mut self, value: impl AsRef<[u8]>) {
        self.map.insert(
            "k".to_string(),
            Value::String(base64::encode_config(&value, base64::URL_SAFE_NO_PAD)),
        );
    }

    /// Return a value for a key value parameter (k) of a oct type.
    pub fn key_value(&self) -> Option<Vec<u8>> {
        match self.map.get("k") {
            Some(Value::String(val)) => match base64::decode_config(val, base64::URL_SAFE_NO_PAD) {
                Ok(val) => Some(val),
                Err(_) => None,
            },
            _ => None,
        }
    }

    /// Set a value for a parameter of a specified key.
    ///
    /// # Arguments
    /// * `key` - A key name of a parameter
    /// * `value` - A typed value of a parameter
    pub fn set_parameter(&mut self, key: &str, value: Option<Value>) -> Result<(), JoseError> {
        match value {
            Some(val) => {
                Self::check_parameter(key, &val)?;
                self.map.insert(key.to_string(), val);
            }
            None => {
                (|| -> anyhow::Result<()> {
                    match key {
                        "kty" => bail!("The JWK {} parameter must be required.", key),
                        _ => {}
                    }
                    Ok(())
                })()
                .map_err(|err| JoseError::InvalidJwkFormat(err))?;

                self.map.remove(key);
            }
        }

        Ok(())
    }

    /// Return a value for a parameter of a specified key.
    ///
    /// # Arguments
    /// * `key` - A key name of a parameter
    pub fn parameter(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }

    pub(crate) fn check_map(map: &Map<String, Value>) -> Result<(), JoseError> {
        for (key, value) in map {
            Self::check_parameter(key, value)?;
        }

        (|| -> anyhow::Result<()> {
            if !map.contains_key("kty") {
                bail!("The JWK kty parameter is required.");
            }
            Ok(())
        })()
        .map_err(|err| JoseError::InvalidJwsFormat(err))
    }

    fn check_parameter(key: &str, value: &Value) -> Result<(), JoseError> {
        (|| -> anyhow::Result<()> {
            match key {
                "kty" | "use" | "alg" | "kid" | "x5u" | "crv" => match &value {
                    Value::String(_) => {}
                    _ => bail!("The JWK {} parameter must be a string.", key),
                },
                "key_ops" => match &value {
                    Value::Array(vals) => {
                        for val in vals {
                            match val {
                                Value::String(_) => {}
                                _ => bail!(
                                    "An element of the JWK {} parameter must be a string.",
                                    key
                                ),
                            }
                        }
                    }
                    _ => bail!("The JWK {} parameter must be a array of string.", key),
                },
                "x5t" | "x5t#S256" | "k" | "d" | "p" | "q" | "dp" | "dq" | "qi" | "x" | "y" => {
                    match &value {
                        Value::String(val) => {
                            if !util::is_base64_url_safe_nopad(val) {
                                bail!("The JWK {} parameter must be a base64 string.", key);
                            }
                        }
                        _ => bail!("The JWK {} parameter must be a string.", key),
                    }
                }
                "x5c" => match &value {
                    Value::Array(vals) => {
                        for val in vals {
                            match val {
                                Value::String(val) => {
                                    if !util::is_base64_url_safe_nopad(val) {
                                        bail!("The JWK {} parameter must be a base64 string.", key);
                                    }
                                }
                                _ => bail!(
                                    "An element of the JWK {} parameter must be a string.",
                                    key
                                ),
                            }
                        }
                    }
                    _ => bail!("The JWK {} parameter must be a array of string.", key),
                },
                _ => {}
            }

            Ok(())
        })()
        .map_err(|err| JoseError::InvalidJwkFormat(err))
    }
}

impl AsRef<Map<String, Value>> for Jwk {
    fn as_ref(&self) -> &Map<String, Value> {
        &self.map
    }
}

impl Into<Map<String, Value>> for Jwk {
    fn into(self) -> Map<String, Value> {
        self.map
    }
}

impl Display for Jwk {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let val = serde_json::to_string(&self.map).map_err(|_e| std::fmt::Error {})?;
        fmt.write_str(&val)
    }
}
