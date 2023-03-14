// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Common cached file object for `FileCacheMgr` and `FsCacheMgr`.
//!
//! The `FileCacheEntry` manages local cached blob objects from remote backends to improve
//! performance. It may be used by both the userspace `FileCacheMgr` or the `FsCacheMgr` based
//! on the in-kernel fscache system.

use std::fs::File;
use std::io::{ErrorKind, Result, Seek, SeekFrom};
use std::mem::ManuallyDrop;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::slice;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fuse_backend_rs::file_buf::FileVolatileSlice;
use nix::sys::uio;
use nix::unistd::dup;
use nydus_utils::metrics::{BlobcacheMetrics, Metric};
use nydus_utils::{compress, digest};
use tokio::runtime::Runtime;

use crate::backend::BlobReader;
use crate::cache::state::ChunkMap;
use crate::cache::worker::{AsyncPrefetchConfig, AsyncPrefetchMessage, AsyncWorkerMgr};
use crate::cache::{BlobCache, BlobIoMergeState};
use crate::device::{
    BlobChunkInfo, BlobInfo, BlobIoChunk, BlobIoDesc, BlobIoRange, BlobIoSegment, BlobIoTag,
    BlobIoVec, BlobObject, BlobPrefetchRequest,
};
use crate::meta::{BlobMetaChunk, BlobMetaInfo};
use crate::utils::{alloc_buf, copyv, readv, MemSliceCursor};
use crate::{StorageError, StorageResult, RAFS_DEFAULT_CHUNK_SIZE};

const DOWNLOAD_META_RETRY_COUNT: u32 = 20;
const DOWNLOAD_META_RETRY_DELAY: u64 = 500;

#[derive(Default, Clone)]
pub(crate) struct FileCacheMeta {
    has_error: Arc<AtomicBool>,
    meta: Arc<Mutex<Option<Arc<BlobMetaInfo>>>>,
}

impl FileCacheMeta {
    pub(crate) fn new(
        blob_file: String,
        blob_info: Arc<BlobInfo>,
        reader: Option<Arc<dyn BlobReader>>,
    ) -> Result<Self> {
        let meta = FileCacheMeta {
            has_error: Arc::new(AtomicBool::new(false)),
            meta: Arc::new(Mutex::new(None)),
        };
        let meta1 = meta.clone();

        std::thread::spawn(move || {
            let mut retry = 0;
            while retry < DOWNLOAD_META_RETRY_COUNT {
                match BlobMetaInfo::new(&blob_file, &blob_info, reader.as_ref()) {
                    Ok(m) => {
                        *meta1.meta.lock().unwrap() = Some(Arc::new(m));
                        return;
                    }
                    Err(e) => {
                        info!("temporarily failed to get blob.meta, {}", e);
                        std::thread::sleep(Duration::from_millis(DOWNLOAD_META_RETRY_DELAY));
                        retry += 1;
                    }
                }
            }
            warn!("failed to get blob.meta");
            meta1.has_error.store(true, Ordering::Release);
        });

        Ok(meta)
    }

    pub(crate) fn get_blob_meta(&self) -> Option<Arc<BlobMetaInfo>> {
        loop {
            let meta = self.meta.lock().unwrap();
            if meta.is_some() {
                return meta.clone();
            }
            drop(meta);
            if self.has_error.load(Ordering::Acquire) {
                return None;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

pub(crate) struct FileCacheEntry {
    pub(crate) blob_info: Arc<BlobInfo>,
    pub(crate) chunk_map: Arc<dyn ChunkMap>,
    pub(crate) file: Arc<File>,
    pub(crate) meta: Option<FileCacheMeta>,
    pub(crate) metrics: Arc<BlobcacheMetrics>,
    pub(crate) prefetch_state: Arc<AtomicU32>,
    pub(crate) reader: Arc<dyn BlobReader>,
    pub(crate) runtime: Arc<Runtime>,
    pub(crate) workers: Arc<AsyncWorkerMgr>,

    pub(crate) blob_compressed_size: u64,
    pub(crate) blob_uncompressed_size: u64,
    pub(crate) compressor: compress::Algorithm,
    pub(crate) digester: digest::Algorithm,
    // Whether `get_blob_object()` is supported.
    pub(crate) is_get_blob_object_supported: bool,
    // The compressed data instead of uncompressed data is cached if `compressed` is true.
    pub(crate) is_compressed: bool,
    // Whether direct chunkmap is used.
    pub(crate) is_direct_chunkmap: bool,
    // The blob is for an stargz image.
    pub(crate) is_stargz: bool,
    // True if direct IO is enabled for the `self.file`, supported for fscache only.
    pub(crate) dio_enabled: bool,
    // Data from the file cache should be validated before use.
    pub(crate) need_validate: bool,
    pub(crate) prefetch_config: Arc<AsyncPrefetchConfig>,
}

impl FileCacheEntry {
    pub(crate) fn get_blob_size(reader: &Arc<dyn BlobReader>, blob_info: &BlobInfo) -> Result<u64> {
        // Stargz needs blob size information, so hacky!
        let size = if blob_info.is_stargz() {
            reader.blob_size().map_err(|e| einval!(e))?
        } else {
            0
        };

        Ok(size)
    }
}

impl AsRawFd for FileCacheEntry {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

impl BlobCache for FileCacheEntry {
    fn blob_id(&self) -> &str {
        self.blob_info.blob_id()
    }

    fn blob_uncompressed_size(&self) -> Result<u64> {
        Ok(self.blob_uncompressed_size)
    }

    fn blob_compressed_size(&self) -> Result<u64> {
        Ok(self.blob_compressed_size)
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
        self.need_validate
    }

    fn reader(&self) -> &dyn BlobReader {
        &*self.reader
    }

    fn get_chunk_map(&self) -> &Arc<dyn ChunkMap> {
        &self.chunk_map
    }

    fn get_blob_object(&self) -> Option<&dyn BlobObject> {
        if self.is_get_blob_object_supported {
            Some(self)
        } else {
            None
        }
    }

    fn prefetch(
        &self,
        blob_cache: Arc<dyn BlobCache>,
        prefetches: &[BlobPrefetchRequest],
        bios: &[BlobIoDesc],
    ) -> StorageResult<usize> {
        let mut bios = bios.to_vec();
        let mut fail = false;
        bios.iter_mut().for_each(|b| {
            if let BlobIoChunk::Address(_blob_index, chunk_index) = b.chunkinfo {
                if let Some(meta) = self.meta.as_ref() {
                    if let Some(bm) = meta.get_blob_meta() {
                        let cki = BlobMetaChunk::new(chunk_index as usize, &bm.state);
                        // TODO: Improve the type conversion
                        b.chunkinfo = BlobIoChunk::Base(cki);
                    } else {
                        warn!("failed to get blob.meta for prefetch");
                        fail = true;
                    }
                }
            }
        });
        if fail {
            bios = vec![];
        } else {
            bios.sort_by_key(|entry| entry.chunkinfo.compressed_offset());
            self.metrics.prefetch_unmerged_chunks.add(bios.len() as u64);
        }

        // Handle blob prefetch request first, it may help performance.
        for req in prefetches {
            let msg = AsyncPrefetchMessage::new_blob_prefetch(
                blob_cache.clone(),
                req.offset as u64,
                req.len as u64,
            );
            let _ = self.workers.send_prefetch_message(msg);
        }

        // Then handle fs prefetch
        let merging_size = self.prefetch_config.merging_size;
        BlobIoMergeState::merge_and_issue(&bios, merging_size, |req: BlobIoRange| {
            let msg = AsyncPrefetchMessage::new_fs_prefetch(blob_cache.clone(), req);
            let _ = self.workers.send_prefetch_message(msg);
        });

        Ok(0)
    }

    fn start_prefetch(&self) -> StorageResult<()> {
        self.prefetch_state.fetch_add(1, Ordering::Release);
        Ok(())
    }

    fn stop_prefetch(&self) -> StorageResult<()> {
        loop {
            let val = self.prefetch_state.load(Ordering::Acquire);
            if val > 0
                && self
                    .prefetch_state
                    .compare_exchange(val, val - 1, Ordering::AcqRel, Ordering::Relaxed)
                    .is_err()
            {
                continue;
            }

            if val == 0 {
                warn!("storage: inaccurate prefetch status");
            }
            if val == 0 || val == 1 {
                self.workers
                    .flush_pending_prefetch_requests(self.blob_info.blob_id());
                return Ok(());
            }
        }
    }

    fn is_prefetch_active(&self) -> bool {
        self.prefetch_state.load(Ordering::Acquire) > 0
    }

    fn prefetch_range(&self, range: &BlobIoRange) -> Result<usize> {
        let mut pending = Vec::with_capacity(range.chunks.len());
        if !self.chunk_map.is_persist() {
            let mut d_size = 0;
            for c in range.chunks.iter() {
                d_size = std::cmp::max(d_size, c.uncompressed_size() as usize);
            }
            let mut buf = alloc_buf(d_size);

            for c in range.chunks.iter() {
                if let Ok(true) = self.chunk_map.check_ready_and_mark_pending(c.as_ref()) {
                    // The chunk is ready, so skip it.
                    continue;
                }

                // For digested chunk map, we must check whether the cached data is valid because
                // the digested chunk map cannot persist readiness state.
                let d_size = c.uncompressed_size() as usize;
                match self.read_raw_chunk(c.as_ref(), &mut buf[0..d_size], true, None) {
                    Ok(_v) => {
                        // The cached data is valid, set the chunk as ready.
                        let _ = self
                            .chunk_map
                            .set_ready_and_clear_pending(c.as_ref())
                            .map_err(|e| error!("Failed to set chunk ready: {:?}", e));
                    }
                    Err(_e) => {
                        // The cached data is invalid, queue the chunk for reading from backend.
                        pending.push(c.clone());
                    }
                }
            }
        } else {
            for c in range.chunks.iter() {
                if let Ok(true) = self.chunk_map.check_ready_and_mark_pending(c.as_ref()) {
                    // The chunk is ready, so skip it.
                    continue;
                } else {
                    pending.push(c.clone());
                }
            }
        }

        let mut total_size = 0;
        let mut start = 0;
        while start < pending.len() {
            // Be careful that `end` is inclusive.
            let mut end = start;

            // Figure out the range with continuous chunk ids.
            while end < pending.len() - 1 && pending[end + 1].id() == pending[end].id() + 1 {
                end += 1;
            }

            // Don't forget to clear its pending state whenever backend IO fails.
            let blob_offset = pending[start].compressed_offset();
            let blob_end = pending[end].compressed_offset() + pending[end].compressed_size() as u64;
            let blob_size = (blob_end - blob_offset) as usize;

            match self.read_chunks(blob_offset, blob_size, &pending[start..=end], true) {
                Ok(v) => {
                    total_size += blob_size;
                    for idx in start..=end {
                        let offset = if self.is_compressed {
                            pending[idx].compressed_offset()
                        } else {
                            pending[idx].uncompressed_offset()
                        };
                        match Self::persist_chunk(&self.file, offset, &v[idx - start]) {
                            Ok(_) => {
                                let _ = self
                                    .chunk_map
                                    .set_ready_and_clear_pending(pending[idx].as_ref());
                            }
                            Err(_) => self.chunk_map.clear_pending(pending[idx].as_ref()),
                        }
                    }
                }
                Err(_e) => {
                    // Clear the pending flag for all chunks in processing.
                    for chunk in &mut pending[start..=end] {
                        self.chunk_map.clear_pending(chunk.as_ref());
                    }
                }
            }

            start = end + 1;
        }

        Ok(total_size)
    }

    fn read(&self, iovec: &mut BlobIoVec, buffers: &[FileVolatileSlice]) -> Result<usize> {
        debug_assert!(iovec.validate());
        self.metrics.total.inc();
        self.workers.consume_prefetch_budget(iovec.bi_size);

        if let Some(meta) = self.meta.as_ref() {
            if let Some(bm) = meta.get_blob_meta() {
                // Convert `BlocIoChunk::Address` to `BlobIoChunk::Base` since rafs v6 has no chunks' meta
                // in bootstrap.
                for b in iovec.bi_vec.iter_mut() {
                    if let BlobIoChunk::Address(_blob_index, chunk_index) = b.chunkinfo {
                        b.chunkinfo =
                            BlobIoChunk::Base(BlobMetaChunk::new(chunk_index as usize, &bm.state));
                    }
                }
            } else {
                return Err(einval!("failed to get blob.meta for read"));
            }
        }

        if iovec.bi_vec.is_empty() {
            Ok(0)
        } else if iovec.bi_vec.len() == 1 {
            let mut state = FileIoMergeState::new();
            let mut cursor = MemSliceCursor::new(buffers);
            let req = BlobIoRange::new(&iovec.bi_vec[0], 1);

            self.dispatch_one_range(&req, &mut cursor, &mut state)
        } else {
            self.read_iter(&mut iovec.bi_vec, buffers)
        }
    }
}

impl BlobObject for FileCacheEntry {
    fn base_offset(&self) -> u64 {
        0
    }

    fn is_all_data_ready(&self) -> bool {
        if let Some(b) = self.chunk_map.as_range_map() {
            b.is_range_all_ready()
        } else {
            false
        }
    }

    fn fetch_range_compressed(&self, offset: u64, size: u64) -> Result<usize> {
        let meta = self.meta.as_ref().ok_or_else(|| einval!())?;
        let meta = meta.get_blob_meta().ok_or_else(|| einval!())?;
        let chunks = meta.get_chunks_compressed(offset, size, RAFS_DEFAULT_CHUNK_SIZE * 2)?;
        debug_assert!(!chunks.is_empty());
        self.do_fetch_chunks(&chunks, true)
    }

    fn fetch_range_uncompressed(&self, offset: u64, size: u64) -> Result<usize> {
        let meta = self.meta.as_ref().ok_or_else(|| einval!())?;
        let meta = meta.get_blob_meta().ok_or_else(|| einval!())?;
        let chunks = meta.get_chunks_uncompressed(offset, size, RAFS_DEFAULT_CHUNK_SIZE * 2)?;
        debug_assert!(!chunks.is_empty());
        self.do_fetch_chunks(&chunks, false)
    }

    fn prefetch_chunks(&self, range: &BlobIoRange) -> Result<usize> {
        let chunks = &range.chunks;
        if chunks.is_empty() {
            return Ok(0);
        }

        let mut ready_or_pending = matches!(
            self.chunk_map.is_ready_or_pending(chunks[0].as_ref()),
            Ok(true)
        );
        for idx in 1..chunks.len() {
            if chunks[idx - 1].id() + 1 != chunks[idx].id() {
                return Err(einval!("chunks for fetch_chunks() must be continuous"));
            }
            if ready_or_pending
                && !matches!(
                    self.chunk_map.is_ready_or_pending(chunks[idx].as_ref()),
                    Ok(true)
                )
            {
                ready_or_pending = false;
            }
        }
        // All chunks to be prefetched are already pending for downloading, no need to reissue.
        if ready_or_pending {
            return Ok(0);
        }

        if range.blob_size < RAFS_DEFAULT_CHUNK_SIZE {
            let max_size = RAFS_DEFAULT_CHUNK_SIZE - range.blob_size;
            if let Some(meta) = self.meta.as_ref() {
                if let Some(bm) = meta.get_blob_meta() {
                    if let Some(chunks) = bm.add_more_chunks(chunks, max_size) {
                        return self.do_fetch_chunks(&chunks, true);
                    }
                } else {
                    return Err(einval!("failed to get blob.meta"));
                }
            }
        }

        self.do_fetch_chunks(chunks, true)
    }
}

impl FileCacheEntry {
    fn do_fetch_chunks(&self, chunks: &[Arc<dyn BlobChunkInfo>], prefetch: bool) -> Result<usize> {
        if self.is_stargz() {
            // FIXME: for stargz, we need to implement fetching multiple chunks. here
            // is a heavy overhead workaround, needs to be optimized.
            for chunk in chunks {
                let mut buf = alloc_buf(chunk.uncompressed_size() as usize);
                self.read_raw_chunk(chunk.as_ref(), &mut buf, false, None)
                    .map_err(|e| {
                        eio!(format!(
                            "read_raw_chunk failed to read and decompress stargz chunk, {:?}",
                            e
                        ))
                    })?;
                if self.dio_enabled {
                    self.adjust_buffer_for_dio(&mut buf)
                }
                Self::persist_chunk(&self.file, chunk.uncompressed_offset(), &buf).map_err(
                    |e| {
                        eio!(format!(
                            "do_fetch_chunk failed to persist stargz chunk, {:?}",
                            e
                        ))
                    },
                )?;
                self.chunk_map
                    .set_ready_and_clear_pending(chunk.as_ref())
                    .unwrap_or_else(|e| error!("set stargz chunk ready failed, {}", e));
            }
            return Ok(0);
        }

        debug_assert!(!chunks.is_empty());
        let bitmap = self
            .chunk_map
            .as_range_map()
            .ok_or_else(|| einval!("invalid chunk_map for do_fetch_chunks()"))?;
        let chunk_index = chunks[0].id();
        let count = chunks.len() as u32;

        // Get chunks not ready yet, also marking them as inflight.
        let pending = match bitmap.check_range_ready_and_mark_pending(chunk_index, count)? {
            None => return Ok(0),
            Some(v) => v,
        };

        let mut total_size = 0;
        let mut start = 0;
        while start < pending.len() {
            let mut end = start + 1;
            while end < pending.len() && pending[end] == pending[end - 1] + 1 {
                end += 1;
            }

            let start_idx = (pending[start] - chunk_index) as usize;
            let end_idx = start_idx + (end - start) - 1;
            let blob_offset = chunks[start_idx].compressed_offset();
            let blob_end =
                chunks[end_idx].compressed_offset() + chunks[end_idx].compressed_size() as u64;
            let blob_size = (blob_end - blob_offset) as usize;

            match self.read_chunks(
                blob_offset,
                blob_size,
                &chunks[start_idx..=end_idx],
                prefetch,
            ) {
                Ok(mut v) => {
                    total_size += blob_size;
                    trace!(
                        "range persist chunk start {} {} pending {} {}",
                        start,
                        end,
                        start_idx,
                        end_idx
                    );
                    for idx in start_idx..=end_idx {
                        let offset = if self.is_compressed {
                            chunks[idx].compressed_offset()
                        } else {
                            chunks[idx].uncompressed_offset()
                        };
                        let buf = &mut v[idx - start_idx];
                        if self.dio_enabled {
                            self.adjust_buffer_for_dio(buf)
                        }
                        trace!("persist_chunk idx {}", idx);
                        if let Err(e) = Self::persist_chunk(&self.file, offset, buf) {
                            bitmap.clear_range_pending(pending[start], (end - start) as u32);
                            return Err(eio!(format!(
                                "do_fetch_chunk failed to persist data, {:?}",
                                e
                            )));
                        }
                    }

                    bitmap
                        .set_range_ready_and_clear_pending(pending[start], (end - start) as u32)?;
                }
                Err(e) => {
                    bitmap.clear_range_pending(pending[start], (end - start) as u32);
                    return Err(e);
                }
            }

            start = end;
        }

        if !bitmap.wait_for_range_ready(chunk_index, count)? {
            if prefetch {
                return Err(eio!("failed to read data from storage backend"));
            }
            // if we are in ondemand path, retry for the timeout chunks
            for chunk in chunks {
                if self.chunk_map.is_ready(chunk.as_ref())? {
                    continue;
                }
                info!("retry for timeout chunk, {}", chunk.id());
                let mut buf = alloc_buf(chunk.uncompressed_size() as usize);
                self.read_raw_chunk(chunk.as_ref(), &mut buf, false, None)
                    .map_err(|e| eio!(format!("read_raw_chunk failed, {:?}", e)))?;
                if self.dio_enabled {
                    self.adjust_buffer_for_dio(&mut buf)
                }
                Self::persist_chunk(&self.file, chunk.uncompressed_offset(), &buf)
                    .map_err(|e| eio!(format!("do_fetch_chunk failed to persist data, {:?}", e)))?;
                self.chunk_map
                    .set_ready_and_clear_pending(chunk.as_ref())
                    .unwrap_or_else(|e| error!("set chunk ready failed, {}", e));
            }
            Ok(total_size)
        } else {
            Ok(total_size)
        }
    }

    fn adjust_buffer_for_dio(&self, buf: &mut Vec<u8>) {
        debug_assert!(buf.capacity() % 0x1000 == 0);
        if buf.len() != buf.capacity() {
            // Padding with 0 for direct IO.
            buf.resize(buf.capacity(), 0);
        }
    }
}

impl FileCacheEntry {
    // There are some assumption applied to the `bios` passed to `read_iter()`.
    // - The blob address of chunks in `bios` are continuous.
    // - There is at most one user io request in the `bios`.
    // - The user io request may not be aligned on chunk boundary.
    // - The user io request may partially consume data from the first and last chunk of user io
    //   request.
    // - Optionally there may be some prefetch/read amplify requests following the user io request.
    // - The optional prefetch/read amplify requests may be silently dropped.
    fn read_iter(&self, bios: &mut [BlobIoDesc], buffers: &[FileVolatileSlice]) -> Result<usize> {
        // Merge requests with continuous blob addresses.
        let requests = self
            .merge_requests_for_user(bios, RAFS_DEFAULT_CHUNK_SIZE as usize * 2)
            .ok_or_else(|| einval!("Empty bios list"))?;
        let mut state = FileIoMergeState::new();
        let mut cursor = MemSliceCursor::new(buffers);
        let mut total_read: usize = 0;

        for req in requests {
            total_read += self.dispatch_one_range(&req, &mut cursor, &mut state)?;
            state.reset();
        }

        Ok(total_read)
    }

    fn dispatch_one_range(
        &self,
        req: &BlobIoRange,
        cursor: &mut MemSliceCursor,
        state: &mut FileIoMergeState,
    ) -> Result<usize> {
        let mut total_read: usize = 0;

        trace!("dispatch single io range {:?}", req);
        for (i, chunk) in req.chunks.iter().enumerate() {
            let is_ready = match self.chunk_map.check_ready_and_mark_pending(chunk.as_ref()) {
                Ok(true) => true,
                Ok(false) => false,
                Err(StorageError::Timeout) => false, // Retry if waiting for inflight IO timeouts
                Err(e) => return Err(einval!(e)),
            };

            // Directly read data from the file cache into the user buffer iff:
            // - the chunk is ready in the file cache
            // - the data in the file cache is uncompressed.
            // - data validation is disabled
            if is_ready && !self.is_compressed && !self.need_validate {
                // Internal IO should not be committed to local cache region, just
                // commit this region without pushing any chunk to avoid discontinuous
                // chunks in a region.
                if req.tags[i].is_user_io() {
                    state.push(
                        RegionType::CacheFast,
                        chunk.uncompressed_offset(),
                        chunk.uncompressed_size(),
                        req.tags[i].clone(),
                        None,
                    )?;
                } else {
                    state.commit()
                }
            } else if self.is_stargz || !self.is_direct_chunkmap || is_ready {
                // Case to try loading data from cache
                // - chunk is ready but data validation is needed.
                // - direct chunk map is not used, so there may be data in the file cache but
                //   the readiness flag has been lost.
                // - special path for stargz blobs. An stargz blob is abstracted as a compressed
                //   file cache always need validation.
                if req.tags[i].is_user_io() {
                    state.push(
                        RegionType::CacheSlow,
                        chunk.uncompressed_offset(),
                        chunk.uncompressed_size(),
                        req.tags[i].clone(),
                        Some(req.chunks[i].clone()),
                    )?;
                } else {
                    state.commit();
                    // On slow path, don't try to handle internal(read amplification) IO.
                    if !is_ready {
                        self.chunk_map.clear_pending(chunk.as_ref());
                    }
                }
            } else {
                let tag = if let BlobIoTag::User(ref s) = req.tags[i] {
                    BlobIoTag::User(s.clone())
                } else {
                    BlobIoTag::Internal(chunk.compressed_offset())
                };
                // NOTE: Only this request region can read more chunks from backend with user io.
                state.push(
                    RegionType::Backend,
                    chunk.compressed_offset(),
                    chunk.compressed_size(),
                    tag,
                    Some(chunk.clone()),
                )?;
            }
        }

        for r in &state.regions {
            use RegionType::*;

            total_read += match r.r#type {
                CacheFast => self.dispatch_cache_fast(cursor, r)?,
                CacheSlow => self.dispatch_cache_slow(cursor, r)?,
                Backend => self.dispatch_backend(cursor, r)?,
            }
        }

        Ok(total_read)
    }

    // Directly read data requested by user from the file cache into the user memory buffer.
    fn dispatch_cache_fast(&self, cursor: &mut MemSliceCursor, region: &Region) -> Result<usize> {
        let offset = region.blob_address + region.seg.offset as u64;
        let size = region.seg.len as usize;
        let mut iovec = cursor.consume(size);

        self.metrics.partial_hits.inc();
        readv(self.file.as_raw_fd(), &mut iovec, offset)
    }

    fn dispatch_cache_slow(&self, cursor: &mut MemSliceCursor, region: &Region) -> Result<usize> {
        let mut total_read = 0;

        for (i, c) in region.chunks.iter().enumerate() {
            let user_offset = if i == 0 { region.seg.offset } else { 0 };
            let size = std::cmp::min(
                c.uncompressed_size() - user_offset,
                region.seg.len - total_read as u32,
            );
            total_read += self.read_single_chunk(c.clone(), user_offset, size, cursor)?;
        }

        Ok(total_read)
    }

    fn dispatch_backend(&self, mem_cursor: &mut MemSliceCursor, region: &Region) -> Result<usize> {
        if region.chunks.is_empty() {
            return Ok(0);
        } else if !region.has_user_io() {
            debug!("No user data");
            for c in &region.chunks {
                self.chunk_map.clear_pending(c.as_ref());
            }
            return Ok(0);
        }

        let blob_size = region.blob_len as usize;
        debug!(
            "{} try to read {} bytes of {} chunks from backend",
            std::thread::current().name().unwrap_or_default(),
            blob_size,
            region.chunks.len()
        );

        let mut chunks = self.read_chunks(region.blob_address, blob_size, &region.chunks, false)?;
        assert_eq!(region.chunks.len(), chunks.len());

        let mut chunk_buffers = Vec::with_capacity(region.chunks.len());
        let mut buffer_holder = Vec::with_capacity(region.chunks.len());
        for (i, v) in chunks.drain(..).enumerate() {
            let d = Arc::new(DataBuffer::Allocated(v));
            if region.tags[i] {
                buffer_holder.push(d.clone());
            }
            self.delay_persist(region.chunks[i].clone(), d);
        }
        for d in buffer_holder.iter() {
            chunk_buffers.push(d.as_ref().slice());
        }

        let total_read = copyv(
            &chunk_buffers,
            mem_cursor.mem_slice,
            region.seg.offset as usize,
            region.seg.len as usize,
            mem_cursor.index,
            mem_cursor.offset,
        )
        .map(|(n, _)| n)
        .map_err(|e| {
            error!("failed to copy from chunk buf to buf: {:?}", e);
            eio!(e)
        })?;
        mem_cursor.move_cursor(total_read);

        Ok(total_read)
    }

    fn delay_persist(&self, chunk_info: Arc<dyn BlobChunkInfo>, buffer: Arc<DataBuffer>) {
        let delayed_chunk_map = self.chunk_map.clone();
        let file = self.file.clone();
        let offset = if self.is_compressed {
            chunk_info.compressed_offset()
        } else {
            chunk_info.uncompressed_offset()
        };
        let metrics = self.metrics.clone();

        metrics.buffered_backend_size.add(buffer.size() as u64);
        self.runtime.spawn_blocking(move || {
            metrics.buffered_backend_size.sub(buffer.size() as u64);
            match Self::persist_chunk(&file, offset, buffer.slice()) {
                Ok(_) => delayed_chunk_map
                    .set_ready_and_clear_pending(chunk_info.as_ref())
                    .unwrap_or_else(|e| {
                        error!(
                            "Failed change caching state for chunk of offset {}, {:?}",
                            chunk_info.compressed_offset(),
                            e
                        )
                    }),
                Err(e) => {
                    error!(
                        "Persist chunk of offset {} failed, {:?}",
                        chunk_info.compressed_offset(),
                        e
                    );
                    delayed_chunk_map.clear_pending(chunk_info.as_ref())
                }
            }
        });
    }

    /// Persist a single chunk into local blob cache file. We have to write to the cache
    /// file in unit of chunk size
    fn persist_chunk(file: &Arc<File>, offset: u64, buffer: &[u8]) -> Result<()> {
        let fd = file.as_raw_fd();

        let n = loop {
            let ret = uio::pwrite(fd, buffer, offset as i64).map_err(|_| last_error!());
            match ret {
                Ok(nr_write) => {
                    trace!("write {}(offset={}) bytes to cache file", nr_write, offset);
                    break nr_write;
                }
                Err(err) => {
                    // Retry if the IO is interrupted by signal.
                    if err.kind() != ErrorKind::Interrupted {
                        return Err(err);
                    }
                }
            }
        };

        if n != buffer.len() {
            Err(eio!("failed to write data to file cache"))
        } else {
            Ok(())
        }
    }

    fn read_single_chunk(
        &self,
        chunk: Arc<dyn BlobChunkInfo>,
        user_offset: u32,
        size: u32,
        mem_cursor: &mut MemSliceCursor,
    ) -> Result<usize> {
        debug!("single bio, blob offset {}", chunk.compressed_offset());

        let is_ready = self.chunk_map.is_ready(chunk.as_ref())?;
        let buffer_holder;
        let d_size = chunk.uncompressed_size() as usize;
        let mut d = DataBuffer::Allocated(alloc_buf(d_size));

        // Try to read and validate data from cache if:
        // - it's an stargz image and the chunk is ready.
        // - chunk data validation is enabled.
        // - digested or dummy chunk map is used.
        let try_cache = is_ready || (!self.is_stargz && !self.is_direct_chunkmap);
        let buffer = if try_cache && self.read_file_cache(chunk.as_ref(), d.mut_slice()).is_ok() {
            self.metrics.whole_hits.inc();
            self.chunk_map.set_ready_and_clear_pending(chunk.as_ref())?;
            trace!(
                "recover blob cache {} {} offset {} size {}",
                chunk.id(),
                d_size,
                user_offset,
                size,
            );
            &d
        } else if !self.is_compressed {
            self.read_raw_chunk(chunk.as_ref(), d.mut_slice(), false, None)?;
            buffer_holder = Arc::new(d.convert_to_owned_buffer());
            self.delay_persist(chunk.clone(), buffer_holder.clone());
            buffer_holder.as_ref()
        } else {
            let persist_compressed = |buffer: &[u8]| match Self::persist_chunk(
                &self.file,
                chunk.compressed_offset(),
                buffer,
            ) {
                Ok(_) => {
                    self.chunk_map
                        .set_ready_and_clear_pending(chunk.as_ref())
                        .unwrap_or_else(|e| error!("set ready failed, {}", e));
                }
                Err(e) => {
                    error!("Failed in writing compressed blob cache index, {}", e);
                    self.chunk_map.clear_pending(chunk.as_ref())
                }
            };
            self.read_raw_chunk(
                chunk.as_ref(),
                d.mut_slice(),
                false,
                Some(&persist_compressed),
            )?;
            &d
        };

        let dst_buffers = mem_cursor.inner_slice();
        let read_size = copyv(
            &[buffer.slice()],
            dst_buffers,
            user_offset as usize,
            size as usize,
            mem_cursor.index,
            mem_cursor.offset,
        )
        .map(|r| r.0)
        .map_err(|e| {
            error!("failed to copy from chunk buf to buf: {:?}", e);
            eother!(e)
        })?;
        mem_cursor.move_cursor(read_size);

        Ok(read_size)
    }

    fn read_file_cache(&self, chunk: &dyn BlobChunkInfo, buffer: &mut [u8]) -> Result<()> {
        let offset = if self.is_compressed {
            chunk.compressed_offset()
        } else {
            chunk.uncompressed_offset()
        };

        let mut d;
        let raw_buffer = if self.is_compressed && !self.is_stargz {
            // Need to put compressed data into a temporary buffer so as to perform decompression.
            //
            // gzip is special that it doesn't carry compress_size, instead, we make an IO stream
            // out of the file cache. So no need for an internal buffer here.
            let c_size = chunk.compressed_size() as usize;
            d = alloc_buf(c_size);
            d.as_mut_slice()
        } else {
            // We have this unsafe assignment as it can directly store data into call's buffer.
            unsafe { slice::from_raw_parts_mut(buffer.as_mut_ptr(), buffer.len()) }
        };

        let mut raw_stream = None;
        if self.is_stargz {
            debug!("using blobcache file offset {} as data stream", offset,);
            // FIXME: In case of multiple threads duplicating the same fd, they still share the
            // same file offset.
            let fd = dup(self.file.as_raw_fd()).map_err(|_| last_error!())?;
            let mut f = unsafe { File::from_raw_fd(fd) };
            f.seek(SeekFrom::Start(offset)).map_err(|_| last_error!())?;
            raw_stream = Some(f)
        } else {
            debug!(
                "reading blob cache file offset {} size {}",
                offset,
                raw_buffer.len()
            );
            let nr_read = uio::pread(self.file.as_raw_fd(), raw_buffer, offset as i64)
                .map_err(|_| last_error!())?;
            if nr_read == 0 || nr_read != raw_buffer.len() {
                return Err(einval!());
            }
        }

        // Try to validate data just fetched from backend inside.
        self.process_raw_chunk(
            chunk,
            raw_buffer,
            raw_stream,
            buffer,
            self.is_compressed,
            false,
        )?;

        Ok(())
    }

    fn merge_requests_for_user(
        &self,
        bios: &[BlobIoDesc],
        merging_size: usize,
    ) -> Option<Vec<BlobIoRange>> {
        let mut requests: Vec<BlobIoRange> = Vec::with_capacity(bios.len());

        BlobIoMergeState::merge_and_issue(bios, merging_size, |mr: BlobIoRange| {
            requests.push(mr);
        });

        if requests.is_empty() {
            None
        } else {
            Some(requests)
        }
    }
}

/// An enum to reuse existing buffers for IO operations, and CoW on demand.
#[allow(dead_code)]
enum DataBuffer {
    Reuse(ManuallyDrop<Vec<u8>>),
    Allocated(Vec<u8>),
}

impl DataBuffer {
    fn slice(&self) -> &[u8] {
        match self {
            Self::Reuse(data) => data.as_slice(),
            Self::Allocated(data) => data.as_slice(),
        }
    }

    fn mut_slice(&mut self) -> &mut [u8] {
        match self {
            Self::Reuse(ref mut data) => data.as_mut_slice(),
            Self::Allocated(ref mut data) => data.as_mut_slice(),
        }
    }

    fn size(&self) -> usize {
        match self {
            Self::Reuse(_) => 0,
            Self::Allocated(data) => data.capacity(),
        }
    }

    /// Make sure it owns the underlying memory buffer.
    fn convert_to_owned_buffer(self) -> Self {
        if let DataBuffer::Reuse(data) = self {
            DataBuffer::Allocated((*data).to_vec())
        } else {
            self
        }
    }

    #[allow(dead_code)]
    unsafe fn from_mut_slice(buf: &mut [u8]) -> Self {
        DataBuffer::Reuse(ManuallyDrop::new(Vec::from_raw_parts(
            buf.as_mut_ptr(),
            buf.len(),
            buf.len(),
        )))
    }
}

#[derive(PartialEq, Debug)]
enum RegionStatus {
    Init,
    Open,
    Committed,
}

#[derive(PartialEq, Copy, Clone)]
enum RegionType {
    // Fast path to read data from the cache directly, no decompression and validation needed.
    CacheFast,
    // Slow path to read data from the cache, due to decompression or validation.
    CacheSlow,
    // Need to read data from storage backend.
    Backend,
}

impl RegionType {
    fn joinable(&self, other: Self) -> bool {
        *self == other
    }
}

/// A continuous region in cache file or backend storage/blob, it may contain several chunks.
struct Region {
    r#type: RegionType,
    status: RegionStatus,
    // For debug and trace purpose implying how many chunks are concatenated
    count: u32,

    chunks: Vec<Arc<dyn BlobChunkInfo>>,
    tags: Vec<bool>,

    // The range [blob_address, blob_address + blob_len) specifies data to be read from backend.
    blob_address: u64,
    blob_len: u32,
    // The range specifying data to return to user.
    seg: BlobIoSegment,
}

impl Region {
    fn new(region_type: RegionType) -> Self {
        Region {
            r#type: region_type,
            status: RegionStatus::Init,
            count: 0,
            chunks: Vec::with_capacity(8),
            tags: Vec::with_capacity(8),
            blob_address: 0,
            blob_len: 0,
            seg: Default::default(),
        }
    }

    fn append(
        &mut self,
        start: u64,
        len: u32,
        tag: BlobIoTag,
        chunk: Option<Arc<dyn BlobChunkInfo>>,
    ) -> StorageResult<()> {
        debug_assert!(self.status != RegionStatus::Committed);

        if self.status == RegionStatus::Init {
            self.status = RegionStatus::Open;
            self.blob_address = start;
            self.blob_len = len;
            self.count = 1;
        } else {
            debug_assert!(self.status == RegionStatus::Open);
            if self.blob_address + self.blob_len as u64 != start
                || start.checked_add(len as u64).is_none()
            {
                return Err(StorageError::NotContinuous);
            }
            self.blob_len += len;
            self.count += 1;
        }

        // Maintain information for user triggered IO requests.
        if let BlobIoTag::User(ref s) = tag {
            if self.seg.is_empty() {
                self.seg = BlobIoSegment::new(s.offset, s.len);
            } else {
                self.seg.append(s.offset, s.len);
            }
        }

        if let Some(c) = chunk {
            self.chunks.push(c);
            self.tags.push(tag.is_user_io());
        }

        Ok(())
    }

    fn has_user_io(&self) -> bool {
        !self.seg.is_empty()
    }
}

struct FileIoMergeState {
    regions: Vec<Region>,
    // Whether last region can take in more io chunks. If not, a new region has to be
    // created for following chunks.
    last_region_joinable: bool,
}

impl FileIoMergeState {
    fn new() -> Self {
        FileIoMergeState {
            regions: Vec::with_capacity(8),
            last_region_joinable: true,
        }
    }

    fn push(
        &mut self,
        region_type: RegionType,
        start: u64,
        len: u32,
        tag: BlobIoTag,
        chunk: Option<Arc<dyn BlobChunkInfo>>,
    ) -> Result<()> {
        if self.regions.is_empty() || !self.joinable(region_type) {
            self.regions.push(Region::new(region_type));
            self.last_region_joinable = true;
        }

        let idx = self.regions.len() - 1;
        self.regions[idx]
            .append(start, len, tag, chunk)
            .map_err(|e| einval!(e))
    }

    // Committing current region ensures a new region will be created when more
    // chunks has to be added since `push` checks if newly pushed chunk is continuous
    // After committing, following `push` will create a new region.
    fn commit(&mut self) {
        self.last_region_joinable = false;
    }

    fn reset(&mut self) {
        self.regions.truncate(0);
    }

    #[inline]
    fn joinable(&self, region_type: RegionType) -> bool {
        debug_assert!(!self.regions.is_empty());
        let idx = self.regions.len() - 1;

        self.regions[idx].r#type.joinable(region_type) && self.last_region_joinable
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_buffer() {
        let mut buf1 = vec![0x1u8; 8];
        let buf2 = unsafe { DataBuffer::from_mut_slice(buf1.as_mut_slice()) };

        assert_eq!(buf2.slice()[1], 0x1);
        let mut buf2 = buf2.convert_to_owned_buffer();
        buf2.mut_slice()[1] = 0x2;
        assert_eq!(buf1[1], 0x1);
    }

    #[test]
    fn test_region_type() {
        assert!(RegionType::CacheFast.joinable(RegionType::CacheFast));
        assert!(RegionType::CacheSlow.joinable(RegionType::CacheSlow));
        assert!(RegionType::Backend.joinable(RegionType::Backend));

        assert!(!RegionType::CacheFast.joinable(RegionType::CacheSlow));
        assert!(!RegionType::CacheFast.joinable(RegionType::Backend));
        assert!(!RegionType::CacheSlow.joinable(RegionType::CacheFast));
        assert!(!RegionType::CacheSlow.joinable(RegionType::Backend));
        assert!(!RegionType::Backend.joinable(RegionType::CacheFast));
        assert!(!RegionType::Backend.joinable(RegionType::CacheSlow));
    }

    #[test]
    fn test_region_new() {
        let region = Region::new(RegionType::CacheFast);

        assert_eq!(region.status, RegionStatus::Init);
        assert!(!region.has_user_io());
        assert!(region.seg.is_empty());
        assert_eq!(region.chunks.len(), 0);
        assert_eq!(region.tags.len(), 0);
        assert_eq!(region.blob_address, 0);
        assert_eq!(region.blob_len, 0);
    }

    #[test]
    fn test_region_append() {
        let mut region = Region::new(RegionType::CacheFast);

        let tag = BlobIoTag::User(BlobIoSegment {
            offset: 0x1800,
            len: 0x1800,
        });
        region.append(0x1000, 0x2000, tag, None).unwrap();
        assert_eq!(region.status, RegionStatus::Open);
        assert_eq!(region.blob_address, 0x1000);
        assert_eq!(region.blob_len, 0x2000);
        assert_eq!(region.chunks.len(), 0);
        assert_eq!(region.tags.len(), 0);
        assert!(!region.seg.is_empty());
        assert!(region.has_user_io());

        let tag = BlobIoTag::User(BlobIoSegment {
            offset: 0x4000,
            len: 0x2000,
        });
        region.append(0x4000, 0x2000, tag, None).unwrap_err();
        assert_eq!(region.status, RegionStatus::Open);
        assert_eq!(region.blob_address, 0x1000);
        assert_eq!(region.blob_len, 0x2000);
        assert_eq!(region.seg.offset, 0x1800);
        assert_eq!(region.seg.len, 0x1800);
        assert_eq!(region.chunks.len(), 0);
        assert_eq!(region.tags.len(), 0);
        assert!(region.has_user_io());

        let tag = BlobIoTag::User(BlobIoSegment {
            offset: 0x3000,
            len: 0x2000,
        });
        region.append(0x3000, 0x2000, tag, None).unwrap();
        assert_eq!(region.status, RegionStatus::Open);
        assert_eq!(region.blob_address, 0x1000);
        assert_eq!(region.blob_len, 0x4000);
        assert_eq!(region.seg.offset, 0x1800);
        assert_eq!(region.seg.len, 0x3800);
        assert_eq!(region.chunks.len(), 0);
        assert_eq!(region.tags.len(), 0);
        assert!(!region.seg.is_empty());
        assert!(region.has_user_io());
    }

    #[test]
    fn test_file_io_merge_state() {
        let mut state = FileIoMergeState::new();
        assert_eq!(state.regions.len(), 0);

        let tag = BlobIoTag::User(BlobIoSegment {
            offset: 0x1800,
            len: 0x1800,
        });
        state
            .push(RegionType::CacheFast, 0x1000, 0x2000, tag, None)
            .unwrap();
        assert_eq!(state.regions.len(), 1);

        let tag = BlobIoTag::User(BlobIoSegment {
            offset: 0x3000,
            len: 0x2000,
        });
        state
            .push(RegionType::CacheFast, 0x3000, 0x2000, tag, None)
            .unwrap();
        assert_eq!(state.regions.len(), 1);

        let tag = BlobIoTag::User(BlobIoSegment {
            offset: 0x5000,
            len: 0x2000,
        });
        state
            .push(RegionType::CacheSlow, 0x5000, 0x2000, tag, None)
            .unwrap();
        assert_eq!(state.regions.len(), 2);
    }
}
