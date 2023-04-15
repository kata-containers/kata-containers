// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// Copyright 2023 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Implementation of cache_info module.
//! This module is abstracted from cache_info.rs file. It exposes a public
//! interface: read_cache_config. This function is used by fdt to read cache
//! information from host cache files.
//! Besides, it defines Error and Result type for cache_info module. The error
//! will be sent to fdt error(ReadCacheInfo(CacheInfoError)).

use std::collections::HashMap;

use log::debug;

/// Module for CacheEngine.
pub(crate) mod cache_engine;
use cache_engine::CacheEngine;

/// Module for CacheEntry.
pub(crate) mod cache_entry;
use cache_entry::*;

/// Module for CacheType.
pub(crate) mod cache_type;

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

/// Default max cache level.
/// Based on https://elixir.free-electrons.com/linux/v4.9.62/source/arch/arm64/kernel/cacheinfo.c#L29.
const MAX_CACHE_LEVEL: usize = 7;

/// CacheInfoMap abstracts the type of mapping relationship between vcpu and
/// cache information.
type CacheInfoMap = HashMap<usize, (Vec<CacheEntry>, Vec<CacheEntry>)>;

/// Public interface to read cache information from host sysfs.
///
/// # Arguments
/// * `cpu_map`: mappings from vcpu to pcpu
/// * `max_level_opt`: max cache level, default is `MAX_CACHE_LEVEL`
pub(crate) fn read_cache_config(
    cpu_map: Vec<u8>,
    max_level_opt: Option<usize>,
) -> Result<CacheInfoMap> {
    let engine = CacheEngine::new(cpu_map);

    let cache_info = if let Some(max_level) = max_level_opt {
        engine.get_cache_information(max_level)?
    } else {
        engine.get_cache_information(MAX_CACHE_LEVEL)?
    };

    debug!("Successfully read cache information: {cache_info:?}");

    Ok(cache_info)
}

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;

    use super::*;

    fn helper_check_attributes(
        value: &CacheEntry,
        cache_dir: &str,
        level: u8,
        d_cache: Option<bool>,
    ) {
        let level_dir = format!(
            "{cache_dir}/index{}",
            if let Some(is_d_cache) = d_cache {
                if is_d_cache {
                    0
                } else {
                    1
                }
            } else {
                level
            }
        );

        if let Some(size) = value.size_ {
            let path = format!("{level_dir}/size");
            let mut s = read_to_string(path.as_str()).unwrap().trim().to_string();
            let s = get_cache_size_value(&mut s).unwrap();
            assert_eq!(size, s);
        }
        if let Some(nos) = value.number_of_sets {
            let path = format!("{level_dir}/number_of_sets");
            let number_of_sets = read_to_string(path.as_str()).unwrap().trim().to_string();
            assert_eq!(nos, number_of_sets.parse::<u32>().unwrap());
        }
        if let Some(line_size) = value.line_size {
            let path = format!("{level_dir}/coherency_line_size");
            let coherency_line_size = read_to_string(path.as_str()).unwrap().trim().to_string();
            assert_eq!(line_size, coherency_line_size.parse::<u16>().unwrap());
        }
        let path = format!("{level_dir}/shared_cpu_map");
        let shared_cpu_map = read_to_string(path.as_str()).unwrap().trim().to_string();
        assert_eq!(
            value.cpus_per_unit,
            mask_str2bit_count(shared_cpu_map.as_str()).unwrap()
        );
    }

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

    #[test]
    fn test_cache_cacheinfo_read_cache_config() {
        let success = read_cache_config((0..128).collect::<Vec<u8>>(), Some(3));
        assert!(success.is_ok());
        let cache_map = success.unwrap();
        assert_eq!(cache_map.len(), 128);
        for (&key, value) in cache_map.iter() {
            assert!(key < 128);
            assert_eq!(value.0.len(), 2);
            assert_eq!(value.1.len(), 2);

            let cache_dir = format!("/sys/devices/system/cpu/cpu{key}/cache");

            // l1 d-cache
            assert_eq!(value.0[0].level, 1);
            assert_eq!(format!("{:?}", value.0[0].type_), "Data");
            helper_check_attributes(value.0.get(0).unwrap(), cache_dir.as_str(), 1, Some(true));

            // l1 i-cache
            assert_eq!(value.0[1].level, 1);
            assert_eq!(format!("{:?}", value.0[1].type_), "Instruction");
            helper_check_attributes(value.0.get(1).unwrap(), cache_dir.as_str(), 1, Some(true));

            // l2 cache
            assert_eq!(value.1[0].level, 2);
            assert_eq!(format!("{:?}", value.1[0].type_), "Unified");
            helper_check_attributes(value.1.get(0).unwrap(), cache_dir.as_str(), 2, None);

            // l3 cache
            assert_eq!(value.1[1].level, 3);
            assert_eq!(format!("{:?}", value.1[1].type_), "Unified");
            helper_check_attributes(value.1.get(1).unwrap(), cache_dir.as_str(), 3, None);
        }

        let failure = read_cache_config((0..128).collect::<Vec<u8>>(), None);
        assert!(failure.is_err());
        assert_eq!(
            format!("{}", failure.unwrap_err()),
            "Failed to read config info: 'level', from file: /sys/devices/system/cpu/cpu0/cache/index4/level."
            .to_string()
        );
    }
}
