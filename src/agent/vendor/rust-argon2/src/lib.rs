// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Library for hashing passwords using
//! [Argon2](https://github.com/P-H-C/phc-winner-argon2), the password-hashing
//! function that won the
//! [Password Hashing Competition (PHC)](https://password-hashing.net).
//!
//! # Usage
//!
//! To use this crate, add the following to your Cargo.toml:
//!
//! ```toml
//! [dependencies]
//! rust-argon2 = "0.8"
//! ```
//!
//! And the following to your crate root:
//!
//! ```rust
//! extern crate argon2;
//! ```
//!
//! # Examples
//!
//! Create a password hash using the defaults and verify it:
//!
//! ```rust
//! use argon2::{self, Config};
//!
//! let password = b"password";
//! let salt = b"randomsalt";
//! let config = Config::default();
//! let hash = argon2::hash_encoded(password, salt, &config).unwrap();
//! let matches = argon2::verify_encoded(&hash, password).unwrap();
//! assert!(matches);
//! ```
//!
//! Create a password hash with custom settings and verify it:
//!
//! ```rust
//! use argon2::{self, Config, ThreadMode, Variant, Version};
//!
//! let password = b"password";
//! let salt = b"othersalt";
//! let config = Config {
//!     variant: Variant::Argon2i,
//!     version: Version::Version13,
//!     mem_cost: 65536,
//!     time_cost: 10,
#![cfg_attr(feature = "crossbeam-utils", doc = "    lanes: 4,")]
#![cfg_attr(
    feature = "crossbeam-utils",
    doc = "    thread_mode: ThreadMode::Parallel,"
)]
#![cfg_attr(not(feature = "crossbeam-utils"), doc = "    lanes: 1,")]
#![cfg_attr(
    not(feature = "crossbeam-utils"),
    doc = "    thread_mode: ThreadMode::Sequential,"
)]
//!     secret: &[],
//!     ad: &[],
//!     hash_length: 32
//! };
//! let hash = argon2::hash_encoded(password, salt, &config).unwrap();
//! let matches = argon2::verify_encoded(&hash, password).unwrap();
//! assert!(matches);
//! ```
//!
//! # Limitations
//!
//! This crate has the same limitation as the `blake2-rfc` crate that it uses.
//! It does not attempt to clear potentially sensitive data from its work
//! memory. To do so correctly without a heavy performance penalty would
//! require help from the compiler. It's better to not attempt to do so than to
//! present a false assurance.
//!
//! This version uses the standard implementation and does not yet implement
//! optimizations. Therefore, it is not the fastest implementation available.

mod argon2;
mod block;
mod common;
mod config;
mod context;
mod core;
mod decoded;
mod encoding;
mod error;
mod memory;
mod result;
mod thread_mode;
mod variant;
mod version;

pub use crate::argon2::*;
pub use crate::config::Config;
pub use crate::error::Error;
pub use crate::result::Result;
pub use crate::thread_mode::ThreadMode;
pub use crate::variant::Variant;
pub use crate::version::Version;
