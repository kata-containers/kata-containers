//! Implementation of AEAD using pure Rust cryptographic libraries.

use std::cmp;

use cipher::{BlockCipher, NewBlockCipher};
use cipher::block::Block;
use cipher::consts::U16;
use eax::online::{Eax, Encrypt, Decrypt};
use generic_array::{ArrayLength, GenericArray};

use crate::{Error, Result};
use crate::crypto::aead::{Aead, CipherOp};
use crate::seal;
use crate::types::{AEADAlgorithm, SymmetricAlgorithm};

trait GenericArrayExt<T, N: ArrayLength<T>> {
    const LEN: usize;

    /// Like [`GenericArray::from_slice`], but fallible.
    fn try_from_slice(slice: &[T]) -> Result<&GenericArray<T, N>> {
        if slice.len() == Self::LEN {
            Ok(GenericArray::from_slice(slice))
        } else {
            Err(Error::InvalidArgument(
                format!("Invalid slice length, want {}, got {}",
                        Self::LEN, slice.len())).into())
        }
    }
}

impl<T, N: ArrayLength<T>> GenericArrayExt<T, N> for GenericArray<T, N> {
    const LEN: usize = N::USIZE;
}

impl<Cipher> Aead for Eax<Cipher, Encrypt>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
{
    fn update(&mut self, ad: &[u8]) {
        self.update_assoc(ad)
    }

    fn digest_size(&self) -> usize {
        eax::Tag::LEN
    }

    fn digest(&mut self, digest: &mut [u8]) {
        let tag = self.tag_clone();
        digest[..tag.len()].copy_from_slice(&tag[..]);
    }

    fn encrypt(&mut self, dst: &mut [u8], src: &[u8]) {
        let len = cmp::min(dst.len(), src.len());
        dst[..len].copy_from_slice(&src[..len]);
        Self::encrypt(self, &mut dst[..len])
    }

    fn decrypt(&mut self, _dst: &mut [u8], _src: &[u8]) {
        panic!("AEAD decryption called in the encryption context")
    }
}

impl<Cipher> Aead for Eax<Cipher, Decrypt>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
{
    fn update(&mut self, ad: &[u8]) {
        self.update_assoc(ad)
    }

    fn digest_size(&self) -> usize {
        eax::Tag::LEN
    }

    fn digest(&mut self, digest: &mut [u8]) {
        let tag = self.tag_clone();
        digest[..tag.len()].copy_from_slice(&tag[..]);
    }

    fn encrypt(&mut self, _dst: &mut [u8], _src: &[u8]) {
        panic!("AEAD encryption called in the decryption context")
    }

    fn decrypt(&mut self, dst: &mut [u8], src: &[u8]) {
        let len = core::cmp::min(dst.len(), src.len());
        dst[..len].copy_from_slice(&src[..len]);
        self.decrypt_unauthenticated_hazmat(&mut dst[..len])
    }
}

impl<Cipher, Op> seal::Sealed for Eax<Cipher, Op>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
    Op: eax::online::CipherOp,
{}

impl AEADAlgorithm {
    pub(crate) fn context(
        &self,
        sym_algo: SymmetricAlgorithm,
        key: &[u8],
        nonce: &[u8],
        op: CipherOp,
    ) -> Result<Box<dyn Aead>> {
        match self {
            AEADAlgorithm::EAX => match sym_algo {
                SymmetricAlgorithm::AES128 => match op {
                    CipherOp::Encrypt => Ok(Box::new(
                        Eax::<aes::Aes128, Encrypt>::with_key_and_nonce(
                            GenericArray::try_from_slice(key)?,
                            GenericArray::try_from_slice(nonce)?))),
                    CipherOp::Decrypt => Ok(Box::new(
                        Eax::<aes::Aes128, Decrypt>::with_key_and_nonce(
                            GenericArray::try_from_slice(key)?,
                            GenericArray::try_from_slice(nonce)?))),
                }
                SymmetricAlgorithm::AES192 => match op {
                    CipherOp::Encrypt => Ok(Box::new(
                        Eax::<aes::Aes192, Encrypt>::with_key_and_nonce(
                            GenericArray::try_from_slice(key)?,
                            GenericArray::try_from_slice(nonce)?))),
                    CipherOp::Decrypt => Ok(Box::new(
                        Eax::<aes::Aes192, Decrypt>::with_key_and_nonce(
                            GenericArray::try_from_slice(key)?,
                            GenericArray::try_from_slice(nonce)?))),
                }
                SymmetricAlgorithm::AES256 => match op {
                    CipherOp::Encrypt => Ok(Box::new(
                        Eax::<aes::Aes256, Encrypt>::with_key_and_nonce(
                            GenericArray::try_from_slice(key)?,
                            GenericArray::try_from_slice(nonce)?))),
                    CipherOp::Decrypt => Ok(Box::new(
                        Eax::<aes::Aes256, Decrypt>::with_key_and_nonce(
                            GenericArray::try_from_slice(key)?,
                            GenericArray::try_from_slice(nonce)?))),
                }
                | SymmetricAlgorithm::IDEA
                | SymmetricAlgorithm::TripleDES
                | SymmetricAlgorithm::CAST5
                | SymmetricAlgorithm::Blowfish
                | SymmetricAlgorithm::Twofish
                | SymmetricAlgorithm::Camellia128
                | SymmetricAlgorithm::Camellia192
                | SymmetricAlgorithm::Camellia256
                | SymmetricAlgorithm::Private(_)
                | SymmetricAlgorithm::Unknown(_)
                | SymmetricAlgorithm::Unencrypted =>
                    Err(Error::UnsupportedSymmetricAlgorithm(sym_algo).into()),
            },
            AEADAlgorithm::OCB | AEADAlgorithm::Private(_) | AEADAlgorithm::Unknown(_) =>
                Err(Error::UnsupportedAEADAlgorithm(*self).into()),
        }
    }
}
