// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::io::Result;

use crate::cache::state::persist_map::PersistMap;
use crate::cache::state::RangeMap;

/// The name suffix of blob chunk_map file, named $blob_id.chunk_map.
const FILE_SUFFIX: &str = "range_map";

/// An implementation of [RangeMap] to support cache state tracking by using a bitmap file.
///
/// The `BlobRangeMap` is an implementation of [RangeMap] which uses a bitmap file and atomic
/// bitmap operations to track readiness state. It creates or opens a file with the name
/// `$blob_id.range_map` to record whether a data range has been cached by the blob cache, and
/// atomic bitmap operations are used to manipulate the state bit. The bitmap file will be persisted
/// to disk.
pub struct BlobRangeMap {
    pub(crate) shift: u32,
    map: PersistMap,
}

impl BlobRangeMap {
    /// Create a new instance of `BlobRangeMap`.
    pub fn new(blob_path: &str, count: u32, shift: u32) -> Result<Self> {
        let filename = format!("{}.{}", blob_path, FILE_SUFFIX);
        debug_assert!(shift < 64);

        PersistMap::open(&filename, count, true, true).map(|map| BlobRangeMap { shift, map })
    }

    /// Create a new instance of `BlobRangeMap` from an existing chunk map file.
    pub fn open(blob_id: &str, workdir: &str, count: u32, shift: u32) -> Result<Self> {
        let filename = format!("{}/{}.{}", workdir, blob_id, FILE_SUFFIX);
        debug_assert!(shift < 64);

        PersistMap::open(&filename, count, false, true).map(|map| BlobRangeMap { shift, map })
    }

    pub(crate) fn get_range(&self, start: u64, count: u64) -> Result<(u32, u32)> {
        if let Some(end) = start.checked_add(count) {
            let start_index = start >> self.shift as u64;
            let end_index = (end - 1) >> self.shift as u64;
            if start_index > u32::MAX as u64 || end_index > u32::MAX as u64 {
                Err(einval!())
            } else {
                self.map.validate_index(start_index as u32)?;
                self.map.validate_index(end_index as u32)?;
                Ok((start_index as u32, end_index as u32 + 1))
            }
        } else {
            Err(einval!())
        }
    }
}

impl RangeMap for BlobRangeMap {
    type I = u64;

    fn is_range_all_ready(&self) -> bool {
        self.map.is_range_all_ready()
    }

    /// Check whether all data in the range are ready for use.
    fn is_range_ready(&self, start: u64, count: u64) -> Result<bool> {
        if !self.is_range_all_ready() {
            let (start_index, end_index) = self.get_range(start, count)?;
            for index in start_index..end_index {
                if !self.map.is_chunk_ready(index).0 {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    fn check_range_ready_and_mark_pending(
        &self,
        start: u64,
        count: u64,
    ) -> Result<Option<Vec<u64>>> {
        if self.is_range_all_ready() {
            Ok(None)
        } else {
            let (start_index, end_index) = self.get_range(start, count)?;
            let mut vec = Vec::with_capacity(count as usize);

            for index in start_index..end_index {
                if !self.map.is_chunk_ready(index).0 {
                    vec.push((index as u64) << self.shift);
                }
            }

            if vec.is_empty() {
                Ok(None)
            } else {
                Ok(Some(vec))
            }
        }
    }

    fn set_range_ready_and_clear_pending(&self, start: u64, count: u64) -> Result<()> {
        if !self.is_range_all_ready() {
            let (start_index, end_index) = self.get_range(start, count)?;

            for index in start_index..end_index {
                self.map.set_chunk_ready(index)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;
    use std::time::Instant;

    use vmm_sys_util::tempdir::TempDir;

    use super::super::BlobStateMap;
    use super::*;

    #[test]
    fn test_range_map() {
        let dir = TempDir::new().unwrap();
        let blob_path = dir.as_path().join("blob-1");
        let blob_path = blob_path.as_os_str().to_str().unwrap().to_string();
        let range_count = 1000000;
        let skip_index = 77;

        let map1 = Arc::new(BlobStateMap::from_range_map(
            BlobRangeMap::new(&blob_path, range_count, 12).unwrap(),
        ));
        let map2 = Arc::new(BlobStateMap::from_range_map(
            BlobRangeMap::new(&blob_path, range_count, 12).unwrap(),
        ));
        let map3 = Arc::new(BlobStateMap::from_range_map(
            BlobRangeMap::new(&blob_path, range_count, 12).unwrap(),
        ));

        let now = Instant::now();

        let h1 = thread::spawn(move || {
            for idx in 0..range_count {
                if idx % skip_index != 0 {
                    let addr = ((idx as u64) << 12) + (idx as u64 % 0x1000);
                    map1.set_range_ready_and_clear_pending(addr, 1).unwrap();
                }
            }
        });

        let h2 = thread::spawn(move || {
            for idx in 0..range_count {
                if idx % skip_index != 0 {
                    let addr = ((idx as u64) << 12) + (idx as u64 % 0x1000);
                    map2.set_range_ready_and_clear_pending(addr, 1).unwrap();
                }
            }
        });

        h1.join()
            .map_err(|e| {
                error!("Join error {:?}", e);
                e
            })
            .unwrap();
        h2.join()
            .map_err(|e| {
                error!("Join error {:?}", e);
                e
            })
            .unwrap();

        println!("BlobRangeMap Concurrency: {}ms", now.elapsed().as_millis());

        for idx in 0..range_count {
            let addr = ((idx as u64) << 12) + (idx as u64 % 0x1000);

            let is_ready = map3.is_range_ready(addr, 1).unwrap();
            if idx % skip_index == 0 {
                if is_ready {
                    panic!("indexed chunk map: index {} shouldn't be ready", idx);
                }
            } else if !is_ready {
                panic!("indexed chunk map: index {} should be ready", idx);
            }
        }
    }
}
