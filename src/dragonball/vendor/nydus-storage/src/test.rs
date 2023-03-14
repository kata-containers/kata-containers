// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use nydus_utils::digest::RafsDigest;
use nydus_utils::metrics::BackendMetrics;

use super::impl_getter;
use crate::backend::{BackendResult, BlobBackend, BlobReader};
use crate::device::v5::BlobV5ChunkInfo;
use crate::device::{BlobChunkFlags, BlobChunkInfo};
use std::any::Any;

pub(crate) struct MockBackend {
    pub metrics: Arc<BackendMetrics>,
}

impl BlobReader for MockBackend {
    fn blob_size(&self) -> BackendResult<u64> {
        Ok(0)
    }

    fn try_read(&self, buf: &mut [u8], _offset: u64) -> BackendResult<usize> {
        let mut i = 0;
        while i < buf.len() {
            buf[i] = i as u8;
            i += 1;
        }
        Ok(i)
    }

    fn metrics(&self) -> &BackendMetrics {
        // Safe because nydusd must have backend attached with id, only image builder can no id
        // but use backend instance to upload blob.
        &self.metrics
    }
}

impl BlobBackend for MockBackend {
    fn shutdown(&self) {}

    fn metrics(&self) -> &BackendMetrics {
        // Safe because nydusd must have backend attached with id, only image builder can no id
        // but use backend instance to upload blob.
        &self.metrics
    }

    fn get_reader(&self, _blob_id: &str) -> BackendResult<Arc<dyn BlobReader>> {
        Ok(Arc::new(MockBackend {
            metrics: self.metrics.clone(),
        }))
    }
}

#[derive(Default, Clone)]
pub(crate) struct MockChunkInfo {
    pub block_id: RafsDigest,
    pub blob_index: u32,
    pub flags: BlobChunkFlags,
    pub compress_size: u32,
    pub uncompress_size: u32,
    pub compress_offset: u64,
    pub uncompress_offset: u64,
    pub file_offset: u64,
    pub index: u32,
    #[allow(unused)]
    pub reserved: u32,
}

impl MockChunkInfo {
    pub fn new() -> Self {
        MockChunkInfo::default()
    }
}

impl BlobChunkInfo for MockChunkInfo {
    fn chunk_id(&self) -> &RafsDigest {
        &self.block_id
    }
    fn id(&self) -> u32 {
        self.index
    }
    fn is_compressed(&self) -> bool {
        self.flags.contains(BlobChunkFlags::COMPRESSED)
    }
    fn is_hole(&self) -> bool {
        self.flags.contains(BlobChunkFlags::HOLECHUNK)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    impl_getter!(blob_index, blob_index, u32);
    impl_getter!(compressed_offset, compress_offset, u64);
    impl_getter!(compressed_size, compress_size, u32);
    impl_getter!(uncompressed_offset, uncompress_offset, u64);
    impl_getter!(uncompressed_size, uncompress_size, u32);
}

impl BlobV5ChunkInfo for MockChunkInfo {
    fn as_base(&self) -> &dyn BlobChunkInfo {
        self
    }

    impl_getter!(index, index, u32);
    impl_getter!(file_offset, file_offset, u64);
    impl_getter!(flags, flags, BlobChunkFlags);
}
