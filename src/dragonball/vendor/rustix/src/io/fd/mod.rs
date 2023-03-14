//! The following is derived from Rust's
//! library/std/src/os/fd/mod.rs at revision
//! dca3f1b786efd27be3b325ed1e01e247aa589c3b.
//!
//! Owned and borrowed Unix-like file descriptors.

#![cfg_attr(staged_api, unstable(feature = "io_safety", issue = "87074"))]
#![deny(unsafe_op_in_unsafe_fn)]

// `RawFd`, `AsRawFd`, etc.
mod raw;

// `OwnedFd`, `AsFd`, etc.
mod owned;

pub use owned::*;
pub use raw::*;
