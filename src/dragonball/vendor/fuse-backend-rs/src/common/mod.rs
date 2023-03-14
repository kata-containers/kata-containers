// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Some utilities to support fuse-backend-rs.
//!
//! ### Wrappers for Rust async io
//! It's challenging to support Rust async io, and it's even more challenging to support Rust async io with Linux io-uring.
//!
//! This `common` module adds a wrapper layer over [tokio](https://github.com/tokio-rs/tokio) and [tokio-uring](https://github.com/tokio-rs/tokio-uring) to simplify the way to support Rust async io by providing:
//! - [FileReadWriteVolatile](https://github.com/dragonflyoss/image-service): A trait similar to [std::io::Read] and [std::io::Write], but uses [FileVolatileSlice](https://github.com/dragonflyoss/image-service) objects as data buffers.
//! - [FileVolatileSlice](crate::buf::FileVolatileSlice): An adapter structure to work around limitations of the [vm-memory](https://github.com/rust-vmm/vm-memory) crate.
//! - [FileVolatileBuf](crate::buf::FileVolatileBuf): An adapter structure to support [io-uring](https://github.com/tokio-rs/io-uring) based asynchronous IO.
//! - [File](crate::async_file::File): An adapter for for [tokio::fs::File] and [tokio-uring::fs::File].
//! - [Runtime](crate::async_runtime::Runtime): An adapter for for [tokio::runtime::Runtime] and [tokio-uring::Runtime].

pub mod file_buf;
pub mod file_traits;

#[cfg(feature = "async-io")]
pub mod async_file;
#[cfg(feature = "async-io")]
pub mod async_runtime;
#[cfg(feature = "async-io")]
pub mod mpmc;

// Temporarily include all source code tokio-uring.
// Will switch to upstream once our enhancement have been merged and new version available.
#[cfg(all(feature = "async-io", target_os = "linux"))]
pub mod tokio_uring;
#[cfg(all(feature = "async-io", target_os = "linux"))]
pub(crate) use self::tokio_uring::{buf, driver, fs, future, BufResult};

#[cfg(target_os = "linux")]
#[doc(hidden)]
pub use libc::{off64_t, pread64, preadv64, pwrite64, pwritev64};
#[cfg(target_os = "macos")]
#[doc(hidden)]
pub use libc::{
    off_t as off64_t, pread as pread64, preadv as preadv64, pwrite as pwrite64,
    pwritev as pwritev64,
};
