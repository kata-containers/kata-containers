//! # `object`
//!
//! The `object` crate provides a unified interface to working with object files
//! across platforms. It supports reading object files and executable files,
//! and writing object files.
//!
//! See the [`File` struct](./read/struct.File.html) for details.

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

#[cfg(feature = "elf")]
pub mod elf;
#[cfg(feature = "macho")]
pub mod macho;
#[cfg(any(feature = "coff", feature = "pe"))]
pub mod pe;
