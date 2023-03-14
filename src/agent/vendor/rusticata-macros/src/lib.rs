//! # Rusticata-macros
//!
//! Helper macros for the [rusticata](https://github.com/rusticata) project.
//!
//! This crate contains some additions to [nom](https://github.com/Geal/nom).
//!
//! For example, the [`combinator::cond_else`] function allows to apply the first parser if the
//! condition is true, and the second if the condition is false:
//!
//! ```rust
//! # use nom::IResult;
//! # use nom::combinator::map;
//! # use nom::number::streaming::*;
//! use rusticata_macros::combinator::cond_else;
//! # fn parser(s:&[u8]) {
//! let r: IResult<_, _, ()> = cond_else(
//!         || s.len() > 1,
//!         be_u16,
//!         map(be_u8, u16::from)
//!     )(s);
//! # }
//! ```
//!
//! See the documentation for more details and examples.

#![deny(
    missing_docs,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]

pub mod combinator;
pub mod debug;
pub use macros::*;
#[macro_use]
pub mod macros;

mod traits;
pub use traits::*;

// re-exports
pub use nom;
