//! [X.509] Subject Public Key Info (SPKI) types describing public keys and their
//! associated [`AlgorithmIdentifier`] OIDs.
//!
//! Described in [RFC 5280 Section 4.1].
//!
//! # Minimum Supported Rust Version
//!
//! This crate requires **Rust 1.47** at a minimum.
//!
//! # Usage
//!
//! The following example demonstrates how to use an OID as the `parameters`
//! of an [`AlgorithmIdentifier`].
//!
//! Borrow the [`ObjectIdentifier`] first then use [`Into`] (or `Any::from`):
//!
//! ```
//! use spki::{AlgorithmIdentifier, ObjectIdentifier};
//!
//! let alg_oid = "1.2.840.10045.2.1".parse::<ObjectIdentifier>().unwrap();
//! let params_oid = "1.2.840.10045.3.1.7".parse::<ObjectIdentifier>().unwrap();
//!
//! let alg_id = AlgorithmIdentifier {
//!     oid: alg_oid,
//!     parameters: Some((&params_oid).into())
//! };
//! ```
//!
//! [X.509]: https://en.wikipedia.org/wiki/X.509
//! [RFC 5280 Section 4.1]: https://tools.ietf.org/html/rfc5280#section-4.1

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_root_url = "https://docs.rs/spki/0.3.0"
)]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

mod algorithm;
mod spki;

pub use crate::{algorithm::AlgorithmIdentifier, spki::SubjectPublicKeyInfo};
pub use der::{self, ObjectIdentifier};
