//! Generic implementations of CTR mode for block ciphers.
//!
//! Mode functionality is accessed using traits from re-exported
//! [`cipher`](https://docs.rs/cipher) crate.
//!
//! # ⚠️ Security Warning: [Hazmat!]
//!
//! This crate does not ensure ciphertexts are authentic! Thus ciphertext integrity
//! is not verified, which can lead to serious vulnerabilities!
//!
//! # `Ctr128` Usage Example
//!
//! ```
//! use ctr::cipher::stream::{NewStreamCipher, SyncStreamCipher, SyncStreamCipherSeek};
//!
//! // `aes` crate provides AES block cipher implementation
//! type Aes128Ctr = ctr::Ctr128<aes::Aes128>;
//!
//! let mut data = [1, 2, 3, 4, 5, 6, 7];
//!
//! let key = b"very secret key.";
//! let nonce = b"and secret nonce";
//!
//! // create cipher instance
//! let mut cipher = Aes128Ctr::new(key.into(), nonce.into());
//!
//! // apply keystream (encrypt)
//! cipher.apply_keystream(&mut data);
//! assert_eq!(data, [6, 245, 126, 124, 180, 146, 37]);
//!
//! // seek to the keystream beginning and apply it again to the `data` (decrypt)
//! cipher.seek(0);
//! cipher.apply_keystream(&mut data);
//! assert_eq!(data, [1, 2, 3, 4, 5, 6, 7]);
//! ```
//!
//! [Hazmat!]: https://github.com/RustCrypto/meta/blob/master/HAZMAT.md

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg"
)]
#![warn(missing_docs, rust_2018_idioms)]

mod ctr128;
mod ctr32;

pub use crate::{
    ctr128::Ctr128,
    ctr32::{Ctr32BE, Ctr32LE},
};
pub use cipher;
