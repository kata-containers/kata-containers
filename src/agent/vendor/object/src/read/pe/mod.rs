//! Support for reading ELF files.
//!
//! Defines traits to abstract over the difference between PE32/PE32+,
//! and implements read functionality in terms of these traits.
//!
//! This module reuses some of the COFF functionality.
//!
//! Also provides `PeFile` and related types which implement the `Object` trait.

mod file;
pub use file::*;

mod section;
pub use section::*;

pub use super::coff::{SectionTable, SymbolTable};
