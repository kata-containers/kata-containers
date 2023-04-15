// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// Copyright 2023 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Implementation of CacheType structure.
//! CPU caches is divided into three types: Data, Instruction and Unified.
//! For level 1 caches, they are one of data caches and instruction caches.
//! For non level 1 caches, they all belongs to unified type.

use std::result::Result;

/// CacheType enum. Support only three types: Data, Instruction or Unified.
#[derive(Debug, Default)]
// Based on https://elixir.free-electrons.com/linux/v4.9.62/source/include/linux/cacheinfo.h#L11.
pub enum CacheType {
    /// Data cache
    Data,
    /// Instruction cache
    Instruction,
    /// Unified cache
    #[default]
    Unified,
}

impl CacheType {
    /// Convert cache type string into CacheType enum.
    ///
    /// # Arguments
    /// * `cache_type`: type to be converted
    pub(crate) fn try_from(cache_type: &str) -> Result<Self, ()> {
        match cache_type.trim() {
            "Data" => Ok(Self::Data),
            "Instruction" => Ok(Self::Instruction),
            "Unified" => Ok(Self::Unified),
            _ => Err(()),
        }
    }

    // The below are auxiliary functions used for constructing the FDT.
    /// Get the name of fdt cache_size attribute.
    pub(crate) fn of_cache_size(&self) -> &str {
        match self {
            Self::Data => "d-cache-size",
            Self::Instruction => "i-cache-size",
            Self::Unified => "cache-size",
        }
    }

    /// Get the name of fdt cache_line_size attribute.
    pub(crate) fn of_cache_line_size(&self) -> &str {
        match self {
            Self::Data => "d-cache-line-size",
            Self::Instruction => "i-cache-line-size",
            Self::Unified => "cache-line-size",
        }
    }

    /// Get fdt cache_type attribute.
    pub(crate) fn of_cache_type(&self) -> Option<&'static str> {
        match self {
            Self::Data => None,
            Self::Instruction => None,
            Self::Unified => Some("cache-unified"),
        }
    }

    /// Get the name of fdt cache_sets attribute.
    pub(crate) fn of_cache_sets(&self) -> &str {
        match self {
            Self::Data => "d-cache-sets",
            Self::Instruction => "i-cache-sets",
            Self::Unified => "cache-sets",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn helper_generate_test_cachetype() -> (CacheType, CacheType, CacheType) {
        (CacheType::Data, CacheType::Instruction, CacheType::Unified)
    }

    #[test]
    fn test_cache_cachetype_debug() {
        assert_eq!(format!("{:?}", CacheType::Data), "Data".to_string());
        assert_eq!(
            format!("{:?}", CacheType::Instruction),
            "Instruction".to_string()
        );
        assert_eq!(format!("{:?}", CacheType::Unified), "Unified".to_string());
    }

    #[test]
    fn test_cache_cachetype_default() {
        assert_eq!(format!("{:?}", CacheType::default()), "Unified".to_string());
    }

    #[test]
    fn test_cache_cachetype_try_from() {
        let r1 = CacheType::try_from("Data");
        assert!(r1.is_ok());
        assert_eq!(format!("{:?}", r1.unwrap()), "Data".to_string());

        let r2 = CacheType::try_from("Instruction");
        assert!(r2.is_ok());
        assert_eq!(format!("{:?}", r2.unwrap()), "Instruction".to_string());

        let r3 = CacheType::try_from("Unified");
        assert!(r3.is_ok());
        assert_eq!(format!("{:?}", r3.unwrap()), "Unified".to_string());

        let r4 = CacheType::try_from("Error");
        assert!(r4.is_err());
    }

    #[test]
    fn test_cache_cachetype_of_cache_size() {
        let (c1, c2, c3) = helper_generate_test_cachetype();
        assert_eq!(c1.of_cache_size(), "d-cache-size");
        assert_eq!(c2.of_cache_size(), "i-cache-size");
        assert_eq!(c3.of_cache_size(), "cache-size");
    }

    #[test]
    fn test_cache_cachetype_of_cache_line_size() {
        let (c1, c2, c3) = helper_generate_test_cachetype();
        assert_eq!(c1.of_cache_line_size(), "d-cache-line-size");
        assert_eq!(c2.of_cache_line_size(), "i-cache-line-size");
        assert_eq!(c3.of_cache_line_size(), "cache-line-size");
    }

    #[test]
    fn test_cache_cachetype_of_cache_type() {
        let (c1, c2, c3) = helper_generate_test_cachetype();
        assert!(c1.of_cache_type().is_none());
        assert!(c2.of_cache_type().is_none());
        assert_eq!(c3.of_cache_type().unwrap(), "cache-unified");
    }

    #[test]
    fn test_cache_cachetype_of_cache_sets() {
        let (c1, c2, c3) = helper_generate_test_cachetype();
        assert_eq!(c1.of_cache_sets(), "d-cache-sets");
        assert_eq!(c2.of_cache_sets(), "i-cache-sets");
        assert_eq!(c3.of_cache_sets(), "cache-sets");
    }
}
