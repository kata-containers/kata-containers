//! A minimal implementation of SHA1 for rust.
//!
//! This implementation supports no_std which is the default mode.  The
//! following features are available and can be optionally enabled:
//!
//! * ``serde``: when enabled the `Digest` type can be serialized.
//! * ``std``: when enabled errors from this library implement `std::error::Error`
//!   and the `hexdigest` shortcut becomes available.
//!
//! **Note:** future versions of this crate with the old code are now under
//! `sha1_smol`, the `sha1` crate name with versions beyond the 0.6 line now
//! refer to the `RustCrypto` implementation.
//!
//! ## Example
//!
//! ```rust
//! # fn main() {
//!
//! let mut m = sha1_smol::Sha1::new();
//! m.update(b"Hello World!");
//! assert_eq!(m.digest().to_string(),
//!            "2ef7bde608ce5404e97d5f042f95f89f1c232871");
//! # }
//! ```
//!
//! The sha1 object can be updated multiple times.  If you only need to use
//! it once you can also use shortcuts (requires std):
//!
//! ```
//! # trait X { fn hexdigest(&self) -> &'static str { "2ef7bde608ce5404e97d5f042f95f89f1c232871" }}
//! # impl X for sha1_smol::Sha1 {}
//! # fn main() {
//! assert_eq!(sha1_smol::Sha1::from("Hello World!").hexdigest(),
//!            "2ef7bde608ce5404e97d5f042f95f89f1c232871");
//! # }
//! ```

pub use sha1_smol::*;
