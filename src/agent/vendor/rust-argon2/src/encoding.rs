// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::context::Context;
use crate::decoded::Decoded;
use crate::error::Error;
use crate::result::Result;
use crate::variant::Variant;
use crate::version::Version;
use base64;

/// Structure containing the options.
struct Options {
    mem_cost: u32,
    time_cost: u32,
    parallelism: u32,
}

/// Gets the base64 encoded length of a byte slice with the specified length.
pub fn base64_len(length: u32) -> u32 {
    let olen = (length / 3) << 2;
    match length % 3 {
        2 => olen + 3,
        1 => olen + 2,
        _ => olen,
    }
}

/// Attempts to decode the encoded string slice.
pub fn decode_string(encoded: &str) -> Result<Decoded> {
    let items: Vec<&str> = encoded.split('$').collect();
    if items.len() == 6 {
        decode_empty(items[0])?;
        let variant = decode_variant(items[1])?;
        let version = decode_version(items[2])?;
        let options = decode_options(items[3])?;
        let salt = base64::decode(items[4])?;
        let hash = base64::decode(items[5])?;

        Ok(Decoded {
            variant,
            version,
            mem_cost: options.mem_cost,
            time_cost: options.time_cost,
            parallelism: options.parallelism,
            salt,
            hash,
        })
    } else if items.len() == 5 {
        decode_empty(items[0])?;
        let variant = decode_variant(items[1])?;
        let options = decode_options(items[2])?;
        let salt = base64::decode(items[3])?;
        let hash = base64::decode(items[4])?;

        Ok(Decoded {
            variant,
            version: Version::Version10,
            mem_cost: options.mem_cost,
            time_cost: options.time_cost,
            parallelism: options.parallelism,
            salt,
            hash,
        })
    } else {
        Err(Error::DecodingFail)
    }
}

fn decode_empty(str: &str) -> Result<()> {
    if str == "" {
        Ok(())
    } else {
        Err(Error::DecodingFail)
    }
}

fn decode_options(str: &str) -> Result<Options> {
    let items: Vec<&str> = str.split(',').collect();
    if items.len() == 3 {
        Ok(Options {
            mem_cost: decode_option(items[0], "m")?,
            time_cost: decode_option(items[1], "t")?,
            parallelism: decode_option(items[2], "p")?,
        })
    } else {
        Err(Error::DecodingFail)
    }
}

fn decode_option(str: &str, name: &str) -> Result<u32> {
    let items: Vec<&str> = str.split('=').collect();
    if items.len() == 2 {
        if items[0] == name {
            decode_u32(items[1])
        } else {
            Err(Error::DecodingFail)
        }
    } else {
        Err(Error::DecodingFail)
    }
}

fn decode_u32(str: &str) -> Result<u32> {
    match str.parse() {
        Ok(i) => Ok(i),
        Err(_) => Err(Error::DecodingFail),
    }
}

fn decode_variant(str: &str) -> Result<Variant> {
    Variant::from_str(str)
}

fn decode_version(str: &str) -> Result<Version> {
    let items: Vec<&str> = str.split('=').collect();
    if items.len() == 2 {
        if items[0] == "v" {
            Version::from_str(items[1])
        } else {
            Err(Error::DecodingFail)
        }
    } else {
        Err(Error::DecodingFail)
    }
}

/// Encodes the hash and context.
pub fn encode_string(context: &Context, hash: &[u8]) -> String {
    format!(
        "${}$v={}$m={},t={},p={}${}${}",
        context.config.variant,
        context.config.version,
        context.config.mem_cost,
        context.config.time_cost,
        context.config.lanes,
        base64::encode_config(context.salt, base64::STANDARD_NO_PAD),
        base64::encode_config(hash, base64::STANDARD_NO_PAD),
    )
}

/// Gets the string length of the specified number.
pub fn num_len(number: u32) -> u32 {
    let mut len = 1;
    let mut num = number;
    while num >= 10 {
        len += 1;
        num /= 10;
    }
    len
}

#[cfg(test)]
mod tests {

    #[cfg(feature = "crossbeam-utils")]
    use crate::config::Config;
    #[cfg(feature = "crossbeam-utils")]
    use crate::context::Context;
    use crate::decoded::Decoded;
    #[cfg(feature = "crossbeam-utils")]
    use crate::encoding::encode_string;
    use crate::encoding::{base64_len, decode_string, num_len};
    use crate::error::Error;
    #[cfg(feature = "crossbeam-utils")]
    use crate::thread_mode::ThreadMode;
    use crate::variant::Variant;
    use crate::version::Version;

    #[test]
    fn base64_len_returns_correct_length() {
        let tests = vec![
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 6),
            (5, 7),
            (6, 8),
            (7, 10),
            (8, 11),
            (9, 12),
            (10, 14),
        ];
        for (len, expected) in tests {
            let actual = base64_len(len);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn decode_string_with_version10_returns_correct_result() {
        let encoded = "$argon2i$v=16$m=4096,t=3,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let expected = Decoded {
            variant: Variant::Argon2i,
            version: Version::Version10,
            mem_cost: 4096,
            time_cost: 3,
            parallelism: 1,
            salt: b"salt1234".to_vec(),
            hash: b"12345678901234567890123456789012".to_vec(),
        };
        let actual = decode_string(encoded).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn decode_string_with_version13_returns_correct_result() {
        let encoded = "$argon2i$v=19$m=4096,t=3,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let expected = Decoded {
            variant: Variant::Argon2i,
            version: Version::Version13,
            mem_cost: 4096,
            time_cost: 3,
            parallelism: 1,
            salt: b"salt1234".to_vec(),
            hash: b"12345678901234567890123456789012".to_vec(),
        };
        let actual = decode_string(encoded).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn decode_string_without_version_returns_correct_result() {
        let encoded = "$argon2i$m=4096,t=3,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let expected = Decoded {
            variant: Variant::Argon2i,
            version: Version::Version10,
            mem_cost: 4096,
            time_cost: 3,
            parallelism: 1,
            salt: b"salt1234".to_vec(),
            hash: b"12345678901234567890123456789012".to_vec(),
        };
        let actual = decode_string(encoded).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn decode_string_without_variant_returns_error_result() {
        let encoded = "$m=4096,t=3,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_with_empty_variant_returns_error_result() {
        let encoded = "$$m=4096,t=3,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_with_invalid_variant_returns_error_result() {
        let encoded = "$argon$m=4096,t=3,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_without_mem_cost_returns_error_result() {
        let encoded = "$argon2i$t=3,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_with_empty_mem_cost_returns_error_result() {
        let encoded = "$argon2i$m=,t=3,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_with_non_numeric_mem_cost_returns_error_result() {
        let encoded = "$argon2i$m=a,t=3,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_without_time_cost_returns_error_result() {
        let encoded = "$argon2i$m=4096,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_with_empty_time_cost_returns_error_result() {
        let encoded = "$argon2i$m=4096,t=,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_with_non_numeric_time_cost_returns_error_result() {
        let encoded = "$argon2i$m=4096,t=a,p=1\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_without_parallelism_returns_error_result() {
        let encoded = "$argon2i$m=4096,t=3\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_with_empty_parallelism_returns_error_result() {
        let encoded = "$argon2i$m=4096,t=3,p=\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_with_non_numeric_parallelism_returns_error_result() {
        let encoded = "$argon2i$m=4096,t=3,p=a\
                       $c2FsdDEyMzQ=$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_without_salt_returns_error_result() {
        let encoded = "$argon2i$m=4096,t=3,p=1\
                       $MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_without_hash_returns_error_result() {
        let encoded = "$argon2i$m=4096,t=3,p=a\
                       $c2FsdDEyMzQ=";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[test]
    fn decode_string_with_empty_hash_returns_error_result() {
        let encoded = "$argon2i$m=4096,t=3,p=a\
                       $c2FsdDEyMzQ=$";
        let result = decode_string(encoded);
        assert_eq!(result, Err(Error::DecodingFail));
    }

    #[cfg(feature = "crossbeam-utils")]
    #[test]
    fn encode_string_returns_correct_string() {
        let hash = b"12345678901234567890123456789012".to_vec();
        let config = Config {
            ad: &[],
            hash_length: hash.len() as u32,
            lanes: 1,
            mem_cost: 4096,
            secret: &[],
            thread_mode: ThreadMode::Parallel,
            time_cost: 3,
            variant: Variant::Argon2i,
            version: Version::Version13,
        };
        let pwd = b"password".to_vec();
        let salt = b"salt1234".to_vec();
        let context = Context::new(config, &pwd, &salt).unwrap();
        let expected = "$argon2i$v=19$m=4096,t=3,p=1\
                        $c2FsdDEyMzQ$MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI";
        let actual = encode_string(&context, &hash);
        assert_eq!(actual, expected);
    }

    #[test]
    fn num_len_returns_correct_length() {
        let tests = vec![
            (1, 1),
            (10, 2),
            (110, 3),
            (1230, 4),
            (12340, 5),
            (123457, 6),
        ];
        for (num, expected) in tests {
            let actual = num_len(num);
            assert_eq!(actual, expected);
        }
    }
}
