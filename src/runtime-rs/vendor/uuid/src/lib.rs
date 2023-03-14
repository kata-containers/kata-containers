// Copyright 2013-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Generate and parse UUIDs
//!
//! Provides support for Universally Unique Identifiers (UUIDs). A UUID is a
//! unique 128-bit number, stored as 16 octets.  UUIDs are used to  assign
//! unique identifiers to entities without requiring a central allocating
//! authority.
//!
//! They are particularly useful in distributed systems, though can be used in
//! disparate areas, such as databases and network protocols.  Typically a UUID
//! is displayed in a readable string form as a sequence of hexadecimal digits,
//! separated into groups by hyphens.
//!
//! The uniqueness property is not strictly guaranteed, however for all
//! practical purposes, it can be assumed that an unintentional collision would
//! be extremely unlikely.
//!
//! # Dependencies
//!
//! This crate by default has no dependencies and is `#![no_std]` compatible.
//! The following Cargo features, however, can be used to enable various pieces
//! of functionality.
//!
//! * `use_std` - adds in functionality available when linking to the standard
//!   library, currently this is only the `impl Error for ParseError`.
//! * `v4` - adds the `Uuid::new_v4` function and the ability to randomly
//!   generate a `Uuid`.
//! * `v5` - adds the `Uuid::new_v5` function and the ability to create a V5
//!   UUID based on the SHA1 hash of some data.
//! * `rustc-serialize` - adds the ability to serialize and deserialize a `Uuid`
//!   using the `rustc-serialize` crate.
//! * `serde` - adds the ability to serialize and deserialize a `Uuid` using the
//!   `serde` crate.
//!
//! By default, `uuid` can be depended on with:
//!
//! ```toml
//! [dependencies]
//! uuid = "0.2"
//! ```
//!
//! To activate various features, use syntax like:
//!
//! ```toml
//! [dependencies]
//! uuid = { version = "0.2", features = ["serde", "v4"] }
//! ```
//!
//! # Examples
//!
//! To parse a UUID given in the simple format and print it as a urn:
//!
//! ```rust
//! use uuid::Uuid;
//!
//! fn main() {
//!     let my_uuid = Uuid::parse_str("936DA01F9ABD4d9d80C702AF85C822A8").unwrap();
//!     println!("{}", my_uuid.urn());
//! }
//! ```
//!
//! To create a new random (V4) UUID and print it out in hexadecimal form:
//!
//! ```ignore,rust
//! // Note that this requires the `v4` feature enabled in the uuid crate.
//!
//! use uuid::Uuid;
//!
//! fn main() {
//!     let my_uuid = Uuid::new_v4();
//!     println!("{}", my_uuid);
//! }
//! ```
//!
//! # Strings
//!
//! Examples of string representations:
//!
//! * simple: `936DA01F9ABD4d9d80C702AF85C822A8`
//! * hyphenated: `550e8400-e29b-41d4-a716-446655440000`
//! * urn: `urn:uuid:F9168C5E-CEB2-4faa-B6BF-329BF39FA1E4`
//!
//! # References
//!
//! * [Wikipedia: Universally Unique Identifier](
//!     http://en.wikipedia.org/wiki/Universally_unique_identifier)
//! * [RFC4122: A Universally Unique IDentifier (UUID) URN Namespace](
//!     http://tools.ietf.org/html/rfc4122)

#![doc(html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
       html_favicon_url = "https://www.rust-lang.org/favicon.ico",
       html_root_url = "https://doc.rust-lang.org/uuid/")]

#![deny(warnings)]
#![no_std]

#[cfg(feature = "v4")]
extern crate rand;
#[cfg(feature = "v5")]
extern crate sha1;


use core::fmt;
use core::hash;
use core::str::FromStr;

// rustc-serialize and serde link to std, so go ahead an pull in our own std
// support in those situations as well.
#[cfg(any(feature = "use_std",
          feature = "rustc-serialize",
          feature = "serde"))]
mod std_support;
#[cfg(feature = "rustc-serialize")]
mod rustc_serialize;
#[cfg(feature = "serde")]
mod serde;

#[cfg(feature = "v4")]
use rand::Rng;
#[cfg(feature = "v5")]
use sha1::Sha1;

/// A 128-bit (16 byte) buffer containing the ID.
pub type UuidBytes = [u8; 16];

/// The version of the UUID, denoting the generating algorithm.
#[derive(PartialEq, Copy, Clone)]
pub enum UuidVersion {
    /// Version 1: MAC address
    Mac = 1,
    /// Version 2: DCE Security
    Dce = 2,
    /// Version 3: MD5 hash
    Md5 = 3,
    /// Version 4: Random
    Random = 4,
    /// Version 5: SHA-1 hash
    Sha1 = 5,
}

/// The reserved variants of UUIDs.
#[derive(PartialEq, Copy, Clone)]
pub enum UuidVariant {
    /// Reserved by the NCS for backward compatibility
    NCS,
    /// As described in the RFC4122 Specification (default)
    RFC4122,
    /// Reserved by Microsoft for backward compatibility
    Microsoft,
    /// Reserved for future expansion
    Future,
}

/// A Universally Unique Identifier (UUID).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Uuid {
    /// The 128-bit number stored in 16 bytes
    bytes: UuidBytes,
}

/// An adaptor for formatting a `Uuid` as a simple string.
pub struct Simple<'a> {
    inner: &'a Uuid,
}

/// An adaptor for formatting a `Uuid` as a hyphenated string.
pub struct Hyphenated<'a> {
    inner: &'a Uuid,
}

/// A UUID of the namespace of fully-qualified domain names
pub const NAMESPACE_DNS: Uuid = Uuid {
    bytes: [0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1,
            0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8]
};

/// A UUID of the namespace of URLs
pub const NAMESPACE_URL: Uuid = Uuid {
    bytes: [0x6b, 0xa7, 0xb8, 0x11, 0x9d, 0xad, 0x11, 0xd1,
            0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8]
};

/// A UUID of the namespace of ISO OIDs
pub const NAMESPACE_OID: Uuid = Uuid {
    bytes: [0x6b, 0xa7, 0xb8, 0x12, 0x9d, 0xad, 0x11, 0xd1,
            0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8]
};

/// A UUID of the namespace of X.500 DNs (in DER or a text output format)
pub const NAMESPACE_X500: Uuid = Uuid {
    bytes: [0x6b, 0xa7, 0xb8, 0x14, 0x9d, 0xad, 0x11, 0xd1,
            0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8]
};


/// An adaptor for formatting a `Uuid` as a URN string.
pub struct Urn<'a> {
    inner: &'a Uuid,
}

/// Error details for string parsing failures.
#[allow(missing_docs)]
#[derive(PartialEq, Eq, Copy, Clone, Debug)]
pub enum ParseError {
    InvalidLength(usize),
    InvalidCharacter(char, usize),
    InvalidGroups(usize),
    InvalidGroupLength(usize, usize, u8),
}

const SIMPLE_LENGTH: usize = 32;
const HYPHENATED_LENGTH: usize = 36;

/// Converts a ParseError to a string.
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ParseError::InvalidLength(found) => {
                write!(f,
                       "Invalid length; expecting {} or {} chars, found {}",
                       SIMPLE_LENGTH, HYPHENATED_LENGTH, found)
            }
            ParseError::InvalidCharacter(found, pos) => {
                write!(f,
                       "Invalid character; found `{}` (0x{:02x}) at offset {}",
                       found,
                       found as usize,
                       pos)
            }
            ParseError::InvalidGroups(found) => {
                write!(f,
                       "Malformed; wrong number of groups: expected 1 or 5, found {}",
                       found)
            }
            ParseError::InvalidGroupLength(group, found, expecting) => {
                write!(f,
                       "Malformed; length of group {} was {}, expecting {}",
                       group,
                       found,
                       expecting)
            }
        }
    }
}

// Length of each hyphenated group in hex digits.
const GROUP_LENS: [u8; 5] = [8, 4, 4, 4, 12];
// Accumulated length of each hyphenated group in hex digits.
const ACC_GROUP_LENS: [u8; 5] = [8, 12, 16, 20, 32];

impl Uuid {
    /// The 'nil UUID'.
    ///
    /// The nil UUID is special form of UUID that is specified to have all
    /// 128 bits set to zero, as defined in [IETF RFC 4122 Section 4.1.7][RFC].
    ///
    /// [RFC]: https://tools.ietf.org/html/rfc4122.html#section-4.1.7
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use uuid::Uuid;
    ///
    /// let uuid = Uuid::nil();
    ///
    /// assert_eq!(uuid.hyphenated().to_string(),
    ///            "00000000-0000-0000-0000-000000000000");
    /// ```
    pub fn nil() -> Uuid {
        Uuid { bytes: [0; 16] }
    }

    /// Creates a new `Uuid`.
    ///
    /// Note that not all versions can be generated currently and `None` will be
    /// returned if the specified version cannot be generated.
    ///
    /// To generate a random UUID (`UuidVersion::Random`), then the `v4`
    /// feature must be enabled for this crate.
    pub fn new(v: UuidVersion) -> Option<Uuid> {
        match v {
            #[cfg(feature = "v4")]
            UuidVersion::Random => Some(Uuid::new_v4()),
            _ => None,
        }
    }

    /// Creates a random `Uuid`.
    ///
    /// This uses the `rand` crate's default task RNG as the source of random numbers.
    /// If you'd like to use a custom generator, don't use this method: use the
    /// [`rand::Rand trait`]'s `rand()` method instead.
    ///
    /// [`rand::Rand trait`]: ../../rand/rand/trait.Rand.html#tymethod.rand
    ///
    /// Note that usage of this method requires the `v4` feature of this crate
    /// to be enabled.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use uuid::Uuid;
    ///
    /// let uuid = Uuid::new_v4();
    /// ```
    #[cfg(feature = "v4")]
    pub fn new_v4() -> Uuid {
        rand::thread_rng().gen()
    }

    /// Creates a UUID using a name from a namespace, based on the SHA-1 hash.
    ///
    /// A number of namespaces are available as constants in this crate:
    ///
    /// * `NAMESPACE_DNS`
    /// * `NAMESPACE_URL`
    /// * `NAMESPACE_OID`
    /// * `NAMESPACE_X500`
    ///
    /// Note that usage of this method requires the `v5` feature of this crate
    /// to be enabled.
    #[cfg(feature = "v5")]
    pub fn new_v5(namespace: &Uuid, name: &str) -> Uuid {
        let mut hash = Sha1::new();
        hash.update(namespace.as_bytes());
        hash.update(name.as_bytes());
        let buffer = hash.digest().bytes();
        let mut uuid = Uuid { bytes: [0; 16] };
        copy_memory(&mut uuid.bytes, &buffer[..16]);
        uuid.set_variant(UuidVariant::RFC4122);
        uuid.set_version(UuidVersion::Sha1);
        uuid
    }

    /// Creates a `Uuid` from four field values.
    ///
    /// # Errors
    ///
    /// This function will return an error if `d4`'s length is not 8 bytes.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use uuid::Uuid;
    ///
    /// let d4 = [12, 3, 9, 56, 54, 43, 8, 9];
    ///
    /// let uuid = Uuid::from_fields(42, 12, 5, &d4);
    /// let uuid = uuid.map(|uuid| uuid.hyphenated().to_string());
    ///
    /// let expected_uuid = Ok(String::from("0000002a-000c-0005-0c03-0938362b0809"));
    ///
    /// assert_eq!(expected_uuid, uuid);
    /// ```
    ///
    /// An invalid length:
    ///
    /// ```
    /// use uuid::Uuid;
    /// use uuid::ParseError;
    ///
    /// let d4 = [12];
    ///
    /// let uuid = Uuid::from_fields(42, 12, 5, &d4);
    ///
    /// let expected_uuid = Err(ParseError::InvalidLength(1));
    ///
    /// assert_eq!(expected_uuid, uuid);
    /// ```
    pub fn from_fields(d1: u32,
                       d2: u16,
                       d3: u16,
                       d4: &[u8]) -> Result<Uuid, ParseError>  {
        if d4.len() != 8 {
            return Err(ParseError::InvalidLength(d4.len()))
        }

        Ok(Uuid {
            bytes: [
                (d1 >> 24) as u8,
                (d1 >> 16) as u8,
                (d1 >>  8) as u8,
                (d1 >>  0) as u8,
                (d2 >>  8) as u8,
                (d2 >>  0) as u8,
                (d3 >>  8) as u8,
                (d3 >>  0) as u8,
                d4[0], d4[1], d4[2], d4[3], d4[4], d4[5], d4[6], d4[7]
            ],
        })
    }

    /// Creates a `Uuid` using the supplied bytes.
    ///
    /// # Errors
    ///
    /// This function will return an error if `b` has any length other than 16.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// use uuid::Uuid;
    ///
    /// let bytes = [4, 54, 67, 12, 43, 2, 98, 76,
    ///              32, 50, 87, 5, 1, 33, 43, 87];
    ///
    /// let uuid = Uuid::from_bytes(&bytes);
    /// let uuid = uuid.map(|uuid| uuid.hyphenated().to_string());
    ///
    /// let expected_uuid = Ok(String::from("0436430c-2b02-624c-2032-570501212b57"));
    ///
    /// assert_eq!(expected_uuid, uuid);
    /// ```
    ///
    /// An incorrect number of bytes:
    ///
    /// ```
    /// use uuid::Uuid;
    /// use uuid::ParseError;
    ///
    /// let bytes = [4, 54, 67, 12, 43, 2, 98, 76];
    ///
    /// let uuid = Uuid::from_bytes(&bytes);
    ///
    /// let expected_uuid = Err(ParseError::InvalidLength(8));
    ///
    /// assert_eq!(expected_uuid, uuid);
    /// ```
    pub fn from_bytes(b: &[u8]) -> Result<Uuid, ParseError> {
        let len = b.len();
        if len != 16 {
            return Err(ParseError::InvalidLength(len));
        }

        let mut uuid = Uuid { bytes: [0; 16] };
        copy_memory(&mut uuid.bytes, b);
        Ok(uuid)
    }

    /// Specifies the variant of the UUID structure
    #[allow(dead_code)]
    fn set_variant(&mut self, v: UuidVariant) {
        // Octet 8 contains the variant in the most significant 3 bits
        self.bytes[8] = match v {
            UuidVariant::NCS => self.bytes[8] & 0x7f,                // b0xx...
            UuidVariant::RFC4122 => (self.bytes[8] & 0x3f) | 0x80,   // b10x...
            UuidVariant::Microsoft => (self.bytes[8] & 0x1f) | 0xc0, // b110...
            UuidVariant::Future => (self.bytes[8] & 0x1f) | 0xe0,    // b111...
        }
    }

    /// Returns the variant of the `Uuid` structure.
    ///
    /// This determines the interpretation of the structure of the UUID.
    /// Currently only the RFC4122 variant is generated by this module.
    ///
    /// * [Variant Reference](http://tools.ietf.org/html/rfc4122#section-4.1.1)
    pub fn get_variant(&self) -> Option<UuidVariant> {
        match self.bytes[8] {
            x if x & 0x80 == 0x00 => Some(UuidVariant::NCS),
            x if x & 0xc0 == 0x80 => Some(UuidVariant::RFC4122),
            x if x & 0xe0 == 0xc0 => Some(UuidVariant::Microsoft),
            x if x & 0xe0 == 0xe0 => Some(UuidVariant::Future),
            _ => None,
        }
    }

    /// Specifies the version number of the `Uuid`.
    #[allow(dead_code)]
    fn set_version(&mut self, v: UuidVersion) {
        self.bytes[6] = (self.bytes[6] & 0xF) | ((v as u8) << 4);
    }

    /// Returns the version number of the `Uuid`.
    ///
    /// This represents the algorithm used to generate the contents.
    ///
    /// Currently only the Random (V4) algorithm is supported by this
    /// module.  There are security and privacy implications for using
    /// older versions - see [Wikipedia: Universally Unique Identifier](
    /// http://en.wikipedia.org/wiki/Universally_unique_identifier) for
    /// details.
    ///
    /// * [Version Reference](http://tools.ietf.org/html/rfc4122#section-4.1.3)
    pub fn get_version_num(&self) -> usize {
        (self.bytes[6] >> 4) as usize
    }

    /// Returns the version of the `Uuid`.
    ///
    /// This represents the algorithm used to generate the contents
    pub fn get_version(&self) -> Option<UuidVersion> {
        let v = self.bytes[6] >> 4;
        match v {
            1 => Some(UuidVersion::Mac),
            2 => Some(UuidVersion::Dce),
            3 => Some(UuidVersion::Md5),
            4 => Some(UuidVersion::Random),
            5 => Some(UuidVersion::Sha1),
            _ => None,
        }
    }

    /// Return an array of 16 octets containing the UUID data
    ///
    /// # Examples
    ///
    /// ```
    /// use uuid::Uuid;
    ///
    /// let uuid = Uuid::nil();
    /// assert_eq!(uuid.as_bytes(), &[0; 16]);
    ///
    /// let uuid = Uuid::parse_str("936DA01F9ABD4d9d80C702AF85C822A8").unwrap();
    /// assert_eq!(uuid.as_bytes(),
    ///            &[147, 109, 160, 31, 154, 189, 77, 157,
    ///              128, 199, 2, 175, 133, 200, 34, 168]);
    /// ```
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.bytes
    }

    /// Returns a wrapper which when formatted via `fmt::Display` will format a
    /// string of 32 hexadecimal digits.
    ///
    /// # Examples
    ///
    /// ```
    /// use uuid::Uuid;
    ///
    /// let uuid = Uuid::nil();
    /// assert_eq!(uuid.simple().to_string(),
    ///            "00000000000000000000000000000000");
    /// ```
    pub fn simple(&self) -> Simple {
        Simple { inner: self }
    }

    /// Returns a wrapper which when formatted via `fmt::Display` will format a
    /// string of hexadecimal digits separated into groups with a hyphen.
    ///
    /// # Examples
    ///
    /// ```
    /// use uuid::Uuid;
    ///
    /// let uuid = Uuid::nil();
    /// assert_eq!(uuid.hyphenated().to_string(),
    ///            "00000000-0000-0000-0000-000000000000");
    /// ```
    pub fn hyphenated(&self) -> Hyphenated {
        Hyphenated { inner: self }
    }

    /// Returns a wrapper which when formatted via `fmt::Display` will format a
    /// string of the UUID as a full URN string.
    ///
    /// # Examples
    ///
    /// ```
    /// use uuid::Uuid;
    ///
    /// let uuid = Uuid::nil();
    /// assert_eq!(uuid.urn().to_string(),
    ///            "urn:uuid:00000000-0000-0000-0000-000000000000");
    /// ```
    pub fn urn(&self) -> Urn {
        Urn { inner: self }
    }

    /// Parses a `Uuid` from a string of hexadecimal digits with optional hyphens.
    ///
    /// Any of the formats generated by this module (simple, hyphenated, urn) are
    /// supported by this parsing function.
    pub fn parse_str(mut input: &str) -> Result<Uuid, ParseError> {
        // Ensure length is valid for any of the supported formats
        let len = input.len();
        if len == (HYPHENATED_LENGTH + 9) && input.starts_with("urn:uuid:") {
            input = &input[9..];
        } else if len != SIMPLE_LENGTH && len != HYPHENATED_LENGTH {
            return Err(ParseError::InvalidLength(len));
        }

        // `digit` counts only hexadecimal digits, `i_char` counts all chars.
        let mut digit = 0;
        let mut group = 0;
        let mut acc = 0;
        let mut buffer = [0u8; 16];

        for (i_char, chr) in input.chars().enumerate() {
            if digit as usize >= SIMPLE_LENGTH && group == 0 {
                return Err(ParseError::InvalidLength(len));
            }

            if digit % 2 == 0 {
                // First digit of the byte.
                match chr {
                    // Calulate upper half.
                    '0'...'9' => acc = chr as u8 - '0' as u8,
                    'a'...'f' => acc = chr as u8 - 'a' as u8 + 10,
                    'A'...'F' => acc = chr as u8 - 'A' as u8 + 10,
                    // Found a group delimiter
                    '-' => {
                        if ACC_GROUP_LENS[group] != digit {
                            // Calculate how many digits this group consists of in the input.
                            let found = if group > 0 {
                                digit - ACC_GROUP_LENS[group - 1]
                            } else {
                                digit
                            };
                            return Err(ParseError::InvalidGroupLength(group,
                                                                      found as usize,
                                                                      GROUP_LENS[group]));
                        }
                        // Next group, decrement digit, it is incremented again at the bottom.
                        group += 1;
                        digit -= 1;
                    }
                    _ => return Err(ParseError::InvalidCharacter(chr, i_char)),
                }
            } else {
                // Second digit of the byte, shift the upper half.
                acc *= 16;
                match chr {
                    '0'...'9' => acc += chr as u8 - '0' as u8,
                    'a'...'f' => acc += chr as u8 - 'a' as u8 + 10,
                    'A'...'F' => acc += chr as u8 - 'A' as u8 + 10,
                    '-' => {
                        // The byte isn't complete yet.
                        let found = if group > 0 {
                            digit - ACC_GROUP_LENS[group - 1]
                        } else {
                            digit
                        };
                        return Err(ParseError::InvalidGroupLength(group,
                                                                  found as usize,
                                                                  GROUP_LENS[group]));
                    }
                    _ => return Err(ParseError::InvalidCharacter(chr, i_char)),
                }
                buffer[(digit / 2) as usize] = acc;
            }
            digit += 1;
        }

        // Now check the last group.
        if group != 0 && group != 4 {
            return Err(ParseError::InvalidGroups(group + 1));
        } else if ACC_GROUP_LENS[4] != digit {
            return Err(ParseError::InvalidGroupLength(group,
                                                      (digit - ACC_GROUP_LENS[3]) as usize,
                                                      GROUP_LENS[4]));
        }

        Ok(Uuid::from_bytes(&mut buffer).unwrap())
    }

    /// Tests if the UUID is nil
    pub fn is_nil(&self) -> bool {
        self.bytes.iter().all(|&b| b == 0)
    }
}

fn copy_memory(dst: &mut [u8], src: &[u8]) {
    for (slot, val) in dst.iter_mut().zip(src.iter()) {
        *slot = *val;
    }
}

impl Default for Uuid {
    /// Returns the nil UUID, which is all zeroes
    fn default() -> Uuid {
        Uuid::nil()
    }
}

impl FromStr for Uuid {
    type Err = ParseError;

    /// Parse a hex string and interpret as a `Uuid`.
    ///
    /// Accepted formats are a sequence of 32 hexadecimal characters,
    /// with or without hyphens (grouped as 8, 4, 4, 4, 12).
    fn from_str(us: &str) -> Result<Uuid, ParseError> {
        Uuid::parse_str(us)
    }
}

impl fmt::Debug for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Uuid(\"{}\")", self.hyphenated())
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.hyphenated().fmt(f)
    }
}

impl hash::Hash for Uuid {
    fn hash<S: hash::Hasher>(&self, state: &mut S) {
        self.bytes.hash(state)
    }
}

impl<'a> fmt::Display for Simple<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for byte in self.inner.bytes.iter() {
            try!(write!(f, "{:02x}", byte));
        }
        Ok(())
    }
}

impl<'a> fmt::Display for Hyphenated<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let data1 = ((self.inner.bytes[0] as u32) << 24) |
                    ((self.inner.bytes[1] as u32) << 16) |
                    ((self.inner.bytes[2] as u32) <<  8) |
                    ((self.inner.bytes[3] as u32) <<  0);
        let data2 = ((self.inner.bytes[4] as u16) <<  8) |
                    ((self.inner.bytes[5] as u16) <<  0);
        let data3 = ((self.inner.bytes[6] as u16) <<  8) |
                    ((self.inner.bytes[7] as u16) <<  0);

        write!(f, "{:08x}-\
                   {:04x}-\
                   {:04x}-\
                   {:02x}{:02x}-\
                   {:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
               data1,
               data2,
               data3,
               self.inner.bytes[8],
               self.inner.bytes[9],
               self.inner.bytes[10],
               self.inner.bytes[11],
               self.inner.bytes[12],
               self.inner.bytes[13],
               self.inner.bytes[14],
               self.inner.bytes[15])
    }
}

impl<'a> fmt::Display for Urn<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "urn:uuid:{}", self.inner.hyphenated())
    }
}

/// Generates a random `Uuid` (V4 conformant).
#[cfg(feature = "v4")]
impl rand::Rand for Uuid {
    fn rand<R: rand::Rng>(rng: &mut R) -> Uuid {
        let mut uuid = Uuid { bytes: [0; 16] };
        rng.fill_bytes(&mut uuid.bytes);
        uuid.set_variant(UuidVariant::RFC4122);
        uuid.set_version(UuidVersion::Random);
        uuid
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use self::std::prelude::v1::*;

    use super::{NAMESPACE_DNS, NAMESPACE_URL, NAMESPACE_OID, NAMESPACE_X500};
    use super::{Uuid, UuidVariant, UuidVersion};

    #[cfg(feature = "v4")]
    use rand;

    fn new() -> Uuid {
        Uuid::parse_str("F9168C5E-CEB2-4FAA-B6BF-329BF39FA1E4").unwrap()
    }

    fn new2() -> Uuid {
        Uuid::parse_str("F9168C5E-CEB2-4FAB-B6BF-329BF39FA1E4").unwrap()
    }

    #[cfg(feature = "v5")]
    static FIXTURE_V5: &'static [(&'static Uuid, &'static str, &'static str)] = &[
        (&NAMESPACE_DNS, "example.org",    "aad03681-8b63-5304-89e0-8ca8f49461b5"),
        (&NAMESPACE_DNS, "rust-lang.org",  "c66bbb60-d62e-5f17-a399-3a0bd237c503"),
        (&NAMESPACE_DNS, "42",             "7c411b5e-9d3f-50b5-9c28-62096e41c4ed"),
        (&NAMESPACE_DNS, "lorem ipsum",    "97886a05-8a68-5743-ad55-56ab2d61cf7b"),
        (&NAMESPACE_URL, "example.org",    "54a35416-963c-5dd6-a1e2-5ab7bb5bafc7"),
        (&NAMESPACE_URL, "rust-lang.org",  "c48d927f-4122-5413-968c-598b1780e749"),
        (&NAMESPACE_URL, "42",             "5c2b23de-4bad-58ee-a4b3-f22f3b9cfd7d"),
        (&NAMESPACE_URL, "lorem ipsum",    "15c67689-4b85-5253-86b4-49fbb138569f"),
        (&NAMESPACE_OID, "example.org",    "34784df9-b065-5094-92c7-00bb3da97a30"),
        (&NAMESPACE_OID, "rust-lang.org",  "8ef61ecb-977a-5844-ab0f-c25ef9b8d5d6"),
        (&NAMESPACE_OID, "42",             "ba293c61-ad33-57b9-9671-f3319f57d789"),
        (&NAMESPACE_OID, "lorem ipsum",    "6485290d-f79e-5380-9e64-cb4312c7b4a6"),
        (&NAMESPACE_X500, "example.org",   "e3635e86-f82b-5bbc-a54a-da97923e5c76"),
        (&NAMESPACE_X500, "rust-lang.org", "26c9c3e9-49b7-56da-8b9f-a0fb916a71a3"),
        (&NAMESPACE_X500, "42",            "e4b88014-47c6-5fe0-a195-13710e5f6e27"),
        (&NAMESPACE_X500, "lorem ipsum",   "b11f79a5-1e6d-57ce-a4b5-ba8531ea03d0"),
    ];

    #[test]
    fn test_nil() {
        let nil = Uuid::nil();
        let not_nil = new();

        assert!(nil.is_nil());
        assert!(!not_nil.is_nil());
    }

    #[test]
    fn test_new() {
        if cfg!(feature = "v4") {
            let uuid1 = Uuid::new(UuidVersion::Random).unwrap();
            let s = uuid1.simple().to_string();

            assert!(s.len() == 32);
            assert!(uuid1.get_version().unwrap() == UuidVersion::Random);
        } else {
            assert!(Uuid::new(UuidVersion::Random).is_none());
        }

        // Test unsupported versions
        assert!(Uuid::new(UuidVersion::Mac) == None);
        assert!(Uuid::new(UuidVersion::Dce) == None);
        assert!(Uuid::new(UuidVersion::Md5) == None);
        assert!(Uuid::new(UuidVersion::Sha1) == None);
    }

    #[test]
    #[cfg(feature = "v4")]
    fn test_new_v4() {
        let uuid1 = Uuid::new_v4();

        assert!(uuid1.get_version().unwrap() == UuidVersion::Random);
        assert!(uuid1.get_variant().unwrap() == UuidVariant::RFC4122);
    }

    #[cfg(feature = "v5")]
    #[test]
    fn test_new_v5() {
        for &(ref ns, ref name, _) in FIXTURE_V5 {
            let uuid = Uuid::new_v5(*ns, *name);
            assert!(uuid.get_version().unwrap() == UuidVersion::Sha1);
            assert!(uuid.get_variant().unwrap() == UuidVariant::RFC4122);
        }
    }

    #[test]
    fn test_predefined_namespaces() {
        assert_eq!(NAMESPACE_DNS.hyphenated().to_string(),
                   "6ba7b810-9dad-11d1-80b4-00c04fd430c8");
        assert_eq!(NAMESPACE_URL.hyphenated().to_string(),
                   "6ba7b811-9dad-11d1-80b4-00c04fd430c8");
        assert_eq!(NAMESPACE_OID.hyphenated().to_string(),
                   "6ba7b812-9dad-11d1-80b4-00c04fd430c8");
        assert_eq!(NAMESPACE_X500.hyphenated().to_string(),
                   "6ba7b814-9dad-11d1-80b4-00c04fd430c8");
    }

    #[test]
    #[cfg(feature = "v4")]
    fn test_get_version_v4() {
        let uuid1 = Uuid::new_v4();

        assert!(uuid1.get_version().unwrap() == UuidVersion::Random);
        assert_eq!(uuid1.get_version_num(), 4);
    }

    #[cfg(feature = "v5")]
    #[test]
    fn test_get_version_v5() {
        let uuid2 = Uuid::new_v5(&NAMESPACE_DNS, "rust-lang.org");

        assert!(uuid2.get_version().unwrap() == UuidVersion::Sha1);
        assert_eq!(uuid2.get_version_num(), 5);
    }

    #[test]
    fn test_get_variant() {
        let uuid1 = new();
        let uuid2 = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let uuid3 = Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();
        let uuid4 = Uuid::parse_str("936DA01F9ABD4d9dC0C702AF85C822A8").unwrap();
        let uuid5 = Uuid::parse_str("F9168C5E-CEB2-4faa-D6BF-329BF39FA1E4").unwrap();
        let uuid6 = Uuid::parse_str("f81d4fae-7dec-11d0-7765-00a0c91e6bf6").unwrap();

        assert!(uuid1.get_variant().unwrap() == UuidVariant::RFC4122);
        assert!(uuid2.get_variant().unwrap() == UuidVariant::RFC4122);
        assert!(uuid3.get_variant().unwrap() == UuidVariant::RFC4122);
        assert!(uuid4.get_variant().unwrap() == UuidVariant::Microsoft);
        assert!(uuid5.get_variant().unwrap() == UuidVariant::Microsoft);
        assert!(uuid6.get_variant().unwrap() == UuidVariant::NCS);
    }

    #[test]
    fn test_parse_uuid_v4() {
        use super::ParseError::*;

        // Invalid
        assert_eq!(Uuid::parse_str(""), Err(InvalidLength(0)));
        assert_eq!(Uuid::parse_str("!"), Err(InvalidLength(1)));
        assert_eq!(Uuid::parse_str("F9168C5E-CEB2-4faa-B6BF-329BF39FA1E45"),
                   Err(InvalidLength(37)));
        assert_eq!(Uuid::parse_str("F9168C5E-CEB2-4faa-BBF-329BF39FA1E4"),
                   Err(InvalidLength(35)));
        assert_eq!(Uuid::parse_str("F9168C5E-CEB2-4faa-BGBF-329BF39FA1E4"),
                   Err(InvalidCharacter('G', 20)));
        assert_eq!(Uuid::parse_str("F9168C5E-CEB2-4faa-B6BFF329BF39FA1E4"),
                   Err(InvalidGroups(4)));
        assert_eq!(Uuid::parse_str("F9168C5E-CEB2-4faa"),
                   Err(InvalidLength(18)));
        assert_eq!(Uuid::parse_str("F9168C5E-CEB2-4faaXB6BFF329BF39FA1E4"),
                   Err(InvalidCharacter('X', 18)));
        assert_eq!(Uuid::parse_str("F9168C5E-CEB-24fa-eB6BFF32-BF39FA1E4"),
                   Err(InvalidGroupLength(1, 3, 4)));
        assert_eq!(Uuid::parse_str("01020304-1112-2122-3132-41424344"),
                   Err(InvalidGroupLength(4, 8, 12)));
        assert_eq!(Uuid::parse_str("67e5504410b1426f9247bb680e5fe0c"),
                   Err(InvalidLength(31)));
        assert_eq!(Uuid::parse_str("67e5504410b1426f9247bb680e5fe0c88"),
                   Err(InvalidLength(33)));
        assert_eq!(Uuid::parse_str("67e5504410b1426f9247bb680e5fe0cg8"),
                   Err(InvalidLength(33)));
        assert_eq!(Uuid::parse_str("67e5504410b1426%9247bb680e5fe0c8"),
                   Err(InvalidCharacter('%', 15)));
        assert_eq!(Uuid::parse_str("231231212212423424324323477343246663"),
                   Err(InvalidLength(36)));

        // Valid
        assert!(Uuid::parse_str("00000000000000000000000000000000").is_ok());
        assert!(Uuid::parse_str("67e55044-10b1-426f-9247-bb680e5fe0c8").is_ok());
        assert!(Uuid::parse_str("F9168C5E-CEB2-4faa-B6BF-329BF39FA1E4").is_ok());
        assert!(Uuid::parse_str("67e5504410b1426f9247bb680e5fe0c8").is_ok());
        assert!(Uuid::parse_str("01020304-1112-2122-3132-414243444546").is_ok());
        assert!(Uuid::parse_str("urn:uuid:67e55044-10b1-426f-9247-bb680e5fe0c8").is_ok());

        // Nil
        let nil = Uuid::nil();
        assert!(Uuid::parse_str("00000000000000000000000000000000").unwrap() == nil);
        assert!(Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap() == nil);

        // Round-trip
        let uuid_orig = new();
        let orig_str = uuid_orig.to_string();
        let uuid_out = Uuid::parse_str(&orig_str).unwrap();
        assert!(uuid_orig == uuid_out);

        // Test error reporting
        assert_eq!(Uuid::parse_str("67e5504410b1426f9247bb680e5fe0c"),
                   Err(InvalidLength(31)));
        assert_eq!(Uuid::parse_str("67e550X410b1426f9247bb680e5fe0cd"),
                   Err(InvalidCharacter('X', 6)));
        assert_eq!(Uuid::parse_str("67e550-4105b1426f9247bb680e5fe0c"),
                   Err(InvalidGroupLength(0, 6, 8)));
        assert_eq!(Uuid::parse_str("F9168C5E-CEB2-4faa-B6BF1-02BF39FA1E4"),
                   Err(InvalidGroupLength(3, 5, 4)));
    }

    #[test]
    fn test_to_simple_string() {
        let uuid1 = new();
        let s = uuid1.simple().to_string();

        assert!(s.len() == 32);
        assert!(s.chars().all(|c| c.is_digit(16)));
    }

    #[test]
    fn test_to_string() {
        let uuid1 = new();
        let s = uuid1.to_string();

        assert!(s.len() == 36);
        assert!(s.chars().all(|c| c.is_digit(16) || c == '-'));
    }

    #[test]
    fn test_display() {
        let uuid1 = new();
        let s = uuid1.to_string();

        assert_eq!(s, uuid1.hyphenated().to_string());
    }

    #[test]
    fn test_to_hyphenated_string() {
        let uuid1 = new();
        let s = uuid1.hyphenated().to_string();

        assert!(s.len() == 36);
        assert!(s.chars().all(|c| c.is_digit(16) || c == '-'));
    }

    #[cfg(feature = "v5")]
    #[test]
    fn test_v5_to_hypenated_string() {
        for &(ref ns, ref name, ref expected) in FIXTURE_V5 {
            let uuid = Uuid::new_v5(*ns, *name);
            assert_eq!(uuid.hyphenated().to_string(), *expected);
        }
    }

    #[test]
    fn test_to_urn_string() {
        let uuid1 = new();
        let ss = uuid1.urn().to_string();
        let s = &ss[9..];

        assert!(ss.starts_with("urn:uuid:"));
        assert!(s.len() == 36);
        assert!(s.chars().all(|c| c.is_digit(16) || c == '-'));
    }

    #[test]
    fn test_to_simple_string_matching() {
        let uuid1 = new();

        let hs = uuid1.hyphenated().to_string();
        let ss = uuid1.simple().to_string();

        let hsn = hs.chars().filter(|&c| c != '-').collect::<String>();

        assert!(hsn == ss);
    }

    #[test]
    fn test_string_roundtrip() {
        let uuid = new();

        let hs = uuid.hyphenated().to_string();
        let uuid_hs = Uuid::parse_str(&hs).unwrap();
        assert_eq!(uuid_hs, uuid);

        let ss = uuid.to_string();
        let uuid_ss = Uuid::parse_str(&ss).unwrap();
        assert_eq!(uuid_ss, uuid);
    }

    #[test]
    fn test_compare() {
        let uuid1 = new();
        let uuid2 = new2();

        assert!(uuid1 == uuid1);
        assert!(uuid2 == uuid2);
        assert!(uuid1 != uuid2);
        assert!(uuid2 != uuid1);
    }

    #[test]
    fn test_from_fields() {
        let d1: u32 = 0xa1a2a3a4;
        let d2: u16 = 0xb1b2;
        let d3: u16 = 0xc1c2;
        let d4 = [0xd1, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8];

        let u = Uuid::from_fields(d1, d2, d3, &d4).unwrap();

        let expected = "a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8";
        let result = u.simple().to_string();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_from_bytes() {
        let b = [0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2,
                 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8];

        let u = Uuid::from_bytes(&b).unwrap();
        let expected = "a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8";

        assert_eq!(u.simple().to_string(), expected);
    }

    #[test]
    fn test_as_bytes() {
        let u = new();
        let ub = u.as_bytes();

        assert_eq!(ub.len(), 16);
        assert!(!ub.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_bytes_roundtrip() {
        let b_in: [u8; 16] = [0xa1, 0xa2, 0xa3, 0xa4, 0xb1, 0xb2, 0xc1, 0xc2, 0xd1, 0xd2, 0xd3,
                              0xd4, 0xd5, 0xd6, 0xd7, 0xd8];

        let u = Uuid::from_bytes(&b_in).unwrap();

        let b_out = u.as_bytes();

        assert_eq!(&b_in, b_out);
    }

    #[test]
    fn test_operator_eq() {
        let u1 = new();
        let u2 = u1.clone();
        let u3 = new2();

        assert!(u1 == u1);
        assert!(u1 == u2);
        assert!(u2 == u1);

        assert!(u1 != u3);
        assert!(u3 != u1);
        assert!(u2 != u3);
        assert!(u3 != u2);
    }

    #[test]
    #[cfg(feature = "v4")]
    fn test_rand_rand() {
        let mut rng = rand::thread_rng();
        let u: Uuid = rand::Rand::rand(&mut rng);
        let ub = u.as_bytes();

        assert!(ub.len() == 16);
        assert!(!ub.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_iterbytes_impl_for_uuid() {
        let mut set = std::collections::HashSet::new();
        let id1 = new();
        let id2 = new2();
        set.insert(id1.clone());
        assert!(set.contains(&id1));
        assert!(!set.contains(&id2));
    }
}
