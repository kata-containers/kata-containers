use std::convert::TryFrom;
use std::sync::Mutex;

use win_crypto_ng::symmetric as cng;

use crate::crypto::mem::Protected;
use crate::crypto::symmetric::Mode;

use crate::{Error, Result};
use crate::types::SymmetricAlgorithm;

struct KeyWrapper {
    key: Mutex<cng::SymmetricAlgorithmKey>,
    iv: Option<Protected>,
}

impl KeyWrapper {
    fn new(key: cng::SymmetricAlgorithmKey, iv: Option<Vec<u8>>) -> KeyWrapper {
        KeyWrapper {
            key: Mutex::new(key),
            iv: iv.map(|iv| iv.into()),
        }
    }
}

impl Mode for KeyWrapper {
    fn block_size(&self) -> usize {
        self.key.lock().expect("Mutex not to be poisoned")
            .block_size().expect("CNG not to fail internally")
    }

    fn encrypt(
        &mut self,
        dst: &mut [u8],
        src: &[u8],
    ) -> Result<()> {
        let block_size = Mode::block_size(self);
        // If necessary, round up to the next block size and pad with zeroes
        // NOTE: In theory CFB doesn't need this but CNG always requires
        // passing full blocks.
        let mut _src = vec![];
        let missing = (block_size - (src.len() % block_size)) % block_size;
        let src = if missing != 0 {
            _src = vec![0u8; src.len() + missing];
            _src[..src.len()].copy_from_slice(src);
            &_src
        } else {
            src
        };

        let len = std::cmp::min(src.len(), dst.len());
        let buffer = cng::SymmetricAlgorithmKey::encrypt(
            &*self.key.lock().expect("Mutex not to be poisoned"),
            self.iv.as_deref_mut(), src, None)?;
        Ok(dst[..len].copy_from_slice(&buffer.as_slice()[..len]))
    }

    fn decrypt(
        &mut self,
        dst: &mut [u8],
        src: &[u8],
    ) -> Result<()> {
        let block_size = Mode::block_size(self);
        // If necessary, round up to the next block size and pad with zeroes
        // NOTE: In theory CFB doesn't need this but CNG always requires
        // passing full blocks.
        let mut _src = vec![];
        let missing = (block_size - (src.len() % block_size)) % block_size;
        let src = if missing != 0 {
            _src = vec![0u8; src.len() + missing];
            _src[..src.len()].copy_from_slice(src);
            &_src
        } else {
            src
        };

        let len = std::cmp::min(src.len(), dst.len());
        let buffer = cng::SymmetricAlgorithmKey::decrypt(
            &*self.key.lock().expect("Mutex not to be poisoned"),
            self.iv.as_deref_mut(), src, None)?;
        dst[..len].copy_from_slice(&buffer.as_slice()[..len]);

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Unsupported algorithm: {0}")]
pub struct UnsupportedAlgorithm(SymmetricAlgorithm);
assert_send_and_sync!(UnsupportedAlgorithm);

impl From<UnsupportedAlgorithm> for Error {
    fn from(value: UnsupportedAlgorithm) -> Error {
        Error::UnsupportedSymmetricAlgorithm(value.0)
    }
}

impl TryFrom<SymmetricAlgorithm> for (cng::SymmetricAlgorithmId, usize) {
    type Error = UnsupportedAlgorithm;
    fn try_from(value: SymmetricAlgorithm) -> std::result::Result<Self, Self::Error> {
        Ok(match value {
            SymmetricAlgorithm::TripleDES => (cng::SymmetricAlgorithmId::TripleDes, 168),
            SymmetricAlgorithm::AES128 => (cng::SymmetricAlgorithmId::Aes, 128),
            SymmetricAlgorithm::AES192 => (cng::SymmetricAlgorithmId::Aes, 192),
            SymmetricAlgorithm::AES256 => (cng::SymmetricAlgorithmId::Aes, 256),
            algo => Err(UnsupportedAlgorithm(algo))?,
        })
    }
}

impl SymmetricAlgorithm {
    /// Returns whether this algorithm is supported by the crypto backend.
    ///
    /// All backends support all the AES variants.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::SymmetricAlgorithm;
    ///
    /// assert!(SymmetricAlgorithm::AES256.is_supported());
    /// assert!(SymmetricAlgorithm::TripleDES.is_supported());
    ///
    /// assert!(!SymmetricAlgorithm::IDEA.is_supported());
    /// assert!(!SymmetricAlgorithm::Unencrypted.is_supported());
    /// assert!(!SymmetricAlgorithm::Private(101).is_supported());
    /// ```
    pub fn is_supported(&self) -> bool {
        use self::SymmetricAlgorithm::*;
        match self {
            AES128 | AES192 | AES256 | TripleDES => true,
            _ => false,
        }
    }

    /// Length of a key for this algorithm in bytes.
    ///
    /// Fails if the crypto backend does not support this algorithm.
    pub fn key_size(self) -> Result<usize> {
        Ok(match self {
            SymmetricAlgorithm::TripleDES => 24,
            SymmetricAlgorithm::AES128 => 16,
            SymmetricAlgorithm::AES192 => 24,
            SymmetricAlgorithm::AES256 => 32,
            _ => Err(UnsupportedAlgorithm(self))?,
        })
    }

    /// Length of a block for this algorithm in bytes.
    ///
    /// Fails if the crypto backend does not support this algorithm.
    pub fn block_size(self) -> Result<usize> {
        Ok(match self {
            SymmetricAlgorithm::TripleDES => 8,
            SymmetricAlgorithm::AES128 => 16,
            SymmetricAlgorithm::AES192 => 16,
            SymmetricAlgorithm::AES256 => 16,
            _ => Err(UnsupportedAlgorithm(self))?,
        })
    }

    /// Creates a symmetric cipher context for encrypting in CFB mode.
    pub(crate) fn make_encrypt_cfb(self, key: &[u8], iv: Vec<u8>) -> Result<Box<dyn Mode>> {
        let (algo, _) = TryFrom::try_from(self)?;

        let algo = cng::SymmetricAlgorithm::open(algo, cng::ChainingMode::Cfb)?;
        let mut key = algo.new_key(key)?;
        // Use full-block CFB mode as expected everywhere else (by default it's
        // set to 8-bit CFB)
        key.set_msg_block_len(key.block_size()?)?;

        Ok(Box::new(KeyWrapper::new(key, Some(iv))))
    }

    /// Creates a symmetric cipher context for decrypting in CFB mode.
    pub(crate) fn make_decrypt_cfb(self, key: &[u8], iv: Vec<u8>) -> Result<Box<dyn Mode>> {
        Self::make_encrypt_cfb(self, key, iv)
    }

    /// Creates a symmetric cipher context for encrypting in ECB mode.
    pub(crate) fn make_encrypt_ecb(self, key: &[u8]) -> Result<Box<dyn Mode>> {
        let (algo, _) = TryFrom::try_from(self)?;

        let algo = cng::SymmetricAlgorithm::open(algo, cng::ChainingMode::Ecb)?;
        let key = algo.new_key(key)?;

        Ok(Box::new(KeyWrapper::new(key, None)))
    }

    /// Creates a symmetric cipher context for decrypting in ECB mode.
    pub(crate) fn make_decrypt_ecb(self, key: &[u8]) -> Result<Box<dyn Mode>> {
        Self::make_encrypt_ecb(self, key)
    }
}
