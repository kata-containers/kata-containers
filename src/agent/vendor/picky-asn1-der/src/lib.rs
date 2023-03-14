//! [![Crates.io](https://img.shields.io/crates/v/picky-asn1-der.svg)](https://crates.io/crates/picky-asn1-der)
//! [![docs.rs](https://docs.rs/picky-asn1-der/badge.svg)](https://docs.rs/picky-asn1-der)
//! ![Crates.io](https://img.shields.io/crates/l/picky-asn1-der)
//!
//! # picky-asn1-der
//!
//! Portions of project [serde_asn1_der](https://github.com/KizzyCode/serde_asn1_der) are held by
//! Keziah Biermann, 2019 as part of this project.
//!
//! This crate implements an ASN.1-DER subset for serde.
//!
//! The following types have built-in support:
//! - `bool`: The ASN.1-BOOLEAN-type
//! - `u8`, `u16`, `u32`, `u64`, `u128`, `usize`: The ASN.1-INTEGER-type
//! - `()`: The ASN.1-NULL-type
//! - `&[u8]`, `Vec<u8>`: The ASN.1-OctetString-type
//! - `&str`, `String`: The ASN.1-UTF8String-type
//!
//! More advanced types are supported through wrappers:
//! - Integer (as big integer)
//! - Bit String
//! - Object Identifier
//! - Utf8 String
//! - Numeric String
//! - Printable String
//! - IA5 String
//! - Generalized Time
//! - UTC Time
//! - Application Tags from 0 to 15
//! - Context Tags from 0 to 15
//!
//! Everything sequence-like combined out of this types is also supported out of the box.
//!
//! ```rust
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)] // Now our struct supports all DER-conversion-traits
//! struct Address {
//!     street: String,
//!     house_number: u128,
//!     postal_code: u128,
//!     state: String,
//!     country: String
//! }
//!
//! #[derive(Serialize, Deserialize)] // Now our struct supports all DER-conversion-traits too
//! struct Customer {
//!     name: String,
//!     e_mail_address: String,
//!     postal_address: Address
//! }
//! ```
//!
//!
//! # Example
//! ```rust
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct TestStruct {
//!     number: u8,
//!     #[serde(with = "serde_bytes")]
//!     vec: Vec<u8>,
//!     tuple: (usize, ())
//! }
//!
//! let plain = TestStruct{ number: 7, vec: b"Testolope".to_vec(), tuple: (4, ()) };
//! let serialized = picky_asn1_der::to_vec(&plain).unwrap();
//! let deserialized: TestStruct = picky_asn1_der::from_bytes(&serialized).unwrap();
//! ```

#[macro_use]
mod debug_log;

pub mod application_tag;
mod de;
pub(crate) mod misc;
mod raw_der;
mod ser;

pub use crate::de::{from_bytes, from_reader, from_reader_with_max_len, Deserializer};
pub use crate::raw_der::Asn1RawDer;
pub use crate::ser::{to_byte_buf, to_bytes, to_vec, to_writer, Serializer};

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io;

/// A `picky_asn1_der`-related error
#[derive(Debug)]
pub enum Asn1DerError {
    /// The data is truncated
    TruncatedData,

    /// The data is invalid
    InvalidData,

    /// The value may be valid but is unsupported (e.g. an integer that is too large)
    UnsupportedValue,

    /// The data type is not supported by the (de-)serializer
    UnsupportedType,

    /// The provided sink is unable to accept all bytes
    InvalidSink,

    /// A custom message produced by `serde`
    Message(String),

    /// Some other underlying error (e.g. an IO error)
    Other(Box<dyn Error + Send + Sync + 'static>),
}

impl Display for Asn1DerError {
    fn fmt(&self, t: &mut Formatter) -> fmt::Result {
        write!(t, "{:?}", self)
    }
}

impl Error for Asn1DerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Asn1DerError::Other(source) => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl serde::de::Error for Asn1DerError {
    fn custom<T: Display>(msg: T) -> Self {
        Asn1DerError::Message(msg.to_string())
    }
}

impl serde::ser::Error for Asn1DerError {
    fn custom<T: Display>(msg: T) -> Self {
        Asn1DerError::Message(msg.to_string())
    }
}

impl From<io::Error> for Asn1DerError {
    fn from(io_error: io::Error) -> Self {
        match io_error.kind() {
            io::ErrorKind::UnexpectedEof => Asn1DerError::TruncatedData,
            io::ErrorKind::WriteZero => Asn1DerError::InvalidSink,
            _ => Asn1DerError::Other(Box::new(io_error)),
        }
    }
}

pub type Result<T> = std::result::Result<T, Asn1DerError>;
