// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::error::Error;
use crate::result::Result;
use std::fmt;

/// The Argon2 variant.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Variant {
    /// Argon2 using data-dependent memory access to thwart tradeoff attacks.
    /// Recommended for cryptocurrencies and backend servers.
    Argon2d = 0,

    /// Argon2 using data-independent memory access to thwart side-channel
    /// attacks. Recommended for password hashing and password-based key
    /// derivation.
    Argon2i = 1,

    /// Argon2 using hybrid construction.
    Argon2id = 2,
}

impl Variant {
    /// Gets the lowercase string slice representation of the variant.
    pub fn as_lowercase_str(&self) -> &'static str {
        match *self {
            Variant::Argon2d => "argon2d",
            Variant::Argon2i => "argon2i",
            Variant::Argon2id => "argon2id",
        }
    }

    /// Gets the u32 representation of the variant.
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }

    /// Gets the u64 representation of the variant.
    pub fn as_u64(&self) -> u64 {
        *self as u64
    }

    /// Gets the uppercase string slice representation of the variant.
    pub fn as_uppercase_str(&self) -> &'static str {
        match *self {
            Variant::Argon2d => "Argon2d",
            Variant::Argon2i => "Argon2i",
            Variant::Argon2id => "Argon2id",
        }
    }

    /// Attempts to create a variant from a string slice.
    pub fn from_str(str: &str) -> Result<Variant> {
        match str {
            "Argon2d" => Ok(Variant::Argon2d),
            "Argon2i" => Ok(Variant::Argon2i),
            "Argon2id" => Ok(Variant::Argon2id),
            "argon2d" => Ok(Variant::Argon2d),
            "argon2i" => Ok(Variant::Argon2i),
            "argon2id" => Ok(Variant::Argon2id),
            _ => Err(Error::DecodingFail),
        }
    }

    /// Attempts to create a variant from an u32.
    pub fn from_u32(val: u32) -> Result<Variant> {
        match val {
            0 => Ok(Variant::Argon2d),
            1 => Ok(Variant::Argon2i),
            2 => Ok(Variant::Argon2id),
            _ => Err(Error::IncorrectType),
        }
    }
}

impl Default for Variant {
    fn default() -> Variant {
        Variant::Argon2i
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_lowercase_str())
    }
}

#[cfg(test)]
mod tests {

    use crate::error::Error;
    use crate::variant::Variant;

    #[test]
    fn as_lowercase_str_returns_correct_str() {
        assert_eq!(Variant::Argon2d.as_lowercase_str(), "argon2d");
        assert_eq!(Variant::Argon2i.as_lowercase_str(), "argon2i");
        assert_eq!(Variant::Argon2id.as_lowercase_str(), "argon2id");
    }

    #[test]
    fn as_u32_returns_correct_u32() {
        assert_eq!(Variant::Argon2d.as_u32(), 0);
        assert_eq!(Variant::Argon2i.as_u32(), 1);
        assert_eq!(Variant::Argon2id.as_u32(), 2);
    }

    #[test]
    fn as_u64_returns_correct_u64() {
        assert_eq!(Variant::Argon2d.as_u64(), 0);
        assert_eq!(Variant::Argon2i.as_u64(), 1);
        assert_eq!(Variant::Argon2id.as_u64(), 2);
    }

    #[test]
    fn as_uppercase_str_returns_correct_str() {
        assert_eq!(Variant::Argon2d.as_uppercase_str(), "Argon2d");
        assert_eq!(Variant::Argon2i.as_uppercase_str(), "Argon2i");
        assert_eq!(Variant::Argon2id.as_uppercase_str(), "Argon2id");
    }

    #[test]
    fn default_returns_correct_variant() {
        assert_eq!(Variant::default(), Variant::Argon2i);
    }

    #[test]
    fn display_returns_correct_string() {
        assert_eq!(format!("{}", Variant::Argon2d), "argon2d");
        assert_eq!(format!("{}", Variant::Argon2i), "argon2i");
        assert_eq!(format!("{}", Variant::Argon2id), "argon2id");
    }

    #[test]
    fn from_str_returns_correct_result() {
        assert_eq!(Variant::from_str("Argon2d"), Ok(Variant::Argon2d));
        assert_eq!(Variant::from_str("Argon2i"), Ok(Variant::Argon2i));
        assert_eq!(Variant::from_str("Argon2id"), Ok(Variant::Argon2id));
        assert_eq!(Variant::from_str("argon2d"), Ok(Variant::Argon2d));
        assert_eq!(Variant::from_str("argon2i"), Ok(Variant::Argon2i));
        assert_eq!(Variant::from_str("argon2id"), Ok(Variant::Argon2id));
        assert_eq!(Variant::from_str("foobar"), Err(Error::DecodingFail));
    }

    #[test]
    fn from_u32_returns_correct_result() {
        assert_eq!(Variant::from_u32(0), Ok(Variant::Argon2d));
        assert_eq!(Variant::from_u32(1), Ok(Variant::Argon2i));
        assert_eq!(Variant::from_u32(2), Ok(Variant::Argon2id));
        assert_eq!(Variant::from_u32(3), Err(Error::IncorrectType));
    }
}
