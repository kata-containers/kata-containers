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

/// The Argon2 version.
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum Version {
    /// Version 0x10.
    Version10 = 0x10,

    /// Version 0x13 (Recommended).
    Version13 = 0x13,
}

impl Version {
    /// Gets the u32 representation of the version.
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }

    /// Attempts to create a version from a string slice.
    pub fn from_str(str: &str) -> Result<Version> {
        match str {
            "16" => Ok(Version::Version10),
            "19" => Ok(Version::Version13),
            _ => Err(Error::DecodingFail),
        }
    }

    /// Attempts to create a version from an u32.
    pub fn from_u32(val: u32) -> Result<Version> {
        match val {
            0x10 => Ok(Version::Version10),
            0x13 => Ok(Version::Version13),
            _ => Err(Error::IncorrectVersion),
        }
    }
}

impl Default for Version {
    fn default() -> Version {
        Version::Version13
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_u32())
    }
}

#[cfg(test)]
mod tests {

    use crate::error::Error;
    use crate::version::Version;

    #[test]
    fn as_u32_returns_correct_u32() {
        assert_eq!(Version::Version10.as_u32(), 0x10);
        assert_eq!(Version::Version13.as_u32(), 0x13);
    }

    #[test]
    fn default_returns_correct_version() {
        assert_eq!(Version::default(), Version::Version13);
    }

    #[test]
    fn display_returns_correct_string() {
        assert_eq!(format!("{}", Version::Version10), "16");
        assert_eq!(format!("{}", Version::Version13), "19");
    }

    #[test]
    fn from_str_returns_correct_result() {
        assert_eq!(Version::from_str("16"), Ok(Version::Version10));
        assert_eq!(Version::from_str("19"), Ok(Version::Version13));
        assert_eq!(Version::from_str("11"), Err(Error::DecodingFail));
    }

    #[test]
    fn from_u32_returns_correct_result() {
        assert_eq!(Version::from_u32(0x10), Ok(Version::Version10));
        assert_eq!(Version::from_u32(0x13), Ok(Version::Version13));
        assert_eq!(Version::from_u32(0), Err(Error::IncorrectVersion));
    }
}
