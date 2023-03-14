//! This crate contains generic implementation of [block cipher modes of
//! operation][1].
//!
//! Note that some block modes (such as CTR, CFB, and OFB) transform block ciphers
//! into stream ciphers. Implementations in this crate require padding, so if you
//! want use those modes as stream ciphers (i.e. without padding), then check out
//! crates in the [RustCrypto/stream-ciphers][2] repository.
//!
//! # Usage example
//! ```
//! use aes::Aes128;
//! use block_modes::{BlockMode, Cbc};
//! use block_modes::block_padding::Pkcs7;
//! use hex_literal::hex;
//!
//! // create an alias for convenience
//! type Aes128Cbc = Cbc<Aes128, Pkcs7>;
//!
//! # fn main() {
//! let key = hex!("000102030405060708090a0b0c0d0e0f");
//! let iv = hex!("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff");
//! let plaintext = b"Hello world!";
//! let cipher = Aes128Cbc::new_var(&key, &iv).unwrap();
//!
//! // buffer must have enough space for message+padding
//! let mut buffer = [0u8; 32];
//! // copy message to the buffer
//! let pos = plaintext.len();
//! buffer[..pos].copy_from_slice(plaintext);
//! let ciphertext = cipher.encrypt(&mut buffer, pos).unwrap();
//!
//! assert_eq!(ciphertext, hex!("1b7a4c403124ae2fb52bedc534d82fa8"));
//!
//! // re-create cipher mode instance
//! let cipher = Aes128Cbc::new_var(&key, &iv).unwrap();
//! let mut buf = ciphertext.to_vec();
//! let decrypted_ciphertext = cipher.decrypt(&mut buf).unwrap();
//!
//! assert_eq!(decrypted_ciphertext, plaintext);
//! # }
//! ```
//!
//! With an enabled `alloc` feature (which is enabled by default) you can use
//! `encrypt_vec` and `descrypt_vec` methods:
//! ```
//! # use aes::Aes128;
//! # use block_modes::{BlockMode, Cbc};
//! # use block_modes::block_padding::Pkcs7;
//! # use hex_literal::hex;
//! #
//! # // create an alias for convenience
//! # type Aes128Cbc = Cbc<Aes128, Pkcs7>;
//! #
//! # fn main() {
//! # let key = hex!("000102030405060708090a0b0c0d0e0f");
//! # let iv = hex!("f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff");
//! # let plaintext = b"Hello world!";
//! let cipher = Aes128Cbc::new_var(&key, &iv).unwrap();
//! let ciphertext = cipher.encrypt_vec(plaintext);
//!
//! assert_eq!(ciphertext, hex!("1b7a4c403124ae2fb52bedc534d82fa8"));
//!
//! let cipher = Aes128Cbc::new_var(&key, &iv).unwrap();
//! let decrypted_ciphertext = cipher.decrypt_vec(&ciphertext).unwrap();
//!
//! assert_eq!(decrypted_ciphertext, plaintext);
//! # }
//! ```
//!
//! [1]: https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation
//! [2]: https://github.com/RustCrypto/stream-ciphers

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg"
)]
#![deny(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod errors;
mod traits;
mod utils;

mod cbc;
mod cfb;
mod cfb8;
mod ecb;
mod ofb;
mod pcbc;

pub use block_padding;
pub use cipher;

pub use crate::{
    cbc::Cbc,
    cfb::Cfb,
    cfb8::Cfb8,
    ecb::Ecb,
    errors::{BlockModeError, InvalidKeyIvLength},
    ofb::Ofb,
    pcbc::Pcbc,
    traits::BlockMode,
};
