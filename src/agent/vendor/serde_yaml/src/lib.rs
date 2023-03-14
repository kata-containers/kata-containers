//! [![github]](https://github.com/dtolnay/serde-yaml)&ensp;[![crates-io]](https://crates.io/crates/serde-yaml)&ensp;[![docs-rs]](https://docs.rs/serde-yaml)
//!
//! [github]: https://img.shields.io/badge/github-8da0cb?style=for-the-badge&labelColor=555555&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-fc8d62?style=for-the-badge&labelColor=555555&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs
//!
//! <br>
//!
//! This crate is a Rust library for using the [Serde] serialization framework
//! with data in [YAML] file format.
//!
//! This library does not reimplement a YAML parser; it uses [yaml-rust] which
//! is a pure Rust YAML 1.2 implementation.
//!
//! [Serde]: https://github.com/serde-rs/serde
//! [YAML]: https://yaml.org/
//! [yaml-rust]: https://github.com/chyh1990/yaml-rust
//!
//! # Examples
//!
//! ```
//! use std::collections::BTreeMap;
//!
//! fn main() -> Result<(), serde_yaml::Error> {
//!     // You have some type.
//!     let mut map = BTreeMap::new();
//!     map.insert("x".to_string(), 1.0);
//!     map.insert("y".to_string(), 2.0);
//!
//!     // Serialize it to a YAML string.
//!     let s = serde_yaml::to_string(&map)?;
//!     assert_eq!(s, "---\nx: 1.0\ny: 2.0\n");
//!
//!     // Deserialize it back to a Rust type.
//!     let deserialized_map: BTreeMap<String, f64> = serde_yaml::from_str(&s)?;
//!     assert_eq!(map, deserialized_map);
//!     Ok(())
//! }
//! ```
//!
//! ## Using Serde derive
//!
//! It can also be used with Serde's serialization code generator `serde_derive` to
//! handle structs and enums defined in your own program.
//!
//! ```
//! # use serde_derive::{Serialize, Deserialize};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! struct Point {
//!     x: f64,
//!     y: f64,
//! }
//!
//! fn main() -> Result<(), serde_yaml::Error> {
//!     let point = Point { x: 1.0, y: 2.0 };
//!
//!     let s = serde_yaml::to_string(&point)?;
//!     assert_eq!(s, "---\nx: 1.0\ny: 2.0\n");
//!
//!     let deserialized_point: Point = serde_yaml::from_str(&s)?;
//!     assert_eq!(point, deserialized_point);
//!     Ok(())
//! }
//! ```

#![doc(html_root_url = "https://docs.rs/serde_yaml/0.8.26")]
#![deny(missing_docs)]
// Suppressed clippy_pedantic lints
#![allow(
    // buggy
    clippy::iter_not_returning_iterator, // https://github.com/rust-lang/rust-clippy/issues/8285
    clippy::question_mark, // https://github.com/rust-lang/rust-clippy/issues/7859
    // private Deserializer::next
    clippy::should_implement_trait,
    // things are often more readable this way
    clippy::cast_lossless,
    clippy::checked_conversions,
    clippy::if_not_else,
    clippy::manual_assert,
    clippy::match_like_matches_macro,
    clippy::match_same_arms,
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::redundant_else,
    clippy::single_match_else,
    // code is acceptable
    clippy::blocks_in_if_conditions,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::derive_partial_eq_without_eq,
    clippy::doc_markdown,
    clippy::items_after_statements,
    clippy::return_self_not_must_use,
    // noisy
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
)]

pub use crate::de::{from_reader, from_slice, from_str, Deserializer};
pub use crate::error::{Error, Location, Result};
pub use crate::ser::{to_string, to_vec, to_writer, Serializer};
pub use crate::value::{from_value, to_value, Index, Number, Sequence, Value};

#[doc(inline)]
pub use crate::mapping::Mapping;

/// Entry points for deserializing with pre-existing state.
///
/// These functions are only exposed this way because we don't yet expose a
/// Deserializer type. Data formats that have a public Deserializer should not
/// copy these signatures.
pub mod seed {
    pub use super::de::{from_reader_seed, from_slice_seed, from_str_seed};
}

mod de;
mod error;
pub mod mapping;
mod number;
mod path;
mod ser;
mod value;
