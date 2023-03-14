use std::borrow::Cow;
use std::fmt::Display;
use std::ops::Deref;

use anyhow::bail;

use crate::jwe::{JweAlgorithm, JweContentEncryption, JweDecrypter, JweEncrypter, JweHeader};
use crate::jwk::Jwk;
use crate::{JoseError, Value};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum DirectJweAlgorithm {
    /// Direct use of a shared symmetric key as the CEK
    Dir,
}

impl DirectJweAlgorithm {
    pub fn encrypter_from_bytes(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<DirectJweEncrypter, JoseError> {
        let cencryption_key = input.as_ref();

        Ok(DirectJweEncrypter {
            algorithm: self.clone(),
            cencryption_key: cencryption_key.to_vec(),
            key_id: None,
        })
    }

    pub fn encrypter_from_jwk(&self, jwk: &Jwk) -> Result<DirectJweEncrypter, JoseError> {
        (|| -> anyhow::Result<DirectJweEncrypter> {
            match jwk.key_type() {
                val if val == "oct" => {}
                val => bail!("A parameter kty must be oct: {}", val),
            }
            match jwk.key_use() {
                Some(val) if val == "enc" => {}
                None => {}
                Some(val) => bail!("A parameter use must be enc: {}", val),
            }
            if !jwk.is_for_key_operation("encrypt") {
                bail!("A parameter key_ops must contains encrypt.");
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

            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(DirectJweEncrypter {
                algorithm: self.clone(),
                cencryption_key: k,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn decrypter_from_bytes(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<DirectJweDecrypter, JoseError> {
        let cencryption_key = input.as_ref();

        Ok(DirectJweDecrypter {
            algorithm: self.clone(),
            cencryption_key: cencryption_key.to_vec(),
            key_id: None,
        })
    }

    pub fn decrypter_from_jwk(&self, jwk: &Jwk) -> Result<DirectJweDecrypter, JoseError> {
        (|| -> anyhow::Result<DirectJweDecrypter> {
            match jwk.key_type() {
                val if val == "oct" => {}
                val => bail!("A parameter kty must be oct: {}", val),
            }
            match jwk.key_use() {
                Some(val) if val == "enc" => {}
                None => {}
                Some(val) => bail!("A parameter use must be enc: {}", val),
            }
            if !jwk.is_for_key_operation("decrypt") {
                bail!("A parameter key_ops must contains decrypt.");
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

            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(DirectJweDecrypter {
                algorithm: self.clone(),
                cencryption_key: k,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }
}

impl JweAlgorithm for DirectJweAlgorithm {
    fn name(&self) -> &str {
        match self {
            Self::Dir => "dir",
        }
    }

    fn box_clone(&self) -> Box<dyn JweAlgorithm> {
        Box::new(self.clone())
    }
}

impl Display for DirectJweAlgorithm {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str(self.name())
    }
}

impl Deref for DirectJweAlgorithm {
    type Target = dyn JweAlgorithm;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct DirectJweEncrypter {
    algorithm: DirectJweAlgorithm,
    cencryption_key: Vec<u8>,
    key_id: Option<String>,
}

impl DirectJweEncrypter {
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JweEncrypter for DirectJweEncrypter {
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
        _header: &mut JweHeader,
    ) -> Result<Option<Cow<[u8]>>, JoseError> {
        (|| -> anyhow::Result<Option<Cow<[u8]>>> {
            let actual_len = self.cencryption_key.len();
            if cencryption.key_len() != actual_len {
                bail!(
                    "The key size is expected to be {}: {}",
                    cencryption.key_len(),
                    actual_len
                );
            }

            Ok(Some(Cow::Borrowed(&self.cencryption_key)))
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    fn encrypt(
        &self,
        _key: &[u8],
        _merged: &JweHeader,
        _header: &mut JweHeader,
    ) -> Result<Option<Vec<u8>>, JoseError> {
        Ok(None)
    }

    fn box_clone(&self) -> Box<dyn JweEncrypter> {
        Box::new(self.clone())
    }
}

impl Deref for DirectJweEncrypter {
    type Target = dyn JweEncrypter;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct DirectJweDecrypter {
    algorithm: DirectJweAlgorithm,
    cencryption_key: Vec<u8>,
    key_id: Option<String>,
}

impl DirectJweDecrypter {
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JweDecrypter for DirectJweDecrypter {
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
        _header: &JweHeader,
    ) -> Result<Cow<[u8]>, JoseError> {
        (|| -> anyhow::Result<Cow<[u8]>> {
            if let Some(_) = encrypted_key {
                bail!("The encrypted_key must not exist.");
            }

            Ok(Cow::Borrowed(&self.cencryption_key))
        })()
        .map_err(|err| JoseError::InvalidJweFormat(err))
    }

    fn box_clone(&self) -> Box<dyn JweDecrypter> {
        Box::new(self.clone())
    }
}

impl Deref for DirectJweDecrypter {
    type Target = dyn JweDecrypter;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use serde_json::json;

    use super::DirectJweAlgorithm;
    use crate::jwe::enc::aescbc_hmac::AescbcHmacJweEncryption;
    use crate::jwe::JweHeader;
    use crate::jwk::Jwk;

    #[test]
    fn encrypt_and_decrypt_direct() -> Result<()> {
        let enc = AescbcHmacJweEncryption::A128cbcHs256;
        let jwk = {
            let mut jwk = Jwk::new("oct");
            jwk.set_key_use("enc");
            jwk.set_parameter(
                "k",
                Some(json!("MDEyMzQ1Njc4OUFCQ0RFRjAxMjM0NTY3ODlBQkNERUY")),
            )?;
            jwk
        };

        for alg in vec![DirectJweAlgorithm::Dir] {
            let mut header = JweHeader::new();
            header.set_content_encryption(enc.name());

            let encrypter = alg.encrypter_from_jwk(&jwk)?;
            let mut out_header = header.clone();
            let src_key =
                encrypter.compute_content_encryption_key(&enc, &header, &mut out_header)?;
            let src_key = src_key.unwrap();
            let encrypted_key = encrypter.encrypt(&src_key, &header, &mut out_header)?;
            assert_eq!(encrypted_key, None);

            let decrypter = alg.decrypter_from_jwk(&jwk)?;
            let dst_key = decrypter.decrypt(encrypted_key.as_deref(), &enc, &out_header)?;

            assert_eq!(&src_key, &dst_key);
        }

        Ok(())
    }
}
