#![doc = include_str!("../README.md")]

//! ## About this crate
//! This library provides generalized PKCS#8 support designed to work with a
//! number of different algorithms. It supports `no_std` platforms including
//! ones without a heap (albeit with reduced functionality).
//!
//! It supports decoding/encoding the following types:
//!
//! - [`EncryptedPrivateKeyInfo`]: (with `pkcs5` feature) encrypted key.
//! - [`PrivateKeyInfo`]: algorithm identifier and data representing a private key.
//!   Optionally also includes public key data for asymmetric keys.
//! - [`SubjectPublicKeyInfo`]: algorithm identifier and data representing a public key
//!   (re-exported from the [`spki`] crate)
//!
//! When the `alloc` feature is enabled, the following additional types are
//! available which provide more convenient decoding/encoding support:
//!
//! - [`EncryptedPrivateKeyDocument`]: (with `pkcs5` feature) heap-backed encrypted key.
//! - [`PrivateKeyDocument`]: heap-backed storage for serialized [`PrivateKeyInfo`].
//! - [`PublicKeyDocument`]: heap-backed storage for serialized [`SubjectPublicKeyInfo`].
//!
//! When the `pem` feature is enabled, it also supports decoding/encoding
//! documents from "PEM encoding" format as defined in RFC 7468.
//!
//! ## Encrypted Private Key Support
//! [`EncryptedPrivateKeyInfo`] supports decoding/encoding encrypted PKCS#8
//! private keys and is gated under the `pkcs5` feature. The corresponding
//! [`EncryptedPrivateKeyDocument`] type provides heap-backed storage
//! (`alloc` feature required).
//!
//! When the `encryption` feature of this crate is enabled, it provides
//! [`EncryptedPrivateKeyInfo::decrypt`] and [`PrivateKeyInfo::encrypt`]
//! functions which are able to decrypt/encrypt keys using the following
//! algorithms:
//!
//! - [PKCS#5v2 Password Based Encryption Scheme 2 (RFC 8018)]
//!   - Key derivation functions:
//!     - [scrypt] ([RFC 7914])
//!     - PBKDF2 ([RFC 8018](https://datatracker.ietf.org/doc/html/rfc8018#section-5.2))
//!       - SHA-2 based PRF with HMAC-SHA224, HMAC-SHA256, HMAC-SHA384, or HMAC-SHA512
//!       - SHA-1 based PRF with HMAC-SHA1, when the `sha1` feature of this crate is enabled.
//!   - Symmetric encryption: AES-128-CBC, AES-192-CBC, or AES-256-CBC
//!     (best available options for PKCS#5v2)
//!  
//! ## Legacy DES-CBC and DES-EDE3-CBC (3DES) support (optional)
//! When the `des-insecure` and/or `3des` features are enabled this crate provides support for
//! private keys encrypted with with DES-CBC and DES-EDE3-CBC (3DES or Triple DES) symmetric
//! encryption, respectively.
//!
//! ⚠️ WARNING ⚠️
//!
//! DES support (gated behind the `des-insecure` feature) is implemented to
//! allow for decryption of legacy PKCS#8 files only.
//!
//! Such PKCS#8 documents should be considered *INSECURE* due to the short
//! 56-bit key size of DES.
//!
//! New keys should use AES instead.
//!
//! [RFC 5208]: https://tools.ietf.org/html/rfc5208
//! [RFC 5958]: https://tools.ietf.org/html/rfc5958
//! [RFC 7914]: https://datatracker.ietf.org/doc/html/rfc7914
//! [PKCS#5v2 Password Based Encryption Scheme 2 (RFC 8018)]: https://tools.ietf.org/html/rfc8018#section-6.2
//! [scrypt]: https://en.wikipedia.org/wiki/Scrypt

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_root_url = "https://docs.rs/pkcs8/0.8.0"
)]
#![forbid(unsafe_code, clippy::unwrap_used)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod error;
mod private_key_info;
mod traits;
mod version;

#[cfg(feature = "alloc")]
mod document;

#[cfg(feature = "pkcs5")]
pub(crate) mod encrypted_private_key_info;

pub use crate::{
    error::{Error, Result},
    private_key_info::PrivateKeyInfo,
    traits::DecodePrivateKey,
    version::Version,
};
pub use der::{self, asn1::ObjectIdentifier};
pub use spki::{self, AlgorithmIdentifier, DecodePublicKey, SubjectPublicKeyInfo};

#[cfg(feature = "alloc")]
pub use {
    crate::{document::private_key::PrivateKeyDocument, traits::EncodePrivateKey},
    spki::{EncodePublicKey, PublicKeyDocument},
};

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
pub use der::pem::LineEnding;

#[cfg(feature = "pkcs5")]
pub use {crate::encrypted_private_key_info::EncryptedPrivateKeyInfo, pkcs5};

#[cfg(feature = "rand_core")]
pub use rand_core;

#[cfg(all(feature = "alloc", feature = "pkcs5"))]
pub use crate::document::encrypted_private_key::EncryptedPrivateKeyDocument;
