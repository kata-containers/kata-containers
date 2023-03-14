// Copyright 2020 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Storage backends to read blob data from Registry, OSS, disk, file system etc.
//!
//! There are several types of storage backend drivers implemented:
//! - [Registry](registry/struct.Registry.html): backend driver to access blobs on container image
//!   registry.
//! - [Oss](oss/struct.Oss.html): backend driver to access blobs on Oss(Object Storage System).
//! - [LocalFs](localfs/struct.LocalFs.html): backend driver to access blobs on local file system.
//!   The [LocalFs](localfs/struct.LocalFs.html) storage backend supports backend level data
//!   prefetching, which is to load data into page cache.

use std::sync::Arc;

use fuse_backend_rs::file_buf::FileVolatileSlice;
use nydus_utils::metrics::{BackendMetrics, ERROR_HOLDER};

use crate::utils::{alloc_buf, copyv};
use crate::StorageError;

#[cfg(any(feature = "backend-oss", feature = "backend-registry"))]
pub mod connection;
#[cfg(feature = "backend-localfs")]
pub mod localfs;
#[cfg(feature = "backend-oss")]
pub mod oss;
#[cfg(feature = "backend-registry")]
pub mod registry;

/// Error codes related to storage backend operations.
#[derive(Debug)]
pub enum BackendError {
    /// Unsupported operation.
    Unsupported(String),
    /// Failed to copy data from/into blob.
    CopyData(StorageError),
    #[cfg(feature = "backend-registry")]
    /// Error from Registry storage backend.
    Registry(self::registry::RegistryError),
    #[cfg(feature = "backend-localfs")]
    /// Error from LocalFs storage backend.
    LocalFs(self::localfs::LocalFsError),
    #[cfg(feature = "backend-oss")]
    /// Error from OSS storage backend.
    Oss(self::oss::OssError),
}

/// Specialized `Result` for storage backends.
pub type BackendResult<T> = std::result::Result<T, BackendError>;

/// Trait to read data from a on storage backend.
pub trait BlobReader: Send + Sync {
    /// Get size of the blob file.
    fn blob_size(&self) -> BackendResult<u64>;

    /// Try to read a range of data from the blob file into the provided buffer.
    ///
    /// Try to read data of range [offset, offset + buf.len()) from the blob file, and returns:
    /// - bytes of data read, which may be smaller than buf.len()
    /// - error code if error happens
    fn try_read(&self, buf: &mut [u8], offset: u64) -> BackendResult<usize>;

    /// Read a range of data from the blob file into the provided buffer.
    ///
    /// Read data of range [offset, offset + buf.len()) from the blob file, and returns:
    /// - bytes of data read, which may be smaller than buf.len()
    /// - error code if error happens
    ///
    /// It will try `BlobBackend::retry_limit()` times at most and return the first successfully
    /// read data.
    fn read(&self, buf: &mut [u8], offset: u64) -> BackendResult<usize> {
        let mut retry_count = self.retry_limit();
        let begin_time = self.metrics().begin();

        loop {
            match self.try_read(buf, offset) {
                Ok(size) => {
                    self.metrics().end(&begin_time, buf.len(), false);
                    return Ok(size);
                }
                Err(err) => {
                    if retry_count > 0 {
                        warn!(
                            "Read from backend failed: {:?}, retry count {}",
                            err, retry_count
                        );
                        retry_count -= 1;
                    } else {
                        self.metrics().end(&begin_time, buf.len(), true);
                        ERROR_HOLDER
                            .lock()
                            .unwrap()
                            .push(&format!("{:?}", err))
                            .unwrap_or_else(|_| error!("Failed when try to hold error"));
                        return Err(err);
                    }
                }
            }
        }
    }

    /// Read a range of data from the blob file into the provided buffers.
    ///
    /// Read data of range [offset, offset + max_size) from the blob file, and returns:
    /// - bytes of data read, which may be smaller than max_size
    /// - error code if error happens
    ///
    /// It will try `BlobBackend::retry_limit()` times at most and return the first successfully
    /// read data.
    fn readv(
        &self,
        bufs: &[FileVolatileSlice],
        offset: u64,
        max_size: usize,
    ) -> BackendResult<usize> {
        if bufs.len() == 1 && max_size >= bufs[0].len() {
            let buf = unsafe { std::slice::from_raw_parts_mut(bufs[0].as_ptr(), bufs[0].len()) };
            self.read(buf, offset)
        } else {
            // Use std::alloc to avoid zeroing the allocated buffer.
            let size = bufs.iter().fold(0usize, move |size, s| size + s.len());
            let size = std::cmp::min(size, max_size);
            let mut data = alloc_buf(size);

            let result = self.read(&mut data, offset)?;
            copyv(&[&data], bufs, 0, result, 0, 0)
                .map(|r| r.0)
                .map_err(BackendError::CopyData)
        }
    }

    /// Get metrics object.
    fn metrics(&self) -> &BackendMetrics;

    /// Get maximum number of times to retry when encountering IO errors.
    fn retry_limit(&self) -> u8 {
        0
    }
}

/// Trait to access blob files on backend storages, such as OSS, registry, local fs etc.
pub trait BlobBackend: Send + Sync {
    /// Destroy the `BlobBackend` storage object.
    fn shutdown(&self);

    /// Get metrics object.
    fn metrics(&self) -> &BackendMetrics;

    /// Get a blob reader object to access blod `blob_id`.
    fn get_reader(&self, blob_id: &str) -> BackendResult<Arc<dyn BlobReader>>;
}
