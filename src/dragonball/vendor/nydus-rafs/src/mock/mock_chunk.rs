// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::any::Any;
use std::sync::Arc;

use nydus_utils::digest::RafsDigest;
use storage::device::v5::BlobV5ChunkInfo;
use storage::device::{BlobChunkFlags, BlobChunkInfo};

/// Cached information about an Rafs Data Chunk.
#[derive(Clone, Default, Debug)]
pub struct MockChunkInfo {
    // block hash
    c_block_id: Arc<RafsDigest>,
    // blob containing the block
    c_blob_index: u32,
    // chunk index in blob
    c_index: u32,
    // position of the block within the file
    c_file_offset: u64,
    // offset of the block within the blob
    c_compress_offset: u64,
    c_decompress_offset: u64,
    // size of the block, compressed
    c_compr_size: u32,
    c_decompress_size: u32,
    c_flags: BlobChunkFlags,
}

impl MockChunkInfo {
    pub fn mock(
        file_offset: u64,
        compress_offset: u64,
        compress_size: u32,
        decompress_offset: u64,
        decompress_size: u32,
    ) -> Self {
        MockChunkInfo {
            c_file_offset: file_offset,
            c_compress_offset: compress_offset,
            c_compr_size: compress_size,
            c_decompress_offset: decompress_offset,
            c_decompress_size: decompress_size,
            ..Default::default()
        }
    }
}

impl BlobChunkInfo for MockChunkInfo {
    fn is_compressed(&self) -> bool {
        self.c_flags.contains(BlobChunkFlags::COMPRESSED)
    }

    fn is_hole(&self) -> bool {
        self.c_flags.contains(BlobChunkFlags::HOLECHUNK)
    }

    fn chunk_id(&self) -> &RafsDigest {
        &self.c_block_id
    }

    fn id(&self) -> u32 {
        self.c_index
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    impl_getter!(blob_index, c_blob_index, u32);
    impl_getter!(compressed_offset, c_compress_offset, u64);
    impl_getter!(compressed_size, c_compr_size, u32);
    impl_getter!(uncompressed_offset, c_decompress_offset, u64);
    impl_getter!(uncompressed_size, c_decompress_size, u32);
}

impl BlobV5ChunkInfo for MockChunkInfo {
    fn index(&self) -> u32 {
        self.c_index
    }

    fn file_offset(&self) -> u64 {
        self.c_file_offset
    }

    fn flags(&self) -> BlobChunkFlags {
        self.c_flags
    }

    fn as_base(&self) -> &dyn BlobChunkInfo {
        self
    }
}
