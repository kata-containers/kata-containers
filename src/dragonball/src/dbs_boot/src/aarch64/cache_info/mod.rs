// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// Copyright 2023 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Implementation of cache_info module.
//! This module is abstracted from cache_info.rs file. It exposes a public
//! interface: read_cache_config. This function is used by fdt to read cache
//! information from host cache directory.
//! Besides, it defines Error and Result type for cache_info module. The error
//! will be sent to fdt error(ReadCacheInfo(CacheInfoError)).

/// Error types for cache_info.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Cannot read cache attribute from the path.
    #[error("Failed to read config info: '{attr}', from file: {path}.")]
    ReadCacheInfoFailure {
        /// attribute name
        attr: String,
        /// path of the attribute
        path: String,
    },
    /// The attribute is invalid.
    #[error("Invalid cache attribute found '{attr}': {value}.")]
    InvalidCacheAttr {
        /// attribute name
        attr: String,
        /// value of the attribute
        value: String,
    },
    /// Some optional attribute is missing.
    #[error("Missing optional attribute: '{attr}', cache_entry: {cache_entry:?}.")]
    MissingOptionalAttr {
        /// attribute name
        attr: String,
        /// value of the current cache entry
        cache_entry: CacheEntry,
    },
    /// Failure happends when reading cache information in multiple threads.
    #[error("Failed in parallel processing cache: {0}.")]
    FailedParallelProcessCache(String),
}

/// Result type for cache_info.
pub(crate) type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_error_display() {
        assert_eq!(
            format!(
                "{}",
                Error::ReadCacheInfoFailure {
                    attr: "level".to_string(),
                    path: "/testfile".to_string()
                }
            ),
            "Failed to read config info: 'level', from file: /testfile."
        );
        assert_eq!(
            format!(
                "{}",
                Error::InvalidCacheAttr {
                    attr: "level".to_string(),
                    value: "10".to_string()
                }
            ),
            "Invalid cache attribute found 'level': 10."
        );
        assert_eq!(
            format!(
                "{}",
                Error::MissingOptionalAttr{ attr: "level".to_string(), cache_entry: CacheEntry::default() }
            ),
            "Missing optional attribute: 'level', cache_entry: CacheEntry { level: 0, type_: Unified, size_: None, number_of_sets: None, line_size: None, cpus_per_unit: 0 }."
        );
        assert_eq!(
            format!(
                "{}",
                Error::FailedParallelProcessCache("error message".to_string())
            ),
            "Failed in parallel processing cache: error message."
        );
    }

    #[test]
    fn test_cache_error_debug() {
        assert_eq!(
            format!(
                "{:?}",
                Error::ReadCacheInfoFailure {
                    attr: "level".to_string(),
                    path: "/testfile".to_string()
                }
            ),
            "ReadCacheInfoFailure { attr: \"level\", path: \"/testfile\" }"
        );
        assert_eq!(
            format!(
                "{:?}",
                Error::InvalidCacheAttr {
                    attr: "level".to_string(),
                    value: "10".to_string()
                }
            ),
            "InvalidCacheAttr { attr: \"level\", value: \"10\" }"
        );
        assert_eq!(
            format!(
                "{:?}",
                Error::MissingOptionalAttr{ attr: "level".to_string(), cache_entry: CacheEntry::default() }
            ),
            "MissingOptionalAttr { attr: \"level\", cache_entry: CacheEntry { level: 0, type_: Unified, size_: None, number_of_sets: None, line_size: None, cpus_per_unit: 0 } }"
        );
        assert_eq!(
            format!(
                "{:?}",
                Error::FailedParallelProcessCache("error message".to_string())
            ),
            "FailedParallelProcessCache(\"error message\")"
        );
    }
}
