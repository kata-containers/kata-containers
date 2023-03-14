//! This crate is a wrapper around different implementations of AES block ciphers.
//!
//! Currently it uses:
//! - [`aes-soft`](https://docs.rs/aes-soft) hardware independent bit-sliced
//! implementation
//! - [`aesni`](https://docs.rs/aesni) implementation using
//! [AES-NI](https://en.wikipedia.org/wiki/AES_instruction_set) instruction set.
//! Used for x86-64 and x86 target architectures with enabled `aes` and `sse2`
//! target features (the latter is usually enabled by default).
//!
//! Crate switches between implementations automatically at compile time.
//! (i.e. it does not use run-time feature detection)
//!
//! # Usage example
//! ```
//! use aes::cipher::generic_array::GenericArray;
//! use aes::cipher::{BlockCipher, NewBlockCipher};
//! use aes::Aes128;
//!
//! let key = GenericArray::from_slice(&[0u8; 16]);
//! let mut block = GenericArray::clone_from_slice(&[0u8; 16]);
//! let mut block8 = GenericArray::clone_from_slice(&[block; 8]);
//! // Initialize cipher
//! let cipher = Aes128::new(&key);
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
//! For implementations of block cipher modes of operation see
//! [`block-modes`](https://docs.rs/block-modes) crate.

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg"
)]
#![deny(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

pub use cipher::{self, BlockCipher, NewBlockCipher};

#[cfg(not(all(
    target_feature = "aes",
    target_feature = "sse2",
    any(target_arch = "x86_64", target_arch = "x86"),
)))]
pub use aes_soft::{Aes128, Aes192, Aes256};

#[cfg(all(
    target_feature = "aes",
    target_feature = "sse2",
    any(target_arch = "x86_64", target_arch = "x86"),
))]
pub use aesni::{Aes128, Aes192, Aes256};
