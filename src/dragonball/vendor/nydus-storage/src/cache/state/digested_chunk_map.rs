// Copyright 2021 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! A chunk state tracking driver for legacy Nydus images without chunk array
//!
//! This module provides a chunk state tracking driver for legacy Rafs images without chunk array,
//! which uses chunk digest as id to track chunk readiness state. The [DigestedChunkMap] is not
//! optimal in case of performance and memory consumption. So it is only used only to keep backward
/// compatibility with the old nydus image format.
use std::collections::HashSet;
use std::io::Result;
use std::sync::RwLock;

use nydus_utils::digest::RafsDigest;

use crate::cache::state::{ChunkIndexGetter, ChunkMap};
use crate::device::BlobChunkInfo;

/// An implementation of [ChunkMap](trait.ChunkMap.html) to support chunk state tracking by using
/// `HashSet<RafsDigest>`.
///
/// The `DigestedChunkMap` is an implementation of [ChunkMap] which uses a hash set
/// (HashSet<chunk_digest>) to record whether a chunk has already been cached by the blob cache.
/// The implementation is memory and computation heavy, so it is used only to keep backward
/// compatibility with the previous old nydus bootstrap format. For new clients, please use other
/// alternative implementations.
#[derive(Default)]
pub struct DigestedChunkMap {
    cache: RwLock<HashSet<RafsDigest>>,
}

impl DigestedChunkMap {
    /// Create a new instance of `DigestedChunkMap`.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashSet::new()),
        }
    }
}

impl ChunkMap for DigestedChunkMap {
    fn is_ready(&self, chunk: &dyn BlobChunkInfo) -> Result<bool> {
        Ok(self.cache.read().unwrap().contains(chunk.chunk_id()))
    }

    fn set_ready_and_clear_pending(&self, chunk: &dyn BlobChunkInfo) -> Result<()> {
        // Do not expect poisoned lock.
        self.cache.write().unwrap().insert(*chunk.chunk_id());
        Ok(())
    }
}

impl ChunkIndexGetter for DigestedChunkMap {
    type Index = RafsDigest;

    fn get_index(chunk: &dyn BlobChunkInfo) -> Self::Index {
        *chunk.chunk_id()
    }
}
