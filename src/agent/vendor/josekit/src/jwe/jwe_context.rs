use std::borrow::Cow;
use std::cmp::Eq;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use anyhow::bail;

use crate::jwe::enc::{A128CBC_HS256, A128GCM, A192CBC_HS384, A192GCM, A256CBC_HS512, A256GCM};
use crate::jwe::zip::Def;
use crate::jwe::{
    JweCompression, JweContentEncryption, JweDecrypter, JweEncrypter, JweHeader, JweHeaderSet,
};
use crate::util;
use crate::{JoseError, JoseHeader, Map, Value};

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct JweContext {
    acceptable_criticals: BTreeSet<String>,
    compressions: BTreeMap<String, Box<dyn JweCompression>>,
    content_encryptions: BTreeMap<String, Box<dyn JweContentEncryption>>,
}

impl JweContext {
    pub fn new() -> Self {
        Self {
            acceptable_criticals: BTreeSet::new(),
            compressions: {
                let compressions: Vec<Box<dyn JweCompression>> = vec![Box::new(Def)];

                let mut map = BTreeMap::new();
                for compression in compressions {
                    map.insert(compression.name().to_string(), compression);
                }
                map
            },
            content_encryptions: {
                let content_encryptions: Vec<Box<dyn JweContentEncryption>> = vec![
                    Box::new(A128CBC_HS256),
                    Box::new(A192CBC_HS384),
                    Box::new(A256CBC_HS512),
                    Box::new(A128GCM),
                    Box::new(A192GCM),
                    Box::new(A256GCM),
                ];

                let mut map = BTreeMap::new();
                for content_encryption in content_encryptions {
                    map.insert(content_encryption.name().to_string(), content_encryption);
                }
                map
            },
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

    /// Get a compression algorithm for zip header claim value.
    ///
    /// # Arguments
    ///
    /// * `name` - a zip header claim name
    pub fn get_compression(&self, name: &str) -> Option<&dyn JweCompression> {
        match self.compressions.get(name) {
            Some(val) => Some(val.as_ref()),
            None => None,
        }
    }

    /// Add a compression algorithm for zip header claim name.
    ///
    /// # Arguments
    ///
    /// * `compression` - a compression algorithm
    pub fn add_compression(&mut self, compression: Box<dyn JweCompression>) {
        self.compressions
            .insert(compression.name().to_string(), compression);
    }

    /// Remove a compression algorithm for zip header claim name.
    ///
    /// # Arguments
    ///
    /// * `name` - a zip header claim name
    pub fn remove_compression(&mut self, name: &str) {
        self.compressions.remove(name);
    }

    /// Get a content encryption algorithm for enc header claim value.
    ///
    /// # Arguments
    ///
    /// * `name` - a content encryption header claim name
    pub fn get_content_encryption(&self, name: &str) -> Option<&dyn JweContentEncryption> {
        match self.content_encryptions.get(name) {
            Some(val) => Some(val.as_ref()),
            None => None,
        }
    }

    /// Add a content encryption algorithm for enc header claim name.
    ///
    /// # Arguments
    ///
    /// * `content_encryption` - a content encryption algorithm
    pub fn add_content_encryption(&mut self, content_encryption: Box<dyn JweContentEncryption>) {
        self.content_encryptions
            .insert(content_encryption.name().to_string(), content_encryption);
    }

    /// Remove a content encryption algorithm for enc header claim name.
    ///
    /// # Arguments
    ///
    /// * `name` - a enc header claim name
    pub fn remove_content_encryption(&mut self, name: &str) {
        self.content_encryptions.remove(name);
    }

    /// Return a representation of the data that is formatted by compact serialization.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWS heaser claims.
    /// * `encrypter` - The JWS encrypter.
    pub fn serialize_compact(
        &self,
        payload: &[u8],
        header: &JweHeader,
        encrypter: &dyn JweEncrypter,
    ) -> Result<String, JoseError> {
        self.serialize_compact_with_selector(payload, header, |_header| Some(encrypter))
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
        header: &JweHeader,
        selector: F,
    ) -> Result<String, JoseError>
    where
        F: Fn(&JweHeader) -> Option<&'a dyn JweEncrypter>,
    {
        (|| -> anyhow::Result<String> {
            let encrypter = match selector(header) {
                Some(val) => val,
                None => bail!("A encrypter is not found."),
            };

            let cencryption = match header.content_encryption() {
                Some(enc) => match self.get_content_encryption(enc) {
                    Some(val) => val,
                    None => bail!("A content encryption is not registered: {}", enc),
                },
                None => bail!("A enc header claim is required."),
            };

            let compression = match header.compression() {
                Some(zip) => match self.get_compression(zip) {
                    Some(val) => Some(val),
                    None => bail!("A compression algorithm is not registered: {}", zip),
                },
                None => None,
            };

            let mut out_header = header.clone();

            let key_len = cencryption.key_len();
            let key = match encrypter.compute_content_encryption_key(
                cencryption,
                &header,
                &mut out_header,
            )? {
                Some(val) => val,
                None => Cow::Owned(util::random_bytes(key_len)),
            };

            let encrypted_key = encrypter.encrypt(&key, &header, &mut out_header)?;
            if let None = header.claim("kid") {
                if let Some(key_id) = encrypter.key_id() {
                    out_header.set_key_id(key_id);
                }
            }

            out_header.set_algorithm(encrypter.algorithm().name());

            let header_bytes = serde_json::to_vec(out_header.claims_set())?;
            let header_b64 = base64::encode_config(header_bytes, base64::URL_SAFE_NO_PAD);

            let compressed;
            let content = if let Some(compression) = compression {
                compressed = compression.compress(payload)?;
                &compressed
            } else {
                payload
            };

            let iv_vec;
            let iv = if cencryption.iv_len() > 0 {
                iv_vec = util::random_bytes(cencryption.iv_len());
                Some(iv_vec.as_slice())
            } else {
                None
            };

            let (ciphertext, tag) =
                cencryption.encrypt(&key, iv, content, header_b64.as_bytes())?;

            let mut capacity = 4;
            capacity += header_b64.len();
            if let Some(val) = &encrypted_key {
                capacity += util::ceiling(val.len() * 4, 3);
            }
            if let Some(val) = iv {
                capacity += util::ceiling(val.len() * 4, 3);
            }
            capacity += util::ceiling(ciphertext.len() * 4, 3);
            if let Some(val) = &tag {
                capacity += util::ceiling(val.len() * 4, 3);
            }

            let mut message = String::with_capacity(capacity);
            message.push_str(&header_b64);
            message.push_str(".");
            if let Some(val) = &encrypted_key {
                base64::encode_config_buf(val, base64::URL_SAFE_NO_PAD, &mut message);
            }
            message.push_str(".");
            if let Some(val) = iv {
                base64::encode_config_buf(val, base64::URL_SAFE_NO_PAD, &mut message);
            }
            message.push_str(".");
            base64::encode_config_buf(ciphertext, base64::URL_SAFE_NO_PAD, &mut message);
            message.push_str(".");
            if let Some(val) = &tag {
                base64::encode_config_buf(val, base64::URL_SAFE_NO_PAD, &mut message);
            }

            Ok(message)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJweFormat(err),
        })
    }

    /// Return a representation of the data that is formatted by general json serialization.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWE shared protected and unprotected header claims.
    /// * `recipients` - The JWE header claims and the JWE encrypter pair for recipients.
    /// * `aad` - The JWE additional authenticated data.
    pub fn serialize_general_json(
        &self,
        payload: &[u8],
        header: Option<&JweHeaderSet>,
        recipients: &[(Option<&JweHeader>, &dyn JweEncrypter)],
        aad: Option<&[u8]>,
    ) -> Result<String, JoseError> {
        self.serialize_general_json_with_selector(
            payload,
            header,
            recipients
                .iter()
                .map(|(header, _)| header.as_deref())
                .collect::<Vec<Option<&JweHeader>>>()
                .as_slice(),
            aad,
            |i, _header| Some(recipients[i].1),
        )
    }

    /// Return a representation of the data that is formatted by general json serialization.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWS shared protected and unprotected header claims.
    /// * `recipient_headers` - The JWE unprotected header claims for recipients.
    /// * `aad` - The JWE additional authenticated data.
    /// * `selector` - a function for selecting the encrypting algorithm.
    pub fn serialize_general_json_with_selector<'a, F>(
        &self,
        payload: &[u8],
        header: Option<&JweHeaderSet>,
        recipient_headers: &[Option<&JweHeader>],
        aad: Option<&[u8]>,
        selector: F,
    ) -> Result<String, JoseError>
    where
        F: Fn(usize, &JweHeader) -> Option<&'a dyn JweEncrypter>,
    {
        (|| -> anyhow::Result<String> {
            if recipient_headers.len() == 0 {
                bail!(
                    "A size of recipients must be 1 or more: {}",
                    recipient_headers.len()
                );
            }

            let mut compression = None;
            if let Some(header) = header {
                match header.claims_set(true).get("zip") {
                    Some(Value::String(val)) => match self.get_compression(val) {
                        Some(val) => {
                            compression = Some(val);
                        }
                        None => bail!("A compression algorithm is not registered: {}", val),
                    },
                    Some(_) => bail!("A zip header claim must be a string."),
                    None => {}
                }
            };

            let merged_map = match header {
                Some(val) => val.to_map(),
                None => Map::new(),
            };

            let mut merged_list = Vec::new();
            let mut recipient_header_list = Vec::new();
            let mut encrypter_list = Vec::new();

            let mut selected_cencryption: Option<&dyn JweContentEncryption> = None;
            let mut selected_key: Option<Cow<[u8]>> = None;
            for (i, recipient_header) in recipient_headers.iter().enumerate() {
                let mut merged_map = merged_map.clone();

                if let Some(val) = recipient_header {
                    for (key, value) in val.claims_set() {
                        if merged_map.contains_key(key) {
                            bail!("Duplicate key exists: {}", key);
                        }
                        merged_map.insert(key.clone(), value.clone());
                    }
                }

                let merged = JweHeader::from_map(merged_map)?;

                let cencryption = match merged.claim("enc") {
                    Some(Value::String(enc)) => {
                        if let Some(selected_cencryption) = selected_cencryption {
                            if enc.as_str() != selected_cencryption.name() {
                                bail!("A content encryption must be same for all recipients.");
                            } else {
                                selected_cencryption
                            }
                        } else {
                            match self.get_content_encryption(enc) {
                                Some(val) => {
                                    selected_cencryption = Some(val);
                                    val
                                }
                                None => bail!("A content encryption is not registered: {}", enc),
                            }
                        }
                    }
                    Some(_) => bail!("A enc header claim must be a string."),
                    None => bail!("A enc header claim is required."),
                };

                let encrypter = match selector(i, &merged) {
                    Some(val) => val,
                    None => bail!("A encrypter is not found."),
                };

                let mut recipient_header = match recipient_header {
                    Some(val) => (*val).clone(),
                    None => JweHeader::new(),
                };

                if let Some(key) = encrypter.compute_content_encryption_key(
                    cencryption,
                    &merged,
                    &mut recipient_header,
                )? {
                    if let Some(selected_key) = &selected_key {
                        if key.as_ref() != selected_key.as_ref() {
                            bail!("A content encryption key must be only one.");
                        }
                    } else {
                        selected_key = Some(key);
                    }
                };

                match merged.algorithm() {
                    Some(val) if val == encrypter.algorithm().name() => {}
                    Some(_) => bail!("A signer is unmatched."),
                    None => {
                        recipient_header.set_algorithm(encrypter.algorithm().name().to_string());
                    }
                }

                if let None = merged.key_id() {
                    if let Some(key_id) = encrypter.key_id() {
                        recipient_header.set_key_id(key_id.to_string());
                    }
                }

                merged_list.push(merged);
                recipient_header_list.push(recipient_header);
                encrypter_list.push(encrypter);
            }

            let cencryption = match selected_cencryption {
                Some(val) => val,
                None => bail!("A enc header claim is required."),
            };

            let key = match &selected_key {
                Some(val) => Cow::Borrowed(val.as_ref()),
                None => Cow::Owned(util::random_bytes(cencryption.key_len())),
            };

            let iv = if cencryption.iv_len() > 0 {
                Some(util::random_bytes(cencryption.iv_len()))
            } else {
                None
            };

            let protected_b64 = match header {
                Some(header) => {
                    let protected_map = header.claims_set(true);
                    if protected_map.len() > 0 {
                        let protected_json = serde_json::to_vec(header.claims_set(true))?;
                        let protected_b64 =
                            base64::encode_config(protected_json, base64::URL_SAFE_NO_PAD);
                        Some(protected_b64)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            let aad_b64 = match aad {
                Some(val) => Some(base64::encode_config(val, base64::URL_SAFE_NO_PAD)),
                None => None,
            };

            let mut full_aad = String::with_capacity({
                let mut full_aad_capacity = 1;
                if let Some(val) = &protected_b64 {
                    full_aad_capacity += val.len();
                }
                if let Some(val) = &aad_b64 {
                    full_aad_capacity += val.len();
                }
                full_aad_capacity
            });
            if let Some(val) = &protected_b64 {
                full_aad.push_str(&val);
            }
            if let Some(val) = &aad_b64 {
                full_aad.push_str(".");
                full_aad.push_str(&val);
            }

            let compressed;
            let content = if let Some(compression) = compression {
                compressed = compression.compress(payload)?;
                &compressed
            } else {
                payload
            };

            let (ciphertext, tag) =
                cencryption.encrypt(&key, iv.as_deref(), content, full_aad.as_bytes())?;

            let mut writed = false;
            let mut json = String::new();
            if let Some(val) = protected_b64 {
                json.push_str("{\"protected\":\"");
                json.push_str(&val);
                json.push_str("\"");
                writed = true;
            }

            if let Some(val) = header {
                let unprotected_map = val.claims_set(false);
                if unprotected_map.len() > 0 {
                    let unprotected = serde_json::to_string(unprotected_map)?;
                    json.push_str(if writed { "," } else { "{" });
                    json.push_str("\"unprotected\":");
                    json.push_str(&unprotected);
                    writed = true;
                }
            }

            json.push_str(if writed { "," } else { "{" });
            json.push_str("\"recipients\":[");
            for i in 0..recipient_headers.len() {
                if i > 0 {
                    json.push_str(",");
                }

                let merged = &merged_list[i];
                let mut header = &mut recipient_header_list[i];
                let encrypter = encrypter_list[i];

                let encrypted_key = encrypter.encrypt(&key, &merged, &mut header)?;

                if header.len() == 0 {
                   bail!("The per-recipient header must not be empty");
                }
                let header_json = serde_json::to_string(header.claims_set())?;
                json.push_str("{\"header\":");
                json.push_str(&header_json);

                if let Some(val) = encrypted_key {
                    json.push_str(",\"encrypted_key\":\"");
                    base64::encode_config_buf(&val, base64::URL_SAFE_NO_PAD, &mut json);
                    json.push_str("\"");
                }
                json.push_str("}");
            }
            json.push_str("]");

            if let Some(val) = aad_b64 {
                json.push_str(",\"aad\":\"");
                json.push_str(&val);
                json.push_str("\"");
            }

            json.push_str(",\"iv\":\"");
            if let Some(val) = iv {
                base64::encode_config_buf(&val, base64::URL_SAFE_NO_PAD, &mut json);
            }
            json.push_str("\"");

            json.push_str(",\"ciphertext\":\"");
            base64::encode_config_buf(&ciphertext, base64::URL_SAFE_NO_PAD, &mut json);
            json.push_str("\"");

            json.push_str(",\"tag\":\"");
            if let Some(val) = tag {
                base64::encode_config_buf(&val, base64::URL_SAFE_NO_PAD, &mut json);
            }
            json.push_str("\"");

            json.push_str("}");

            Ok(json)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJweFormat(err),
        })
    }

    /// Return a representation of the data that is formatted by flattened json serialization.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWE shared protected and unprotected header claims.
    /// * `recipient_header` - The JWE unprotected header claims per recipient.
    /// * `aad` - The JWE additional authenticated data.
    /// * `encrypter` - The JWS encrypter.
    pub fn serialize_flattened_json(
        &self,
        payload: &[u8],
        header: Option<&JweHeaderSet>,
        recipient_header: Option<&JweHeader>,
        aad: Option<&[u8]>,
        encrypter: &dyn JweEncrypter,
    ) -> Result<String, JoseError> {
        self.serialize_flattened_json_with_selector(
            payload,
            header,
            recipient_header,
            aad,
            |_header| Some(encrypter),
        )
    }

    /// Return a representation of the data that is formatted by flatted json serialization.
    ///
    /// # Arguments
    ///
    /// * `payload` - The payload data.
    /// * `header` - The JWE shared protected and unprotected header claims.
    /// * `recipient_header` - The JWE unprotected header claims per recipient.
    /// * `aad` - The JWE additional authenticated data.
    /// * `selector` - a function for selecting the encrypting algorithm.
    pub fn serialize_flattened_json_with_selector<'a, F>(
        &self,
        payload: &[u8],
        header: Option<&JweHeaderSet>,
        recipient_header: Option<&JweHeader>,
        aad: Option<&[u8]>,
        selector: F,
    ) -> Result<String, JoseError>
    where
        F: Fn(&JweHeader) -> Option<&'a dyn JweEncrypter>,
    {
        (|| -> anyhow::Result<String> {
            let mut compression = None;
            if let Some(header) = header {
                match header.claims_set(true).get("zip") {
                    Some(Value::String(val)) => match self.get_compression(val) {
                        Some(val) => {
                            compression = Some(val);
                        }
                        None => bail!("A compression algorithm is not registered: {}", val),
                    },
                    Some(_) => bail!("A zip header claim must be a string."),
                    None => {}
                }
            };

            let mut merged_map = match header {
                Some(val) => val.to_map(),
                None => Map::new(),
            };

            if let Some(val) = recipient_header {
                for (key, value) in val.claims_set() {
                    if merged_map.contains_key(key) {
                        bail!("Duplicate key exists: {}", key);
                    }
                    merged_map.insert(key.clone(), value.clone());
                }
            }

            let merged = JweHeader::from_map(merged_map)?;

            let cencryption = match merged.claim("enc") {
                Some(Value::String(enc)) => match self.get_content_encryption(enc) {
                    Some(val) => val,
                    None => bail!("A content encryption is not registered: {}", enc),
                },
                Some(_) => bail!("A enc header claim must be a string."),
                None => bail!("A enc header claim is required."),
            };

            let encrypter = match selector(&merged) {
                Some(val) => val,
                None => bail!("A encrypter is not found."),
            };

            let mut protected = match header {
                Some(val) => JweHeader::from_map(val.claims_set(true).clone())?,
                None => JweHeader::new(),
            };

            let key = match encrypter.compute_content_encryption_key(
                cencryption,
                &merged,
                &mut protected,
            )? {
                Some(val) => val,
                None => Cow::Owned(util::random_bytes(cencryption.key_len())),
            };

            let encrypted_key = encrypter.encrypt(&key, &merged, &mut protected)?;

            match merged.algorithm() {
                Some(val) if val == encrypter.algorithm().name() => {}
                Some(_) => bail!("A signer is unmatched."),
                None => {
                    protected.set_algorithm(encrypter.algorithm().name().to_string());
                }
            }

            if let None = merged.key_id() {
                if let Some(key_id) = encrypter.key_id() {
                    protected.set_key_id(key_id.to_string());
                }
            }

            let iv_vec;
            let iv = if cencryption.iv_len() > 0 {
                iv_vec = util::random_bytes(cencryption.iv_len());
                Some(iv_vec.as_slice())
            } else {
                None
            };

            let protected_b64 = if protected.len() > 0 {
                let protected_json = serde_json::to_vec(protected.claims_set())?;
                let protected_b64 = base64::encode_config(protected_json, base64::URL_SAFE_NO_PAD);
                Some(protected_b64)
            } else {
                None
            };

            let aad_b64 = match aad {
                Some(val) => Some(base64::encode_config(val, base64::URL_SAFE_NO_PAD)),
                None => None,
            };

            let mut full_aad = String::with_capacity({
                let mut full_aad_capacity = 1;
                if let Some(val) = &protected_b64 {
                    full_aad_capacity += val.len();
                }
                if let Some(val) = &aad_b64 {
                    full_aad_capacity += val.len();
                }
                full_aad_capacity
            });
            if let Some(val) = &protected_b64 {
                full_aad.push_str(&val);
            }
            if let Some(val) = &aad_b64 {
                full_aad.push_str(".");
                full_aad.push_str(&val);
            }

            let compressed;
            let content = if let Some(compression) = compression {
                compressed = compression.compress(payload)?;
                &compressed
            } else {
                payload
            };

            let (ciphertext, tag) = cencryption.encrypt(&key, iv, content, full_aad.as_bytes())?;

            let mut writed = false;
            let mut json = String::new();
            if let Some(val) = protected_b64 {
                json.push_str("{\"protected\":\"");
                json.push_str(&val);
                json.push_str("\"");
                writed = true;
            }

            if let Some(val) = header {
                let unprotected_map = val.claims_set(false);
                if unprotected_map.len() > 0 {
                    let unprotected = serde_json::to_string(unprotected_map)?;
                    json.push_str(if writed { "," } else { "{" });
                    json.push_str("\"unprotected\":");
                    json.push_str(&unprotected);
                    writed = true;
                }
            }

            if let Some(val) = recipient_header {
                let header_map = val.claims_set();
                if header_map.len() > 0 {
                    let header = serde_json::to_string(header_map)?;
                    json.push_str(if writed { "," } else { "{" });
                    json.push_str("\"header\":");
                    json.push_str(&header);
                }
            }

            if let Some(val) = encrypted_key {
                json.push_str(",\"encrypted_key\":\"");
                base64::encode_config_buf(&val, base64::URL_SAFE_NO_PAD, &mut json);
                json.push_str("\"");
            }

            if let Some(val) = aad_b64 {
                json.push_str(",\"aad\":\"");
                json.push_str(&val);
                json.push_str("\"");
            }

            json.push_str(",\"iv\":\"");
            if let Some(val) = iv {
                base64::encode_config_buf(&val, base64::URL_SAFE_NO_PAD, &mut json);
            }
            json.push_str("\"");

            json.push_str(",\"ciphertext\":\"");
            base64::encode_config_buf(&ciphertext, base64::URL_SAFE_NO_PAD, &mut json);
            json.push_str("\"");

            json.push_str(",\"tag\":\"");
            if let Some(val) = tag {
                base64::encode_config_buf(&val, base64::URL_SAFE_NO_PAD, &mut json);
            }
            json.push_str("\"}");

            Ok(json)
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJweFormat(err),
        })
    }

    /// Deserialize the input that is formatted by compact serialization.
    ///
    /// # Arguments
    ///
    /// * `input` - The input data.
    /// * `decrypter` - The JWS decrypter.
    pub fn deserialize_compact(
        &self,
        input: impl AsRef<[u8]>,
        decrypter: &dyn JweDecrypter,
    ) -> Result<(Vec<u8>, JweHeader), JoseError> {
        self.deserialize_compact_with_selector(input, |_header| Ok(Some(decrypter)))
    }

    /// Deserialize the input that is formatted by compact serialization.
    ///
    /// # Arguments
    ///
    /// * `input` - The input data.
    /// * `selector` - a function for selecting the decrypting algorithm.
    pub fn deserialize_compact_with_selector<'a, F>(
        &self,
        input: impl AsRef<[u8]>,
        selector: F,
    ) -> Result<(Vec<u8>, JweHeader), JoseError>
    where
        F: Fn(&JweHeader) -> Result<Option<&'a dyn JweDecrypter>, JoseError>,
    {
        (|| -> anyhow::Result<(Vec<u8>, JweHeader)> {
            let input = input.as_ref();
            let indexies: Vec<usize> = input
                .iter()
                .enumerate()
                .filter(|(_, b)| **b == b'.' as u8)
                .map(|(pos, _)| pos)
                .collect();
            if indexies.len() != 4 {
                bail!(
                    "The compact serialization form of JWE must be five parts separated by colon."
                );
            }

            let header_b64 = &input[0..indexies[0]];

            let encrypted_key_b64 = &input[(indexies[0] + 1)..(indexies[1])];
            let encrypted_key_vec;
            let encrypted_key = if encrypted_key_b64.len() > 0 {
                encrypted_key_vec =
                    base64::decode_config(encrypted_key_b64, base64::URL_SAFE_NO_PAD)?;
                Some(encrypted_key_vec.as_slice())
            } else {
                None
            };

            let iv_b64 = &input[(indexies[1] + 1)..(indexies[2])];
            let iv_vec;
            let iv = if iv_b64.len() > 0 {
                iv_vec = base64::decode_config(iv_b64, base64::URL_SAFE_NO_PAD)?;
                Some(iv_vec.as_slice())
            } else {
                None
            };

            let ciphertext_b64 = &input[(indexies[2] + 1)..(indexies[3])];
            let ciphertext = base64::decode_config(ciphertext_b64, base64::URL_SAFE_NO_PAD)?;

            let tag_b64 = &input[(indexies[3] + 1)..];
            let tag_vec;
            let tag = if tag_b64.len() > 0 {
                tag_vec = base64::decode_config(tag_b64, base64::URL_SAFE_NO_PAD)?;
                Some(tag_vec.as_slice())
            } else {
                None
            };

            let header = base64::decode_config(header_b64, base64::URL_SAFE_NO_PAD)?;
            let merged: Map<String, Value> = serde_json::from_slice(&header)?;
            let merged = JweHeader::from_map(merged)?;

            let decrypter = match selector(&merged)? {
                Some(val) => val,
                None => bail!("A decrypter is not found."),
            };

            let cencryption = match merged.claim("enc") {
                Some(Value::String(val)) => match self.get_content_encryption(val) {
                    Some(val2) => val2,
                    None => bail!("A content encryption is not registered: {}", val),
                },
                Some(_) => bail!("A enc header claim must be a string."),
                None => bail!("A enc header claim is required."),
            };

            let compression = match merged.claim("zip") {
                Some(Value::String(val)) => match self.get_compression(val) {
                    Some(val2) => Some(val2),
                    None => bail!("A compression algorithm is not registered: {}", val),
                },
                Some(_) => bail!("A enc header claim must be a string."),
                None => None,
            };

            match merged.claim("alg") {
                Some(Value::String(val)) => {
                    let expected_alg = decrypter.algorithm().name();
                    if val != expected_alg {
                        bail!("The JWE alg header claim is not {}: {}", expected_alg, val);
                    }
                }
                Some(_) => bail!("A alg header claim must be a string."),
                None => bail!("The JWE alg header claim is required."),
            }

            match decrypter.key_id() {
                Some(expected) => match merged.key_id() {
                    Some(actual) if expected == actual => {}
                    Some(actual) => bail!("The JWE kid header claim is mismatched: {}", actual),
                    None => bail!("The JWE kid header claim is required."),
                },
                None => {}
            }

            let key = decrypter.decrypt(encrypted_key, cencryption, &merged)?;
            if key.len() != cencryption.key_len() {
                bail!(
                    "The key size is expected to be {}: {}",
                    cencryption.key_len(),
                    key.len()
                );
            }

            let content = cencryption.decrypt(&key, iv, &ciphertext, header_b64, tag)?;
            let content = match compression {
                Some(val) => val.decompress(&content)?,
                None => content,
            };

            Ok((content, merged))
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJweFormat(err),
        })
    }

    /// Deserialize the input that is formatted by json serialization.
    ///
    /// # Arguments
    ///
    /// * `input` - The input data.
    /// * `decrypter` - The JWE decrypter.
    pub fn deserialize_json<'a>(
        &self,
        input: impl AsRef<[u8]>,
        decrypter: &'a dyn JweDecrypter,
    ) -> Result<(Vec<u8>, JweHeader), JoseError> {
        self.deserialize_json_with_selector(input, |header| {
            match header.algorithm() {
                Some(val) => {
                    let expected_alg = decrypter.algorithm().name();
                    if val != expected_alg {
                        return Ok(None);
                    }
                }
                _ => return Ok(None),
            }

            match decrypter.key_id() {
                Some(expected) => match header.key_id() {
                    Some(actual) if expected == actual => {}
                    _ => return Ok(None),
                },
                None => {}
            }

            Ok(Some(decrypter))
        })
    }

    /// Deserialize the input that is formatted by json serialization.
    ///
    /// # Arguments
    ///
    /// * `input` - The input data.
    /// * `selector` - a function for selecting the decrypting algorithm.
    pub fn deserialize_json_with_selector<'a, F>(
        &self,
        input: impl AsRef<[u8]>,
        selector: F,
    ) -> Result<(Vec<u8>, JweHeader), JoseError>
    where
        F: Fn(&JweHeader) -> Result<Option<&'a dyn JweDecrypter>, JoseError>,
    {
        (|| -> anyhow::Result<(Vec<u8>, JweHeader)> {
            let input = input.as_ref();
            let mut map: Map<String, Value> = serde_json::from_slice(input)?;

            let (protected, protected_b64) = match map.remove("protected") {
                Some(Value::String(val)) => {
                    if val.len() == 0 {
                        bail!("The protected field must be empty.");
                    }
                    let vec = base64::decode_config(&val, base64::URL_SAFE_NO_PAD)?;
                    let json: Map<String, Value> = serde_json::from_slice(&vec)?;
                    (Some(json), Some(val))
                }
                Some(_) => bail!("The protected field must be a string."),
                None => (None, None),
            };
            let unprotected = match map.remove("unprotected") {
                Some(Value::Object(val)) => {
                    if val.len() == 0 {
                        bail!("The unprotected field must be empty.");
                    }
                    Some(val)
                }
                Some(_) => bail!("The JWE unprotected field must be string."),
                None => None,
            };
            let aad_b64 = match map.remove("aad") {
                Some(Value::String(val)) => {
                    if val.len() == 0 {
                        bail!("The JWE aad field must be empty.");
                    } else if !util::is_base64_url_safe_nopad(&val) {
                        bail!("The JWE aad field must be a base64 string.");
                    }
                    Some(val)
                }
                Some(_) => bail!("The JWE aad field must be string."),
                None => None,
            };
            let iv_vec;
            let iv = match map.remove("iv") {
                Some(Value::String(val)) => {
                    if val.len() == 0 {
                        bail!("The iv field must be empty.");
                    }
                    iv_vec = base64::decode_config(&val, base64::URL_SAFE_NO_PAD)?;
                    Some(iv_vec.as_slice())
                }
                Some(_) => bail!("The iv field must be string."),
                None => None,
            };
            let ciphertext = match map.remove("ciphertext") {
                Some(Value::String(val)) => {
                    if val.len() == 0 {
                        bail!("The ciphertext field must be empty.");
                    }
                    base64::decode_config(&val, base64::URL_SAFE_NO_PAD)?
                }
                Some(_) => bail!("The ciphertext field must be string."),
                None => bail!("The ciphertext field is required."),
            };
            let tag_vec;
            let tag = match map.remove("tag") {
                Some(Value::String(val)) => {
                    if val.len() == 0 {
                        bail!("The tag field must be empty.");
                    }
                    tag_vec = base64::decode_config(&val, base64::URL_SAFE_NO_PAD)?;
                    Some(tag_vec.as_slice())
                }
                Some(_) => bail!("The tag field must be string."),
                None => None,
            };

            let recipients = match map.remove("recipients") {
                Some(Value::Array(vals)) => {
                    if vals.len() == 0 {
                        bail!("The recipients field must be empty.");
                    }
                    let mut vec = Vec::with_capacity(vals.len());
                    for val in vals {
                        if let Value::Object(val) = val {
                            vec.push(val);
                        } else {
                            bail!("The recipients field must be a array of object.");
                        }
                    }
                    vec
                }
                Some(_) => bail!("The recipients field must be a array."),
                None => {
                    let mut vec = Vec::with_capacity(1);
                    vec.push(map);
                    vec
                }
            };

            for mut recipient in recipients {
                let header = recipient.remove("header");

                let encrypted_key_vec;
                let encrypted_key = match recipient.get("encrypted_key") {
                    Some(Value::String(val)) => {
                        if val.len() == 0 {
                            bail!("The encrypted_key field must be empty.");
                        }
                        encrypted_key_vec = base64::decode_config(&val, base64::URL_SAFE_NO_PAD)?;
                        Some(encrypted_key_vec.as_slice())
                    }
                    Some(_) => bail!("The encrypted_key field must be a string."),
                    None => None,
                };

                let mut merged = match header {
                    Some(Value::Object(val)) => val,
                    Some(_) => bail!("The protected field must be a object."),
                    None => Map::new(),
                };

                if let Some(val) = &unprotected {
                    for (key, value) in val {
                        if merged.contains_key(key) {
                            bail!("A duplicate key exists: {}", key);
                        } else {
                            merged.insert(key.clone(), value.clone());
                        }
                    }
                }

                if let Some(val) = &protected {
                    for (key, value) in val {
                        if merged.contains_key(key) {
                            bail!("A duplicate key exists: {}", key);
                        } else {
                            merged.insert(key.clone(), value.clone());
                        }
                    }
                }

                let merged = JweHeader::from_map(merged)?;

                let decrypter = match selector(&merged)? {
                    Some(val) => val,
                    None => continue,
                };

                let cencryption = match merged.claim("enc") {
                    Some(Value::String(val)) => match self.get_content_encryption(val) {
                        Some(val2) => val2,
                        None => bail!("A content encryption is not registered: {}", val),
                    },
                    Some(_) => bail!("A enc header claim must be string."),
                    None => bail!("A enc header claim is required."),
                };

                let compression = match merged.claim("zip") {
                    Some(Value::String(val)) => match self.get_compression(val) {
                        Some(val2) => Some(val2),
                        None => bail!("A compression algorithm is not registered: {}", val),
                    },
                    Some(_) => bail!("A enc header claim must be string."),
                    None => None,
                };

                match merged.algorithm() {
                    Some(val) => {
                        let expected_alg = decrypter.algorithm().name();
                        if val != expected_alg {
                            bail!("The JWE alg header claim is not {}: {}", expected_alg, val);
                        }
                    }
                    None => bail!("The JWE alg header claim is required."),
                }

                match decrypter.key_id() {
                    Some(expected) => match merged.key_id() {
                        Some(actual) if expected == actual => {}
                        Some(actual) => bail!("The JWE kid header claim is mismatched: {}", actual),
                        None => bail!("The JWE kid header claim is required."),
                    },
                    None => {}
                }

                let mut full_aad = match protected_b64 {
                    Some(val) => val,
                    None => String::new(),
                };
                if let Some(val) = aad_b64 {
                    full_aad.push_str(".");
                    full_aad.push_str(&val);
                }

                let key = decrypter.decrypt(encrypted_key, cencryption, &merged)?;
                if key.len() != cencryption.key_len() {
                    bail!(
                        "The key size is expected to be {}: {}",
                        cencryption.key_len(),
                        key.len()
                    );
                }

                let content =
                    cencryption.decrypt(&key, iv, &ciphertext, full_aad.as_bytes(), tag)?;
                let content = match compression {
                    Some(val) => val.decompress(&content)?,
                    None => content,
                };

                return Ok((content, merged));
            }

            bail!("A recipient that matched the header claims is not found.");
        })()
        .map_err(|err| match err.downcast::<JoseError>() {
            Ok(err) => err,
            Err(err) => JoseError::InvalidJweFormat(err),
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use crate::jwe::{
        alg::direct::DirectJweAlgorithm,
        JweHeader, JweHeaderSet,
        serialize_compact, deserialize_compact,
        serialize_flattened_json, serialize_general_json, deserialize_json
    };

    const CONTENT_CIPHERS: [(&str, usize); 6] = [
        ("A128CBC-HS256", 32), ("A192CBC-HS384", 48), ("A256CBC-HS512", 64),
        ("A128GCM", 16), ("A192GCM", 24), ("A256GCM", 32)
    ];

    #[test]
    fn compact_dir() -> Result<()> {
        let payload = b"hello world";
        let alg = DirectJweAlgorithm::Dir;
        for (cipher, keylen) in CONTENT_CIPHERS {
            let mut header = JweHeader::new();
            header.set_content_encryption(cipher);
            let key = vec![0; keylen];
            let encrypter = alg.encrypter_from_bytes(&key)?;
            let jwe = serialize_compact(payload, &header, &encrypter)?;
            println!("{}", jwe);

            let decrypter = alg.decrypter_from_bytes(&key)?;
            let (data, _header) = deserialize_compact(&jwe, &decrypter)?;
            assert_eq!(data, payload);
        }
        Ok(())
    }

    #[test]
    fn flattened_json_dir() -> Result<()> {
        let payload = b"hello world";
        let alg = DirectJweAlgorithm::Dir;
        for (cipher, keylen) in CONTENT_CIPHERS {
            let mut hs = JweHeaderSet::new();
            hs.set_content_encryption(cipher, true);
            let key = vec![0; keylen];
            let encrypter = alg.encrypter_from_bytes(&key)?;
            let jwe = serialize_flattened_json(
                payload, Some(&hs), None, None, &encrypter)?;
            println!("{}", jwe);

            let decrypter = alg.decrypter_from_bytes(&key)?;
            let (data, _header) = deserialize_json(&jwe, &decrypter)?;
            assert_eq!(data, payload);
        }
        Ok(())
    }

    #[test]
    fn general_json_dir() -> Result<()> {
        let payload = b"hello world";
        let alg = DirectJweAlgorithm::Dir;
        for (cipher, keylen) in CONTENT_CIPHERS {
            let mut hs = JweHeaderSet::new();
            hs.set_content_encryption(cipher, true);
            let key = vec![0; keylen];
            let encrypter = alg.encrypter_from_bytes(&key)?;
            let jwe = serialize_general_json(
                payload, Some(&hs), &[(None, &encrypter)], None)?;
            println!("{}", jwe);

            let decrypter = alg.decrypter_from_bytes(&key)?;
            let (data, _header) = deserialize_json(&jwe, &decrypter)?;
            assert_eq!(data, payload);
        }
        Ok(())
    }
}
