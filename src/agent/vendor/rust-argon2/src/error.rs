// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::{error, fmt};

/// Error type for Argon2 errors.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// The output (hash) is too short (minimum is 4).
    OutputTooShort,

    /// The output (hash) is too long (maximum is 2^32 - 1).
    OutputTooLong,

    /// The password is too short (minimum is 0).
    PwdTooShort,

    /// The password is too long (maximum is 2^32 - 1).
    PwdTooLong,

    /// The salt is too short (minimum is 8).
    SaltTooShort,

    /// The salt is too long (maximum is 2^32 - 1).
    SaltTooLong,

    /// The associated data is too short (minimum is 0).
    AdTooShort,

    /// The associated data is too long (maximum is 2^32 - 1).
    AdTooLong,

    /// The secret value is too short (minimum is 0).
    SecretTooShort,

    /// The secret value is too long (maximum is 2^32 - 1).
    SecretTooLong,

    /// The time cost (passes) is too small (minimum is 1).
    TimeTooSmall,

    /// The time cost (passes) is too large (maximum is 2^32 - 1).
    TimeTooLarge,

    /// The memory cost is too small (minimum is 8 x parallelism).
    MemoryTooLittle,

    /// The memory cost is too large (maximum 2GiB on 32-bit or 4TiB on 64-bit).
    MemoryTooMuch,

    /// The number of lanes (parallelism) is too small (minimum is 1).
    LanesTooFew,

    /// The number of lanes (parallelism) is too large (maximum is 2^24 - 1).
    LanesTooMany,

    /// Incorrect Argon2 variant.
    IncorrectType,

    /// Incorrect Argon2 version.
    IncorrectVersion,

    /// The decoding of the encoded data has failed.
    DecodingFail,
}

impl Error {
    fn msg(&self) -> &str {
        match *self {
            Error::OutputTooShort => "Output is too short",
            Error::OutputTooLong => "Output is too long",
            Error::PwdTooShort => "Password is too short",
            Error::PwdTooLong => "Password is too long",
            Error::SaltTooShort => "Salt is too short",
            Error::SaltTooLong => "Salt is too long",
            Error::AdTooShort => "Associated data is too short",
            Error::AdTooLong => "Associated data is too long",
            Error::SecretTooShort => "Secret is too short",
            Error::SecretTooLong => "Secret is too long",
            Error::TimeTooSmall => "Time cost is too small",
            Error::TimeTooLarge => "Time cost is too large",
            Error::MemoryTooLittle => "Memory cost is too small",
            Error::MemoryTooMuch => "Memory cost is too large",
            Error::LanesTooFew => "Too few lanes",
            Error::LanesTooMany => "Too many lanes",
            Error::IncorrectType => "There is no such type of Argon2",
            Error::IncorrectVersion => "There is no such version of Argon2",
            Error::DecodingFail => "Decoding failed",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg())
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        self.msg()
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

impl From<base64::DecodeError> for Error {
    fn from(_: base64::DecodeError) -> Self {
        Error::DecodingFail
    }
}
