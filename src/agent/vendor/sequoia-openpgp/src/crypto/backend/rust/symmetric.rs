use std::slice;

use block_modes::{BlockMode, Cfb, Ecb};
use block_padding::ZeroPadding;
use cipher::{BlockCipher, NewBlockCipher};
use generic_array::{ArrayLength, GenericArray};
use typenum::Unsigned;

use crate::{Error, Result};
use crate::crypto::symmetric::Mode;
use crate::types::SymmetricAlgorithm;

macro_rules! impl_mode {
    ($mode:ident) => {
        impl<C> Mode for $mode<C, ZeroPadding>
        where
            C: BlockCipher + NewBlockCipher + Send + Sync,
        {
            fn block_size(&self) -> usize {
                C::BlockSize::to_usize()
            }

            fn encrypt(
                &mut self,
                dst: &mut [u8],
                src: &[u8],
            ) -> Result<()> {
                debug_assert_eq!(dst.len(), src.len());
                let bs = self.block_size();
                let missing = (bs - (dst.len() % bs)) % bs;
                if missing > 0 {
                    let mut buf = vec![0u8; src.len() + missing];
                    buf[..src.len()].copy_from_slice(src);
                    self.encrypt_blocks(to_blocks(&mut buf));
                    dst.copy_from_slice(&buf[..dst.len()]);
                } else {
                    dst.copy_from_slice(src);
                    self.encrypt_blocks(to_blocks(dst));
                }
                Ok(())
            }

            fn decrypt(
                &mut self,
                dst: &mut [u8],
                src: &[u8],
            ) -> Result<()> {
                debug_assert_eq!(dst.len(), src.len());
                let bs = self.block_size();
                let missing = (bs - (dst.len() % bs)) % bs;
                if missing > 0 {
                    let mut buf = vec![0u8; src.len() + missing];
                    buf[..src.len()].copy_from_slice(src);
                    self.decrypt_blocks(to_blocks(&mut buf));
                    dst.copy_from_slice(&buf[..dst.len()]);
                } else {
                    dst.copy_from_slice(src);
                    self.decrypt_blocks(to_blocks(dst));
                }
                Ok(())
            }
        }
    }
}

impl_mode!(Cfb);
impl_mode!(Ecb);

fn to_blocks<N>(data: &mut [u8]) -> &mut [GenericArray<u8, N>]
where
    N: ArrayLength<u8>,
{
    let n = N::to_usize();
    debug_assert!(data.len() % n == 0);
    unsafe {
        slice::from_raw_parts_mut(data.as_ptr() as *mut GenericArray<u8, N>, data.len() / n)
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
    /// assert!(SymmetricAlgorithm::IDEA.is_supported());
    ///
    /// assert!(!SymmetricAlgorithm::Unencrypted.is_supported());
    /// assert!(!SymmetricAlgorithm::Private(101).is_supported());
    /// ```
    pub fn is_supported(&self) -> bool {
        use SymmetricAlgorithm::*;
        match self {
            IDEA => true,
            TripleDES => true,
            CAST5 => true,
            Blowfish => true,
            AES128 => true,
            AES192 => true,
            AES256 => true,
            Twofish => true,
            Camellia128 => false,
            Camellia192 => false,
            Camellia256 => false,
            Private(_) => false,
            Unknown(_) => false,
            Unencrypted => false,
        }
    }

    /// Length of a key for this algorithm in bytes.
    ///
    /// Fails if Sequoia does not support this algorithm.
    pub fn key_size(self) -> Result<usize> {
        use SymmetricAlgorithm::*;
        match self {
            IDEA => Ok(<idea::Idea as NewBlockCipher>::KeySize::to_usize()),
            TripleDES => Ok(<des::TdesEde2 as NewBlockCipher>::KeySize::to_usize()),
            CAST5 => Ok(<cast5::Cast5 as NewBlockCipher>::KeySize::to_usize()),
            Blowfish => Ok(<blowfish::Blowfish as NewBlockCipher>::KeySize::to_usize()),
            AES128 => Ok(<aes::Aes128 as NewBlockCipher>::KeySize::to_usize()),
            AES192 => Ok(<aes::Aes192 as NewBlockCipher>::KeySize::to_usize()),
            AES256 => Ok(<aes::Aes256 as NewBlockCipher>::KeySize::to_usize()),
            Twofish => Ok(<twofish::Twofish as NewBlockCipher>::KeySize::to_usize()),
            Camellia128 | Camellia192 | Camellia256 | Private(_) | Unknown(_) | Unencrypted =>
                Err(Error::UnsupportedSymmetricAlgorithm(self).into()),
        }
    }

    /// Length of a block for this algorithm in bytes.
    ///
    /// Fails if Sequoia does not support this algorithm.
    pub fn block_size(self) -> Result<usize> {
        use SymmetricAlgorithm::*;
        match self {
            IDEA => Ok(<idea::Idea as BlockCipher>::BlockSize::to_usize()),
            TripleDES => Ok(<des::TdesEde2 as BlockCipher>::BlockSize::to_usize()),
            CAST5 => Ok(<cast5::Cast5 as BlockCipher>::BlockSize::to_usize()),
            Blowfish => Ok(<blowfish::Blowfish as BlockCipher>::BlockSize::to_usize()),
            AES128 => Ok(<aes::Aes128 as BlockCipher>::BlockSize::to_usize()),
            AES192 => Ok(<aes::Aes192 as BlockCipher>::BlockSize::to_usize()),
            AES256 => Ok(<aes::Aes256 as BlockCipher>::BlockSize::to_usize()),
            Twofish => Ok(<twofish::Twofish as BlockCipher>::BlockSize::to_usize()),
            Camellia128 | Camellia192 | Camellia256 | Private(_) | Unknown(_) | Unencrypted =>
                Err(Error::UnsupportedSymmetricAlgorithm(self).into()),
        }
    }

    /// Creates a context for encrypting in CFB mode.
    pub(crate) fn make_encrypt_cfb(self, key: &[u8], iv: Vec<u8>) -> Result<Box<dyn Mode>> {
        use SymmetricAlgorithm::*;
        match self {
            IDEA => Ok(Box::new(Cfb::<idea::Idea, ZeroPadding>::new_var(key, &iv)?)),
            TripleDES => Ok(Box::new(Cfb::<des::TdesEde2, ZeroPadding>::new_var(key, &iv)?)),
            CAST5 => Ok(Box::new(Cfb::<cast5::Cast5, ZeroPadding>::new_var(key, &iv)?)),
            Blowfish => Ok(Box::new(Cfb::<blowfish::Blowfish, ZeroPadding>::new_var(key, &iv)?)),
            AES128 => Ok(Box::new(Cfb::<aes::Aes128, ZeroPadding>::new_var(key, &iv)?)),
            AES192 => Ok(Box::new(Cfb::<aes::Aes192, ZeroPadding>::new_var(key, &iv)?)),
            AES256 => Ok(Box::new(Cfb::<aes::Aes256, ZeroPadding>::new_var(key, &iv)?)),
            Twofish => Ok(Box::new(Cfb::<twofish::Twofish, ZeroPadding>::new_var(key, &iv)?)),
            Camellia128 | Camellia192 | Camellia256 | Private(_) | Unknown(_) | Unencrypted =>
                Err(Error::UnsupportedSymmetricAlgorithm(self).into()),
        }
    }

    /// Creates a context for decrypting in CFB mode.
    pub(crate) fn make_decrypt_cfb(self, key: &[u8], iv: Vec<u8>) -> Result<Box<dyn Mode>> {
        self.make_encrypt_cfb(key, iv)
    }

    /// Creates a context for encrypting in ECB mode.
    pub(crate) fn make_encrypt_ecb(self, key: &[u8]) -> Result<Box<dyn Mode>> {
        use SymmetricAlgorithm::*;
        match self {
            IDEA => Ok(Box::new(Ecb::<idea::Idea, ZeroPadding>::new_var(key, &[])?)),
            TripleDES => Ok(Box::new(Ecb::<des::TdesEde2, ZeroPadding>::new_var(key, &[])?)),
            CAST5 => Ok(Box::new(Ecb::<cast5::Cast5, ZeroPadding>::new_var(key, &[])?)),
            Blowfish => Ok(Box::new(Ecb::<blowfish::Blowfish, ZeroPadding>::new_var(key, &[])?)),
            AES128 => Ok(Box::new(Ecb::<aes::Aes128, ZeroPadding>::new_var(key, &[])?)),
            AES192 => Ok(Box::new(Ecb::<aes::Aes192, ZeroPadding>::new_var(key, &[])?)),
            AES256 => Ok(Box::new(Ecb::<aes::Aes256, ZeroPadding>::new_var(key, &[])?)),
            Twofish => Ok(Box::new(Ecb::<twofish::Twofish, ZeroPadding>::new_var(key, &[])?)),
            Camellia128 | Camellia192 | Camellia256 | Private(_) | Unknown(_) | Unencrypted =>
                Err(Error::UnsupportedSymmetricAlgorithm(self).into()),
        }
    }

    /// Creates a context for decrypting in ECB mode.
    pub(crate) fn make_decrypt_ecb(self, key: &[u8]) -> Result<Box<dyn Mode>> {
        self.make_encrypt_ecb(key)
    }
}
