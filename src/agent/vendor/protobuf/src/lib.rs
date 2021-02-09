//! Library to read and write protocol buffers data.

#![deny(missing_docs)]
#![deny(intra_doc_link_resolution_failure)]
// Because we need compat with Rust 1.26
#![allow(bare_trait_objects)]

#[cfg(feature = "bytes")]
extern crate bytes;
#[cfg(feature = "with-serde")]
extern crate serde;
#[macro_use]
#[cfg(feature = "with-serde")]
extern crate serde_derive;
pub use cached_size::CachedSize;
#[cfg(feature = "bytes")]
pub use chars::Chars;
pub use clear::Clear;
pub use core::parse_from_bytes;
#[cfg(feature = "bytes")]
pub use core::parse_from_carllerche_bytes;
pub use core::parse_from_reader;
#[allow(deprecated)]
pub use core::parse_length_delimited_from;
#[allow(deprecated)]
pub use core::parse_length_delimited_from_bytes;
#[allow(deprecated)]
pub use core::parse_length_delimited_from_reader;
pub use core::Message;
pub use enums::ProtobufEnum;
pub use error::ProtobufError;
pub use error::ProtobufResult;
pub use repeated::RepeatedField;
pub use singular::SingularField;
pub use singular::SingularPtrField;
pub use stream::wire_format;
pub use stream::CodedInputStream;
pub use stream::CodedOutputStream;
pub use unknown::UnknownFields;
pub use unknown::UnknownFieldsIter;
pub use unknown::UnknownValue;
pub use unknown::UnknownValueRef;
pub use unknown::UnknownValues;
pub use unknown::UnknownValuesIter;

// generated
pub mod descriptor;
pub mod plugin;
pub mod rustproto;

mod any;

mod clear;
pub mod compiler_plugin;
mod core;
mod enums;
pub mod error;
pub mod ext;
pub mod lazy;
pub mod reflect;
mod repeated;
pub mod rt;
mod singular;
pub mod stream;
pub mod text_format;
pub mod types;
pub mod well_known_types;

// used by test
#[cfg(test)]
#[path = "../../protobuf-test-common/src/hex.rs"]
mod hex;

// used by rust-grpc
pub mod descriptorx;

mod cached_size;
#[cfg(feature = "bytes")]
mod chars;
#[doc(hidden)] // used by codegen
pub mod rust;
mod strx;
mod unknown;
mod varint;
mod zigzag;

mod misc;

mod buf_read_iter;

// so `use protobuf::*` could work in mod descriptor and well_known_types
mod protobuf {
    pub use cached_size::CachedSize;
    pub use clear::Clear;
    pub use core::*;
    pub use descriptor;
    pub use descriptorx;
    pub use enums::ProtobufEnum;
    pub use error::*;
    pub use ext;
    pub use lazy;
    pub use reflect;
    pub use repeated::RepeatedField;
    pub use rt;
    pub use singular::SingularField;
    pub use singular::SingularPtrField;
    pub use stream::*;
    pub use text_format;
    pub use types;
    pub use unknown::UnknownFields;
    pub use unknown::UnknownFieldsIter;
    pub use unknown::UnknownValue;
    pub use unknown::UnknownValueRef;
    pub use unknown::UnknownValues;
    pub use unknown::UnknownValuesIter;
    pub use well_known_types;
}

/// This symbol is in generated `version.rs`, include here for IDE
#[cfg(never)]
pub const VERSION: &str = "";
/// This symbol is in generated `version.rs`, include here for IDE
#[cfg(never)]
#[doc(hidden)]
pub const VERSION_IDENT: &str = "";
include!(concat!(env!("OUT_DIR"), "/version.rs"));
