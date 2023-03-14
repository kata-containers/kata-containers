// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! A dummy implementation of the [BlobCacheMgr](trait.BlobCacheMgr.html) trait.
//!
//! The [DummyCacheMgr](struct.DummyCacheMgr.html) is a dummy implementation of the
//! [BlobCacheMgr](../trait.BlobCacheMgr.html) trait, which doesn't really cache any data.
//! Instead it just reads data from the backend, uncompressed it if needed and then pass on
//! the data to the clients.
//!
//! There are two possible usage mode of the [DummyCacheMgr]:
//! - Read compressed/uncompressed data from remote Registry/OSS backend but not cache the
//!   uncompressed data on local storage. The
//!   [is_chunk_cached()](../trait.BlobCache.html#tymethod.is_chunk_cached)
//!   method always return false to disable data prefetching.
//! - Read uncompressed data from local disk and no need to double cache the data.
//!   The [is_chunk_cached()](../trait.BlobCache.html#tymethod.is_chunk_cached) method always
//!   return true to enable data prefetching.
use std::io::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use fuse_backend_rs::file_buf::FileVolatileSlice;
use nydus_api::http::CacheConfig;
use nydus_utils::{compress, digest};

use crate::backend::{BlobBackend, BlobReader};
use crate::cache::state::{ChunkMap, NoopChunkMap};
use crate::cache::{BlobCache, BlobCacheMgr};
use crate::device::{BlobChunkInfo, BlobInfo, BlobIoDesc, BlobIoVec, BlobPrefetchRequest};
use crate::utils::{alloc_buf, copyv};
use crate::{StorageError, StorageResult};

struct DummyCache {
    blob_id: String,
    chunk_map: Arc<dyn ChunkMap>,
    reader: Arc<dyn BlobReader>,
    compressor: compress::Algorithm,
    digester: digest::Algorithm,
    is_stargz: bool,
    validate: bool,
}

impl BlobCache for DummyCache {
    fn blob_id(&self) -> &str {
        &self.blob_id
    }

    fn blob_uncompressed_size(&self) -> Result<u64> {
        unimplemented!();
    }

    fn blob_compressed_size(&self) -> Result<u64> {
        self.reader.blob_size().map_err(|e| eother!(e))
    }

    fn compressor(&self) -> compress::Algorithm {
        self.compressor
    }

    fn digester(&self) -> digest::Algorithm {
        self.digester
    }

    fn is_stargz(&self) -> bool {
        self.is_stargz
    }

    fn need_validate(&self) -> bool {
        self.validate
    }

    fn reader(&self) -> &dyn BlobReader {
        &*self.reader
    }

    fn get_chunk_map(&self) -> &Arc<dyn ChunkMap> {
        &self.chunk_map
    }

    fn prefetch(
        &self,
        _blob_cache: Arc<dyn BlobCache>,
        _prefetches: &[BlobPrefetchRequest],
        _bios: &[BlobIoDesc],
    ) -> StorageResult<usize> {
        Err(StorageError::Unsupported)
    }

    fn start_prefetch(&self) -> StorageResult<()> {
        Ok(())
    }

    fn stop_prefetch(&self) -> StorageResult<()> {
        Ok(())
    }

    fn is_prefetch_active(&self) -> bool {
        false
    }

    fn read(&self, iovec: &mut BlobIoVec, bufs: &[FileVolatileSlice]) -> Result<usize> {
        let bios = &iovec.bi_vec;

        if iovec.bi_size == 0 || bios.is_empty() {
            return Err(einval!("parameter `bios` is empty"));
        }

        let bios_len = bios.len();
        let offset = bios[0].offset;
        //let chunk = bios[0].chunkinfo.as_v5()?;
        let d_size = bios[0].chunkinfo.uncompressed_size() as usize;
        // Use the destination buffer to receive the uncompressed data if possible.
        if bufs.len() == 1 && bios_len == 1 && offset == 0 && bufs[0].len() >= d_size {
            if !bios[0].user_io {
                return Ok(0);
            }
            let buf = unsafe { std::slice::from_raw_parts_mut(bufs[0].as_ptr(), d_size) };
            return self.read_raw_chunk(&bios[0].chunkinfo, buf, false, None);
        }

        let mut user_size = 0;
        let mut buffer_holder: Vec<Vec<u8>> = Vec::with_capacity(bios.len());
        for bio in bios.iter() {
            if bio.user_io {
                let mut d = alloc_buf(bio.chunkinfo.uncompressed_size() as usize);
                self.read_raw_chunk(&bio.chunkinfo, d.as_mut_slice(), false, None)?;
                buffer_holder.push(d);
                // Even a merged IO can hardly reach u32::MAX. So this is safe
                user_size += bio.size;
            }
        }

        copyv(
            &buffer_holder,
            bufs,
            offset as usize,
            user_size as usize,
            0,
            0,
        )
        .map(|(n, _)| n)
        .map_err(|e| eother!(e))
    }
}

/// A dummy implementation of [BlobCacheMgr](../trait.BlobCacheMgr.html), simply reporting each
/// chunk as cached or not cached according to configuration.
///
/// The `DummyCacheMgr` is a dummy implementation of the `BlobCacheMgr`, which doesn't really cache
/// data. Instead it just reads data from the backend, uncompressed it if needed and then pass on
/// the data to the clients.
pub struct DummyCacheMgr {
    backend: Arc<dyn BlobBackend>,
    cached: bool,
    validate: bool,
    closed: AtomicBool,
}

impl DummyCacheMgr {
    /// Create a new instance of `DummmyCacheMgr`.
    pub fn new(
        config: CacheConfig,
        backend: Arc<dyn BlobBackend>,
        cached: bool,
    ) -> Result<DummyCacheMgr> {
        Ok(DummyCacheMgr {
            backend,
            cached,
            validate: config.cache_validate,
            closed: AtomicBool::new(false),
        })
    }
}

impl BlobCacheMgr for DummyCacheMgr {
    fn init(&self) -> Result<()> {
        Ok(())
    }

    fn destroy(&self) {
        if !self.closed.load(Ordering::Acquire) {
            self.closed.store(true, Ordering::Release);
            self.backend().shutdown();
        }
    }

    fn gc(&self, _id: Option<&str>) -> bool {
        false
    }

    fn backend(&self) -> &(dyn BlobBackend) {
        self.backend.as_ref()
    }

    fn get_blob_cache(&self, blob_info: &Arc<BlobInfo>) -> Result<Arc<dyn BlobCache>> {
        let blob_id = blob_info.blob_id().to_owned();
        let reader = self.backend.get_reader(&blob_id).map_err(|e| eother!(e))?;

        Ok(Arc::new(DummyCache {
            blob_id,
            chunk_map: Arc::new(NoopChunkMap::new(self.cached)),
            reader,
            compressor: blob_info.compressor(),
            digester: blob_info.digester(),
            is_stargz: blob_info.is_stargz(),
            validate: self.validate,
        }))
    }

    fn check_stat(&self) {}
}

impl Drop for DummyCacheMgr {
    fn drop(&mut self) {
        self.destroy();
    }
}
