use nettle::cipher::{self, Cipher};
use nettle::mode::{self};

use crate::crypto::mem::Protected;
use crate::crypto::symmetric::Mode;

use crate::{Error, Result};
use crate::types::SymmetricAlgorithm;

struct ModeWrapper<M>
{
    mode: M,
    iv: Protected,
}

#[allow(clippy::new_ret_no_self)]
impl<M> ModeWrapper<M>
where
    M: nettle::mode::Mode + Send + Sync + 'static,
{
    fn new(mode: M, iv: Vec<u8>) -> Box<dyn Mode> {
        Box::new(ModeWrapper {
            mode,
            iv: iv.into(),
        })
    }
}

impl<M> Mode for ModeWrapper<M>
where
    M: nettle::mode::Mode + Send + Sync,
{
    fn block_size(&self) -> usize {
        self.mode.block_size()
    }

    fn encrypt(
        &mut self,
        dst: &mut [u8],
        src: &[u8],
    ) -> Result<()> {
        self.mode.encrypt(&mut self.iv, dst, src)?;
        Ok(())
    }

    fn decrypt(
        &mut self,
        dst: &mut [u8],
        src: &[u8],
    ) -> Result<()> {
        self.mode.decrypt(&mut self.iv, dst, src)?;
        Ok(())
    }
}

impl<C> Mode for C
where
    C: Cipher + Send + Sync,
{
    fn block_size(&self) -> usize {
        C::BLOCK_SIZE
    }

    fn encrypt(
        &mut self,
        dst: &mut [u8],
        src: &[u8],
    ) -> Result<()> {
        self.encrypt(dst, src);
        Ok(())
    }

    fn decrypt(
        &mut self,
        dst: &mut [u8],
        src: &[u8],
    ) -> Result<()> {
        self.decrypt(dst, src);
        Ok(())
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
        match &self {
            TripleDES | CAST5 | Blowfish | AES128 | AES192 | AES256 | Twofish
                | Camellia128 | Camellia192 | Camellia256
                => true,
            Unencrypted | IDEA | Private(_) | Unknown(_)
                => false,
        }
    }

    /// Length of a key for this algorithm in bytes.
    ///
    /// Fails if Sequoia does not support this algorithm.
    pub fn key_size(self) -> Result<usize> {
        match self {
            SymmetricAlgorithm::TripleDES => Ok(cipher::Des3::KEY_SIZE),
            SymmetricAlgorithm::CAST5 => Ok(cipher::Cast128::KEY_SIZE),
            // RFC4880, Section 9.2: Blowfish (128 bit key, 16 rounds)
            SymmetricAlgorithm::Blowfish => Ok(16),
            SymmetricAlgorithm::AES128 => Ok(cipher::Aes128::KEY_SIZE),
            SymmetricAlgorithm::AES192 => Ok(cipher::Aes192::KEY_SIZE),
            SymmetricAlgorithm::AES256 => Ok(cipher::Aes256::KEY_SIZE),
            SymmetricAlgorithm::Twofish => Ok(cipher::Twofish::KEY_SIZE),
            SymmetricAlgorithm::Camellia128 => Ok(cipher::Camellia128::KEY_SIZE),
            SymmetricAlgorithm::Camellia192 => Ok(cipher::Camellia192::KEY_SIZE),
            SymmetricAlgorithm::Camellia256 => Ok(cipher::Camellia256::KEY_SIZE),
            _ => Err(Error::UnsupportedSymmetricAlgorithm(self).into()),
        }
    }

    /// Length of a block for this algorithm in bytes.
    ///
    /// Fails if Sequoia does not support this algorithm.
    pub fn block_size(self) -> Result<usize> {
        match self {
            SymmetricAlgorithm::TripleDES => Ok(cipher::Des3::BLOCK_SIZE),
            SymmetricAlgorithm::CAST5 => Ok(cipher::Cast128::BLOCK_SIZE),
            SymmetricAlgorithm::Blowfish => Ok(cipher::Blowfish::BLOCK_SIZE),
            SymmetricAlgorithm::AES128 => Ok(cipher::Aes128::BLOCK_SIZE),
            SymmetricAlgorithm::AES192 => Ok(cipher::Aes192::BLOCK_SIZE),
            SymmetricAlgorithm::AES256 => Ok(cipher::Aes256::BLOCK_SIZE),
            SymmetricAlgorithm::Twofish => Ok(cipher::Twofish::BLOCK_SIZE),
            SymmetricAlgorithm::Camellia128 => Ok(cipher::Camellia128::BLOCK_SIZE),
            SymmetricAlgorithm::Camellia192 => Ok(cipher::Camellia192::BLOCK_SIZE),
            SymmetricAlgorithm::Camellia256 => Ok(cipher::Camellia256::BLOCK_SIZE),
            _ => Err(Error::UnsupportedSymmetricAlgorithm(self).into()),
        }
    }

    /// Creates a Nettle context for encrypting in CFB mode.
    pub(crate) fn make_encrypt_cfb(self, key: &[u8], iv: Vec<u8>) -> Result<Box<dyn Mode>> {
        match self {
            SymmetricAlgorithm::TripleDES =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Des3>::with_encrypt_key(key)?, iv)),
            SymmetricAlgorithm::CAST5 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Cast128>::with_encrypt_key(key)?, iv)),
            SymmetricAlgorithm::Blowfish =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Blowfish>::with_encrypt_key(key)?, iv)),
            SymmetricAlgorithm::AES128 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Aes128>::with_encrypt_key(key)?, iv)),
            SymmetricAlgorithm::AES192 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Aes192>::with_encrypt_key(key)?, iv)),
            SymmetricAlgorithm::AES256 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Aes256>::with_encrypt_key(key)?, iv)),
            SymmetricAlgorithm::Twofish =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Twofish>::with_encrypt_key(key)?, iv)),
            SymmetricAlgorithm::Camellia128 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Camellia128>::with_encrypt_key(key)?, iv)),
            SymmetricAlgorithm::Camellia192 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Camellia192>::with_encrypt_key(key)?, iv)),
            SymmetricAlgorithm::Camellia256 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Camellia256>::with_encrypt_key(key)?, iv)),
            _ => Err(Error::UnsupportedSymmetricAlgorithm(self).into()),
        }
    }

    /// Creates a Nettle context for decrypting in CFB mode.
    pub(crate) fn make_decrypt_cfb(self, key: &[u8], iv: Vec<u8>) -> Result<Box<dyn Mode>> {
        match self {
            SymmetricAlgorithm::TripleDES =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Des3>::with_decrypt_key(key)?, iv)),
            SymmetricAlgorithm::CAST5 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Cast128>::with_decrypt_key(key)?, iv)),
            SymmetricAlgorithm::Blowfish =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Blowfish>::with_decrypt_key(key)?, iv)),
            SymmetricAlgorithm::AES128 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Aes128>::with_decrypt_key(key)?, iv)),
            SymmetricAlgorithm::AES192 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Aes192>::with_decrypt_key(key)?, iv)),
            SymmetricAlgorithm::AES256 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Aes256>::with_decrypt_key(key)?, iv)),
            SymmetricAlgorithm::Twofish =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Twofish>::with_decrypt_key(key)?, iv)),
            SymmetricAlgorithm::Camellia128 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Camellia128>::with_decrypt_key(key)?, iv)),
            SymmetricAlgorithm::Camellia192 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Camellia192>::with_decrypt_key(key)?, iv)),
            SymmetricAlgorithm::Camellia256 =>
                Ok(ModeWrapper::new(
                    mode::Cfb::<cipher::Camellia256>::with_decrypt_key(key)?, iv)),
            _ => Err(Error::UnsupportedSymmetricAlgorithm(self).into())
        }
    }

    /// Creates a Nettle context for encrypting in ECB mode.
    pub(crate) fn make_encrypt_ecb(self, key: &[u8]) -> Result<Box<dyn Mode>> {
        match self {
            SymmetricAlgorithm::TripleDES => Ok(Box::new(cipher::Des3::with_encrypt_key(key)?)),
            SymmetricAlgorithm::CAST5 => Ok(Box::new(cipher::Cast128::with_encrypt_key(key)?)),
            SymmetricAlgorithm::Blowfish => Ok(Box::new(cipher::Blowfish::with_encrypt_key(key)?)),
            SymmetricAlgorithm::AES128 => Ok(Box::new(cipher::Aes128::with_encrypt_key(key)?)),
            SymmetricAlgorithm::AES192 => Ok(Box::new(cipher::Aes192::with_encrypt_key(key)?)),
            SymmetricAlgorithm::AES256 => Ok(Box::new(cipher::Aes256::with_encrypt_key(key)?)),
            SymmetricAlgorithm::Twofish => Ok(Box::new(cipher::Twofish::with_encrypt_key(key)?)),
            SymmetricAlgorithm::Camellia128 => Ok(Box::new(cipher::Camellia128::with_encrypt_key(key)?)),
            SymmetricAlgorithm::Camellia192 => Ok(Box::new(cipher::Camellia192::with_encrypt_key(key)?)),
            SymmetricAlgorithm::Camellia256 => Ok(Box::new(cipher::Camellia256::with_encrypt_key(key)?)),
            _ => Err(Error::UnsupportedSymmetricAlgorithm(self).into())
        }
    }

    /// Creates a Nettle context for decrypting in ECB mode.
    pub(crate) fn make_decrypt_ecb(self, key: &[u8]) -> Result<Box<dyn Mode>> {
        match self {
            SymmetricAlgorithm::TripleDES => Ok(Box::new(cipher::Des3::with_decrypt_key(key)?)),
            SymmetricAlgorithm::CAST5 => Ok(Box::new(cipher::Cast128::with_decrypt_key(key)?)),
            SymmetricAlgorithm::Blowfish => Ok(Box::new(cipher::Blowfish::with_decrypt_key(key)?)),
            SymmetricAlgorithm::AES128 => Ok(Box::new(cipher::Aes128::with_decrypt_key(key)?)),
            SymmetricAlgorithm::AES192 => Ok(Box::new(cipher::Aes192::with_decrypt_key(key)?)),
            SymmetricAlgorithm::AES256 => Ok(Box::new(cipher::Aes256::with_decrypt_key(key)?)),
            SymmetricAlgorithm::Twofish => Ok(Box::new(cipher::Twofish::with_decrypt_key(key)?)),
            SymmetricAlgorithm::Camellia128 => Ok(Box::new(cipher::Camellia128::with_decrypt_key(key)?)),
            SymmetricAlgorithm::Camellia192 => Ok(Box::new(cipher::Camellia192::with_decrypt_key(key)?)),
            SymmetricAlgorithm::Camellia256 => Ok(Box::new(cipher::Camellia256::with_decrypt_key(key)?)),
            _ => Err(Error::UnsupportedSymmetricAlgorithm(self).into())
        }
    }
}
