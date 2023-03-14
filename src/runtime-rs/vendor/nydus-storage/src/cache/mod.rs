// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! A blob cache layer over storage backend to improve performance.
//!
//! One of Rafs filesystem's goal is to support "on demand data loading". On demand loading may
//! help to speed up application/container startup, but it may also cause serious performance
//! penalty if all data chunks are retrieved from remoted backend storage. So cache layer is
//! introduced between Rafs filesystem and backend storage, which caches remote data onto local
//! storage and merge small data request into bigger request to improve network performance.
//!
//! There are several cache drivers implemented:
//! - [DummyCacheMgr](dummycache/struct.DummyCacheMgr.html): a dummy implementation of
//!   `BlobCacheMgr`, simply reporting each chunk as cached or not cached according to
//!   configuration.

use std::cmp;
use std::fs::File;
use std::io::Result;
use std::slice;
use std::sync::Arc;
use std::time::Instant;

use fuse_backend_rs::file_buf::FileVolatileSlice;
use nydus_utils::{compress, digest};

use crate::backend::{BlobBackend, BlobReader};
use crate::cache::state::ChunkMap;
use crate::device::{
    BlobChunkInfo, BlobInfo, BlobIoDesc, BlobIoRange, BlobIoVec, BlobObject, BlobPrefetchRequest,
};
use crate::utils::{alloc_buf, digest_check};
use crate::{StorageResult, RAFS_MAX_CHUNK_SIZE};

mod cachedfile;
mod dummycache;
mod filecache;
mod fscache;
mod worker;

pub mod state;

pub use dummycache::DummyCacheMgr;
pub use filecache::FileCacheMgr;
pub use fscache::FsCacheMgr;

/// Timeout in milli-seconds to retrieve blob data from backend storage.
pub const SINGLE_INFLIGHT_WAIT_TIMEOUT: u64 = 2000;

struct BlobIoMergeState<'a, F: FnMut(BlobIoRange)> {
    cb: F,
    size: u32,
    bios: Vec<&'a BlobIoDesc>,
}

impl<'a, F: FnMut(BlobIoRange)> BlobIoMergeState<'a, F> {
    /// Create a new instance of 'IoMergeState`.
    pub fn new(bio: &'a BlobIoDesc, cb: F) -> Self {
        let size = bio.chunkinfo.compressed_size();

        BlobIoMergeState {
            cb,
            size,
            bios: vec![bio],
        }
    }

    /// Get size of pending io operations.
    #[inline]
    pub fn size(&self) -> usize {
        self.size as usize
    }

    /// Push the a new io descriptor into the pending list.
    #[inline]
    pub fn push(&mut self, bio: &'a BlobIoDesc) {
        let size = bio.chunkinfo.compressed_size();

        debug_assert!(self.size.checked_add(size).is_some());
        self.bios.push(bio);
        self.size += bio.chunkinfo.compressed_size();
    }

    /// Issue the pending io descriptors.
    #[inline]
    pub fn issue(&mut self) {
        if !self.bios.is_empty() {
            let mut mr = BlobIoRange::new(self.bios[0], self.bios.len());
            for bio in self.bios[1..].iter() {
                mr.merge(bio);
            }
            (self.cb)(mr);

            self.bios.truncate(0);
            self.size = 0;
        }
    }

    /// Merge and issue all blob Io descriptors.
    pub fn merge_and_issue(bios: &[BlobIoDesc], max_size: usize, op: F) {
        if !bios.is_empty() {
            let mut index = 1;
            let mut state = BlobIoMergeState::new(&bios[0], op);

            for cur_bio in &bios[1..] {
                if !cur_bio.is_continuous(&bios[index - 1]) || state.size() >= max_size {
                    state.issue();
                }
                state.push(cur_bio);
                index += 1
            }
            state.issue();
        }
    }
}

/// Trait representing a cache object for a blob on backend storage.
///
/// The caller may use the `BlobCache` trait to access blob data on backend storage, with an
/// optional intermediate cache layer to improve performance.
pub trait BlobCache: Send + Sync {
    /// Get id of the blob object.
    fn blob_id(&self) -> &str;

    /// Get size of the decompressed blob object.
    fn blob_uncompressed_size(&self) -> Result<u64>;

    /// Get size of the compressed blob object.
    fn blob_compressed_size(&self) -> Result<u64>;

    /// Get data compression algorithm to handle chunks in the blob.
    fn compressor(&self) -> compress::Algorithm;

    /// Get message digest algorithm to handle chunks in the blob.
    fn digester(&self) -> digest::Algorithm;

    /// Check whether the cache object is for an stargz image.
    fn is_stargz(&self) -> bool;

    /// Check whether need to validate the data chunk by digest value.
    fn need_validate(&self) -> bool;

    /// Get the [BlobReader](../backend/trait.BlobReader.html) to read data from storage backend.
    fn reader(&self) -> &dyn BlobReader;

    /// Get the underlying `ChunkMap` object.
    fn get_chunk_map(&self) -> &Arc<dyn ChunkMap>;

    /// Get a `BlobObject` instance to directly access uncompressed blob file.
    fn get_blob_object(&self) -> Option<&dyn BlobObject> {
        None
    }

    /// Start to prefetch requested data in background.
    fn prefetch(
        &self,
        cache: Arc<dyn BlobCache>,
        prefetches: &[BlobPrefetchRequest],
        bios: &[BlobIoDesc],
    ) -> StorageResult<usize>;

    /// Enable prefetching blob data in background.
    ///
    /// It should be paired with stop_prefetch().
    fn start_prefetch(&self) -> StorageResult<()>;

    /// Stop prefetching blob data in background.
    ///
    /// It should be paired with start_prefetch().
    fn stop_prefetch(&self) -> StorageResult<()>;

    // Check whether data prefetch is still active.
    fn is_prefetch_active(&self) -> bool;

    /// Execute filesystem data prefetch.
    fn prefetch_range(&self, _range: &BlobIoRange) -> Result<usize> {
        Err(enosys!("doesn't support prefetch_range()"))
    }

    /// Read chunk data described by the blob Io descriptors from the blob cache into the buffer.
    fn read(&self, iovec: &mut BlobIoVec, buffers: &[FileVolatileSlice]) -> Result<usize>;

    /// Read multiple chunks from the blob cache in batch mode.
    ///
    /// This is an interface to optimize chunk data fetch performance by merging multiple continuous
    /// chunks into one backend request. Callers must ensure that chunks in `chunks` covers a
    /// continuous range, and the range exactly matches [`blob_offset`..`blob_offset` + `blob_size`].
    /// Function `read_chunks()` returns one buffer containing decompressed chunk data for each
    /// entry in the `chunks` array in corresponding order.
    ///
    /// This method returns success only if all requested data are successfully fetched.
    fn read_chunks(
        &self,
        blob_offset: u64,
        blob_size: usize,
        chunks: &[Arc<dyn BlobChunkInfo>],
        prefetch: bool,
    ) -> Result<Vec<Vec<u8>>> {
        // Read requested data from the backend by altogether.
        let mut c_buf = alloc_buf(blob_size);
        let start = Instant::now();
        let nr_read = self
            .reader()
            .read(c_buf.as_mut_slice(), blob_offset)
            .map_err(|e| eio!(e))?;
        if nr_read != blob_size {
            return Err(eio!(format!(
                "request for {} bytes but got {} bytes",
                blob_size, nr_read
            )));
        }
        let duration = Instant::now().duration_since(start).as_millis();
        debug!(
            "read_chunks: {} {} {} bytes at {}, duration {}ms",
            std::thread::current().name().unwrap_or_default(),
            if prefetch { "prefetch" } else { "fetch" },
            blob_size,
            blob_offset,
            duration
        );

        let mut last = blob_offset;
        let mut buffers: Vec<Vec<u8>> = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            // Ensure BlobIoChunk is valid and continuous.
            let offset = chunk.compressed_offset();
            let size = chunk.compressed_size();
            let d_size = chunk.uncompressed_size() as usize;
            if offset != last
                || offset - blob_offset > usize::MAX as u64
                || offset.checked_add(size as u64).is_none()
                || ((!self.is_stargz() && d_size as u64 > RAFS_MAX_CHUNK_SIZE)
                    || (self.is_stargz() && d_size > 4 << 20))
            {
                return Err(eio!(format!("chunks to read_chunks() is invalid, offset {} last {} blob_offset {} d_size {}",
                                        offset, last, blob_offset, d_size)));
            }

            let offset_merged = (offset - blob_offset) as usize;
            let end_merged = offset_merged + size as usize;
            let buf = &c_buf[offset_merged..end_merged];
            let mut buffer = alloc_buf(d_size);

            self.process_raw_chunk(
                chunk.as_ref(),
                buf,
                None,
                &mut buffer,
                chunk.is_compressed(),
                false,
            )?;
            buffers.push(buffer);
            last = offset + size as u64;
        }

        Ok(buffers)
    }

    /// Read a whole chunk directly from the storage backend.
    ///
    /// The fetched chunk data may be compressed or not, which depends chunk information from `chunk`.
    /// Moreover, chunk data from backend storage may be validated per user's configuration.
    /// Above is not redundant with blob cache's validation given IO path backend -> blobcache
    /// `raw_hook` provides caller a chance to read fetched compressed chunk data.
    fn read_raw_chunk(
        &self,
        chunk: &dyn BlobChunkInfo,
        buffer: &mut [u8],
        force_validation: bool,
        raw_hook: Option<&dyn Fn(&[u8])>,
    ) -> Result<usize> {
        let mut d;
        let offset = chunk.compressed_offset();
        let raw_chunk = if chunk.is_compressed() {
            // Need a scratch buffer to decompress compressed data.
            let c_size = if self.is_stargz() {
                let blob_size = self.blob_compressed_size()?;
                let max_size = blob_size.checked_sub(offset).ok_or_else(|| {
                    einval!("chunk compressed offset is bigger than blob file size")
                })?;
                let max_size = cmp::min(max_size, usize::MAX as u64);
                compress::compute_compressed_gzip_size(buffer.len(), max_size as usize)
            } else {
                chunk.compressed_size() as usize
            };
            d = alloc_buf(c_size);
            d.as_mut_slice()
        } else {
            // We have this unsafe assignment as it can directly store data into call's buffer.
            unsafe { slice::from_raw_parts_mut(buffer.as_mut_ptr(), buffer.len()) }
        };

        let size = self.reader().read(raw_chunk, offset).map_err(|e| eio!(e))?;
        if size != raw_chunk.len() {
            return Err(eio!("storage backend returns less data than requested"));
        }

        self.process_raw_chunk(
            chunk,
            raw_chunk,
            None,
            buffer,
            chunk.is_compressed(),
            force_validation,
        )?;
        if let Some(hook) = raw_hook {
            hook(raw_chunk)
        }

        Ok(buffer.len())
    }

    /// Hook point to post-process data received from storage backend.
    ///
    /// This hook method provides a chance to transform data received from storage backend into
    /// data cached on local disk.
    fn process_raw_chunk(
        &self,
        chunk: &dyn BlobChunkInfo,
        raw_buffer: &[u8],
        raw_stream: Option<File>,
        buffer: &mut [u8],
        need_decompress: bool,
        force_validation: bool,
    ) -> Result<usize> {
        if need_decompress {
            compress::decompress(raw_buffer, raw_stream, buffer, self.compressor()).map_err(
                |e| {
                    error!("failed to decompress chunk: {}", e);
                    e
                },
            )?;
        } else if raw_buffer.as_ptr() != buffer.as_ptr() {
            // raw_chunk and chunk may point to the same buffer, so only copy data when needed.
            buffer.copy_from_slice(raw_buffer);
        }

        let d_size = chunk.uncompressed_size() as usize;
        if buffer.len() != d_size {
            Err(eio!("uncompressed size and buffer size doesn't match"))
        } else if (self.need_validate() || force_validation)
            && !digest_check(buffer, chunk.chunk_id(), self.digester())
        {
            Err(eio!())
        } else {
            Ok(d_size)
        }
    }
}

/// Trait representing blob manager to manage a group of [BlobCache](trait.BlobCache.html) objects.
///
/// The main responsibility of the blob cache manager is to create blob cache objects for blobs,
/// all IO requests should be issued to the blob cache object directly.
pub(crate) trait BlobCacheMgr: Send + Sync {
    /// Initialize the blob cache manager.
    fn init(&self) -> Result<()>;

    /// Tear down the blob cache manager.
    fn destroy(&self);

    /// Garbage-collect unused resources.
    ///
    /// Return true if the blob cache manager itself should be garbage-collected.
    fn gc(&self, _id: Option<&str>) -> bool;

    /// Get the underlying `BlobBackend` object of the blob cache object.
    fn backend(&self) -> &(dyn BlobBackend);

    /// Get the blob cache to provide access to the `blob` object.
    fn get_blob_cache(&self, blob_info: &Arc<BlobInfo>) -> Result<Arc<dyn BlobCache>>;

    /// Check the blob cache data status, if data all ready stop prefetch workers.
    fn check_stat(&self);
}

#[cfg(test)]
mod tests {
    use crate::device::{BlobChunkFlags, BlobFeatures};
    use crate::test::MockChunkInfo;

    use super::*;

    #[test]
    fn test_io_merge_state_new() {
        let blob_info = Arc::new(BlobInfo::new(
            1,
            "test1".to_owned(),
            0x200000,
            0x100000,
            0x100000,
            512,
            BlobFeatures::V5_NO_EXT_BLOB_TABLE,
        ));
        let chunk1 = Arc::new(MockChunkInfo {
            block_id: Default::default(),
            blob_index: 1,
            flags: BlobChunkFlags::empty(),
            compress_size: 0x800,
            uncompress_size: 0x1000,
            compress_offset: 0,
            uncompress_offset: 0,
            file_offset: 0,
            index: 0,
            reserved: 0,
        }) as Arc<dyn BlobChunkInfo>;
        let chunk2 = Arc::new(MockChunkInfo {
            block_id: Default::default(),
            blob_index: 1,
            flags: BlobChunkFlags::empty(),
            compress_size: 0x800,
            uncompress_size: 0x1000,
            compress_offset: 0x800,
            uncompress_offset: 0x1000,
            file_offset: 0x1000,
            index: 1,
            reserved: 0,
        }) as Arc<dyn BlobChunkInfo>;
        let chunk3 = Arc::new(MockChunkInfo {
            block_id: Default::default(),
            blob_index: 1,
            flags: BlobChunkFlags::empty(),
            compress_size: 0x800,
            uncompress_size: 0x1000,
            compress_offset: 0x1000,
            uncompress_offset: 0x1000,
            file_offset: 0x1000,
            index: 1,
            reserved: 0,
        }) as Arc<dyn BlobChunkInfo>;

        let cb = |_merged| {};
        let desc1 = BlobIoDesc {
            blob: blob_info.clone(),
            chunkinfo: chunk1.into(),
            offset: 0,
            size: 0x1000,
            user_io: true,
        };
        let mut state = BlobIoMergeState::new(&desc1, cb);
        assert_eq!(state.size(), 0x800);
        assert_eq!(state.bios.len(), 1);

        let desc2 = BlobIoDesc {
            blob: blob_info.clone(),
            chunkinfo: chunk2.into(),
            offset: 0,
            size: 0x1000,
            user_io: true,
        };
        state.push(&desc2);
        assert_eq!(state.size, 0x1000);
        assert_eq!(state.bios.len(), 2);

        state.issue();
        assert_eq!(state.size(), 0x0);
        assert_eq!(state.bios.len(), 0);

        let desc3 = BlobIoDesc {
            blob: blob_info,
            chunkinfo: chunk3.into(),
            offset: 0,
            size: 0x1000,
            user_io: true,
        };
        state.push(&desc3);
        assert_eq!(state.size, 0x800);
        assert_eq!(state.bios.len(), 1);

        state.issue();
        assert_eq!(state.size(), 0x0);
        assert_eq!(state.bios.len(), 0);

        let mut count = 0;
        BlobIoMergeState::merge_and_issue(
            &[desc1.clone(), desc2.clone(), desc3.clone()],
            0x4000,
            |_v| count += 1,
        );
        assert_eq!(count, 1);

        let mut count = 0;
        BlobIoMergeState::merge_and_issue(
            &[desc1.clone(), desc2.clone(), desc3.clone()],
            0x1000,
            |_v| count += 1,
        );
        assert_eq!(count, 2);

        let mut count = 0;
        BlobIoMergeState::merge_and_issue(&[desc1.clone(), desc3.clone()], 0x4000, |_v| count += 1);
        assert_eq!(count, 2);

        assert!(desc2.is_continuous(&desc1));
        assert!(!desc3.is_continuous(&desc1));
    }
}
