// Copyright 2021 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Chunk or data readiness state tracking drivers.
//!
//! To cache data from remote backend storage onto local storage, a cache state tracking mechanism
//! is needed to track whether a specific chunk or data is ready on local storage and to cooperate
//! on concurrent data downloading. The [ChunkMap](trait.ChunkMap.html) trait is the main mechanism
//! to track chunk state. And [BlobStateMap](struct.BlobStateMap.html) is an adapter structure of
//! [ChunkMap] to support concurrent data downloading, which is based on a base [ChunkMap]
//! implementation to track chunk readiness state. And [RangeMap](trait.RangeMap.html) objects are
//! used to track readiness for a range of chunks or data, with support of batch operation.
//!
//! There are several implementation of the [ChunkMap] and [RangeMap] trait to track chunk and data
//! readiness state:
//! - [BlobStateMap](struct.BlobStateMap.html): an adapter structure to enable concurrent
//!   synchronization manipulation of readiness state, based on an underlying base [ChunkMap] or
//!   [RangeMap] object.
//! - [BlobRangeMap](struct.BlobRangeMap.html): a data state tracking driver using a bitmap file
//!   to persist state, indexed by data address range.
//! - [DigestedChunkMap](struct.DigestedChunkMap.html): a chunk state tracking driver
//!   for legacy Rafs images without chunk array, which uses chunk digest as the id to track chunk
//!   readiness state. The [DigestedChunkMap] is not optimal in case of performance and memory
//!   consumption.
//! - [IndexedChunkMap](struct.IndexedChunkMap.html): a chunk state tracking driver using a bitmap
//!   file to persist state, indexed by chunk index. There's a state bit in the bitmap file for each
//!   chunk, and atomic operations are used to manipulate the bitmap for concurrent state
//!   manipulating. It's the recommended state tracking driver.
//! - [NoopChunkMap](struct.NoopChunkMap.html): a no-operation chunk state tracking driver,
//!   which just reports every chunk as always ready to use or not. It may be used to support disk
//!   based backend storage or dummy cache.

use std::any::Any;
use std::io::Result;

use crate::device::BlobChunkInfo;
use crate::StorageResult;

pub use blob_state_map::BlobStateMap;
pub use digested_chunk_map::DigestedChunkMap;
pub use indexed_chunk_map::IndexedChunkMap;
pub use noop_chunk_map::NoopChunkMap;
pub use range_map::BlobRangeMap;

mod blob_state_map;
mod digested_chunk_map;
mod indexed_chunk_map;
mod noop_chunk_map;
mod persist_map;
mod range_map;

/// Trait to track chunk readiness state.
pub trait ChunkMap: Any + Send + Sync {
    /// Check whether the chunk is ready for use.
    fn is_ready(&self, chunk: &dyn BlobChunkInfo) -> Result<bool>;

    /// Check whether the chunk is pending for downloading.
    fn is_pending(&self, _chunk: &dyn BlobChunkInfo) -> Result<bool> {
        Ok(false)
    }

    /// Check whether a chunk is ready for use or pending for downloading.
    fn is_ready_or_pending(&self, chunk: &dyn BlobChunkInfo) -> Result<bool> {
        if matches!(self.is_pending(chunk), Ok(true)) {
            Ok(true)
        } else {
            self.is_ready(chunk)
        }
    }

    /// Check whether the chunk is ready for use, and mark it as pending if not ready yet.
    ///
    /// The function returns:
    /// - `Err(Timeout)` waiting for inflight backend IO timeouts.
    /// - `Ok(true)` if the the chunk is ready.
    /// - `Ok(false)` marks the chunk as pending, either set_ready_and_clear_pending() or
    ///   clear_pending() must be called to clear the pending state.
    fn check_ready_and_mark_pending(&self, _chunk: &dyn BlobChunkInfo) -> StorageResult<bool> {
        panic!("no support of check_ready_and_mark_pending()");
    }

    /// Set the chunk to ready for use and clear the pending state.
    fn set_ready_and_clear_pending(&self, _chunk: &dyn BlobChunkInfo) -> Result<()> {
        panic!("no support of check_ready_and_mark_pending()");
    }

    /// Clear the pending state of the chunk.
    fn clear_pending(&self, _chunk: &dyn BlobChunkInfo) {
        panic!("no support of clear_pending()");
    }

    /// Check whether the implementation supports state persistence.
    fn is_persist(&self) -> bool {
        false
    }

    /// Convert the objet to an [RangeMap](trait.RangeMap.html) object.
    fn as_range_map(&self) -> Option<&dyn RangeMap<I = u32>> {
        None
    }
}

/// Trait to track chunk or data readiness state.
///
/// A `RangeMap` object tracks readiness state of a chunk or data range, indexed by chunk index or
/// data address. The trait methods are designed to support batch operations for improving
/// performance by avoid frequently acquire/release locks.
pub trait RangeMap: Send + Sync {
    type I: Send + Sync;

    /// Check whether all chunks or data managed by the `RangeMap` object are ready.
    fn is_range_all_ready(&self) -> bool {
        false
    }

    /// Check whether all chunks or data in the range are ready for use.
    fn is_range_ready(&self, _start: Self::I, _count: Self::I) -> Result<bool> {
        Err(enosys!())
    }

    /// Check whether all chunks or data in the range [start, start + count) are ready.
    ///
    /// This function checks readiness of a range of chunks or data. If a chunk or data is both not
    /// ready and not pending(inflight), it will be marked as pending and returned. Following
    /// actions should be:
    /// - call set_range_ready_and_clear_pending() to mark data or chunks as ready and clear pending
    ///   state.
    /// - clear_range_pending() to clear the pending state without marking data or chunks as ready.
    /// - wait_for_range_ready() to wait for all data or chunks to clear pending state, including
    ///   data or chunks marked as pending by other threads.
    fn check_range_ready_and_mark_pending(
        &self,
        _start: Self::I,
        _count: Self::I,
    ) -> Result<Option<Vec<Self::I>>> {
        Err(enosys!())
    }

    /// Mark all chunks or data in the range as ready for use.
    fn set_range_ready_and_clear_pending(&self, _start: Self::I, _count: Self::I) -> Result<()> {
        Err(enosys!())
    }

    /// Clear the pending state for all chunks or data in the range.
    fn clear_range_pending(&self, _start: Self::I, _count: Self::I) {}

    /// Wait for all chunks or data in the range to be ready until timeout.
    fn wait_for_range_ready(&self, _start: Self::I, _count: Self::I) -> Result<bool> {
        Err(enosys!())
    }
}

/// Trait to convert a [BlobChunkInfo](../../device/trait.BlobChunkInfo.html) object to an index
/// needed by [ChunkMap](trait.ChunkMap.html).
pub trait ChunkIndexGetter {
    /// Type of index needed by [ChunkMap].
    type Index;

    /// Get the chunk's id/key for state tracking.
    fn get_index(chunk: &dyn BlobChunkInfo) -> Self::Index;
}
