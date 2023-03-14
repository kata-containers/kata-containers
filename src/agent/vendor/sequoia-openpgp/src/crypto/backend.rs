//! Concrete implementation of the crypto primitives used by the rest of the
//! crypto API.

pub(crate) mod sha1cd;

#[cfg(feature = "crypto-nettle")]
mod nettle;
#[cfg(feature = "crypto-nettle")]
pub use self::nettle::*;

#[cfg(feature = "crypto-rust")]
mod rust;
#[cfg(feature = "crypto-rust")]
pub use self::rust::*;

#[cfg(feature = "crypto-cng")]
mod cng;
#[cfg(feature = "crypto-cng")]
pub use self::cng::*;
