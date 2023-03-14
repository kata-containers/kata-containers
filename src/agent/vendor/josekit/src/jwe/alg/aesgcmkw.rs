use std::borrow::Cow;
use std::fmt::Display;
use std::ops::Deref;

use anyhow::bail;
use openssl::symm::{self, Cipher};

use crate::jwe::{JweAlgorithm, JweContentEncryption, JweDecrypter, JweEncrypter, JweHeader};
use crate::jwk::Jwk;
use crate::util;
use crate::{JoseError, JoseHeader, Value};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum AesgcmkwJweAlgorithm {
    /// Key wrapping with AES GCM using 128-bit key
    A128gcmkw,
    /// Key wrapping with AES GCM using 192-bit key
    A192gcmkw,
    /// Key wrapping with AES GCM using 256-bit key
    A256gcmkw,
}

impl AesgcmkwJweAlgorithm {
    pub fn encrypter_from_bytes(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<AesgcmkwJweEncrypter, JoseError> {
        (|| -> anyhow::Result<AesgcmkwJweEncrypter> {
            let private_key = input.as_ref().to_vec();

            if private_key.len() != self.key_len() {
                bail!(
                    "The key size must be {}: {}",
                    self.key_len(),
                    private_key.len()
                );
            }

            Ok(AesgcmkwJweEncrypter {
                algorithm: self.clone(),
                private_key,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn encrypter_from_jwk(&self, jwk: &Jwk) -> Result<AesgcmkwJweEncrypter, JoseError> {
        (|| -> anyhow::Result<AesgcmkwJweEncrypter> {
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

            if k.len() != self.key_len() {
                bail!("The key size must be {}: {}", self.key_len(), k.len());
            }

            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(AesgcmkwJweEncrypter {
                algorithm: self.clone(),
                private_key: k,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn decrypter_from_bytes(
        &self,
        input: impl AsRef<[u8]>,
    ) -> Result<AesgcmkwJweDecrypter, JoseError> {
        (|| -> anyhow::Result<AesgcmkwJweDecrypter> {
            let private_key = input.as_ref().to_vec();

            if private_key.len() != self.key_len() {
                bail!(
                    "The key size must be {}: {}",
                    self.key_len(),
                    private_key.len()
                );
            }

            Ok(AesgcmkwJweDecrypter {
                algorithm: self.clone(),
                private_key,
                key_id: None,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    pub fn decrypter_from_jwk(&self, jwk: &Jwk) -> Result<AesgcmkwJweDecrypter, JoseError> {
        (|| -> anyhow::Result<AesgcmkwJweDecrypter> {
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

            if k.len() != self.key_len() {
                bail!("The key size must be {}: {}", self.key_len(), k.len());
            }

            let key_id = jwk.key_id().map(|val| val.to_string());

            Ok(AesgcmkwJweDecrypter {
                algorithm: self.clone(),
                private_key: k,
                key_id,
            })
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    fn key_len(&self) -> usize {
        match self {
            Self::A128gcmkw => 16,
            Self::A192gcmkw => 24,
            Self::A256gcmkw => 32,
        }
    }

    fn cipher(&self) -> Cipher {
        match self {
            Self::A128gcmkw => Cipher::aes_128_gcm(),
            Self::A192gcmkw => Cipher::aes_192_gcm(),
            Self::A256gcmkw => Cipher::aes_256_gcm(),
        }
    }
}

impl JweAlgorithm for AesgcmkwJweAlgorithm {
    fn name(&self) -> &str {
        match self {
            Self::A128gcmkw => "A128GCMKW",
            Self::A192gcmkw => "A192GCMKW",
            Self::A256gcmkw => "A256GCMKW",
        }
    }

    fn box_clone(&self) -> Box<dyn JweAlgorithm> {
        Box::new(self.clone())
    }
}

impl Display for AesgcmkwJweAlgorithm {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str(self.name())
    }
}

impl Deref for AesgcmkwJweAlgorithm {
    type Target = dyn JweAlgorithm;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct AesgcmkwJweEncrypter {
    algorithm: AesgcmkwJweAlgorithm,
    private_key: Vec<u8>,
    key_id: Option<String>,
}

impl AesgcmkwJweEncrypter {
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JweEncrypter for AesgcmkwJweEncrypter {
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
        _in_header: &JweHeader,
        out_header: &mut JweHeader,
    ) -> Result<Option<Vec<u8>>, JoseError> {
        (|| -> anyhow::Result<Option<Vec<u8>>> {
            let iv = util::random_bytes(32);

            let cipher = self.algorithm.cipher();
            let mut tag = [0; 16];
            let encrypted_key =
                symm::encrypt_aead(cipher, &self.private_key, Some(&iv), b"", &key, &mut tag)?;

            let iv = base64::encode_config(&iv, base64::URL_SAFE_NO_PAD);
            out_header.set_claim("iv", Some(Value::String(iv)))?;

            let tag = base64::encode_config(&tag, base64::URL_SAFE_NO_PAD);
            out_header.set_claim("tag", Some(Value::String(tag)))?;

            Ok(Some(encrypted_key))
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

impl Deref for AesgcmkwJweEncrypter {
    type Target = dyn JweEncrypter;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct AesgcmkwJweDecrypter {
    algorithm: AesgcmkwJweAlgorithm,
    private_key: Vec<u8>,
    key_id: Option<String>,
}

impl AesgcmkwJweDecrypter {
    pub fn set_key_id(&mut self, value: impl Into<String>) {
        self.key_id = Some(value.into());
    }

    pub fn remove_key_id(&mut self) {
        self.key_id = None;
    }
}

impl JweDecrypter for AesgcmkwJweDecrypter {
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
                None => bail!("A encrypted_key is required."),
            };

            let iv = match header.claim("iv") {
                Some(Value::String(val)) => base64::decode_config(val, base64::URL_SAFE_NO_PAD)?,
                Some(_) => bail!("The iv header claim must be string."),
                None => bail!("The iv header claim is required."),
            };

            let tag = match header.claim("tag") {
                Some(Value::String(val)) => base64::decode_config(val, base64::URL_SAFE_NO_PAD)?,
                Some(_) => bail!("The tag header claim must be string."),
                None => bail!("The tag header claim is required."),
            };

            let cipher = self.algorithm.cipher();
            let key = symm::decrypt_aead(
                cipher,
                &self.private_key,
                Some(&iv),
                b"",
                encrypted_key,
                &tag,
            )?;

            Ok(Cow::Owned(key))
        })()
        .map_err(|err| JoseError::InvalidJweFormat(err))
    }

    fn box_clone(&self) -> Box<dyn JweDecrypter> {
        Box::new(self.clone())
    }
}

impl Deref for AesgcmkwJweDecrypter {
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

    use super::AesgcmkwJweAlgorithm;
    use crate::jwe::enc::aescbc_hmac::AescbcHmacJweEncryption;
    use crate::jwe::JweHeader;
    use crate::jwk::Jwk;
    use crate::util;

    #[test]
    fn encrypt_and_decrypt_aes_gcm() -> Result<()> {
        let enc = AescbcHmacJweEncryption::A128cbcHs256;

        for alg in vec![
            AesgcmkwJweAlgorithm::A128gcmkw,
            AesgcmkwJweAlgorithm::A192gcmkw,
            AesgcmkwJweAlgorithm::A256gcmkw,
        ] {
            let mut header = JweHeader::new();
            header.set_content_encryption(enc.name());

            let jwk = {
                let key = util::random_bytes(alg.key_len());
                let key = base64::encode_config(&key, base64::URL_SAFE_NO_PAD);

                let mut jwk = Jwk::new("oct");
                jwk.set_key_use("enc");
                jwk.set_parameter("k", Some(json!(key)))?;
                jwk
            };

            let encrypter = alg.encrypter_from_jwk(&jwk)?;
            let src_key = util::random_bytes(enc.key_len());
            let mut out_header = header.clone();
            let encrypted_key = encrypter.encrypt(&src_key, &header, &mut out_header)?;

            let decrypter = alg.decrypter_from_jwk(&jwk)?;
            let dst_key = decrypter.decrypt(encrypted_key.as_deref(), &enc, &out_header)?;

            assert_eq!(&src_key as &[u8], &dst_key as &[u8]);
        }

        Ok(())
    }
}
