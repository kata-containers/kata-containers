#![doc = include_str!("../README.md")]

//! # Usage
//!
//! ```
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "alloc")]
//! # {
//! /// Example PEM document
//! /// NOTE: do not actually put private key literals into your source code!!!
//! let example_pem = "\
//! -----BEGIN PRIVATE KEY-----
//! MC4CAQAwBQYDK2VwBCIEIBftnHPp22SewYmmEoMcX8VwI4IHwaqd+9LFPj/15eqF
//! -----END PRIVATE KEY-----
//! ";
//!
//! // Decode PEM
//! let (type_label, data) = pem_rfc7468::decode_vec(example_pem.as_bytes())?;
//! assert_eq!(type_label, "PRIVATE KEY");
//! assert_eq!(
//!     data,
//!     &[
//!         48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, 112, 4, 34, 4, 32, 23, 237, 156, 115, 233, 219,
//!         100, 158, 193, 137, 166, 18, 131, 28, 95, 197, 112, 35, 130, 7, 193, 170, 157, 251,
//!         210, 197, 62, 63, 245, 229, 234, 133
//!     ]
//! );
//!
//! // Encode PEM
//! use pem_rfc7468::LineEnding;
//! let encoded_pem = pem_rfc7468::encode_string(type_label, LineEnding::default(), &data)?;
//! assert_eq!(&encoded_pem, example_pem);
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! [RFC 1421]: https://datatracker.ietf.org/doc/html/rfc1421
//! [RFC 7468]: https://datatracker.ietf.org/doc/html/rfc7468
//! [RFC 7468 p6]: https://datatracker.ietf.org/doc/html/rfc7468#page-6
//! [Util::Lookup]: https://arxiv.org/pdf/2108.04600.pdf

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo.svg",
    html_root_url = "https://docs.rs/pem-rfc7468/0.3.1"
)]
#![forbid(unsafe_code, clippy::unwrap_used)]
#![warn(missing_docs, rust_2018_idioms, unused_qualifications)]

#[cfg(feature = "alloc")]
#[macro_use]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod decoder;
mod encoder;
mod error;
mod grammar;

pub use crate::{
    decoder::{decode, decode_label, Decoder},
    encoder::{encode, encoded_len, LineEnding},
    error::{Error, Result},
};

#[cfg(feature = "alloc")]
pub use crate::{decoder::decode_vec, encoder::encode_string};

/// The pre-encapsulation boundary appears before the encapsulated text.
///
/// From RFC 7468 Section 2:
/// > There are exactly five hyphen-minus (also known as dash) characters ("-")
/// > on both ends of the encapsulation boundaries, no more, no less.
const PRE_ENCAPSULATION_BOUNDARY: &[u8] = b"-----BEGIN ";

/// The post-encapsulation boundary appears immediately after the encapsulated text.
const POST_ENCAPSULATION_BOUNDARY: &[u8] = b"-----END ";

/// Delimiter of encapsulation boundaries.
const ENCAPSULATION_BOUNDARY_DELIMITER: &[u8] = b"-----";

/// Width at which Base64 must be wrapped.
///
/// From RFC 7468 Section 2:
///
/// > Generators MUST wrap the base64-encoded lines so that each line
/// > consists of exactly 64 characters except for the final line, which
/// > will encode the remainder of the data (within the 64-character line
/// > boundary), and they MUST NOT emit extraneous whitespace.  Parsers MAY
/// > handle other line sizes.
const BASE64_WRAP_WIDTH: usize = 64;

/// Marker trait for types with an associated PEM type label.
pub trait PemLabel {
    /// Expected PEM type label for a given document, e.g. `"PRIVATE KEY"`
    const TYPE_LABEL: &'static str;
}
