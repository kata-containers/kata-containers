// Copyright The ocicrypt Authors.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::io::Read;

use anyhow::{anyhow, Result};
use base64_serde::base64_serde_type;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

mod aes_ctr;
use aes_ctr::AESCTRBlockCipher;

/// Type of the cipher algorithm used to encrypt/decrypt image layers.
pub type LayerCipherType = String;

// TODO: Should be obtained from OCI spec once included
/// The default cipher algorithm for image layer encryption/decryption.
pub const AES256CTR: &str = "AES_256_CTR_HMAC_SHA256";

base64_serde_type!(Base64Vec, base64::STANDARD);

fn base64_hashmap_s<S>(value: &HashMap<String, Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let b64_encoded: HashMap<_, _> = value
        .iter()
        .map(|(k, v)| (k.clone(), base64::encode_config(v, base64::STANDARD)))
        .collect();
    b64_encoded.serialize(serializer)
}

fn base64_hashmap_d<'de, D>(deserializer: D) -> Result<HashMap<String, Vec<u8>>, D::Error>
where
    D: Deserializer<'de>,
{
    let b64_encoded: HashMap<String, String> = serde::Deserialize::deserialize(deserializer)?;
    b64_encoded
        .iter()
        .map(|(k, v)| -> Result<(String, Vec<u8>), D::Error> {
            Ok((
                k.clone(),
                base64::decode_config(v, base64::STANDARD).map_err(de::Error::custom)?,
            ))
        })
        .collect()
}

/// The information required to encrypt/decrypt an image layer which are sensitive and should not
/// be in plaintext.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PrivateLayerBlockCipherOptions {
    /// The symmetric key used for encryption/decryption.
    ///
    /// This field should be populated by the LayerBlockCipher::encrypt() method.
    #[serde(rename = "symkey", with = "Base64Vec")]
    pub symmetric_key: Vec<u8>,

    /// The cipher metadata used to configure the encryption/decryption algorithm.
    ///
    /// This field should be populated by the LayerBlockCipher::encrypt()/decrypt() methods.
    #[serde(
        rename = "cipheroptions",
        serialize_with = "base64_hashmap_s",
        deserialize_with = "base64_hashmap_d"
    )]
    pub cipher_options: HashMap<String, Vec<u8>>,

    /// The digest of the original data.
    ///
    /// This field is NOT populated by the LayerBlockCipher::encrypt()/decrypt() methods.
    pub digest: String,
}

/// The information required to encrypt/decrypt an image layer which are public and can be
/// deduplicated in plaintext across multiple recipients.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PublicLayerBlockCipherOptions {
    /// The cipher algorithm used to encrypt/decrypt the image layer.
    #[serde(rename = "cipher")]
    pub cipher_type: LayerCipherType,

    /// The cipher metadata used to configure the encryption/decryption algorithm.
    ///
    /// This field should be populated by the LayerBlockCipher::encrypt()/decrypt() methods.
    #[serde(
        rename = "cipheroptions",
        serialize_with = "base64_hashmap_s",
        deserialize_with = "base64_hashmap_d"
    )]
    pub cipher_options: HashMap<String, Vec<u8>>,

    /// The hashed message authentication code used to verify the layer data.
    ///
    /// This field should be populated by LayerBlockCipher::encrypt() methods.
    #[serde(with = "Base64Vec")]
    pub hmac: Vec<u8>,
}

/// The public and private configuration information required to encrypt/decrypt an image layer.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct LayerBlockCipherOptions {
    /// The public configuration information for image layer encryption/decryption.
    pub public: PublicLayerBlockCipherOptions,
    /// The private configuration information for image layer encryption/decryption.
    pub private: PrivateLayerBlockCipherOptions,
}

impl LayerBlockCipherOptions {
    /// Get an option from the public or private configuration, value from the public configuration
    /// has higher priority.
    pub fn get_opt(&self, key: &str) -> Option<Vec<u8>> {
        self.public
            .cipher_options
            .get(key)
            .or_else(|| self.private.cipher_options.get(key))
            .map(|v| v.to_vec())
    }
}

/// Trait to setup the encryption/decryption context.
pub trait LayerBlockCipher<R> {
    /// Create a symmetric key for encryption.
    fn generate_key(&self) -> Result<Vec<u8>>;

    /// Setup the context for encryption.
    fn encrypt(&mut self, input: R, opts: &mut LayerBlockCipherOptions) -> Result<()>;

    /// Setup the context for decryption.
    fn decrypt(&mut self, input: R, opts: &mut LayerBlockCipherOptions) -> Result<()>;
}

/// Trait to finalize the encryption operation.
pub trait EncryptionFinalizer {
    /// Update the [`LayerBlockCipherOptions`] object after finishing the encryption operation.
    fn finalized_lbco(&self, opts: &mut LayerBlockCipherOptions) -> Result<()>;
}

/// Handler for image layer encryption/decryption.
pub enum LayerBlockCipherHandler<R> {
    /// AES_256_CTR_HMAC_SHA256
    Aes256Ctr(AESCTRBlockCipher<R>),
}

impl<R> LayerBlockCipherHandler<R> {
    /// Create a [`LayerBlockCipherHandler`] object with default aes ctr block cipher
    pub fn new() -> Result<LayerBlockCipherHandler<R>> {
        let aes_ctr_block_cipher = AESCTRBlockCipher::new(256)?;
        Ok(LayerBlockCipherHandler::Aes256Ctr(aes_ctr_block_cipher))
    }
}

impl<R> LayerBlockCipherHandler<R> {
    /// Setup the context for image layer encryption.
    pub fn encrypt(
        &mut self,
        plain_data_reader: R,
        typ: &str,
        opts: &mut LayerBlockCipherOptions,
    ) -> Result<()> {
        match self {
            LayerBlockCipherHandler::Aes256Ctr(block_cipher) => {
                if typ != AES256CTR {
                    return Err(anyhow!("unsupported cipher type {}", typ));
                }
                opts.private.symmetric_key = block_cipher.generate_key()?;
                opts.public.cipher_type = AES256CTR.to_string();
                block_cipher.encrypt(plain_data_reader, opts)?;
            }
        }

        Ok(())
    }

    /// Setup the context for image layer decryption.
    pub fn decrypt(
        &mut self,
        enc_data_reader: R,
        opts: &mut LayerBlockCipherOptions,
    ) -> Result<()> {
        let typ = &opts.public.cipher_type;

        match self {
            LayerBlockCipherHandler::Aes256Ctr(block_cipher) => {
                if typ != AES256CTR {
                    return Err(anyhow!("unsupported cipher type {}", typ));
                }
                block_cipher.decrypt(enc_data_reader, opts)?;
            }
        }

        Ok(())
    }
}

impl<R> EncryptionFinalizer for LayerBlockCipherHandler<R> {
    fn finalized_lbco(&self, opts: &mut LayerBlockCipherOptions) -> Result<()> {
        match self {
            LayerBlockCipherHandler::Aes256Ctr(block_cipher) => block_cipher.finalized_lbco(opts),
        }
    }
}

impl<R: Read> Read for LayerBlockCipherHandler<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            LayerBlockCipherHandler::Aes256Ctr(block_cipher) => block_cipher.read(buf),
        }
    }
}

#[cfg(feature = "async-io")]
impl<R: tokio::io::AsyncRead> tokio::io::AsyncRead for LayerBlockCipherHandler<R> {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // This is okay because `block_cipher` is pinned when `self` is.
        let aes_ctr_256 = unsafe {
            self.map_unchecked_mut(|v| match v {
                LayerBlockCipherHandler::Aes256Ctr(block_cipher) => block_cipher,
            })
        };
        aes_ctr_256.poll_read(cx, buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_block_cipher_handler() {
        let layer_data: Vec<u8> = b"this is some data".to_vec();

        let mut lbco = LayerBlockCipherOptions::default();
        let mut lbch = LayerBlockCipherHandler::new().unwrap();
        assert!(lbch
            .encrypt(layer_data.as_slice(), AES256CTR, &mut lbco)
            .is_ok());

        let mut encrypted_data: Vec<u8> = Vec::new();
        assert!(lbch
            .encrypt(layer_data.as_slice(), AES256CTR, &mut lbco)
            .is_ok());
        let LayerBlockCipherHandler::Aes256Ctr(mut encryptor) = lbch;
        assert!(encryptor.read_to_end(&mut encrypted_data).is_ok());
        assert!(encryptor.finalized_lbco(&mut lbco).is_ok());

        let serialized_json = serde_json::to_string(&lbco).unwrap();

        // Decrypt with valid key
        let mut lbch = LayerBlockCipherHandler::new().unwrap();
        let mut lbco: LayerBlockCipherOptions =
            serde_json::from_str(&serialized_json).unwrap_or_default();

        assert!(lbch.decrypt(encrypted_data.as_slice(), &mut lbco).is_ok());
        let LayerBlockCipherHandler::Aes256Ctr(mut decryptor) = lbch;
        let mut plaintxt_data: Vec<u8> = Vec::new();
        assert!(decryptor.read_to_end(&mut plaintxt_data).is_ok());

        // Decrypted data should equal to original data
        assert_eq!(layer_data, plaintxt_data);

        // Decrypt with invalid key
        let mut lbch = LayerBlockCipherHandler::new().unwrap();
        lbco.private.symmetric_key = vec![0; 32];
        assert!(lbch.decrypt(encrypted_data.as_slice(), &mut lbco).is_ok());
        let LayerBlockCipherHandler::Aes256Ctr(mut decryptor) = lbch;
        let mut plaintxt_data: Vec<u8> = Vec::new();
        assert!(decryptor.read_to_end(&mut plaintxt_data).is_err());
    }
}
