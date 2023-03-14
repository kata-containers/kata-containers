//! AES block cipher constant-time implementation.
//!
//! The `aes-soft` crate implements the AES algorithm completely in software
//! without using any table lookups or other timing dependant mechanisms.
//!
//! The encryption implementation is based on a technique called [fixslicing][1],
//! while the decryption implementation is heavily based on `aessafe` [module][2],
//! from the original `rust-crypto` crate.
//!
//! # Usage example
//! ```
//! use aes_soft::cipher::generic_array::GenericArray;
//! use aes_soft::cipher::{BlockCipher, NewBlockCipher};
//! use aes_soft::Aes128;
//!
//! let key = GenericArray::from_slice(&[0u8; 16]);
//! let mut block = GenericArray::clone_from_slice(&[0u8; 16]);
//! let mut block8 = GenericArray::clone_from_slice(&[block; 8]);
//! // Initialize cipher
//! let cipher = aes_soft::Aes128::new(&key);
//!
//! let block_copy = block.clone();
//! // Encrypt block in-place
//! cipher.encrypt_block(&mut block);
//! // And decrypt it back
//! cipher.decrypt_block(&mut block);
//! assert_eq!(block, block_copy);
//!
//! // We can encrypt 8 blocks simultaneously using
//! // instruction-level parallelism
//! let block8_copy = block8.clone();
//! cipher.encrypt_blocks(&mut block8);
//! cipher.decrypt_blocks(&mut block8);
//! assert_eq!(block8, block8_copy);
//! ```
//!
//! [1]: https://eprint.iacr.org/2020/1123.pdf
//! [2]: https://github.com/DaGenix/rust-crypto/blob/master/src/aessafe.rs

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg"
)]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg_attr(not(target_pointer_width = "64"), path = "fixslice32.rs")]
#[cfg_attr(target_pointer_width = "64", path = "fixslice64.rs")]
mod fixslice;
mod impls;

pub use crate::impls::{Aes128, Aes192, Aes256};
pub use cipher;

/// 128-bit AES block
pub type Block = cipher::generic_array::GenericArray<u8, cipher::consts::U16>;

/// 8 x 128-bit AES blocks to be processed in parallel
pub type ParBlocks = cipher::generic_array::GenericArray<Block, cipher::consts::U8>;
