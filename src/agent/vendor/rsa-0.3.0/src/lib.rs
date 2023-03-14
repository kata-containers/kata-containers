#![doc(html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo_small.png")]
//! RSA Implementation in pure Rust.
//!
//!
//! # Usage
//!
//! Using PKCS1v15.
//! ```
//! use rsa::{PublicKey, RSAPrivateKey, RSAPublicKey, PaddingScheme};
//! use rand::rngs::OsRng;
//!
//! let mut rng = OsRng;
//! let bits = 2048;
//! let private_key = RSAPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
//! let public_key = RSAPublicKey::from(&private_key);
//!
//! // Encrypt
//! let data = b"hello world";
//! let padding = PaddingScheme::new_pkcs1v15_encrypt();
//! let enc_data = public_key.encrypt(&mut rng, padding, &data[..]).expect("failed to encrypt");
//! assert_ne!(&data[..], &enc_data[..]);
//!
//! // Decrypt
//! let padding = PaddingScheme::new_pkcs1v15_encrypt();
//! let dec_data = private_key.decrypt(padding, &enc_data).expect("failed to decrypt");
//! assert_eq!(&data[..], &dec_data[..]);
//! ```
//!
//! Using OAEP.
//! ```
//! use rsa::{PublicKey, RSAPrivateKey, RSAPublicKey, PaddingScheme};
//! use rand::rngs::OsRng;
//!
//! let mut rng = OsRng;
//! let bits = 2048;
//! let private_key = RSAPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
//! let public_key = RSAPublicKey::from(&private_key);
//!
//! // Encrypt
//! let data = b"hello world";
//! let padding = PaddingScheme::new_oaep::<sha2::Sha256>();
//! let enc_data = public_key.encrypt(&mut rng, padding, &data[..]).expect("failed to encrypt");
//! assert_ne!(&data[..], &enc_data[..]);
//!
//! // Decrypt
//! let padding = PaddingScheme::new_oaep::<sha2::Sha256>();
//! let dec_data = private_key.decrypt(padding, &enc_data).expect("failed to decrypt");
//! assert_eq!(&data[..], &dec_data[..]);
//! ```

#[macro_use]
extern crate lazy_static;

#[cfg(feature = "serde")]
extern crate serde_crate;

#[cfg(test)]
extern crate base64;
#[cfg(test)]
extern crate hex;
#[cfg(all(test, feature = "serde"))]
extern crate serde_test;

pub use num_bigint::BigUint;

/// Useful algorithms.
pub mod algorithms;

/// Error types.
pub mod errors;

/// Supported hash functions.
pub mod hash;

/// Supported padding schemes.
pub mod padding;

#[cfg(feature = "pem")]
pub use pem;

mod key;
mod oaep;
mod parse;
mod pkcs1v15;
mod pss;
mod raw;

pub use self::hash::Hash;
pub use self::key::{PublicKey, PublicKeyParts, RSAPrivateKey, RSAPublicKey};
pub use self::padding::PaddingScheme;

// Optionally expose internals if requested via feature-flag.

#[cfg(not(feature = "expose-internals"))]
mod internals;

/// Internal raw RSA functions.
#[cfg(feature = "expose-internals")]
pub mod internals;
