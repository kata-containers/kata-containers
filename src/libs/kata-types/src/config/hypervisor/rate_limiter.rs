// Copyright (c) 2022-2023 Intel Corporation
//
// Copyright (c) 2024-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

// The DEFAULT_RATE_LIMITER_REFILL_TIME is used for calculating the rate at
// which a TokenBucket is replinished, in cases where a RateLimiter is
// applied to either network or disk I/O.
pub(crate) const DEFAULT_RATE_LIMITER_REFILL_TIME: u64 = 1000;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct TokenBucketConfig {
    pub size: u64,
    pub one_time_burst: Option<u64>,
    pub refill_time: u64,
}

/// Rate limiter configuration for rust vmm hypervisor
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RateLimiterConfig {
    /// Bandwidth rate limiter options
    pub bandwidth: Option<TokenBucketConfig>,
    /// Operations rate limiter options
    pub ops: Option<TokenBucketConfig>,
}

impl RateLimiterConfig {
    /// Helper function: Creates a `TokenBucketConfig` based on the provided rate and burst.
    /// Returns `None` if the `rate` is 0.
    fn create_token_bucket_config(
        rate: u64,
        one_time_burst: Option<u64>,
    ) -> Option<TokenBucketConfig> {
        if rate > 0 {
            Some(TokenBucketConfig {
                size: rate,
                one_time_burst,
                refill_time: DEFAULT_RATE_LIMITER_REFILL_TIME,
            })
        } else {
            None
        }
    }

    /// Creates a new `RateLimiterConfig` instance.
    ///
    /// If both `band_rate` and `ops_rate` are 0 (indicating no rate limiting configured),
    /// it returns `None`. Otherwise, it returns `Some(RateLimiterConfig)` containing
    /// the configured options.
    pub fn new(
        band_rate: u64,
        ops_rate: u64,
        band_onetime_burst: Option<u64>,
        ops_onetime_burst: Option<u64>,
    ) -> Option<RateLimiterConfig> {
        // Use the helper function to create `TokenBucketConfig` for bandwidth and ops
        let bandwidth = Self::create_token_bucket_config(band_rate, band_onetime_burst);
        let ops = Self::create_token_bucket_config(ops_rate, ops_onetime_burst);

        // Use pattern matching to concisely handle the final `Option<RateLimiterConfig>` return.
        // If both bandwidth and ops are `None`, the entire config is `None`.
        // Otherwise, return `Some` with the actual configured options.
        match (bandwidth, ops) {
            (None, None) => None,
            (b, o) => Some(RateLimiterConfig {
                bandwidth: b,
                ops: o,
            }),
        }
    }
}

// Unit tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_all_set() {
        let config = RateLimiterConfig::new(100, 50, Some(10), Some(5)).unwrap();
        assert_eq!(
            config.bandwidth,
            Some(TokenBucketConfig {
                size: 100,
                one_time_burst: Some(10),
                refill_time: DEFAULT_RATE_LIMITER_REFILL_TIME
            })
        );
        assert_eq!(
            config.ops,
            Some(TokenBucketConfig {
                size: 50,
                one_time_burst: Some(5),
                refill_time: DEFAULT_RATE_LIMITER_REFILL_TIME
            })
        );
    }

    #[test]
    fn test_new_bandwidth_only() {
        let config = RateLimiterConfig::new(100, 0, Some(10), None).unwrap();
        assert_eq!(
            config.bandwidth,
            Some(TokenBucketConfig {
                size: 100,
                one_time_burst: Some(10),
                refill_time: DEFAULT_RATE_LIMITER_REFILL_TIME
            })
        );
        assert_eq!(config.ops, None);
    }

    #[test]
    fn test_new_ops_only() {
        let config = RateLimiterConfig::new(0, 50, None, Some(5)).unwrap();
        assert_eq!(config.bandwidth, None);
        assert_eq!(
            config.ops,
            Some(TokenBucketConfig {
                size: 50,
                one_time_burst: Some(5),
                refill_time: DEFAULT_RATE_LIMITER_REFILL_TIME
            })
        );
    }

    #[test]
    fn test_new_no_burst() {
        let config = RateLimiterConfig::new(100, 50, None, None).unwrap();
        assert_eq!(
            config.bandwidth,
            Some(TokenBucketConfig {
                size: 100,
                one_time_burst: None,
                refill_time: DEFAULT_RATE_LIMITER_REFILL_TIME
            })
        );
        assert_eq!(
            config.ops,
            Some(TokenBucketConfig {
                size: 50,
                one_time_burst: None,
                refill_time: DEFAULT_RATE_LIMITER_REFILL_TIME
            })
        );
    }

    #[test]
    fn test_new_none_set() {
        let config = RateLimiterConfig::new(0, 0, None, None);
        assert_eq!(config, None);
    }
}
