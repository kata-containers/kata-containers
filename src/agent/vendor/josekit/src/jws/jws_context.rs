use std::collections::BTreeSet;
use std::fmt::Debug;

use anyhow::bail;

use crate::jws::{JwsHeader, JwsHeaderSet, JwsSigner, JwsVerifier};
use crate::util;
use crate::{JoseError, Map, Value};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct JwsContext {
    acceptable_criticals: BTreeSet<String>,
}

impl JwsContext {
    pub fn new() -> Self {
        Self {
            acceptable_criticals: BTreeSet::new(),
        }
    }

    /// Test a critical header claim name is acceptable.
    ///
    /// # Arguments
    ///
    /// * `name` - a critical header claim name
    pub fn is_acceptable_critical(&self, name: &str) -> bool {
        self.acceptable_criticals.contains(name)
    }

    /// Add a acceptable critical header claim name
    ///
    /// # Arguments
    ///
    /// * `name` - a acceptable critical header claim name
    pub fn add_acceptable_critical(&mut self, name: &str) {
        self.acceptable_criticals.insert(name.to_string());
    }

    /// Remove a acceptable critical header claim name
    ///
    /// # Arguments
    ///
    /// * `name` - a acceptable critical header claim name
    pub fn remove_acceptable_critical(&mut self, name: &str) {
        self.acceptable_criticals.remove(name);
    }

    /// Return a representation of the data that is formatted by compact serialization.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWS heaser claims.
    /// * `signer` - The JWS signer.
    pub fn serialize_compact(
        &self,
        payload: &[u8],
        header: &JwsHeader,
        signer: &dyn JwsSigner,
    ) -> Result<String, JoseError> {
        self.serialize_compact_with_selector(payload, header, |_header| Some(signer))
    }

    /// Return a representation of the data that is formatted by compact serialization.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWS heaser claims.
    /// * `selector` - a function for selecting the signing algorithm.
    pub fn serialize_compact_with_selector<'a, F>(
        &self,
        payload: &[u8],
        header: &JwsHeader,
        selector: F,
    ) -> Result<String, JoseError>
    where
        F: Fn(&JwsHeader) -> Option<&'a dyn JwsSigner>,
    {
        (|| -> anyhow::Result<String> {
            let mut b64 = true;
            if let Some(vals) = header.critical() {
                if vals.contains(&"b64") {
                    if let Some(val) = header.base64url_encode_payload() {
                        b64 = val;
                    }
                }
            }

            let signer = match selector(header) {
                Some(val) => val,
                None => bail!("A signer is not found."),
            };

            let mut header = header.claims_set().clone();
            header.insert(
                "alg".to_string(),
                Value::String(signer.algorithm().name().to_string()),
            );
            if let Some(key_id) = signer.key_id() {
                header.insert("kid".to_string(), Value::String(key_id.to_string()));
            }
            let header_bytes = serde_json::to_vec(&header)?;

            let mut capacity = 2;
            capacity += util::ceiling(header_bytes.len() * 4, 3);
            capacity += if b64 {
                util::ceiling(payload.len() * 4, 3)
            } else {
                payload.len()
            };
            capacity += util::ceiling(signer.signature_len() * 4, 3);

            let mut message = String::with_capacity(capacity);
            base64::encode_config_buf(header_bytes, base64::URL_SAFE_NO_PAD, &mut message);
            message.push_str(".");
            if b64 {
                base64::encode_config_buf(payload, base64::URL_SAFE_NO_PAD, &mut message);
            } else {
                let payload = std::str::from_utf8(payload)?;
                if payload.contains(".") {
                    bail!("A JWS payload cannot contain dot.");
                }
                message.push_str(payload);
            }

            let signature = signer.sign(message.as_bytes())?;

            message.push_str(".");
            base64::encode_config_buf(signature, base64::URL_SAFE_NO_PAD, &mut message);

            Ok(message)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwsFormat(err),
        })
    }

    /// Return a representation of the data that is formatted by flattened json serialization.
    ///
    /// # Arguments
    ///
    /// * `protected` - The JWS protected header claims.
    /// * `header` - The JWS unprotected header claims.
    /// * `payload` - The payload data.
    /// * `signers` - The JWS signer.
    pub fn serialize_general_json(
        &self,
        payload: &[u8],
        signers: &[(&JwsHeaderSet, &dyn JwsSigner)],
    ) -> Result<String, JoseError> {
        self.serialize_general_json_with_selecter(
            payload,
            signers
                .iter()
                .map(|signer| signer.0)
                .collect::<Vec<&JwsHeaderSet>>()
                .as_slice(),
            |i, _header| Some(signers[i].1),
        )
    }

    /// Return a representation of the data that is formatted by flattened json serialization.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `headers` - The JWS headers.
    /// * `selector` - a function for selecting the signing algorithm.
    pub fn serialize_general_json_with_selecter<'a, F>(
        &self,
        payload: &[u8],
        headers: &[&JwsHeaderSet],
        selector: F,
    ) -> Result<String, JoseError>
    where
        F: Fn(usize, &JwsHeader) -> Option<&'a dyn JwsSigner>,
    {
        (|| -> anyhow::Result<String> {
            let payload_b64 = base64::encode_config(payload, base64::URL_SAFE_NO_PAD);

            let mut result = String::new();
            result.push_str("{\"signatures\":[");

            for (i, header) in headers.iter().enumerate() {
                let merged_map = header.to_map();
                let merged = JwsHeader::from_map(merged_map)?;
                let signer = match selector(i, &merged) {
                    Some(val) => val,
                    None => bail!("A signer is not found."),
                };

                let mut protected_map = header.claims_set(true).clone();

                match merged.algorithm() {
                    Some(val) if val == signer.algorithm().name() => {}
                    Some(_) => bail!("A signer is unmatched."),
                    None => {
                        protected_map.insert(
                            "alg".to_string(),
                            Value::String(signer.algorithm().name().to_string()),
                        );
                    }
                }

                if let None = merged.key_id() {
                    if let Some(key_id) = signer.key_id() {
                        protected_map.insert("kid".to_string(), Value::String(key_id.to_string()));
                    }
                }

                if i > 0 {
                    result.push_str(",");
                }

                let protected_bytes = serde_json::to_vec(&protected_map)?;
                let protected_b64 =
                    base64::encode_config(&protected_bytes, base64::URL_SAFE_NO_PAD);

                let unprotected_map = header.claims_set(false);

                let message = format!("{}.{}", &protected_b64, &payload_b64);
                let signature = signer.sign(message.as_bytes())?;

                result.push_str("{\"protected\":\"");
                result.push_str(&protected_b64);
                result.push_str("\"");

                if unprotected_map.len() > 0 {
                    let unprotected = serde_json::to_string(&unprotected_map)?;
                    result.push_str(",\"header\":");
                    result.push_str(&unprotected);
                }

                result.push_str(",\"signature\":\"");
                base64::encode_config_buf(&signature, base64::URL_SAFE_NO_PAD, &mut result);
                result.push_str("\"}");
            }

            result.push_str("],\"payload\":\"");
            result.push_str(&payload_b64);
            result.push_str("\"}");

            Ok(result)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwsFormat(err),
        })
    }

    /// Return a representation of the data that is formatted by flattened json serialization.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWS protected and unprotected header claims.
    /// * `signer` - The JWS signer.
    pub fn serialize_flattened_json(
        &self,
        payload: &[u8],
        header: &JwsHeaderSet,
        signer: &dyn JwsSigner,
    ) -> Result<String, JoseError> {
        self.serialize_flattened_json_with_selector(payload, header, |_header| Some(signer))
    }

    /// Return a representation of the data that is formatted by flatted json serialization.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWS protected and unprotected header claims.
    /// * `selector` - a function for selecting the signing algorithm.
    pub fn serialize_flattened_json_with_selector<'a, F>(
        &self,
        payload: &[u8],
        header: &JwsHeaderSet,
        selector: F,
    ) -> Result<String, JoseError>
    where
        F: Fn(&JwsHeader) -> Option<&'a dyn JwsSigner>,
    {
        (|| -> anyhow::Result<String> {
            let protected_map = header.claims_set(true);
            let mut b64 = true;
            match protected_map.get("crit") {
                Some(Value::Array(vals)) => {
                    if vals.iter().any(|val| match val {
                        Value::String(val2) => val2 == "b64",
                        _ => false,
                    }) {
                        b64 = match protected_map.get("b64") {
                            Some(Value::Bool(val3)) => *val3,
                            _ => false,
                        };
                    }
                }
                _ => {}
            }

            let merged_map = header.to_map();
            let merged = JwsHeader::from_map(merged_map)?;
            let signer = match selector(&merged) {
                Some(val) => val,
                None => bail!("A signer is not found."),
            };

            let mut protected_map = protected_map.clone();

            match merged.algorithm() {
                Some(val) if val == signer.algorithm().name() => {}
                Some(_) => bail!("A signer is unmatched."),
                None => {
                    protected_map.insert(
                        "alg".to_string(),
                        Value::String(signer.algorithm().name().to_string()),
                    );
                }
            }

            if let None = merged.key_id() {
                if let Some(key_id) = signer.key_id() {
                    protected_map.insert("kid".to_string(), Value::String(key_id.to_string()));
                }
            }

            let protected_json = serde_json::to_string(&protected_map)?;
            let protected_b64 = base64::encode_config(protected_json, base64::URL_SAFE_NO_PAD);

            let payload_b64;
            let payload = if b64 {
                payload_b64 = base64::encode_config(payload, base64::URL_SAFE_NO_PAD);
                &payload_b64
            } else {
                std::str::from_utf8(payload)?
            };

            let message = format!("{}.{}", &protected_b64, payload);
            let signature = signer.sign(message.as_bytes())?;

            let mut json = String::new();
            json.push_str("{\"protected\":\"");
            json.push_str(&protected_b64);
            json.push_str("\"");

            let unprotected = header.claims_set(false);
            if unprotected.len() > 0 {
                let unprotcted_json = serde_json::to_string(unprotected)?;
                json.push_str(",\"header\":");
                json.push_str(&unprotcted_json);
            }

            json.push_str(",\"payload\":\"");
            json.push_str(&payload);
            json.push_str("\"");

            json.push_str(",\"signature\":\"");
            base64::encode_config_buf(&signature, base64::URL_SAFE_NO_PAD, &mut json);
            json.push_str("\"}");

            Ok(json)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwsFormat(err),
        })
    }

    /// Deserialize the input that is formatted by compact serialization.
    ///
    /// # Arguments
    ///
    /// * `input` - The input data.
    /// * `header` - The decoded JWS header claims.
    /// * `verifier` - The JWS verifier.
    pub fn deserialize_compact(
        &self,
        input: impl AsRef<[u8]>,
        verifier: &dyn JwsVerifier,
    ) -> Result<(Vec<u8>, JwsHeader), JoseError> {
        self.deserialize_compact_with_selector(input, |_header| Ok(Some(verifier)))
    }

    /// Deserialize the input that is formatted by compact serialization.
    ///
    /// # Arguments
    ///
    /// * `input` - The input data.
    /// * `header` - The decoded JWS header claims.
    /// * `selector` - a function for selecting the verifying algorithm.
    pub fn deserialize_compact_with_selector<'a, F>(
        &self,
        input: impl AsRef<[u8]>,
        selector: F,
    ) -> Result<(Vec<u8>, JwsHeader), JoseError>
    where
        F: Fn(&JwsHeader) -> Result<Option<&'a dyn JwsVerifier>, JoseError>,
    {
        (|| -> anyhow::Result<(Vec<u8>, JwsHeader)> {
            let input = input.as_ref();
            let indexies: Vec<usize> = input
                .iter()
                .enumerate()
                .filter(|(_, b)| **b == b'.' as u8)
                .map(|(pos, _)| pos)
                .collect();
            if indexies.len() != 2 {
                bail!(
                    "The compact serialization form of JWS must be three parts separated by colon."
                );
            }

            let header = &input[0..indexies[0]];
            let payload = &input[(indexies[0] + 1)..(indexies[1])];
            let signature = &input[(indexies[1] + 1)..];

            let header = base64::decode_config(header, base64::URL_SAFE_NO_PAD)?;
            let header: Map<String, Value> = serde_json::from_slice(&header)?;
            let header = JwsHeader::from_map(header)?;

            let verifier = match selector(&header)? {
                Some(val) => val,
                None => bail!("A verifier is not found."),
            };

            match header.claim("alg") {
                Some(Value::String(val)) => {
                    let expected_alg = verifier.algorithm().name();
                    if val != expected_alg {
                        bail!("The JWS alg header claim is not {}: {}", expected_alg, val);
                    }
                }
                Some(_) => bail!("The JWS alg header claim must be a string."),
                None => bail!("The JWS alg header claim is required."),
            }

            match verifier.key_id() {
                Some(expected) => match header.key_id() {
                    Some(actual) if expected == actual => {}
                    Some(actual) => bail!("The JWS kid header claim is mismatched: {}", actual),
                    None => bail!("The JWS kid header claim is required."),
                },
                None => {}
            }

            let mut b64 = true;
            if let Some(Value::Array(vals)) = header.claim("crit") {
                for val in vals {
                    if let Value::String(val2) = val {
                        if !self.is_acceptable_critical(val2) {
                            bail!("The critical name '{}' is not supported.", val2);
                        }

                        if val2 == "b64" {
                            if let Some(val) = header.base64url_encode_payload() {
                                b64 = val;
                            }
                        }
                    }
                }
            }

            let message = &input[..(indexies[1])];
            let signature = base64::decode_config(signature, base64::URL_SAFE_NO_PAD)?;
            verifier.verify(message, &signature)?;

            let payload = if b64 {
                base64::decode_config(payload, base64::URL_SAFE_NO_PAD)?
            } else {
                payload.to_vec()
            };

            Ok((payload, header))
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwsFormat(err),
        })
    }

    /// Deserialize the input that is formatted by json serialization.
    ///
    /// # Arguments
    ///
    /// * `input` - The input data.
    /// * `header` - The decoded JWS header claims.
    /// * `verifier` - The JWS verifier.
    pub fn deserialize_json<'a>(
        &self,
        input: impl AsRef<[u8]>,
        verifier: &'a dyn JwsVerifier,
    ) -> Result<(Vec<u8>, JwsHeader), JoseError> {
        self.deserialize_json_with_selector(input, |header| {
            match header.algorithm() {
                Some(val) => {
                    let expected_alg = verifier.algorithm().name();
                    if val != expected_alg {
                        return Ok(None);
                    }
                }
                _ => return Ok(None),
            }

            match verifier.key_id() {
                Some(expected) => match header.key_id() {
                    Some(actual) if expected == actual => {}
                    _ => return Ok(None),
                },
                None => {}
            }

            Ok(Some(verifier))
        })
    }

    /// Deserialize the input that is formatted by json serialization.
    ///
    /// # Arguments
    ///
    /// * `input` - The input data.
    /// * `header` - The decoded JWS header claims.
    /// * `selector` - a function for selecting the verifying algorithm.
    pub fn deserialize_json_with_selector<'a, F>(
        &self,
        input: impl AsRef<[u8]>,
        selector: F,
    ) -> Result<(Vec<u8>, JwsHeader), JoseError>
    where
        F: Fn(&JwsHeader) -> Result<Option<&'a dyn JwsVerifier>, JoseError>,
    {
        (|| -> anyhow::Result<(Vec<u8>, JwsHeader)> {
            let input = input.as_ref();
            let mut map: Map<String, Value> = serde_json::from_slice(input)?;

            let payload_b64 = match map.remove("payload") {
                Some(Value::String(val)) => val,
                Some(_) => bail!("The payload field must be string."),
                None => bail!("The payload field is required."),
            };

            let signatures = match map.remove("signatures") {
                Some(Value::Array(vals)) => {
                    let mut vec = Vec::with_capacity(vals.len());
                    for val in vals {
                        if let Value::Object(val) = val {
                            vec.push(val);
                        } else {
                            bail!("The signatures field must be a array of object.");
                        }
                    }
                    vec
                }
                Some(_) => bail!("The signatures field must be a array."),
                None => {
                    let mut vec = Vec::with_capacity(1);
                    vec.push(map);
                    vec
                }
            };

            for mut sig in signatures {
                let header = sig.remove("header");

                let protected_b64 = match sig.get("protected") {
                    Some(Value::String(val)) => val,
                    Some(_) => bail!("The protected field must be a string."),
                    None => bail!("The JWS alg header claim must be in protected."),
                };

                let protected_vec = base64::decode_config(&protected_b64, base64::URL_SAFE_NO_PAD)?;
                let protected_map: Map<String, Value> = serde_json::from_slice(&protected_vec)?;

                let mut b64 = true;
                if let Some(Value::Array(vals)) = protected_map.get("critical") {
                    for val in vals {
                        match val {
                            Value::String(name) => {
                                if !self.is_acceptable_critical(name) {
                                    bail!("The critical name '{}' is not supported.", name);
                                }

                                if name == "b64" {
                                    match protected_map.get("b64") {
                                        Some(Value::Bool(b64_val)) => {
                                            b64 = *b64_val;
                                        }
                                        Some(_) => bail!("The JWS b64 header claim must be bool."),
                                        None => {}
                                    }
                                }
                            }
                            _ => bail!("The JWS critical header claim must be a array of string."),
                        }
                    }
                }

                let merged_map = match header {
                    Some(Value::Object(mut val)) => {
                        for (key, value) in protected_map {
                            if val.contains_key(&key) {
                                bail!("A duplicate key exists: {}", key);
                            } else {
                                val.insert(key.clone(), value.clone());
                            }
                        }
                        val
                    }
                    Some(_) => bail!("The protected field must be a object."),
                    None => protected_map.clone(),
                };

                if let None = merged_map.get("alg") {
                    bail!("The JWS alg header claim must be in protected.");
                }

                let signature = match sig.get("signature") {
                    Some(Value::String(val)) => {
                        base64::decode_config(val, base64::URL_SAFE_NO_PAD)?
                    }
                    Some(_) => bail!("The signature field must be string."),
                    None => bail!("The signature field is required."),
                };

                let merged = JwsHeader::from_map(merged_map)?;
                let verifier = match selector(&merged)? {
                    Some(val) => val,
                    None => continue,
                };

                match merged.claim("alg") {
                    Some(Value::String(val)) => {
                        let expected_alg = verifier.algorithm().name();
                        if val != expected_alg {
                            bail!("The JWS alg header claim is not {}: {}", expected_alg, val);
                        }
                    }
                    Some(_) => bail!("The JWS alg header claim must be a string."),
                    None => bail!("The JWS alg header claim is required."),
                }

                match verifier.key_id() {
                    Some(expected) => match merged.key_id() {
                        Some(actual) if expected == actual => {}
                        Some(actual) => bail!("The JWS kid header claim is mismatched: {}", actual),
                        None => bail!("The JWS kid header claim is required."),
                    },
                    None => {}
                }

                let message = format!("{}.{}", &protected_b64, &payload_b64);
                verifier.verify(message.as_bytes(), &signature)?;

                let payload = if b64 {
                    base64::decode_config(&payload_b64, base64::URL_SAFE_NO_PAD)?
                } else {
                    payload_b64.into_bytes()
                };

                return Ok((payload, merged));
            }

            bail!("A signature that matched the header claims is not found.");
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJwsFormat(err),
        })
    }
}
