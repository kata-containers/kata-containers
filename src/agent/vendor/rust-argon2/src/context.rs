// Copyright (c) 2017 Martijn Rijkeboer <mrr@sru-systems.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::common;
use crate::config::Config;
use crate::error::Error;
use crate::result::Result;

/// Structure containing settings for the Argon2 algorithm. A combination of
/// the original argon2_context and argon2_instance_t.
#[derive(Debug, PartialEq)]
pub struct Context<'a> {
    /// The config for this context.
    pub config: Config<'a>,

    /// The length of a lane.
    pub lane_length: u32,

    /// The number of memory blocks.
    pub memory_blocks: u32,

    /// The password.
    pub pwd: &'a [u8],

    /// The salt.
    pub salt: &'a [u8],

    /// The length of a segment.
    pub segment_length: u32,
}

impl<'a> Context<'a> {
    /// Attempts to create a new context.
    pub fn new(config: Config<'a>, pwd: &'a [u8], salt: &'a [u8]) -> Result<Context<'a>> {
        if config.lanes < common::MIN_LANES {
            return Err(Error::LanesTooFew);
        } else if config.lanes > common::MAX_LANES {
            return Err(Error::LanesTooMany);
        }

        let lanes = config.lanes;
        if config.mem_cost < common::MIN_MEMORY {
            return Err(Error::MemoryTooLittle);
        } else if config.mem_cost > common::MAX_MEMORY {
            return Err(Error::MemoryTooMuch);
        } else if config.mem_cost < 8 * lanes {
            return Err(Error::MemoryTooLittle);
        }

        if config.time_cost < common::MIN_TIME {
            return Err(Error::TimeTooSmall);
        } else if config.time_cost > common::MAX_TIME {
            return Err(Error::TimeTooLarge);
        }

        let pwd_len = pwd.len();
        if pwd_len < common::MIN_PWD_LENGTH as usize {
            return Err(Error::PwdTooShort);
        } else if pwd_len > common::MAX_PWD_LENGTH as usize {
            return Err(Error::PwdTooLong);
        }

        let salt_len = salt.len();
        if salt_len < common::MIN_SALT_LENGTH as usize {
            return Err(Error::SaltTooShort);
        } else if salt_len > common::MAX_SALT_LENGTH as usize {
            return Err(Error::SaltTooLong);
        }

        let secret_len = config.secret.len();
        if secret_len < common::MIN_SECRET_LENGTH as usize {
            return Err(Error::SecretTooShort);
        } else if secret_len > common::MAX_SECRET_LENGTH as usize {
            return Err(Error::SecretTooLong);
        }

        let ad_len = config.ad.len();
        if ad_len < common::MIN_AD_LENGTH as usize {
            return Err(Error::AdTooShort);
        } else if ad_len > common::MAX_AD_LENGTH as usize {
            return Err(Error::AdTooLong);
        }

        if config.hash_length < common::MIN_HASH_LENGTH {
            return Err(Error::OutputTooShort);
        } else if config.hash_length > common::MAX_HASH_LENGTH {
            return Err(Error::OutputTooLong);
        }

        let mut memory_blocks = config.mem_cost;
        if memory_blocks < 2 * common::SYNC_POINTS * lanes {
            memory_blocks = 2 * common::SYNC_POINTS * lanes;
        }

        let segment_length = memory_blocks / (lanes * common::SYNC_POINTS);
        let memory_blocks = segment_length * (lanes * common::SYNC_POINTS);
        let lane_length = segment_length * common::SYNC_POINTS;

        Ok(Context {
            config,
            lane_length,
            memory_blocks,
            pwd,
            salt,
            segment_length,
        })
    }
}

#[cfg(test)]
mod tests {

    use crate::config::Config;
    use crate::context::Context;
    use crate::error::Error;
    use crate::thread_mode::ThreadMode;
    use crate::variant::Variant;
    use crate::version::Version;

    #[test]
    fn new_returns_correct_instance() {
        let config = Config {
            ad: b"additionaldata",
            hash_length: 32,
            lanes: 4,
            mem_cost: 4096,
            secret: b"secret",
            thread_mode: ThreadMode::Sequential,
            time_cost: 3,
            variant: Variant::Argon2i,
            version: Version::Version13,
        };
        let pwd = b"password";
        let salt = b"somesalt";
        let result = Context::new(config.clone(), pwd, salt);
        assert!(result.is_ok());

        let context = result.unwrap();
        assert_eq!(context.config, config);
        assert_eq!(context.pwd, pwd);
        assert_eq!(context.salt, salt);
        assert_eq!(context.memory_blocks, 4096);
        assert_eq!(context.segment_length, 256);
        assert_eq!(context.lane_length, 1024);
    }

    #[test]
    fn new_with_too_little_mem_cost_returns_correct_error() {
        let config = Config {
            mem_cost: 7,
            ..Default::default()
        };
        assert_eq!(
            Context::new(config, &[0u8; 8], &[0u8; 8]),
            Err(Error::MemoryTooLittle)
        );
    }

    #[test]
    fn new_with_less_than_8_x_lanes_mem_cost_returns_correct_error() {
        let config = Config {
            lanes: 4,
            mem_cost: 31,
            ..Default::default()
        };
        assert_eq!(
            Context::new(config, &[0u8; 8], &[0u8; 8]),
            Err(Error::MemoryTooLittle)
        );
    }

    #[test]
    fn new_with_too_small_time_cost_returns_correct_error() {
        let config = Config {
            time_cost: 0,
            ..Default::default()
        };
        assert_eq!(
            Context::new(config, &[0u8; 8], &[0u8; 8]),
            Err(Error::TimeTooSmall)
        );
    }

    #[test]
    fn new_with_too_few_lanes_returns_correct_error() {
        let config = Config {
            lanes: 0,
            ..Default::default()
        };
        assert_eq!(
            Context::new(config, &[0u8; 8], &[0u8; 8]),
            Err(Error::LanesTooFew)
        );
    }

    #[test]
    fn new_with_too_many_lanes_returns_correct_error() {
        let config = Config {
            lanes: 1 << 24,
            ..Default::default()
        };
        assert_eq!(
            Context::new(config, &[0u8; 8], &[0u8; 8]),
            Err(Error::LanesTooMany)
        );
    }

    #[test]
    fn new_with_too_short_salt_returns_correct_error() {
        let config = Default::default();
        let salt = [0u8; 7];
        assert_eq!(
            Context::new(config, &[0u8; 8], &salt),
            Err(Error::SaltTooShort)
        );
    }

    #[test]
    fn new_with_too_short_hash_length_returns_correct_error() {
        let config = Config {
            hash_length: 3,
            ..Default::default()
        };
        assert_eq!(
            Context::new(config, &[0u8; 8], &[0u8; 8]),
            Err(Error::OutputTooShort)
        );
    }
}
