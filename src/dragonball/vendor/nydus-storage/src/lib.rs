// Copyright 2020 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Chunked blob storage service to support Rafs filesystem.
//!
//! The Rafs filesystem is blob based filesystem with chunk deduplication. A Rafs filesystem is
//! composed up of a metadata blob and zero or more data blobs. A blob is just a plain object
//! storage containing data chunks. Data chunks may be compressed, encrypted and deduplicated by
//! content digest value. When Rafs file is used for container images, Rafs metadata blob contains
//! all filesystem metadatas, such as directory, file name, permission etc. Actually file contents
//! are split into chunks and stored into data blobs. Rafs may built one data blob for each
//! container image layer or build a  single data blob for the whole image, according to building
//! options.
//!
//! The nydus-storage crate is used to manage and access chunked blobs for Rafs filesystem, which
//! contains three layers:
//! - [Backend](backend/index.html): access raw blob objects on remote storage backends.
//! - [Cache](cache/index.html): cache remote blob contents onto local storage in forms
//!   optimized for performance.
//! - [Device](device/index.html): public APIs for chunked blobs
//!
//! There are several core abstractions provided by the public APIs:
//! - [BlobInfo](device/struct.BlobInfo.html): provides information about blobs, which is typically
//!   constructed from the `blob array` in Rafs filesystem metadata.
//! - [BlobDevice](device/struct.BlobDevice.html): provides access to all blobs of a Rafs filesystem,
//!   which is constructed from an array of [BlobInfo](device/struct.BlobInfo.html) objects.
//! - [BlobChunkInfo](device/trait.BlobChunkInfo.html): provides information about a data chunk, which
//!   is loaded from Rafs metadata.
//! - [BlobIoDesc](device/struct.BlobIoDesc.html): a blob IO descriptor, containing information for a
//!   continuous IO range within a chunk.
//! - [BlobIoVec](device/struct.BlobIoVec.html): a scatter/gather list for blob IO operation, containing
//!   one or more blob IO descriptors
//!
//! To read data from the Rafs filesystem, the Rafs filesystem driver will prepare a
//! [BlobIoVec](device/struct.BlobIoVec.html)
//! object and submit it to the corresponding [BlobDevice](device/struct.BlobDevice.html)
//!  object to actually execute the IO
//! operations.
#[macro_use]
extern crate log;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate nydus_error;
extern crate dbs_uhttp;

use std::fmt::{Display, Formatter};

pub mod backend;
pub mod cache;
pub mod device;
pub mod factory;
pub mod meta;
pub mod remote;
#[cfg(test)]
pub(crate) mod test;
pub mod utils;

// A helper to impl RafsChunkInfo for upper layers like Rafs different metadata mode.
#[doc(hidden)]
#[macro_export]
macro_rules! impl_getter {
    ($G: ident, $F: ident, $U: ty) => {
        fn $G(&self) -> $U {
            self.$F
        }
    };
}

/// Default blob chunk size.
pub const RAFS_DEFAULT_CHUNK_SIZE: u64 = 1024 * 1024;
/// Maximum blob chunk size, 16MB.
pub const RAFS_MAX_CHUNK_SIZE: u64 = 1024 * 1024 * 16;

/// Error codes related to storage subsystem.
#[derive(Debug)]
pub enum StorageError {
    Unsupported,
    Timeout,
    VolatileSlice(vm_memory::VolatileMemoryError),
    MemOverflow,
    NotContinuous,
    CacheIndex(std::io::Error),
}

impl Display for StorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Unsupported => write!(f, "unsupported storage operation"),
            StorageError::Timeout => write!(f, "timeout when reading data from storage backend"),
            StorageError::MemOverflow => write!(f, "memory overflow when doing storage backend IO"),
            StorageError::NotContinuous => write!(f, "address ranges are not continuous"),
            StorageError::VolatileSlice(e) => write!(f, "{}", e),
            StorageError::CacheIndex(e) => write!(f, "Wrong cache index {}", e),
        }
    }
}

/// Specialized std::result::Result for storage subsystem.
pub type StorageResult<T> = std::result::Result<T, StorageError>;
