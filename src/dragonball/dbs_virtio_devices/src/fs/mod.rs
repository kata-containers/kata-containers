// Copyright 2020 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

mod handler;
pub(crate) use self::handler::*;
mod device;
pub use self::device::*;

use std::io::Error as IOError;

use fuse_backend_rs::transport::Error as FuseTransportError;
use fuse_backend_rs::Error as FuseServerError;
use nix::Error as NixError;

pub const VIRTIO_FS_NAME: &str = "virtio-fs";

/// Error for virtio fs device.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Invalid Virtio descriptor chain.
    #[error("invalid descriptorchain: {0}")]
    InvalidDescriptorChain(FuseTransportError),
    /// Processing queue failed.
    #[error("process queue failed: {0}")]
    ProcessQueue(FuseServerError),
    #[error("invalid data.")]
    InvalidData,
    /// Failed to attach/detach a backend fs.
    #[error("attach/detach a backend filesystem failed:: {0}")]
    BackendFs(String),
    /// Error from IO error.
    #[error("io error: {0}")]
    IOError(#[from] IOError),
    /// Failed to create memfd
    #[error("failed to create memfd: {0}")]
    MemFdCreate(NixError),
    /// Failed to set file size
    #[error("failed to set file size: {0}")]
    SetFileSize(IOError),
}

/// Specialized std::result::Result for Virtio fs device operations.
pub type Result<T> = std::result::Result<T, Error>;
