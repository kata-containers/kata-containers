//! Basic Encoding Rules (BER) objects and parser
//!
//! # BER Objects
//!
//! The main object of this crate is [`BerObject`]. It contains a header (ber tag, class, and size)
//! and content.
//!
//! To parse primitive objects (for ex. integers or strings), use the `parse_ber_` set of
//! functions.
//!
//! Constructed objects (like sequences, sets or tagged objects) require to use a combinator. This
//! combinator takes a function or closure as input, and returns a new, specialized parser.
//! See the [nom](https://github.com/geal/nom) parser combinator library for more details on
//! combinators.
//!
//! # Examples
//!
//! Parse two BER integers:
//!
//! ```rust
//! use der_parser::ber::parse_ber_integer;
//!
//! let bytes = [ 0x02, 0x03, 0x01, 0x00, 0x01,
//!               0x02, 0x03, 0x01, 0x00, 0x00,
//! ];
//!
//! let (rem, obj1) = parse_ber_integer(&bytes).expect("parsing failed");
//! let (rem, obj2) = parse_ber_integer(&bytes).expect("parsing failed");
//! ```
//!
//! Parse a BER sequence containing one integer and an octetstring:
//!
//! ```rust
//! use der_parser::ber::*;
//!
//! let bytes = [ 0x30, 0x0a,
//!               0x02, 0x03, 0x01, 0x00, 0x01,
//!               0x04, 0x03, 0x62, 0x61, 0x64,
//! ];
//!
//! let (rem, seq) = parse_ber_sequence_defined(|content| {
//!         let (rem, obj1) = parse_ber_integer(content)?;
//!         let (rem, obj2) = parse_ber_octetstring(rem)?;
//!         Ok((rem, vec![obj1, obj2]))
//!     })(&bytes)
//!     .expect("parsing failed");
//! ```

mod ber;
mod integer;
mod multi;
mod parser;
mod print;
#[cfg(feature = "serialize")]
mod serialize;
mod tagged;
mod visit;
mod visit_mut;
mod wrap_any;

pub use crate::ber::ber::*;
pub use crate::ber::multi::*;
pub use crate::ber::parser::*;
pub use crate::ber::print::*;
#[cfg(feature = "serialize")]
pub use crate::ber::serialize::*;
pub use crate::ber::tagged::*;
pub use crate::ber::visit::*;
pub use crate::ber::visit_mut::*;
pub use crate::ber::wrap_any::*;

pub mod compat;

pub use asn1_rs::{Class, Header, Length, Tag};

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::convert::Into;
