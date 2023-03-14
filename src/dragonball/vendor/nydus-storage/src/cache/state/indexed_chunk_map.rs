// Copyright 2021 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! A chunk state tracking driver based on a bitmap file.
//!
//! This module provides a chunk state tracking driver based on a bitmap file. There's a state bit
//! in the bitmap file for each chunk, and atomic operations are used to manipulate the bitmap.
//! So it supports concurrent downloading.
use std::io::Result;

use crate::cache::state::persist_map::PersistMap;
use crate::cache::state::{ChunkIndexGetter, ChunkMap, RangeMap};
use crate::device::{BlobChunkInfo, BlobInfo};

/// The name suffix of blob chunk_map file, named $blob_id.chunk_map.
const FILE_SUFFIX: &str = "chunk_map";

/// An implementation of [ChunkMap] to support chunk state tracking by using a bitmap file.
///
/// The `IndexedChunkMap` is an implementation of [ChunkMap] which uses a bitmap file and atomic
/// bitmap operations to track readiness state. It creates or opens a file with the name
/// `$blob_id.chunk_map` to record whether a chunk has been cached by the blob cache, and atomic
/// bitmap operations are used to manipulate the state bit. The bitmap file will be persisted to
/// disk.
///
/// This approach can be used to share chunk ready state between multiple nydusd instances.
/// For example: the bitmap file layout is [0b00000000, 0b00000000], when blobcache calls
/// set_ready(3), the layout should be changed to [0b00010000, 0b00000000].
pub struct IndexedChunkMap {
    map: PersistMap,
}

impl IndexedChunkMap {
    /// Create a new instance of `IndexedChunkMap`.
    pub fn new(blob_path: &str, chunk_count: u32, persist: bool) -> Result<Self> {
        let filename = format!("{}.{}", blob_path, FILE_SUFFIX);

        PersistMap::open(&filename, chunk_count, true, persist).map(|map| IndexedChunkMap { map })
    }

    /// Create a new instance of `IndexedChunkMap` from an existing chunk map file.
    pub fn open(blob_info: &BlobInfo, workdir: &str) -> Result<Self> {
        let filename = format!("{}/{}.{}", workdir, blob_info.blob_id(), FILE_SUFFIX);

        PersistMap::open(&filename, blob_info.chunk_count(), false, true)
            .map(|map| IndexedChunkMap { map })
    }
}

impl ChunkMap for IndexedChunkMap {
    fn is_ready(&self, chunk: &dyn BlobChunkInfo) -> Result<bool> {
        if self.is_range_all_ready() {
            Ok(true)
        } else {
            let index = self.map.validate_index(chunk.id())?;
            Ok(self.map.is_chunk_ready(index).0)
        }
    }

    fn set_ready_and_clear_pending(&self, chunk: &dyn BlobChunkInfo) -> Result<()> {
        self.map.set_chunk_ready(chunk.id())
    }

    fn is_persist(&self) -> bool {
        true
    }

    fn as_range_map(&self) -> Option<&dyn RangeMap<I = u32>> {
        Some(self)
    }
}

impl RangeMap for IndexedChunkMap {
    type I = u32;

    #[inline]
    fn is_range_all_ready(&self) -> bool {
        self.map.is_range_all_ready()
    }

    fn is_range_ready(&self, start_index: u32, count: u32) -> Result<bool> {
        if !self.is_range_all_ready() {
            for idx in 0..count {
                let index = self
                    .map
                    .validate_index(start_index.checked_add(idx).ok_or_else(|| einval!())?)?;
                if !self.map.is_chunk_ready(index).0 {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    fn check_range_ready_and_mark_pending(
        &self,
        start_index: u32,
        count: u32,
    ) -> Result<Option<Vec<u32>>> {
        if self.is_range_all_ready() {
            return Ok(None);
        }

        let mut vec = Vec::with_capacity(count as usize);
        let count = std::cmp::min(count, u32::MAX - start_index);
        let end = start_index + count;

        for index in start_index..end {
            if !self.map.is_chunk_ready(index).0 {
                vec.push(index);
            }
        }

        if vec.is_empty() {
            Ok(None)
        } else {
            Ok(Some(vec))
        }
    }

    fn set_range_ready_and_clear_pending(&self, start_index: u32, count: u32) -> Result<()> {
        let count = std::cmp::min(count, u32::MAX - start_index);
        let end = start_index + count;

        for index in start_index..end {
            self.map.set_chunk_ready(index)?;
        }

        Ok(())
    }
}

impl ChunkIndexGetter for IndexedChunkMap {
    type Index = u32;

    fn get_index(chunk: &dyn BlobChunkInfo) -> Self::Index {
        chunk.id()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::sync::atomic::Ordering;
    use vmm_sys_util::tempdir::TempDir;

    use super::super::persist_map::*;
    use super::*;
    use crate::device::v5::BlobV5ChunkInfo;
    use crate::test::MockChunkInfo;

    #[test]
    fn test_indexed_new_invalid_file_size() {
        let dir = TempDir::new().unwrap();
        let blob_path = dir.as_path().join("blob-1");
        let blob_path = blob_path.as_os_str().to_str().unwrap().to_string();

        assert!(IndexedChunkMap::new(&blob_path, 0, false).is_err());

        let cache_path = format!("{}.{}", blob_path, FILE_SUFFIX);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&cache_path)
            .map_err(|err| {
                einval!(format!(
                    "failed to open/create blob chunk_map file {:?}: {:?}",
                    cache_path, err
                ))
            })
            .unwrap();
        file.write_all(&[0x0u8]).unwrap();

        let chunk = MockChunkInfo::new();
        assert_eq!(chunk.id(), 0);

        assert!(IndexedChunkMap::new(&blob_path, 1, true).is_err());
    }

    #[test]
    fn test_indexed_new_zero_file_size() {
        let dir = TempDir::new().unwrap();
        let blob_path = dir.as_path().join("blob-1");
        let blob_path = blob_path.as_os_str().to_str().unwrap().to_string();

        assert!(IndexedChunkMap::new(&blob_path, 0, true).is_err());

        let cache_path = format!("{}.{}", blob_path, FILE_SUFFIX);
        let _file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&cache_path)
            .map_err(|err| {
                einval!(format!(
                    "failed to open/create blob chunk_map file {:?}: {:?}",
                    cache_path, err
                ))
            })
            .unwrap();

        let chunk = MockChunkInfo::new();
        assert_eq!(chunk.id(), 0);

        let map = IndexedChunkMap::new(&blob_path, 1, true).unwrap();
        assert_eq!(map.map.not_ready_count.load(Ordering::Acquire), 1);
        assert_eq!(map.map.count, 1);
        assert_eq!(map.map.size, 0x1001);
        assert!(!map.is_range_all_ready());
        assert!(!map.is_ready(chunk.as_base()).unwrap());
        map.set_ready_and_clear_pending(chunk.as_base()).unwrap();
        assert!(map.is_ready(chunk.as_base()).unwrap());
    }

    #[test]
    fn test_indexed_new_header_not_ready() {
        let dir = TempDir::new().unwrap();
        let blob_path = dir.as_path().join("blob-1");
        let blob_path = blob_path.as_os_str().to_str().unwrap().to_string();

        assert!(IndexedChunkMap::new(&blob_path, 0, true).is_err());

        let cache_path = format!("{}.{}", blob_path, FILE_SUFFIX);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&cache_path)
            .map_err(|err| {
                einval!(format!(
                    "failed to open/create blob chunk_map file {:?}: {:?}",
                    cache_path, err
                ))
            })
            .unwrap();
        file.set_len(0x1001).unwrap();

        let chunk = MockChunkInfo::new();
        assert_eq!(chunk.id(), 0);

        let map = IndexedChunkMap::new(&blob_path, 1, true).unwrap();
        assert_eq!(map.map.not_ready_count.load(Ordering::Acquire), 1);
        assert_eq!(map.map.count, 1);
        assert_eq!(map.map.size, 0x1001);
        assert!(!map.is_range_all_ready());
        assert!(!map.is_ready(chunk.as_base()).unwrap());
        map.set_ready_and_clear_pending(chunk.as_base()).unwrap();
        assert!(map.is_ready(chunk.as_base()).unwrap());
    }

    #[test]
    fn test_indexed_new_all_ready() {
        let dir = TempDir::new().unwrap();
        let blob_path = dir.as_path().join("blob-1");
        let blob_path = blob_path.as_os_str().to_str().unwrap().to_string();

        assert!(IndexedChunkMap::new(&blob_path, 0, true).is_err());

        let cache_path = format!("{}.{}", blob_path, FILE_SUFFIX);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&cache_path)
            .map_err(|err| {
                einval!(format!(
                    "failed to open/create blob chunk_map file {:?}: {:?}",
                    cache_path, err
                ))
            })
            .unwrap();
        let header = Header {
            magic: MAGIC1,
            version: 1,
            magic2: MAGIC2,
            all_ready: MAGIC_ALL_READY,
            reserved: [0x0u8; HEADER_RESERVED_SIZE],
        };

        // write file header and sync to disk.
        file.write_all(header.as_slice()).unwrap();
        file.write_all(&[0x0u8]).unwrap();

        let chunk = MockChunkInfo::new();
        assert_eq!(chunk.id(), 0);

        let map = IndexedChunkMap::new(&blob_path, 1, true).unwrap();
        assert!(map.is_range_all_ready());
        assert_eq!(map.map.count, 1);
        assert_eq!(map.map.size, 0x1001);
        assert!(map.is_ready(chunk.as_base()).unwrap());
        map.set_ready_and_clear_pending(chunk.as_base()).unwrap();
        assert!(map.is_ready(chunk.as_base()).unwrap());
    }

    #[test]
    fn test_indexed_new_load_v0() {
        let dir = TempDir::new().unwrap();
        let blob_path = dir.as_path().join("blob-1");
        let blob_path = blob_path.as_os_str().to_str().unwrap().to_string();

        assert!(IndexedChunkMap::new(&blob_path, 0, true).is_err());

        let cache_path = format!("{}.{}", blob_path, FILE_SUFFIX);
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&cache_path)
            .map_err(|err| {
                einval!(format!(
                    "failed to open/create blob chunk_map file {:?}: {:?}",
                    cache_path, err
                ))
            })
            .unwrap();
        let header = Header {
            magic: MAGIC1,
            version: 0,
            magic2: 0,
            all_ready: 0,
            reserved: [0x0u8; HEADER_RESERVED_SIZE],
        };

        // write file header and sync to disk.
        file.write_all(header.as_slice()).unwrap();
        file.write_all(&[0x0u8]).unwrap();

        let chunk = MockChunkInfo::new();
        assert_eq!(chunk.id(), 0);

        let map = IndexedChunkMap::new(&blob_path, 1, true).unwrap();
        assert_eq!(map.map.not_ready_count.load(Ordering::Acquire), 1);
        assert_eq!(map.map.count, 1);
        assert_eq!(map.map.size, 0x1001);
        assert!(!map.is_range_all_ready());
        assert!(!map.is_ready(chunk.as_base()).unwrap());
        map.set_ready_and_clear_pending(chunk.as_base()).unwrap();
        assert!(map.is_ready(chunk.as_base()).unwrap());
    }
}
