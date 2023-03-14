//! EAX: [Authenticated Encryption and Associated Data (AEAD)][1] cipher
//! based on AES in counter mode.
//!
//! # Usage
//!
//! Simple usage (allocating, no associated data):
//!
//! ```
//! use aes::Aes256;
//! use eax::Eax;
//! use eax::aead::{Aead, NewAead, generic_array::GenericArray};
//!
//! let key = GenericArray::from_slice(b"an example very very secret key.");
//! let cipher = Eax::<Aes256>::new(key);
//!
//! let nonce = GenericArray::from_slice(b"my unique nonces"); // 128-bits; unique per message
//!
//! let ciphertext = cipher.encrypt(nonce, b"plaintext message".as_ref())
//!     .expect("encryption failure!"); // NOTE: handle this error to avoid panics!
//!
//! let plaintext = cipher.decrypt(nonce, ciphertext.as_ref())
//!     .expect("decryption failure!"); // NOTE: handle this error to avoid panics!
//!
//! assert_eq!(&plaintext, b"plaintext message");
//! ```
//!
//! ## In-place Usage (eliminates `alloc` requirement)
//!
//! This crate has an optional `alloc` feature which can be disabled in e.g.
//! microcontroller environments that don't have a heap.
//!
//! The [`AeadInPlace::encrypt_in_place`] and [`AeadInPlace::decrypt_in_place`]
//! methods accept any type that impls the [`aead::Buffer`] trait which
//! contains the plaintext for encryption or ciphertext for decryption.
//!
//! Note that if you enable the `heapless` feature of this crate,
//! you will receive an impl of [`aead::Buffer`] for `heapless::Vec`
//! (re-exported from the [`aead`] crate as [`aead::heapless::Vec`]),
//! which can then be passed as the `buffer` parameter to the in-place encrypt
//! and decrypt methods:
//!
//! ```
//! # #[cfg(feature = "heapless")]
//! # {
//! use aes::Aes256;
//! use eax::Eax;
//! use eax::aead::{AeadInPlace, NewAead, generic_array::GenericArray};
//! use eax::aead::heapless::{Vec, consts::U128};
//!
//! let key = GenericArray::from_slice(b"an example very very secret key.");
//! let cipher = Eax::<Aes256>::new(key);
//!
//! let nonce = GenericArray::from_slice(b"my unique nonces"); // 128-bits; unique per message
//!
//! let mut buffer: Vec<u8, U128> = Vec::new();
//! buffer.extend_from_slice(b"plaintext message");
//!
//! // Encrypt `buffer` in-place, replacing the plaintext contents with ciphertext
//! cipher.encrypt_in_place(nonce, b"", &mut buffer).expect("encryption failure!");
//!
//! // `buffer` now contains the message ciphertext
//! assert_ne!(&buffer, b"plaintext message");
//!
//! // Decrypt `buffer` in-place, replacing its ciphertext context with the original plaintext
//! cipher.decrypt_in_place(nonce, b"", &mut buffer).expect("decryption failure!");
//! assert_eq!(&buffer, b"plaintext message");
//! # }
//! ```
//!
//! [1]: https://en.wikipedia.org/wiki/Authenticated_encryption

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg"
)]
#![deny(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub use aead::{self, AeadInPlace, Error, NewAead, Nonce};
pub use cipher;

use cipher::{
    block::{Block, BlockCipher, Key, NewBlockCipher},
    consts::{U0, U16},
    generic_array::{functional::FunctionalSequence, ArrayLength, GenericArray},
    stream::{FromBlockCipher, SyncStreamCipher},
};
use cmac::{crypto_mac::NewMac, Cmac, Mac};

// TODO Max values?
/// Maximum length of associated data
pub const A_MAX: u64 = 1 << 36;

/// Maximum length of plaintext
pub const P_MAX: u64 = 1 << 36;

/// Maximum length of ciphertext
pub const C_MAX: u64 = (1 << 36) + 16;

/// EAX tags
pub type Tag = GenericArray<u8, U16>;

pub mod online;

/// EAX: generic over an underlying block cipher implementation.
///
/// This type is generic to support substituting alternative cipher
/// implementations.
///
/// If in doubt, use the built-in [`Aes128Eax`] and [`Aes256Eax`] type aliases.
#[derive(Clone)]
pub struct Eax<Cipher>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
{
    /// Encryption key
    key: Key<Cipher>,
}

impl<Cipher> NewAead for Eax<Cipher>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
{
    type KeySize = Cipher::KeySize;

    fn new(key: &Key<Cipher>) -> Self {
        Self { key: key.clone() }
    }
}

impl<Cipher> AeadInPlace for Eax<Cipher>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
{
    type NonceSize = Cipher::BlockSize;
    type TagSize = <Cmac<Cipher> as Mac>::OutputSize;
    type CiphertextOverhead = U0;

    fn encrypt_in_place_detached(
        &self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut [u8],
    ) -> Result<Tag, Error> {
        if buffer.len() as u64 > P_MAX || associated_data.len() as u64 > A_MAX {
            return Err(Error);
        }

        // https://crypto.stackexchange.com/questions/26948/eax-cipher-mode-with-nonce-equal-header
        // has an explanation of eax.

        // l = block cipher size = 128 (for AES-128) = 16 byte
        // 1. n ← OMAC(0 || Nonce)
        // (the 0 means the number zero in l bits)
        let n = Self::cmac_with_iv(&self.key, 0, nonce);

        // 2. h ← OMAC(1 || associated data)
        let h = Self::cmac_with_iv(&self.key, 1, associated_data);

        // 3. enc ← CTR(M) using n as iv
        let mut cipher = ctr::Ctr128::<Cipher>::from_block_cipher(Cipher::new(&self.key), &n);
        cipher.apply_keystream(buffer);

        // 4. c ← OMAC(2 || enc)
        let c = Self::cmac_with_iv(&self.key, 2, buffer);

        // 5. tag ← n ^ h ^ c
        // (^ means xor)
        Ok(n.zip(h, |a, b| a ^ b).zip(c, |a, b| a ^ b))
    }

    fn decrypt_in_place_detached(
        &self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut [u8],
        tag: &Tag,
    ) -> Result<(), Error> {
        if buffer.len() as u64 > C_MAX || associated_data.len() as u64 > A_MAX {
            return Err(Error);
        }

        // 1. n ← OMAC(0 || Nonce)
        let n = Self::cmac_with_iv(&self.key, 0, nonce);

        // 2. h ← OMAC(1 || associated data)
        let h = Self::cmac_with_iv(&self.key, 1, associated_data);

        // 4. c ← OMAC(2 || enc)
        let c = Self::cmac_with_iv(&self.key, 2, buffer);

        // 5. tag ← n ^ h ^ c
        // (^ means xor)
        let expected_tag = n.zip(h, |a, b| a ^ b).zip(c, |a, b| a ^ b);

        let expected_tag = &expected_tag[..tag.len()];

        // Check mac using secure comparison
        use subtle::ConstantTimeEq;
        if expected_tag.ct_eq(tag).unwrap_u8() == 1 {
            // Decrypt
            let mut cipher = ctr::Ctr128::<Cipher>::from_block_cipher(Cipher::new(&self.key), &n);
            cipher.apply_keystream(buffer);
            Ok(())
        } else {
            Err(Error)
        }
    }
}

impl<Cipher> Eax<Cipher>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
{
    /// CMAC/OMAC1
    ///
    /// To avoid constructing new buffers on the heap, an iv encoded into 16
    /// bytes is prepended inside this function.
    fn cmac_with_iv(
        key: &GenericArray<u8, Cipher::KeySize>,
        iv: u8,
        data: &[u8],
    ) -> GenericArray<u8, <Cmac<Cipher> as Mac>::OutputSize> {
        let mut mac = Cmac::<Cipher>::new(key);
        mac.update(&[0; 15]);
        mac.update(&[iv]);
        mac.update(data);

        mac.finalize().into_bytes()
    }
}
