//! Distinguished Encoding Rules (DER) objects and parser
//!
//! All functions in this crate use BER parsing functions (see the `ber` module)
//! internally, adding constraints verification where needed.
//!
//! The objects [`BerObject`] and [`DerObject`] are the same (type alias): all BER functions,
//! combinators and macros can be used, and provide additional tools for DER parsing.
//! However, DER parsing functions enforce DER constraints in addition of their BER counterparts.
//!
//! # DER Objects
//!
//! The main object of this crate is [`DerObject`]. It contains a header (ber tag, class, and size)
//! and content.
//!
//! To parse primitive objects (for ex. integers or strings), use the `parse_der_` set of
//! functions.
//!
//! Constructed objects (like sequences, sets or tagged objects) require to use a combinator. This
//! combinator takes a function or closure as input, and returns a new, specialized parser.
//! See the [nom](https://github.com/geal/nom) parser combinator library for more details on
//! combinators.
//!
//! # Examples
//!
//! Parse two DER integers:
//!
//! ```rust
//! use der_parser::der::parse_der_integer;
//!
//! let bytes = [ 0x02, 0x03, 0x01, 0x00, 0x01,
//!               0x02, 0x03, 0x01, 0x00, 0x00,
//! ];
//!
//! let (rem, obj1) = parse_der_integer(&bytes).expect("parsing failed");
//! let (rem, obj2) = parse_der_integer(&bytes).expect("parsing failed");
//! ```
//!
//! Parse a BER sequence containing one integer and an octetstring:
//!
//! ```rust
//! use der_parser::der::*;
//!
//! let bytes = [ 0x30, 0x0a,
//!               0x02, 0x03, 0x01, 0x00, 0x01,
//!               0x04, 0x03, 0x62, 0x61, 0x64,
//! ];
//!
//! let (rem, seq) = parse_der_sequence_defined(|content| {
//!         let (rem, obj1) = parse_der_integer(content)?;
//!         let (rem, obj2) = parse_der_octetstring(rem)?;
//!         Ok((rem, vec![obj1, obj2]))
//!     })(&bytes)
//!     .expect("parsing failed");
//! ```

use crate::ber::{BerObject, BerObjectContent};
pub use crate::ber::{Class, Header};
pub use asn1_rs::Tag;

mod multi;
mod parser;
mod tagged;
pub use crate::der::multi::*;
pub use crate::der::parser::*;
pub use crate::der::tagged::*;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::convert::Into;

/// DER Object class of tag (same as `BerClass`)
#[deprecated(since = "7.0.0", note = "Use `Class` instead")]
pub type DerClass = Class;

/// DER tag (same as BER tag)
#[deprecated(since = "7.0.0", note = "Use `Tag` instead")]
pub type DerTag = Tag;

/// Representation of a DER-encoded (X.690) object
///
/// Note that a DER object is just a BER object, with additional constraints.
pub type DerObject<'a> = BerObject<'a>;

/// DER object header (identifier and length)
///
/// This is the same object as `BerObjectHeader`.
#[deprecated(since = "7.0.0", note = "Use `Tag` instead")]
pub type DerObjectHeader<'a> = Header<'a>;

/// BER object content
///
/// This is the same object as `BerObjectContent`.
pub type DerObjectContent<'a> = BerObjectContent<'a>;
