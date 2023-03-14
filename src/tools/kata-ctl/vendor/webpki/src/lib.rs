// Copyright 2015 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! webpki: Web PKI X.509 Certificate Validation.
//!
//! See `EndEntityCert`'s documentation for a description of the certificate
//! processing steps necessary for a TLS connection.
//!
//! # Features
//!
//! | Feature | Description |
//! | ------- | ----------- |
//! | `alloc` | Enable features that require use of the heap. Currently all RSA signature algorithms require this feature. |
//! | `std` | Enable features that require libstd. Implies `alloc`. |

#![doc(html_root_url = "https://briansmith.org/rustdoc/")]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(
    clippy::doc_markdown,
    clippy::if_not_else,
    clippy::inline_always,
    clippy::items_after_statements,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::single_match,
    clippy::single_match_else
)]
#![deny(clippy::as_conversions)]

#[cfg(any(test, feature = "alloc"))]
#[cfg_attr(test, macro_use)]
extern crate alloc;

#[macro_use]
mod der;

mod calendar;
mod cert;
mod end_entity;
mod error;
mod name;
mod signed_data;
mod time;
mod trust_anchor;

mod verify_cert;

pub use {
    end_entity::EndEntityCert,
    error::Error,
    name::{DnsNameRef, InvalidDnsNameError},
    signed_data::{
        SignatureAlgorithm, ECDSA_P256_SHA256, ECDSA_P256_SHA384, ECDSA_P384_SHA256,
        ECDSA_P384_SHA384, ED25519,
    },
    time::Time,
    trust_anchor::{TlsClientTrustAnchors, TlsServerTrustAnchors, TrustAnchor},
};

#[cfg(feature = "alloc")]
pub use {
    name::DnsName,
    signed_data::{
        RSA_PKCS1_2048_8192_SHA256, RSA_PKCS1_2048_8192_SHA384, RSA_PKCS1_2048_8192_SHA512,
        RSA_PKCS1_3072_8192_SHA384, RSA_PSS_2048_8192_SHA256_LEGACY_KEY,
        RSA_PSS_2048_8192_SHA384_LEGACY_KEY, RSA_PSS_2048_8192_SHA512_LEGACY_KEY,
    },
};

#[cfg(feature = "alloc")]
#[allow(unknown_lints, clippy::upper_case_acronyms)]
#[deprecated(note = "Use DnsName")]
pub type DNSName = DnsName;

#[deprecated(note = "use DnsNameRef")]
#[allow(unknown_lints, clippy::upper_case_acronyms)]
pub type DNSNameRef<'a> = DnsNameRef<'a>;

#[deprecated(note = "use TlsServerTrustAnchors")]
#[allow(unknown_lints, clippy::upper_case_acronyms)]
pub type TLSServerTrustAnchors<'a> = TlsServerTrustAnchors<'a>;

#[deprecated(note = "use TlsClientTrustAnchors")]
#[allow(unknown_lints, clippy::upper_case_acronyms)]
pub type TLSClientTrustAnchors<'a> = TlsClientTrustAnchors<'a>;
