use std::convert::Into;
use std::time::SystemTime;

use anyhow::bail;

use crate::jwt::JwtPayload;
use crate::{JoseError, Map, Value};

/// Represents JWT payload validator.
#[derive(Debug, Eq, PartialEq)]
pub struct JwtPayloadValidator {
    base_time: Option<SystemTime>,
    min_issued_time: Option<SystemTime>,
    max_issued_time: Option<SystemTime>,
    audience: Option<String>,
    claims: Map<String, Value>,
}

impl JwtPayloadValidator {
    /// Return a new JwtPayloadValidator.
    pub fn new() -> Self {
        Self {
            base_time: None,
            min_issued_time: None,
            max_issued_time: None,
            audience: None,
            claims: Map::new(),
        }
    }

    /// Set a base time for time related claims (exp, nbf) validation.
    ///
    /// # Arguments
    ///
    /// * `base_time` - a min time
    pub fn set_base_time(&mut self, base_time: SystemTime) {
        self.base_time = Some(base_time);
    }

    /// Return the base time for time related claims (exp, nbf) validation.
    pub fn base_time(&self) -> Option<&SystemTime> {
        self.base_time.as_ref()
    }

    /// Set a minimum time for issued at payload claim (iat) validation.
    ///
    /// # Arguments
    ///
    /// * `min_issued_time` - a minimum time at which the JWT was issued.
    pub fn set_min_issued_time(&mut self, min_issued_time: SystemTime) {
        self.min_issued_time = Some(min_issued_time);
    }

    /// Return the minimum time for issued at payload claim (iat).
    pub fn min_issued_time(&self) -> Option<&SystemTime> {
        self.min_issued_time.as_ref()
    }

    /// Set a maximum time for issued at payload claim (iat) validation.
    ///
    /// # Arguments
    ///
    /// * `max_issued_time` - a maximum time at which the JWT was issued.
    pub fn set_max_issued_time(&mut self, max_issued_time: SystemTime) {
        self.max_issued_time = Some(max_issued_time);
    }

    /// Return the maximum time for issued at payload claim (iat).
    pub fn max_issued_time(&self) -> Option<&SystemTime> {
        self.max_issued_time.as_ref()
    }

    /// Set a value for issuer payload claim (iss) validation.
    ///
    /// # Arguments
    ///
    /// * `value` - a issuer
    pub fn set_issuer(&mut self, value: impl Into<String>) {
        let value: String = value.into();
        self.claims.insert("iss".to_string(), Value::String(value));
    }

    /// Return the value for issuer payload claim (iss) validation.
    pub fn issuer(&self) -> Option<&str> {
        match self.claims.get("iss") {
            Some(Value::String(val)) => Some(val),
            _ => None,
        }
    }

    /// Set a value for subject payload claim (sub) validation.
    ///
    /// # Arguments
    ///
    /// * `value` - a subject
    pub fn set_subject(&mut self, value: impl Into<String>) {
        let value: String = value.into();
        self.claims.insert("sub".to_string(), Value::String(value));
    }

    /// Return the value for subject payload claim (sub) validation.
    pub fn subject(&self) -> Option<&str> {
        match self.claims.get("sub") {
            Some(Value::String(val)) => Some(val),
            _ => None,
        }
    }

    /// Set a value for audience payload claim (aud) validation.
    ///
    /// # Arguments
    ///
    /// * `value` - a audience
    pub fn set_audience(&mut self, value: impl Into<String>) {
        let value: String = value.into();
        self.audience = Some(value);
    }

    /// Return the value for audience payload claim (aud) validation.
    pub fn audience(&self) -> Option<&str> {
        match self.audience {
            Some(ref val) => Some(val),
            _ => None,
        }
    }

    /// Set a value for JWT ID payload claim (jti) validation.
    ///
    /// # Arguments
    ///
    /// * `value` - A JWT ID
    pub fn set_jwt_id(&mut self, value: impl Into<String>) {
        let value: String = value.into();
        self.claims.insert("jti".to_string(), Value::String(value));
    }

    /// Return the value for JWT ID payload claim (jti) validation.
    pub fn jwt_id(&self) -> Option<&str> {
        match self.claims.get("jti") {
            Some(Value::String(val)) => Some(val),
            _ => None,
        }
    }

    /// Set a value for payload claim of a specified key.
    ///
    /// # Arguments
    ///
    /// * `key` - a key name of payload claim
    /// * `value` - a typed value of payload claim
    pub fn set_claim(&mut self, key: &str, value: Value) {
        self.claims.insert(key.to_string(), value);
    }

    /// Return the value for payload claim of a specified key.
    ///
    /// # Arguments
    ///
    /// * `key` - a key name of payload claim
    pub fn claim(&self, key: &str) -> Option<&Value> {
        self.claims.get(key)
    }

    /// Validate a decoded JWT payload.
    ///
    /// # Arguments
    ///
    /// * `payload` - a decoded JWT payload.
    pub fn validate(&self, payload: &JwtPayload) -> Result<(), JoseError> {
        (|| -> anyhow::Result<()> {
            let now = SystemTime::now();
            let current_time = self.base_time().unwrap_or(&now);
            let min_issued_time = self.min_issued_time().unwrap_or(&SystemTime::UNIX_EPOCH);
            let max_issued_time = self.max_issued_time().unwrap_or(&now);

            if let Some(not_before) = payload.not_before() {
                if &not_before > current_time {
                    bail!(
                        "The token is not yet valid: {}",
                        time::OffsetDateTime::from(not_before),
                    );
                }
            }

            if let Some(expires_at) = payload.expires_at() {
                if &expires_at <= current_time {
                    bail!(
                        "The token has expired: {}",
                        time::OffsetDateTime::from(expires_at),
                    );
                }
            }

            if let Some(issued_at) = payload.issued_at() {
                if &issued_at < min_issued_time {
                    bail!(
                        "The issued time is too old: {}",
                        time::OffsetDateTime::from(issued_at),
                    );
                }

                if &issued_at > max_issued_time {
                    bail!(
                        "The issued time is too new: {}",
                        time::OffsetDateTime::from(issued_at),
                    );
                }
            }

            if let Some(audience) = &self.audience {
                if let Some(audiences) = payload.audience() {
                    if !audiences.contains(&audience.as_str()) {
                        bail!("Key aud is invalid: {}", audiences.join(", "));
                    }
                }
            }

            for (key, value1) in &self.claims {
                if let Some(value2) = payload.claim(key) {
                    if value1 != value2 {
                        bail!("Key {} is invalid: {}", key, value2);
                    }
                } else {
                    bail!("Key {} is missing.", key);
                }
            }

            Ok(())
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidClaim(err),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use anyhow::Result;
    use serde_json::json;

    use crate::jwt::{JwtPayload, JwtPayloadValidator};

    #[test]
    fn test_jwt_payload_validate() -> Result<()> {
        let mut payload = JwtPayload::new();
        payload.set_issuer("iss");
        payload.set_subject("sub");
        payload.set_audience(vec!["aud0", "aud1"]);
        payload.set_expires_at(&(SystemTime::UNIX_EPOCH + Duration::from_secs(60)));
        payload.set_not_before(&(SystemTime::UNIX_EPOCH + Duration::from_secs(10)));
        payload.set_issued_at(&SystemTime::UNIX_EPOCH);
        payload.set_jwt_id("jti");
        payload.set_claim("payload_claim", Some(json!("payload_claim")))?;

        let mut validator = JwtPayloadValidator::new();
        validator.set_base_time(SystemTime::UNIX_EPOCH + Duration::from_secs(30));
        validator.set_issuer("iss");
        validator.set_audience("aud1");
        validator.set_claim("payload_claim", json!("payload_claim"));
        validator.validate(&payload)?;

        Ok(())
    }
}
