//! # `object`
//!
//! The `object` crate provides a unified interface to working with object files
//! across platforms. It supports reading object files and executable files,
//! and writing object files.
//!
//! ## Raw struct definitions
//!
//! Raw structs are defined for: [ELF](elf), [Mach-O](macho), [PE/COFF](pe), [archive].
//! Types and traits for zerocopy support are defined in [pod] and [endian].
//!
//! ## Unified read API
//!
//! The [read::Object] trait defines the unified interace. This trait is implemented
//! by [read::File], which allows reading any file format, as well as implementations
//! for each file format: [ELF](read::elf::ElfFile), [Mach-O](read::macho::MachOFile),
//! [COFF](read::coff::CoffFile), [PE](read::pe::PeFile), [Wasm](read::wasm::WasmFile).
//!
//! ## Low level read API
//!
//! In addition to the unified read API, the various `read` modules define helpers that
//! operate on the raw structs. These also provide traits that abstract over the differences
//! between 32-bit and 64-bit versions of the file format.
//!
//! ## Unified write API
//!
//! [write::Object] allows building an object and then writing it out.

#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![no_std]

#[cfg(feature = "cargo-all")]
compile_error!("'--all-features' is not supported; use '--features all' instead");

#[allow(unused_imports)]
#[macro_use]
extern crate alloc;

#[cfg(feature = "std")]
#[allow(unused_imports)]
#[macro_use]
extern crate std;

mod common;
pub use common::*;

#[macro_use]
pub mod endian;
pub use endian::*;

#[macro_use]
pub mod pod;
pub use pod::*;

#[cfg(feature = "read_core")]
pub mod read;
#[cfg(feature = "read_core")]
pub use read::*;

#[cfg(feature = "write_core")]
pub mod write;

#[cfg(feature = "archive")]
pub mod archive;
#[cfg(feature = "elf")]
pub mod elf;
#[cfg(feature = "macho")]
pub mod macho;
#[cfg(any(feature = "coff", feature = "pe"))]
pub mod pe;
