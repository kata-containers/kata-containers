// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// Copyright 2023 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Implementation of CacheEngine struct for cache_info.
//! This struct is used to manage the control flow of cache reading. It
//! recevices a cpu_map to get the mapping relationship between vcpu and pcpu,
//! so fdt can get the correct cpucache info of each vcpu from host.

use std::cmp::{max, min};
use std::collections::HashMap;
use std::thread::spawn;

use log::warn;

use super::cache_entry::CacheEntry;
use super::{CacheInfoMap, Error, Result};

/// The number of maximum parallel threads.
const MAX_PARALLEL_THREAD_COUNT: usize = 8;

/// Core method to read cache information. It iterates all the target physical
/// CPUs that we need to read cache information.
///
/// # Arguments
/// * `pcpus`: vector of physical cpus; The index is referred as vcpu.
/// * `start_vcpu`: start vcpu ID
/// * `max_level`: max cache level that needs to be read
fn read_caches(pcpus: Vec<u8>, start_vcpu: usize, max_level: usize) -> Result<CacheInfoMap> {
    let mut cache_entries =
        HashMap::<usize, (Vec<CacheEntry>, Vec<CacheEntry>)>::with_capacity(pcpus.len());

    for (vcpu, pcpu) in pcpus.iter().enumerate() {
        // l1 cache is divided into two types: Data cache and Instruction cache.
        let mut l1_entries = Vec::<CacheEntry>::with_capacity(2);
        let mut non_l1_entries = Vec::<CacheEntry>::with_capacity(max_level - 1);
        for index in 0..=max_level {
            let cache_entry = match CacheEntry::get_cache_entry(*pcpu, index as u8) {
                Ok(entry) => Ok(entry),
                Err(Error::MissingOptionalAttr {
                    attr: missing_str,
                    cache_entry,
                }) => {
                    warn!("Some attributes('{missing_str}') is missing in CacheEntry: {cache_entry:?}");
                    Ok(cache_entry)
                }
                Err(other) => Err(other),
            }?;
            if cache_entry.level == 1 {
                l1_entries.push(cache_entry);
            } else {
                non_l1_entries.push(cache_entry);
            }
        }
        cache_entries.insert(start_vcpu + vcpu, (l1_entries, non_l1_entries));
    }

    Ok(cache_entries)
}

/// CacheEngine struct.
/// Used to manage the control flows of reading cache information.
pub(crate) struct CacheEngine {
    /// mappings from vcpu(index) to pcpu(value)
    cpu_map: Vec<u8>,
}

impl CacheEngine {
    /// Create a new CacheEngine for loading cacheinfo from host.
    ///
    /// # Arguments
    /// * `cpu_map`: the mapping relationship from vcpu to pcpu
    pub(crate) fn new(cpu_map: Vec<u8>) -> Self {
        CacheEngine { cpu_map }
    }

    /// Generate parallel threads to read cache information.
    ///
    /// # Arguments
    /// * `thread_count`: the thread number created for reading cache info
    /// * `pcpu_num_per_thread`: number of pcpus for each thread
    /// * `max_level`: max cache level that needs to be read
    fn parallel_processing_cache(
        &self,
        thread_count: usize,
        pcpu_num_per_thread: usize,
        max_level: usize,
    ) -> Result<CacheInfoMap> {
        let mut cache_entries =
            HashMap::<usize, (Vec<CacheEntry>, Vec<CacheEntry>)>::with_capacity(self.cpu_map.len());
        let mut handles = Vec::with_capacity(MAX_PARALLEL_THREAD_COUNT);
        let mut pcpus = self
            .cpu_map
            .chunks(pcpu_num_per_thread)
            .map(|pcpu_slice| pcpu_slice.to_vec())
            .collect::<Vec<Vec<u8>>>();

        // Create threads to read cache information.
        for thread_id in 0..thread_count {
            if let Some(pcpu_chunk) = pcpus.pop() {
                handles.push(spawn(move || {
                    read_caches(
                        pcpu_chunk,
                        pcpu_num_per_thread * (thread_count - thread_id - 1),
                        max_level,
                    )
                }));
            }
        }
        // Gain the results.
        for _ in 0..thread_count {
            if let Some(handle) = handles.pop() {
                let entries = handle
                    .join()
                    .map_err(|e| Error::FailedParallelProcessCache(format!("{e:?}")))??;
                cache_entries.extend(entries.into_iter());
            }
        }

        Ok(cache_entries)
    }

    /// Get cache information according to `self.cpu_map`.
    /// This method uses a specific algorithm to get the thread number that
    /// we need to create to cost relatively less time for cache reading.
    ///
    /// # Arguments
    /// * `max_level`: max cache level that needs to be read
    pub(crate) fn get_cache_information(&self, max_level: usize) -> Result<CacheInfoMap> {
        let length = self.cpu_map.len();
        // Assume max cpu count is <= 128.
        // Therefore each thread handles cpus less than 16.
        // The division algorithm needs optimization in the future.
        let thread_count = min(
            max((length / MAX_PARALLEL_THREAD_COUNT + 2) >> 1, 1),
            MAX_PARALLEL_THREAD_COUNT,
        );

        if thread_count == 1 {
            self.parallel_processing_cache(1, length, max_level)
        } else {
            self.parallel_processing_cache(thread_count, length / thread_count + 1, max_level)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_cacheengine_read_caches() {
        let max_level = 3;
        let success = read_caches(vec![45], 20, max_level);
        assert!(success.is_ok());

        let cache_map = success.unwrap();
        assert_eq!(cache_map.len(), 1);
        assert!(cache_map.get(&20).is_some());
        assert!(cache_map.get(&45).is_none());
        assert!(cache_map.get(&0).is_none());

        // l1 cache & non-l1 cache
        let cpu_cache = cache_map.get(&20).unwrap();
        assert_eq!(cpu_cache.0.len(), 2);
        assert_eq!(cpu_cache.1.len(), max_level - 1);

        let failure = read_caches(vec![45], 20, 4);
        assert!(failure.is_err());
        assert_eq!(
	    format!("{}", failure.unwrap_err()),
	    "Failed to read config info: 'level', from file: /sys/devices/system/cpu/cpu45/cache/index4/level."
		.to_string()
	);
    }

    #[test]
    fn test_cache_cacheengine_new() {
        let test = vec![1, 2, 3, 4, 5];
        let engine = CacheEngine::new(test);
        assert_eq!(engine.cpu_map.len(), 5);
        assert_eq!(engine.cpu_map, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_cache_cacheengine_parallel_processing_cache() {
        let engine = CacheEngine::new(vec![16, 32, 23, 47, 56]);

        let success = engine.parallel_processing_cache(1, 5, 3);
        assert!(success.is_ok());
        let cache_map = success.unwrap();
        assert_eq!(cache_map.len(), 5);
        assert!(cache_map.get(&0).is_some());
        assert!(cache_map.get(&1).is_some());
        assert!(cache_map.get(&2).is_some());
        assert!(cache_map.get(&3).is_some());
        assert!(cache_map.get(&4).is_some());
        assert!(cache_map.get(&5).is_none());

        let failure = engine.parallel_processing_cache(1, 5, 4);
        assert!(failure.is_err());
        assert_eq!(
	    format!("{}", failure.unwrap_err()),
	    "Failed to read config info: 'level', from file: /sys/devices/system/cpu/cpu16/cache/index4/level."
		.to_string()
	);
    }

    #[test]
    fn test_cache_cacheengine_get_cache_information() {
        let engine = CacheEngine::new(vec![16, 32, 23, 47, 56]);

        let success = engine.get_cache_information(3);
        assert!(success.is_ok());
        let cache_map = success.unwrap();
        assert_eq!(cache_map.len(), 5);
        assert!(cache_map.get(&0).is_some());
        assert!(cache_map.get(&1).is_some());
        assert!(cache_map.get(&2).is_some());
        assert!(cache_map.get(&3).is_some());
        assert!(cache_map.get(&4).is_some());
        assert!(cache_map.get(&5).is_none());

        let failure = engine.parallel_processing_cache(1, 5, 4);
        assert!(failure.is_err());
        assert_eq!(
	    format!("{}", failure.unwrap_err()),
	    "Failed to read config info: 'level', from file: /sys/devices/system/cpu/cpu16/cache/index4/level."
		.to_string()
	);

        // test multiple threads
        let engine = CacheEngine::new((0..128u8).collect::<Vec<u8>>());
        let success = engine.get_cache_information(3);
        assert!(success.is_ok());
        let cache_map = success.unwrap();
        assert_eq!(cache_map.len(), 128);
        for (&key, value) in cache_map.iter() {
            assert!(key < 128);
            assert_eq!(value.0.len(), 2);
            assert_eq!(value.1.len(), 2);

            // l1 d-cache
            assert_eq!(value.0[0].level, 1);
            assert_eq!(value.0[0].cpus_per_unit, 1);
            assert_eq!(format!("{:?}", value.0[0].type_), "Data");

            // l1 i-cache
            assert_eq!(value.0[1].level, 1);
            assert_eq!(value.0[1].cpus_per_unit, 1);
            assert_eq!(format!("{:?}", value.0[1].type_), "Instruction");

            // l2 cache
            assert_eq!(value.1[0].level, 2);
            assert_eq!(value.1[0].cpus_per_unit, 1);
            assert_eq!(format!("{:?}", value.1[0].type_), "Unified");

            // l3 cache
            assert_eq!(value.1[1].level, 3);
            // assert_eq!(value.1[1].cpus_per_unit, 1);
            assert_eq!(format!("{:?}", value.1[1].type_), "Unified");
        }
    }
}
