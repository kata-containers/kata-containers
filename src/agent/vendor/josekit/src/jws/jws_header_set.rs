use std::fmt::{Debug, Display};
use std::ops::Deref;

use crate::jwk::Jwk;
use crate::jws::JwsHeader;
use crate::{JoseError, JoseHeader, Map, Value};

/// Represent JWS protected and unprotected header claims
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct JwsHeaderSet {
    protected: Map<String, Value>,
    unprotected: Map<String, Value>,
}

impl JwsHeaderSet {
    /// Return a JwsHeader instance.
    pub fn new() -> Self {
        Self {
            protected: Map::new(),
            unprotected: Map::new(),
        }
    }

    /// Set a value for algorithm header claim (alg).
    ///
    /// # Arguments
    ///
    /// * `value` - a algorithm
    /// * `protection` - If it dosen't need protection, set false.
    pub fn set_algorithm(&mut self, value: impl Into<String>, protection: bool) {
        let key = "alg";
        let value: String = value.into();
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::String(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::String(value));
        }
    }

    /// Return the value for algorithm header claim (alg).
    pub fn algorithm(&self) -> Option<&str> {
        match self.claim("alg") {
            Some(Value::String(val)) => Some(&val),
            _ => None,
        }
    }

    /// Set a value for JWK set URL header claim (jku).
    ///
    /// # Arguments
    ///
    /// * `value` - a JWK set URL
    pub fn set_jwk_set_url(&mut self, value: impl Into<String>, protection: bool) {
        let key = "jku";
        let value: String = value.into();
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::String(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::String(value));
        }
    }

    /// Return the value for JWK set URL header claim (jku).
    pub fn jwk_set_url(&self) -> Option<&str> {
        match self.claim("jku") {
            Some(Value::String(val)) => Some(val),
            _ => None,
        }
    }

    /// Set a value for JWK header claim (jwk).
    ///
    /// # Arguments
    ///
    /// * `value` - a JWK
    pub fn set_jwk(&mut self, value: Jwk, protection: bool) {
        let key = "jwk";
        let value: Map<String, Value> = value.into();
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::Object(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::Object(value));
        }
    }

    /// Return the value for JWK header claim (jwk).
    pub fn jwk(&self) -> Option<Jwk> {
        match self.claim("jwk") {
            Some(Value::Object(vals)) => match Jwk::from_map(vals.clone()) {
                Ok(val) => Some(val),
                Err(_) => None,
            },
            _ => None,
        }
    }

    /// Set a value for X.509 URL header claim (x5u).
    ///
    /// # Arguments
    ///
    /// * `value` - a X.509 URL
    pub fn set_x509_url(&mut self, value: impl Into<String>, protection: bool) {
        let key = "x5u";
        let value: String = value.into();
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::String(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::String(value));
        }
    }

    /// Return a value for a X.509 URL header claim (x5u).
    pub fn x509_url(&self) -> Option<&str> {
        match self.claim("x5u") {
            Some(Value::String(val)) => Some(val),
            _ => None,
        }
    }

    /// Set values for X.509 certificate chain header claim (x5c).
    ///
    /// # Arguments
    ///
    /// * `values` - X.509 certificate chain
    pub fn set_x509_certificate_chain(&mut self, values: &Vec<impl AsRef<[u8]>>, protection: bool) {
        let key = "x5c";
        let vec = values
            .iter()
            .map(|v| Value::String(base64::encode_config(v.as_ref(), base64::URL_SAFE_NO_PAD)))
            .collect();
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::Array(vec));
        } else {
            self.protected.remove(key);
            self.unprotected.insert(key.to_string(), Value::Array(vec));
        }
    }

    /// Return values for a X.509 certificate chain header claim (x5c).
    pub fn x509_certificate_chain(&self) -> Option<Vec<Vec<u8>>> {
        match self.claim("x5c") {
            Some(Value::Array(vals)) => {
                let mut vec = Vec::with_capacity(vals.len());
                for val in vals {
                    match val {
                        Value::String(val2) => {
                            match base64::decode_config(val2, base64::URL_SAFE_NO_PAD) {
                                Ok(val3) => vec.push(val3.clone()),
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

    /// Set a value for X.509 certificate SHA-1 thumbprint header claim (x5t).
    ///
    /// # Arguments
    ///
    /// * `value` - A X.509 certificate SHA-1 thumbprint
    pub fn set_x509_certificate_sha1_thumbprint(
        &mut self,
        value: impl AsRef<[u8]>,
        protection: bool,
    ) {
        let key = "x5t";
        let value = base64::encode_config(&value, base64::URL_SAFE_NO_PAD);
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::String(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::String(value));
        }
    }

    /// Return the value for X.509 certificate SHA-1 thumbprint header claim (x5t).
    pub fn x509_certificate_sha1_thumbprint(&self) -> Option<Vec<u8>> {
        match self.claim("x5t") {
            Some(Value::String(val)) => match base64::decode_config(val, base64::URL_SAFE_NO_PAD) {
                Ok(val2) => Some(val2),
                Err(_) => None,
            },
            _ => None,
        }
    }

    /// Set a value for a x509 certificate SHA-256 thumbprint header claim (x5t#S256).
    ///
    /// # Arguments
    ///
    /// * `value` - A x509 certificate SHA-256 thumbprint
    pub fn set_x509_certificate_sha256_thumbprint(
        &mut self,
        value: impl AsRef<[u8]>,
        protection: bool,
    ) {
        let key = "x5t#S256";
        let value = base64::encode_config(&value, base64::URL_SAFE_NO_PAD);
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::String(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::String(value));
        }
    }

    /// Return the value for X.509 certificate SHA-256 thumbprint header claim (x5t#S256).
    pub fn x509_certificate_sha256_thumbprint(&self) -> Option<Vec<u8>> {
        match self.claim("x5t#S256") {
            Some(Value::String(val)) => match base64::decode_config(val, base64::URL_SAFE_NO_PAD) {
                Ok(val2) => Some(val2),
                Err(_) => None,
            },
            _ => None,
        }
    }

    /// Set a value for key ID header claim (kid).
    ///
    /// # Arguments
    ///
    /// * `value` - a key ID
    pub fn set_key_id(&mut self, value: impl Into<String>, protection: bool) {
        let key = "kid";
        let value: String = value.into();
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::String(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::String(value));
        }
    }

    /// Return the value for key ID header claim (kid).
    pub fn key_id(&self) -> Option<&str> {
        match self.claim("kid") {
            Some(Value::String(val)) => Some(val),
            _ => None,
        }
    }

    /// Set a value for token type header claim (typ).
    ///
    /// # Arguments
    ///
    /// * `value` - a token type (e.g. "JWT")
    pub fn set_token_type(&mut self, value: impl Into<String>, protection: bool) {
        let key = "typ";
        let value: String = value.into();
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::String(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::String(value));
        }
    }

    /// Return the value for token type header claim (typ).
    pub fn token_type(&self) -> Option<&str> {
        match self.claim("typ") {
            Some(Value::String(val)) => Some(val),
            _ => None,
        }
    }

    /// Set a value for content type header claim (cty).
    ///
    /// # Arguments
    ///
    /// * `value` - a content type (e.g. "JWT")
    pub fn set_content_type(&mut self, value: impl Into<String>, protection: bool) {
        let key = "cty";
        let value: String = value.into();
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::String(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::String(value));
        }
    }

    /// Return the value for content type header claim (cty).
    pub fn content_type(&self) -> Option<&str> {
        match self.claim("cty") {
            Some(Value::String(val)) => Some(val),
            _ => None,
        }
    }

    /// Set values for critical header claim (crit).
    ///
    /// # Arguments
    ///
    /// * `values` - critical claim names
    pub fn set_critical(&mut self, values: &Vec<impl AsRef<str>>) {
        let key = "crit";
        let vec = values
            .iter()
            .map(|v| Value::String(base64::encode_config(v.as_ref(), base64::URL_SAFE_NO_PAD)))
            .collect();
        self.unprotected.remove(key);
        self.protected.insert(key.to_string(), Value::Array(vec));
    }

    /// Return values for critical header claim (crit).
    pub fn critical(&self) -> Option<Vec<&str>> {
        match self.claim("crit") {
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

    /// Set a value for base64url-encode payload header claim (b64).
    ///
    /// # Arguments
    ///
    /// * `value` - is base64url-encode payload
    pub fn set_base64url_encode_payload(&mut self, value: bool) {
        let key = "b64";
        self.unprotected.remove(key);
        self.protected.insert(key.to_string(), Value::Bool(value));
    }

    /// Return the value for base64url-encode payload header claim (b64).
    pub fn base64url_encode_payload(&self) -> Option<bool> {
        match self.claim("b64") {
            Some(Value::Bool(val)) => Some(*val),
            _ => None,
        }
    }

    /// Set a value for url header claim (url).
    ///
    /// # Arguments
    ///
    /// * `value` - a url
    pub fn set_url(&mut self, value: impl Into<String>, protection: bool) {
        let key = "url";
        let value: String = value.into();
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::String(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::String(value));
        }
    }

    /// Return the value for url header claim (url).
    pub fn url(&self) -> Option<&str> {
        match self.claim("url") {
            Some(Value::String(val)) => Some(val),
            _ => None,
        }
    }

    /// Set a value for a nonce header claim (nonce).
    ///
    /// # Arguments
    ///
    /// * `value` - A nonce
    pub fn set_nonce(&mut self, value: impl AsRef<[u8]>, protection: bool) {
        let key = "nonce";
        let value = base64::encode_config(&value, base64::URL_SAFE_NO_PAD);
        if protection {
            self.unprotected.remove(key);
            self.protected.insert(key.to_string(), Value::String(value));
        } else {
            self.protected.remove(key);
            self.unprotected
                .insert(key.to_string(), Value::String(value));
        }
    }

    /// Return the value for nonce header claim (nonce).
    pub fn nonce(&self) -> Option<Vec<u8>> {
        match self.claim("nonce") {
            Some(Value::String(val)) => match base64::decode_config(val, base64::URL_SAFE_NO_PAD) {
                Ok(val2) => Some(val2),
                Err(_) => None,
            },
            _ => None,
        }
    }

    pub fn set_claim(
        &mut self,
        key: &str,
        value: Option<Value>,
        protection: bool,
    ) -> Result<(), JoseError> {
        match value {
            Some(val) => {
                JwsHeader::check_claim(key, &val)?;
                if protection {
                    self.unprotected.remove(key);
                    self.protected.insert(key.to_string(), val);
                } else {
                    self.protected.remove(key);
                    self.unprotected.insert(key.to_string(), val);
                }
            }
            None => {
                self.protected.remove(key);
                self.unprotected.remove(key);
            }
        }

        Ok(())
    }

    /// Return values for header claims set
    pub fn claims_set(&self, protection: bool) -> &Map<String, Value> {
        if protection {
            &self.protected
        } else {
            &self.unprotected
        }
    }

    pub fn to_map(&self) -> Map<String, Value> {
        let mut map = self.protected.clone();
        for (key, value) in &self.unprotected {
            map.insert(key.clone(), value.clone());
        }
        map
    }
}

impl JoseHeader for JwsHeaderSet {
    fn len(&self) -> usize {
        self.protected.len() + self.unprotected.len()
    }

    fn claim(&self, key: &str) -> Option<&Value> {
        if let Some(val) = self.protected.get(key) {
            Some(val)
        } else {
            self.unprotected.get(key)
        }
    }

    fn box_clone(&self) -> Box<dyn JoseHeader> {
        Box::new(self.clone())
    }
}

impl Display for JwsHeaderSet {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let protected = serde_json::to_string(&self.protected).map_err(|_e| std::fmt::Error {})?;
        let unprotected =
            serde_json::to_string(&self.unprotected).map_err(|_e| std::fmt::Error {})?;
        fmt.write_str("{\"protected\":")?;
        fmt.write_str(&protected)?;
        fmt.write_str(",\"unprotected\":")?;
        fmt.write_str(&unprotected)?;
        fmt.write_str("}")?;
        Ok(())
    }
}

impl Deref for JwsHeaderSet {
    type Target = dyn JoseHeader;

    fn deref(&self) -> &Self::Target {
        self
    }
}
