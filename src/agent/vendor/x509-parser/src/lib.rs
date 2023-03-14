//! [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE-MIT)
//! [![Apache License 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE-APACHE)
//! [![docs.rs](https://docs.rs/x509-parser/badge.svg)](https://docs.rs/x509-parser)
//! [![crates.io](https://img.shields.io/crates/v/x509-parser.svg)](https://crates.io/crates/x509-parser)
//! [![Download numbers](https://img.shields.io/crates/d/x509-parser.svg)](https://crates.io/crates/x509-parser)
//! [![Github CI](https://github.com/rusticata/x509-parser/workflows/Continuous%20integration/badge.svg)](https://github.com/rusticata/x509-parser/actions)
//! [![Minimum rustc version](https://img.shields.io/badge/rustc-1.53.0+-lightgray.svg)](#rust-version-requirements)
//!
//! # X.509 Parser
//!
//! A X.509 v3 ([RFC5280]) parser, implemented with the [nom](https://github.com/Geal/nom)
//! parser combinator framework.
//!
//! It is written in pure Rust, fast, and makes extensive use of zero-copy. A lot of care is taken
//! to ensure security and safety of this crate, including design (recursion limit, defensive
//! programming), tests, and fuzzing. It also aims to be panic-free.
//!
//! The code is available on [Github](https://github.com/rusticata/x509-parser)
//! and is part of the [Rusticata](https://github.com/rusticata) project.
//!
//! Certificates are usually encoded in two main formats: PEM (usually the most common format) or
//! DER.  A PEM-encoded certificate is a container, storing a DER object. See the
//! [`pem`](pem/index.html) module for more documentation.
//!
//! To decode a DER-encoded certificate, the main parsing method is
//! [`X509Certificate::from_der`] (
//! part of the [`FromDer`](traits/trait.FromDer.html) trait
//! ), which builds a
//! [`X509Certificate`](certificate/struct.X509Certificate.html) object.
//!
//! An alternative method is to use [`X509CertificateParser`](certificate/struct.X509CertificateParser.html),
//! which allows specifying parsing options (for example, not automatically parsing option contents).
//!
//! The returned objects for parsers follow the definitions of the RFC. This means that accessing
//! fields is done by accessing struct members recursively. Some helper functions are provided, for
//! example [`X509Certificate::issuer()`](certificate/struct.X509Certificate.html#method.issuer) returns the
//! same as accessing `<object>.tbs_certificate.issuer`.
//!
//! For PEM-encoded certificates, use the [`pem`](pem/index.html) module.
//!
//! # Examples
//!
//! Parsing a certificate in DER format:
//!
//! ```rust
//! use x509_parser::prelude::*;
//!
//! static IGCA_DER: &[u8] = include_bytes!("../assets/IGC_A.der");
//!
//! # fn main() {
//! let res = X509Certificate::from_der(IGCA_DER);
//! match res {
//!     Ok((rem, cert)) => {
//!         assert!(rem.is_empty());
//!         //
//!         assert_eq!(cert.version(), X509Version::V3);
//!     },
//!     _ => panic!("x509 parsing failed: {:?}", res),
//! }
//! # }
//! ```
//!
//! To parse a CRL and print information about revoked certificates:
//!
//! ```rust
//! # use x509_parser::prelude::*;
//! #
//! # static DER: &[u8] = include_bytes!("../assets/example.crl");
//! #
//! # fn main() {
//! let res = CertificateRevocationList::from_der(DER);
//! match res {
//!     Ok((_rem, crl)) => {
//!         for revoked in crl.iter_revoked_certificates() {
//!             println!("Revoked certificate serial: {}", revoked.raw_serial_as_string());
//!             println!("  Reason: {}", revoked.reason_code().unwrap_or_default().1);
//!         }
//!     },
//!     _ => panic!("CRL parsing failed: {:?}", res),
//! }
//! # }
//! ```
//!
//! See also `examples/print-cert.rs`.
//!
//! # Features
//!
//! - The `verify` feature adds support for (cryptographic) signature verification, based on `ring`.
//!   It adds the
//!   [`X509Certificate::verify_signature()`](certificate/struct.X509Certificate.html#method.verify_signature)
//!   to `X509Certificate`.
//!
//! ```rust
//! # #[cfg(feature = "verify")]
//! # use x509_parser::certificate::X509Certificate;
//! /// Cryptographic signature verification: returns true if certificate was signed by issuer
//! #[cfg(feature = "verify")]
//! pub fn check_signature(cert: &X509Certificate<'_>, issuer: &X509Certificate<'_>) -> bool {
//!     let issuer_public_key = issuer.public_key();
//!     cert
//!         .verify_signature(Some(issuer_public_key))
//!         .is_ok()
//! }
//! ```
//!
//! - The `validate` features add methods to run more validation functions on the certificate structure
//!   and values using the [`Validate`](validate/trait.Validate.html) trait.
//!   It does not validate any cryptographic parameter (see `verify` above).
//!
//! ## Rust version requirements
//!
//! `x509-parser` requires **Rustc version 1.53 or greater**, based on der-parser
//! dependencies and for proc-macro attributes support.
//!
//! Note that due to breaking changes in the `time` crate, a specific version of this
//! crate must be specified for compiler versions <= 1.57:
//! `cargo update -p time --precise 0.3.9`
//!
//! [RFC5280]: https://tools.ietf.org/html/rfc5280

#![deny(/*missing_docs,*/
        unstable_features,
        unused_import_braces, unused_qualifications)]
#![warn(
    missing_debug_implementations,
    /* missing_docs,
    rust_2018_idioms,*/
    unreachable_pub
)]
#![forbid(unsafe_code)]
#![deny(broken_intra_doc_links)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod certificate;
pub mod certification_request;
pub mod cri_attributes;
pub mod error;
pub mod extensions;
pub mod objects;
pub mod pem;
pub mod prelude;
pub mod public_key;
pub mod revocation_list;
pub mod signature_algorithm;
pub mod signature_value;
pub mod time;
pub mod utils;
#[cfg(feature = "validate")]
#[cfg_attr(docsrs, doc(cfg(feature = "validate")))]
pub mod validate;
pub mod x509;

// reexports
pub use der_parser;
pub use der_parser::num_bigint;
pub use nom;
pub use oid_registry;

use asn1_rs::FromDer;
use certificate::X509Certificate;
use error::X509Result;
use revocation_list::CertificateRevocationList;

/// Parse a **DER-encoded** X.509 Certificate, and return the remaining of the input and the built
/// object.
///
///
/// This function is an alias to [X509Certificate::from_der](certificate::X509Certificate::from_der). See this function
/// for more information.
///
/// For PEM-encoded certificates, use the [`pem`](pem/index.html) module.
#[inline]
pub fn parse_x509_certificate<'a>(i: &'a [u8]) -> X509Result<X509Certificate<'a>> {
    X509Certificate::from_der(i)
}

/// Parse a DER-encoded X.509 v2 CRL, and return the remaining of the input and the built
/// object.
///
/// This function is an alias to [CertificateRevocationList::from_der](revocation_list::CertificateRevocationList::from_der). See this function
/// for more information.
#[inline]
pub fn parse_x509_crl(i: &[u8]) -> X509Result<CertificateRevocationList> {
    CertificateRevocationList::from_der(i)
}

/// Parse a DER-encoded X.509 Certificate, and return the remaining of the input and the built
#[deprecated(
    since = "0.9.0",
    note = "please use `parse_x509_certificate` or `X509Certificate::from_der` instead"
)]
#[inline]
pub fn parse_x509_der<'a>(i: &'a [u8]) -> X509Result<X509Certificate<'a>> {
    X509Certificate::from_der(i)
}

/// Parse a DER-encoded X.509 v2 CRL, and return the remaining of the input and the built
/// object.
#[deprecated(
    since = "0.9.0",
    note = "please use `parse_x509_crl` or `CertificateRevocationList::from_der` instead"
)]
#[inline]
pub fn parse_crl_der(i: &[u8]) -> X509Result<CertificateRevocationList> {
    CertificateRevocationList::from_der(i)
}
