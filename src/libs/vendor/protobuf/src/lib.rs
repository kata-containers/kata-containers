//! # Library to read and write protocol buffers data
//!
//! # Version 2 is stable
//!
//! Currently developed branch of rust-protobuf [is 3](https://docs.rs/protobuf/%3E=3.0.0-alpha).
//! It has the same spirit as version 2, but contains numerous improvements like:
//! * runtime reflection for mutability, not just for access
//! * protobuf text format and JSON parsing (which rely on reflection)
//! * dynamic message support: work with protobuf data without generating code from schema
//!
//! Stable version of rust-protobuf will be supported until version 3 released.
//!
//! [Tracking issue for version 3](https://github.com/stepancheg/rust-protobuf/issues/518).
//!
//! # How to generate rust code
//!
//! There are several ways to generate rust code from `.proto` files
//!
//! ## Invoke `protoc` programmatically with protoc-rust crate (recommended)
//!
//! Have a look at readme in [protoc-rust crate](https://docs.rs/protoc-rust/=2).
//!
//! ## Use pure rust protobuf parser and code generator
//!
//! Readme should be in
//! [protobuf-codegen-pure crate](https://docs.rs/protobuf-codegen-pure/=2).
//!
//! ## Use protoc-gen-rust plugin
//!
//! Readme is [here](https://docs.rs/protobuf-codegen/=2).
//!
//! ## Generated code
//!
//! Have a look at generated files (for current development version),
//! used internally in rust-protobuf:
//!
//! * [descriptor.rs](https://github.com/stepancheg/rust-protobuf/blob/master/protobuf/src/descriptor.rs)
//!   for [descriptor.proto](https://github.com/stepancheg/rust-protobuf/blob/master/protoc-bin-vendored/include/google/protobuf/descriptor.proto)
//!   (that is part of Google protobuf)
//!
//! # Copy on write
//!
//! Rust-protobuf can be used with [bytes crate](https://github.com/tokio-rs/bytes).
//!
//! To enable `Bytes` you need to:
//!
//! 1. Enable `with-bytes` feature in rust-protobuf:
//!
//! ```
//! [dependencies]
//! protobuf = { version = "~2.0", features = ["with-bytes"] }
//! ```
//!
//! 2. Enable bytes option
//!
//! with `Customize` when codegen is invoked programmatically:
//!
//! ```ignore
//! protoc_rust::run(protoc_rust::Args {
//!     ...
//!     customize: Customize {
//!         carllerche_bytes_for_bytes: Some(true),
//!         carllerche_bytes_for_string: Some(true),
//!         ..Default::default()
//!     },
//! });
//! ```
//!
//! or in `.proto` file:
//!
//! ```ignore
//! import "rustproto.proto";
//!
//! option (rustproto.carllerche_bytes_for_bytes_all) = true;
//! option (rustproto.carllerche_bytes_for_string_all) = true;
//! ```
//!
//! With these options enabled, fields of type `bytes` or `string` are
//! generated as `Bytes` or `Chars` respectively. When `CodedInputStream` is constructed
//! from `Bytes` object, fields of these types get subslices of original `Bytes` object,
//! instead of being allocated on heap.
//!
//! # Accompanying crates
//!
//! * [`protoc-rust`](https://docs.rs/protoc-rust/=2)
//!   and [`protobuf-codegen-pure`](https://docs.rs/protobuf-codegen-pure/=2)
//!   can be used to rust code from `.proto` crates.
//! * [`protobuf-codegen`](https://docs.rs/protobuf-codegen/=2) for `protoc-gen-rust` protoc plugin.
//! * [`protoc`](https://docs.rs/protoc/=2) crate can be used to invoke `protoc` programmatically.
//! * [`protoc-bin-vendored`](https://docs.rs/protoc-bin-vendored/=2) contains `protoc` command
//!   packed into the crate.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

#[cfg(feature = "bytes")]
extern crate bytes;
#[cfg(feature = "with-serde")]
extern crate serde;
#[macro_use]
#[cfg(feature = "with-serde")]
extern crate serde_derive;
pub use crate::cached_size::CachedSize;
#[cfg(feature = "bytes")]
pub use crate::chars::Chars;
pub use crate::clear::Clear;
pub use crate::coded_input_stream::CodedInputStream;
pub use crate::coded_output_stream::CodedOutputStream;
pub use crate::enums::ProtobufEnum;
pub use crate::error::ProtobufError;
pub use crate::error::ProtobufResult;
#[allow(deprecated)]
pub use crate::message::parse_from_bytes;
#[cfg(feature = "bytes")]
#[allow(deprecated)]
pub use crate::message::parse_from_carllerche_bytes;
#[allow(deprecated)]
pub use crate::message::parse_from_reader;
#[allow(deprecated)]
pub use crate::message::parse_length_delimited_from;
#[allow(deprecated)]
pub use crate::message::parse_length_delimited_from_bytes;
#[allow(deprecated)]
pub use crate::message::parse_length_delimited_from_reader;
pub use crate::message::Message;
pub use crate::repeated::RepeatedField;
pub use crate::singular::SingularField;
pub use crate::singular::SingularPtrField;
pub use crate::unknown::UnknownFields;
pub use crate::unknown::UnknownFieldsIter;
pub use crate::unknown::UnknownValue;
pub use crate::unknown::UnknownValueRef;
pub use crate::unknown::UnknownValues;
pub use crate::unknown::UnknownValuesIter;

// generated
pub mod descriptor;
pub mod plugin;
pub mod rustproto;

pub mod wire_format;

mod clear;
mod coded_input_stream;
mod coded_output_stream;
pub mod compiler_plugin;
mod enums;
pub mod error;
pub mod ext;
pub mod json;
pub mod lazy;
mod lazy_v2;
mod message;
pub mod reflect;
mod repeated;
pub mod rt;
mod singular;
pub mod text_format;
pub mod types;
pub mod well_known_types;
mod well_known_types_util;

// used by test
#[cfg(test)]
#[path = "../../protobuf-test-common/src/hex.rs"]
mod hex;

// used by rust-grpc
pub mod descriptorx;

mod cached_size;
mod chars;
#[doc(hidden)] // used by codegen
pub mod rust;
mod strx;
mod unknown;
mod varint;
mod zigzag;

mod misc;

mod buf_read_iter;
mod buf_read_or_reader;

/// This symbol is in generated `version.rs`, include here for IDE
#[cfg(never)]
pub const VERSION: &str = "";
/// This symbol is in generated `version.rs`, include here for IDE
#[cfg(never)]
#[doc(hidden)]
pub const VERSION_IDENT: &str = "";
include!(concat!(env!("OUT_DIR"), "/version.rs"));
