use std::borrow::Cow;
use std::convert::TryFrom;
use std::fmt::Display;
use std::ops::Deref;

use anyhow::bail;
use openssl::aes::{self, AesKey};
use openssl::pkcs5;

use crate::jwe::{JweAlgorithm, JweContentEncryption, JweDecrypter, JweEncrypter, JweHeader};
use crate::jwk::Jwk;
use crate::util::{self, HashAlgorithm};
use crate::{JoseError, JoseHeader, Number, Value};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum Pbes2HmacAeskwJweAlgorithm {
    /// PBES2 with HMAC SHA-256 and "A128KW" wrapping
    Pbes2Hs256A128kw,
    /// PBES2 with HMAC SHA-384 and "A192KW" wrapping
    Pbes2Hs384A192kw,
    /// PBES2 with HMAC SHA-512 and "A256KW" wrapping
    Pbes2Hs512A256kw,
}

impl Pbes2HmacAeskwJweAlgorithm {
    pub fn encrypter_from_bytes(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<Pbes2HmacAeskwJweEncrypter, JoseError> {
        (|| -> anyhow::Result<Pbes2HmacAeskwJweEncrypter> {
            let private_key = input.as_ref().to_vec();

            if private_key.len() == 0 {
                bail!("The key size must not be empty.");
            }

            Ok(Pbes2HmacAeskwJweEncrypter {
                algorithm: self.clone(),
                private_key,
                salt_len: 8,
                iter_count: 1000,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn encrypter_from_jwk(&self, jwk: &Jwk) -> Result<Pbes2HmacAeskwJweEncrypter, JoseError> {
        (|| -> anyhow::Result<Pbes2HmacAeskwJweEncrypter> {
            match jwk.key_type() {
                val if val == "oct" => {}
                val => bail!("A parameter kty must be oct: {}", val),
            }
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
            let k = match jwk.parameter("k") {
                Some(Value::String(val)) => base64::decode_config(val, base64::URL_SAFE_NO_PAD)?,
                Some(val) => bail!("A parameter k must be string type but {:?}", val),
                None => bail!("A parameter k is required."),
            };

            if k.len() == 0 {
                bail!("The key size must not be empty.");
            }

            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(Pbes2HmacAeskwJweEncrypter {
                algorithm: self.clone(),
                private_key: k,
                salt_len: 8,
                iter_count: 1000,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn decrypter_from_bytes(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<Pbes2HmacAeskwJweDecrypter, JoseError> {
        (|| -> anyhow::Result<Pbes2HmacAeskwJweDecrypter> {
            let private_key = input.as_ref().to_vec();

            if private_key.len() == 0 {
                bail!("The key size must not be empty.");
            }

            Ok(Pbes2HmacAeskwJweDecrypter {
                algorithm: self.clone(),
                private_key,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn decrypter_from_jwk(&self, jwk: &Jwk) -> Result<Pbes2HmacAeskwJweDecrypter, JoseError> {
        (|| -> anyhow::Result<Pbes2HmacAeskwJweDecrypter> {
            match jwk.key_type() {
                val if val == "oct" => {}
                val => bail!("A parameter kty must be oct: {}", val),
            }
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

            let k = match jwk.parameter("k") {
                Some(Value::String(val)) => base64::decode_config(val, base64::URL_SAFE_NO_PAD)?,
                Some(val) => bail!("A parameter k must be string type but {:?}", val),
                None => bail!("A parameter k is required."),
            };

            if k.len() == 0 {
                bail!("The key size must not be empty.");
            }

            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(Pbes2HmacAeskwJweDecrypter {
                algorithm: self.clone(),
                private_key: k,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    fn hash_algorithm(&self) -> HashAlgorithm {
        match self {
            Self::Pbes2Hs256A128kw => HashAlgorithm::Sha256,
            Self::Pbes2Hs384A192kw => HashAlgorithm::Sha384,
            Self::Pbes2Hs512A256kw => HashAlgorithm::Sha512,
        }
    }

    fn derived_key_len(&self) -> usize {
        match self {
            Self::Pbes2Hs256A128kw => 16,
            Self::Pbes2Hs384A192kw => 24,
            Self::Pbes2Hs512A256kw => 32,
        }
    }
}

impl JweAlgorithm for Pbes2HmacAeskwJweAlgorithm {
    fn name(&self) -> &str {
        match self {
            Self::Pbes2Hs256A128kw => "PBES2-HS256+A128KW",
            Self::Pbes2Hs384A192kw => "PBES2-HS384+A192KW",
            Self::Pbes2Hs512A256kw => "PBES2-HS512+A256KW",
        }
    }

    fn box_clone(&self) -> Box<dyn JweAlgorithm> {
        Box::new(self.clone())
    }
}

impl Display for Pbes2HmacAeskwJweAlgorithm {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str(self.name())
    }
}

impl Deref for Pbes2HmacAeskwJweAlgorithm {
    type Target = dyn JweAlgorithm;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct Pbes2HmacAeskwJweEncrypter {
    algorithm: Pbes2HmacAeskwJweAlgorithm,
    private_key: Vec<u8>,
    salt_len: usize,
    iter_count: usize,
    key_id: Option<String>,
}

impl Pbes2HmacAeskwJweEncrypter {
    pub fn set_salt_len(&mut self, salt_len: usize) {
        if salt_len < 8 {
            panic!("salt_len must be 8 or more: {}", salt_len);
        }
        self.salt_len = salt_len;
    }

    pub fn set_iter_count(&mut self, iter_count: usize) {
        if iter_count < 1000 {
            panic!("iter_count must be 1000 or more: {}", iter_count);
        }
        self.iter_count = iter_count;
    }

    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JweEncrypter for Pbes2HmacAeskwJweEncrypter {
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
        _cencryption: &dyn JweContentEncryption,
        _in_header: &JweHeader,
        _out_header: &mut JweHeader,
    ) -> Result<Option<Cow<[u8]>>, JoseError> {
        Ok(None)
    }

    fn encrypt(
        &self,
        key: &[u8],
        in_header: &JweHeader,
        out_header: &mut JweHeader,
    ) -> Result<Option<Vec<u8>>, JoseError> {
        (|| -> anyhow::Result<Option<Vec<u8>>> {
            let p2s = match in_header.claim("p2s") {
                Some(Value::String(val)) => {
                    let p2s = base64::decode_config(val, base64::URL_SAFE_NO_PAD)?;
                    if p2s.len() < 8 {
                        bail!("The decoded value of p2s header claim must be 8 or more.");
                    }
                    p2s
                }
                Some(_) => bail!("The p2s header claim must be string."),
                None => {
                    let p2s = util::random_bytes(self.salt_len);
                    let p2s_b64 = base64::encode_config(&p2s, base64::URL_SAFE_NO_PAD);
                    out_header.set_claim("p2s", Some(Value::String(p2s_b64)))?;
                    p2s
                }
            };
            let p2c = match in_header.claim("p2c") {
                Some(Value::Number(val)) => match val.as_u64() {
                    Some(val) => usize::try_from(val)?,
                    None => bail!("Overflow u64 value: {}", val),
                },
                Some(_) => bail!("The apv header claim must be string."),
                None => {
                    let p2c = self.iter_count;
                    out_header.set_claim("p2c", Some(Value::Number(Number::from(p2c))))?;
                    p2c
                }
            };

            let mut salt = Vec::with_capacity(self.algorithm().name().len() + 1 + p2s.len());
            salt.extend_from_slice(self.algorithm().name().as_bytes());
            salt.push(0);
            salt.extend_from_slice(&p2s);

            let md = self.algorithm.hash_algorithm().message_digest();
            let mut derived_key = vec![0; self.algorithm.derived_key_len()];
            pkcs5::pbkdf2_hmac(&self.private_key, &salt, p2c, md, &mut derived_key)?;

            let aes = match AesKey::new_encrypt(&derived_key) {
                Ok(val) => val,
                Err(_) => bail!("Failed to set a encryption key."),
            };

            let mut encrypted_key = vec![0; key.len() + 8];
            match aes::wrap_key(&aes, None, &mut encrypted_key, &key) {
                Ok(val) => {
                    if val < encrypted_key.len() {
                        encrypted_key.truncate(val);
                    }
                }
                Err(_) => bail!("Failed to wrap a key."),
            }

            Ok(Some(encrypted_key))
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    fn box_clone(&self) -> Box<dyn JweEncrypter> {
        Box::new(self.clone())
    }
}

impl Deref for Pbes2HmacAeskwJweEncrypter {
    type Target = dyn JweEncrypter;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct Pbes2HmacAeskwJweDecrypter {
    algorithm: Pbes2HmacAeskwJweAlgorithm,
    private_key: Vec<u8>,
    key_id: Option<String>,
}

impl Pbes2HmacAeskwJweDecrypter {
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JweDecrypter for Pbes2HmacAeskwJweDecrypter {
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
        _cencryption: &dyn JweContentEncryption,
        header: &JweHeader,
    ) -> Result<Cow<[u8]>, JoseError> {
        (|| -> anyhow::Result<Cow<[u8]>> {
            let encrypted_key = match encrypted_key {
                Some(val) => val,
                None => bail!("A encrypted_key value is required."),
            };

            let p2s = match header.claim("p2s") {
                Some(Value::String(val)) => {
                    let p2s = base64::decode_config(val, base64::URL_SAFE_NO_PAD)?;
                    if p2s.len() < 8 {
                        bail!("The decoded value of p2s header claim must be 8 or more.");
                    }
                    p2s
                }
                Some(_) => bail!("The p2s header claim must be string."),
                None => bail!("The p2s header claim is required."),
            };
            let p2c = match header.claim("p2c") {
                Some(Value::Number(val)) => match val.as_u64() {
                    Some(val) => usize::try_from(val)?,
                    None => bail!("Overflow u64 value: {}", val),
                },
                Some(_) => bail!("The p2s header claim must be string."),
                None => bail!("The p2c header claim is required."),
            };

            let mut salt = Vec::with_capacity(self.algorithm().name().len() + 1 + p2s.len());
            salt.extend_from_slice(self.algorithm().name().as_bytes());
            salt.push(0);
            salt.extend_from_slice(&p2s);

            let md = self.algorithm.hash_algorithm().message_digest();
            let mut derived_key = vec![0; self.algorithm.derived_key_len()];
            pkcs5::pbkdf2_hmac(&self.private_key, &salt, p2c, md, &mut derived_key)?;

            let aes = match AesKey::new_decrypt(&derived_key) {
                Ok(val) => val,
                Err(_) => bail!("Failed to set a decryption key."),
            };

            let mut key = vec![0; encrypted_key.len() - 8];
            match aes::unwrap_key(&aes, None, &mut key, &encrypted_key) {
                Ok(val) => {
                    if val < key.len() {
                        key.truncate(val);
                    }
                }
                Err(_) => bail!("Failed to unwrap a key."),
            }

            Ok(Cow::Owned(key))
        })()
        .map_err(|err| JoseError::InvalidJweFormat(err))
    }

    fn box_clone(&self) -> Box<dyn JweDecrypter> {
        Box::new(self.clone())
    }
}

impl Deref for Pbes2HmacAeskwJweDecrypter {
    type Target = dyn JweDecrypter;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use base64;
    use serde_json::json;

    use super::Pbes2HmacAeskwJweAlgorithm;
    use crate::jwe::enc::aescbc_hmac::AescbcHmacJweEncryption;
    use crate::jwe::JweHeader;
    use crate::jwk::Jwk;
    use crate::util;

    #[test]
    fn encrypt_and_decrypt_pbes2_hmac() -> Result<()> {
        let enc = AescbcHmacJweEncryption::A128cbcHs256;

        for alg in vec![
            Pbes2HmacAeskwJweAlgorithm::Pbes2Hs256A128kw,
            Pbes2HmacAeskwJweAlgorithm::Pbes2Hs384A192kw,
            Pbes2HmacAeskwJweAlgorithm::Pbes2Hs512A256kw,
        ] {
            let mut header = JweHeader::new();
            header.set_content_encryption(enc.name());

            let jwk = {
                let key = util::random_bytes(8);
                let key = base64::encode_config(&key, base64::URL_SAFE_NO_PAD);

                let mut jwk = Jwk::new("oct");
                jwk.set_key_use("enc");
                jwk.set_parameter("k", Some(json!(key)))?;
                jwk
            };

            let encrypter = alg.encrypter_from_jwk(&jwk)?;
            let mut out_header = header.clone();
            let src_key = util::random_bytes(enc.key_len());
            let encrypted_key = encrypter.encrypt(&src_key, &header, &mut out_header)?;

            let decrypter = alg.decrypter_from_jwk(&jwk)?;

            let dst_key = decrypter.decrypt(encrypted_key.as_deref(), &enc, &out_header)?;

            assert_eq!(&src_key as &[u8], &dst_key as &[u8]);
        }

        Ok(())
    }
}
