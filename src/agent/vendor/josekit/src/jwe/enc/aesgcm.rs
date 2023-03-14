use std::fmt::Display;
use std::ops::Deref;

use anyhow::bail;
use openssl::symm::{self, Cipher};

use crate::jwe::JweContentEncryption;
use crate::JoseError;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum AesgcmJweEncryption {
    /// AES GCM using 128-bit key
    A128gcm,
    /// AES GCM using 192-bit key
    A192gcm,
    /// AES GCM using 256-bit key
    A256gcm,
}

impl AesgcmJweEncryption {
    fn cipher(&self) -> Cipher {
        match self {
            Self::A128gcm => Cipher::aes_128_gcm(),
            Self::A192gcm => Cipher::aes_192_gcm(),
            Self::A256gcm => Cipher::aes_256_gcm(),
        }
    }
}

impl JweContentEncryption for AesgcmJweEncryption {
    fn name(&self) -> &str {
        match self {
            Self::A128gcm => "A128GCM",
            Self::A192gcm => "A192GCM",
            Self::A256gcm => "A256GCM",
        }
    }

    fn key_len(&self) -> usize {
        match self {
            Self::A128gcm => 16,
            Self::A192gcm => 24,
            Self::A256gcm => 32,
        }
    }

    fn iv_len(&self) -> usize {
        12
    }

    fn encrypt(
        &self,
        key: &[u8],
        iv: Option<&[u8]>,
        message: &[u8],
        aad: &[u8],
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), JoseError> {
        (|| -> anyhow::Result<(Vec<u8>, Option<Vec<u8>>)> {
            let expected_len = self.key_len();
            if key.len() != expected_len {
                bail!(
                    "The length of content encryption key must be {}: {}",
                    expected_len,
                    key.len()
                );
            }

            let cipher = self.cipher();
            let mut tag = [0; 16];
            let encrypted_message = symm::encrypt_aead(cipher, key, iv, aad, message, &mut tag)?;
            Ok((encrypted_message, Some(tag.to_vec())))
        })()
        .map_err(|err| JoseError::InvalidKeyFormat(err))
    }

    fn decrypt(
        &self,
        key: &[u8],
        iv: Option<&[u8]>,
        encrypted_message: &[u8],
        aad: &[u8],
        tag: Option<&[u8]>,
    ) -> Result<Vec<u8>, JoseError> {
        (|| -> anyhow::Result<Vec<u8>> {
            let expected_len = self.key_len();
            if key.len() != expected_len {
                bail!(
                    "The length of content encryption key must be {}: {}",
                    expected_len,
                    key.len()
                );
            }

            let tag = match tag {
                Some(val) => val,
                None => bail!("A tag value is required."),
            };

            let cipher = self.cipher();
            let message = symm::decrypt_aead(cipher, key, iv, aad, encrypted_message, tag)?;
            Ok(message)
        })()
        .map_err(|err| JoseError::InvalidJweFormat(err))
    }

    fn box_clone(&self) -> Box<dyn JweContentEncryption> {
        Box::new(self.clone())
    }
}

impl Display for AesgcmJweEncryption {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        fmt.write_str(self.name())
    }
}

impl Deref for AesgcmJweEncryption {
    type Target = dyn JweContentEncryption;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::AesgcmJweEncryption;
    use crate::util;

    #[test]
    fn encrypt_and_decrypt_aes_gcm() -> Result<()> {
        let message = b"abcde12345";
        let aad = b"test";

        for enc in vec![
            AesgcmJweEncryption::A128gcm,
            AesgcmJweEncryption::A192gcm,
            AesgcmJweEncryption::A256gcm,
        ] {
            let key = util::random_bytes(enc.key_len());
            let iv = util::random_bytes(enc.iv_len());

            let (encrypted_message, tag) = enc.encrypt(&key, Some(&iv), message, aad)?;
            let decrypted_message = enc.decrypt(
                &key,
                Some(&iv),
                &encrypted_message,
                &aad[..],
                tag.as_deref(),
            )?;

            assert_eq!(&message[..], &decrypted_message[..]);
        }

        Ok(())
    }
}
