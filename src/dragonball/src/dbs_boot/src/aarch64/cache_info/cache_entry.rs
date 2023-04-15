// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// Copyright 2023 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Implementation of CacheEntry for cache_info.
//! CacheEntry is an abstract for the cache information of a specific level
//! of a specific cpu. For example, a 3-level cpu has four CacheEntries.
//! They are:
//!     1. level 1 instruction cache.
//!     2. level 1 data cache.
//!     3. level 2 unified cache.
//!     4. level 3 unified cache.

use std::fs::read_to_string;
use std::path::PathBuf;

use super::cache_type::CacheType;
use super::{Error, Result};

/// Struct for the cache information of a specific level of a specific cpu.
#[derive(Debug, Default)]
pub struct CacheEntry {
    /// cache level: 1, 2, 3...
    pub level: u8,
    /// cache type: Data, Instruction, Unified.
    pub type_: CacheType,
    /// cache size
    pub size_: Option<usize>,
    /// number of cache sets
    pub number_of_sets: Option<u32>,
    /// cache line size
    pub line_size: Option<u16>,
    /// the sharing cpu number of this CacheEntry
    pub cpus_per_unit: u16,
}

impl CacheEntry {
    /// Core method to get the specific CacheEntry. Read all the attributes,
    /// then generate CacheEntry struct, and return it.
    ///
    /// # Arguments
    /// * `cpu_idx`: cpu index ID
    /// * `cache_idx`: cache index ID
    pub(crate) fn get_cache_entry(cpu_idx: u8, cache_idx: u8) -> Result<CacheEntry> {
        let mut err_str = String::new();
        let mut cache_entry: CacheEntry = CacheEntry::default();

        // If cache level or type cannot be retrieved, just stop the process and exit.
        let level = read_attribute(cpu_idx, cache_idx, "level")?;
        cache_entry.level = level.parse::<u8>().map_err(|_| Error::InvalidCacheAttr {
            attr: "level".to_string(),
            value: level,
        })?;
        let cache_type = read_attribute(cpu_idx, cache_idx, "type")?;
        cache_entry.type_ =
            CacheType::try_from(&cache_type).map_err(|_| Error::InvalidCacheAttr {
                attr: "type".to_string(),
                value: cache_type,
            })?;

        if let Ok(mut size) = read_attribute(cpu_idx, cache_idx, "size") {
            cache_entry.size_ = Some(get_cache_size_value(&mut size)?);
        } else {
            err_str += "size, ";
        }

        if let Ok(number_of_sets) = read_attribute(cpu_idx, cache_idx, "number_of_sets") {
            cache_entry.number_of_sets =
                Some(
                    number_of_sets
                        .parse::<u32>()
                        .map_err(|_| Error::InvalidCacheAttr {
                            attr: "number_of_sets".to_string(),
                            value: number_of_sets,
                        })?,
                );
        } else {
            err_str += "number of sets, ";
        }

        if let Ok(coherency_line_size) = read_attribute(cpu_idx, cache_idx, "coherency_line_size") {
            cache_entry.line_size =
                Some(
                    coherency_line_size
                        .parse::<u16>()
                        .map_err(|_| Error::InvalidCacheAttr {
                            attr: "coherency_line_size".to_string(),
                            value: coherency_line_size,
                        })?,
                );
        } else {
            err_str += "coherency line size, ";
        }

        if let Ok(shared_cpu_map) = read_attribute(cpu_idx, cache_idx, "shared_cpu_map") {
            cache_entry.cpus_per_unit = mask_str2bit_count(shared_cpu_map.trim_end())?;
        } else {
            err_str += "shared cpu map, ";
        }

        // Pop the last 2 chars if a comma and space are present.The
        // unwrap is safe since we check that the string actually ends
        // with those 2 chars.
        if err_str.ends_with(", ") {
            err_str.pop().unwrap();
            err_str.pop().unwrap();
        }

        if !err_str.is_empty() {
            return Err(Error::MissingOptionalAttr {
                attr: err_str,
                cache_entry,
            });
        }

        Ok(cache_entry)
    }
}

/// Helper function to read cache attributes for cache_info.
/// The path is:
/// "/sys/devices/system/cpu/cpu{`cpu_idx`}/cache/index{`cache_idx`}/{`attr`}"
///
/// # Arguments
/// * `cpu_idx`: cpu index ID
/// * `cache_idx`: cache index ID
/// * `attr`: the attribute file name
pub(crate) fn read_attribute(cpu_idx: u8, cache_idx: u8, attr: &str) -> Result<String> {
    let path = PathBuf::from(format!(
        "/sys/devices/system/cpu/cpu{}/cache/index{}/{}",
        cpu_idx, cache_idx, attr
    ));
    let line = read_to_string(&path).map_err(|_| Error::ReadCacheInfoFailure {
        attr: attr.to_string(),
        path: path.display().to_string(),
    })?;
    Ok(line.trim_end().to_string())
}

/// Helper function to convert cache size string into bytes.
/// For example:
/// 10K -> 10 * 1024
/// 10M -> 10 * 1024 * 1024
///
/// # Arguments
/// * `cache_size_pretty`: cahe_size string read from sysfs
pub(crate) fn get_cache_size_value(cache_size_pretty: &mut String) -> Result<usize> {
    match cache_size_pretty.pop() {
        Some('K') => {
            Ok(cache_size_pretty
                .parse::<usize>()
                .map_err(|_| Error::InvalidCacheAttr {
                    attr: "size".to_string(),
                    value: format!("{cache_size_pretty}K"),
                })?
                * 1024)
        }
        Some('M') => {
            Ok(cache_size_pretty
                .parse::<usize>()
                .map_err(|_| Error::InvalidCacheAttr {
                    attr: "size".to_string(),
                    value: format!("{cache_size_pretty}M"),
                })?
                * 1024
                * 1024)
        }
        Some(letter) => {
            cache_size_pretty.push(letter);
            Err(Error::InvalidCacheAttr {
                attr: "size".to_string(),
                value: (*cache_size_pretty).to_string(),
            })
        }
        _ => Err(Error::InvalidCacheAttr {
            attr: "size".to_string(),
            value: "Empty string was provided".to_string(),
        }),
    }
}

/// Helper function to count the number of set bits from a bitmap
/// formatted string (see %*pb in the printk formats).
/// Expected input is a list of 32-bit comma separated hex values,
/// without the 0x prefix.
///
/// # Arguments
/// * `mask_str`: mask string for shared cpus
pub(crate) fn mask_str2bit_count(mask_str: &str) -> Result<u16> {
    let split_mask_iter = mask_str.split(',');
    let mut bit_count: u16 = 0;

    for s in split_mask_iter {
        let s_zero_free = s.trim_start_matches('0');
        if s_zero_free.is_empty() {
            continue;
        }
        bit_count += u32::from_str_radix(s_zero_free, 16)
            .map_err(|_| Error::InvalidCacheAttr {
                attr: "shared_cpu_map".to_string(),
                value: s.to_string(),
            })?
            .count_ones() as u16;
    }
    if bit_count == 0 {
        return Err(Error::InvalidCacheAttr {
            attr: "shared_cpu_map".to_string(),
            value: mask_str.to_string(),
        });
    }

    Ok(bit_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_cacheentry_default() {
        let entry = CacheEntry::default();
        assert_eq!(entry.level, 0);
        assert_eq!(format!("{:?}", entry.type_), "Unified".to_string());
        assert_eq!(entry.size_, None);
        assert_eq!(entry.number_of_sets, None);
        assert_eq!(entry.line_size, None);
        assert_eq!(entry.cpus_per_unit, 0);
    }

    #[test]
    fn test_cache_cacheentry_debug() {
        let entry = CacheEntry::default();
        assert_eq!(
	    format!("{:?}", entry),
	    "CacheEntry { level: 0, type_: Unified, size_: None, number_of_sets: None, line_size: None, cpus_per_unit: 0 }"
		.to_string()
	);
    }

    #[test]
    fn test_cache_cacheentry_read_attribute() {
        let fail_ret = read_attribute(0, 10, "level");
        assert!(fail_ret.is_err());
        assert_eq!(
	    format!("{}", fail_ret.unwrap_err()),
	    "Failed to read config info: 'level', from file: /sys/devices/system/cpu/cpu0/cache/index10/level."
		.to_string()
	);

        let success_ret = read_attribute(0, 0, "level");
        assert!(success_ret.is_ok());
        assert_eq!(success_ret.unwrap(), "1".to_string());
    }

    #[test]
    fn test_cache_cacheentry_get_cache_size_value() {
        fn test_func(mut test: String) -> Result<usize> {
            get_cache_size_value(&mut test)
        }

        let test = "10K".to_string();
        let ret = test_func(test);
        assert!(ret.is_ok());
        assert_eq!(ret.unwrap(), 10 * 1024);

        let test = "10M".to_string();
        let ret = test_func(test);
        assert!(ret.is_ok());
        assert_eq!(ret.unwrap(), 10 * 1024 * 1024);

        let test = "10".to_string();
        let ret = test_func(test);
        assert!(ret.is_err());
        assert_eq!(
            format!("{}", ret.unwrap_err()),
            "Invalid cache attribute found 'size': 10.".to_string()
        );

        let test = "".to_string();
        let ret = test_func(test);
        assert!(ret.is_err());
        assert_eq!(
            format!("{}", ret.unwrap_err()),
            "Invalid cache attribute found 'size': Empty string was provided.".to_string()
        );

        // value exceeds usize: usize::MAX = 18446744073709551615
        let test = "18446744073709551616K".to_string();
        let ret = test_func(test);
        assert!(ret.is_err());
        assert_eq!(
            format!("{}", ret.unwrap_err()),
            "Invalid cache attribute found 'size': 18446744073709551616K.".to_string()
        );

        // value exceeds usize: usize::MAX = 18446744073709551615
        let test = "18446744073709551616M".to_string();
        let ret = test_func(test);
        assert!(ret.is_err());
        assert_eq!(
            format!("{}", ret.unwrap_err()),
            "Invalid cache attribute found 'size': 18446744073709551616M.".to_string()
        );
    }

    #[test]
    fn test_cache_cacheentry_mask_str2bit_count() {
        fn test_func(test: &str) -> Result<u16> {
            mask_str2bit_count(test)
        }

        let test = "ffffffff,ffffffff,00000000,00000000";
        let ret = test_func(&test);
        assert!(ret.is_ok());
        assert_eq!(ret.unwrap(), 64);

        let test = "";
        let ret = test_func(&test);
        assert!(ret.is_err());
        assert_eq!(
            format!("{}", ret.unwrap_err()),
            "Invalid cache attribute found 'shared_cpu_map': .".to_string()
        );

        let test = "ffffyfff";
        let ret = test_func(&test);
        assert!(ret.is_err());
        assert_eq!(
            format!("{}", ret.unwrap_err()),
            "Invalid cache attribute found 'shared_cpu_map': ffffyfff.".to_string()
        )
    }

    #[test]
    fn test_cache_cacheentry_get_cache_entry() {
        fn test_func(cpu_idx: u8, cache_idx: u8) -> Result<CacheEntry> {
            CacheEntry::get_cache_entry(cpu_idx, cache_idx)
        }

        let success = test_func(0, 0);
        assert!(success.is_ok());

        let failure = test_func(0, 4);
        assert!(failure.is_err());
        assert_eq!(
	    format!("{}", failure.unwrap_err()),
	    "Failed to read config info: 'level', from file: /sys/devices/system/cpu/cpu0/cache/index4/level."
		.to_string()
	);
    }
}
