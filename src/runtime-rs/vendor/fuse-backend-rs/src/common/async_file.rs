// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! `File` to wrap over `tokio::fs::File` and `tokio-uring::fs::File`.

use std::fmt::{Debug, Formatter};
use std::io::{ErrorKind, IoSlice, IoSliceMut};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::path::Path;

use crate::async_runtime::{Runtime, CURRENT_RUNTIME};
use crate::file_buf::FileVolatileBuf;
use crate::{off64_t, preadv64, pwritev64};

/// An adapter enum to support both tokio and tokio-uring asynchronous `File`.
pub enum File {
    /// Tokio asynchronous `File`.
    Tokio(tokio::fs::File),
    #[cfg(target_os = "linux")]
    /// Tokio-uring asynchronous `File`.
    Uring(RawFd),
}

impl File {
    /// Asynchronously open a file.
    pub async fn async_open<P: AsRef<Path>>(
        path: P,
        write: bool,
        create: bool,
    ) -> std::io::Result<Self> {
        let ty = CURRENT_RUNTIME.with(|rt| match rt {
            Runtime::Tokio(_) => 1,
            #[cfg(target_os = "linux")]
            Runtime::Uring(_) => 2,
        });

        match ty {
            1 => tokio::fs::OpenOptions::new()
                .read(true)
                .write(write)
                .create(create)
                .open(path)
                .await
                .map(File::Tokio),
            #[cfg(target_os = "linux")]
            2 => crate::tokio_uring::fs::OpenOptions::new()
                .read(true)
                .write(write)
                .create(create)
                .open(path)
                .await
                .map(|v| {
                    // Convert the tokio_uring::fs::File object into RawFd(): v.into_raw_fd().
                    let file = File::Uring(v.as_raw_fd());
                    std::mem::forget(v);
                    file
                }),
            _ => panic!("should not happen"),
        }
    }

    /// Asynchronously read data at `offset` into the buffer.
    pub async fn async_read_at(
        &self,
        buf: FileVolatileBuf,
        offset: u64,
    ) -> (std::io::Result<usize>, FileVolatileBuf) {
        match self {
            File::Tokio(f) => {
                // tokio::fs:File doesn't support read_at() yet.
                //f.read_at(buf, offset).await,
                let mut bufs = [buf];
                let res = preadv(f.as_raw_fd(), &mut bufs, offset);
                (res, bufs[0])
            }
            #[cfg(target_os = "linux")]
            File::Uring(fd) => {
                // Safety: we rely on tokio_uring::fs::File internal implementation details.
                // It should be implemented as self.async_try_clone().await.unwrap().read_at,
                // but that causes two more syscalls.
                let file = unsafe { crate::tokio_uring::fs::File::from_raw_fd(*fd) };
                let res = file.read_at(buf, offset).await;
                std::mem::forget(file);
                res
            }
        }
    }

    /// Asynchronously read data at `offset` into buffers.
    pub async fn async_readv_at(
        &self,
        mut bufs: Vec<FileVolatileBuf>,
        offset: u64,
    ) -> (std::io::Result<usize>, Vec<FileVolatileBuf>) {
        match self {
            File::Tokio(f) => {
                // tokio::fs:File doesn't support read_at() yet.
                //f.read_at(buf, offset).await,
                let res = preadv(f.as_raw_fd(), &mut bufs, offset);
                (res, bufs)
            }
            #[cfg(target_os = "linux")]
            File::Uring(fd) => {
                // Safety: we rely on tokio_uring::fs::File internal implementation details.
                // It should be implemented as self.async_try_clone().await.unwrap().readv_at,
                // but that causes two more syscalls.
                let file = unsafe { crate::tokio_uring::fs::File::from_raw_fd(*fd) };
                let res = file.readv_at(bufs, offset).await;
                std::mem::forget(file);
                res
            }
        }
    }

    /// Asynchronously write data at `offset` from the buffer.
    pub async fn async_write_at(
        &self,
        buf: FileVolatileBuf,
        offset: u64,
    ) -> (std::io::Result<usize>, FileVolatileBuf) {
        match self {
            File::Tokio(f) => {
                // tokio::fs:File doesn't support read_at() yet.
                //f.read_at(buf, offset).await,
                let bufs = [buf];
                let res = pwritev(f.as_raw_fd(), &bufs, offset);
                (res, bufs[0])
            }
            #[cfg(target_os = "linux")]
            File::Uring(fd) => {
                // Safety: we rely on tokio_uring::fs::File internal implementation details.
                // It should be implemented as self.async_try_clone().await.unwrap().write_at,
                // but that causes two more syscalls.
                let file = unsafe { crate::tokio_uring::fs::File::from_raw_fd(*fd) };
                let res = file.write_at(buf, offset).await;
                std::mem::forget(file);
                res
            }
        }
    }

    /// Asynchronously write data at `offset` from buffers.
    pub async fn async_writev_at(
        &self,
        bufs: Vec<FileVolatileBuf>,
        offset: u64,
    ) -> (std::io::Result<usize>, Vec<FileVolatileBuf>) {
        match self {
            File::Tokio(f) => {
                // tokio::fs:File doesn't support read_at() yet.
                //f.read_at(buf, offset).await,
                let res = pwritev(f.as_raw_fd(), &bufs, offset);
                (res, bufs)
            }
            #[cfg(target_os = "linux")]
            File::Uring(fd) => {
                // Safety: we rely on tokio_uring::fs::File internal implementation details.
                // It should be implemented as self.async_try_clone().await.unwrap().writev_at,
                // but that causes two more syscalls.
                let file = unsafe { crate::tokio_uring::fs::File::from_raw_fd(*fd) };
                let res = file.writev_at(bufs, offset).await;
                std::mem::forget(file);
                res
            }
        }
    }

    /// Get metadata about the file.
    pub fn metadata(&self) -> std::io::Result<std::fs::Metadata> {
        // Safe because we have manually forget() the `file` object below.
        let file = unsafe { std::fs::File::from_raw_fd(self.as_raw_fd()) };
        let res = file.metadata();
        std::mem::forget(file);
        res
    }

    /// Try to clone the file object.
    pub async fn async_try_clone(&self) -> std::io::Result<Self> {
        match self {
            File::Tokio(f) => f.try_clone().await.map(File::Tokio),
            #[cfg(target_os = "linux")]
            File::Uring(fd) => {
                // Safe because file.as_raw_fd() is valid RawFd and we have checked the result.
                let fd = unsafe { libc::dup(*fd) };
                if fd < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(File::Uring(fd))
                }
            }
        }
    }
}

impl AsRawFd for File {
    fn as_raw_fd(&self) -> RawFd {
        match self {
            File::Tokio(f) => f.as_raw_fd(),
            #[cfg(target_os = "linux")]
            File::Uring(fd) => *fd,
        }
    }
}

impl Debug for File {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let fd = self.as_raw_fd();
        write!(f, "Async File {}", fd)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if let File::Uring(fd) = self {
            let _ = unsafe { crate::tokio_uring::fs::File::from_raw_fd(*fd) };
        }
    }
}

/// A simple wrapper over posix `preadv` to deal with `FileVolatileBuf`.
pub fn preadv(fd: RawFd, bufs: &mut [FileVolatileBuf], offset: u64) -> std::io::Result<usize> {
    let iov: Vec<IoSliceMut> = bufs.iter().map(|v| v.io_slice_mut()).collect();

    loop {
        // SAFETY: it is ABI compatible, a pointer cast here is valid
        let res = unsafe {
            preadv64(
                fd,
                iov.as_ptr() as *const libc::iovec,
                iov.len() as libc::c_int,
                offset as off64_t,
            )
        };

        if res >= 0 {
            let mut count = res as usize;
            for buf in bufs.iter_mut() {
                let cnt = std::cmp::min(count, buf.cap() - buf.len());
                unsafe { buf.set_size(buf.len() + cnt) };
                count -= cnt;
                if count == 0 {
                    break;
                }
            }
            assert_eq!(count, 0);
            return Ok(res as usize);
        } else {
            let e = std::io::Error::last_os_error();
            // Retry if the IO is interrupted by signal.
            if e.kind() != ErrorKind::Interrupted {
                return Err(e);
            }
        }
    }
}

/// A simple wrapper over posix `pwritev` to deal with `FileVolatileBuf`.
pub fn pwritev(fd: RawFd, bufs: &[FileVolatileBuf], offset: u64) -> std::io::Result<usize> {
    let iov: Vec<IoSlice> = bufs.iter().map(|v| v.io_slice()).collect();

    loop {
        // SAFETY: it is ABI compatible, a pointer cast here is valid
        let res = unsafe {
            pwritev64(
                fd,
                iov.as_ptr() as *const libc::iovec,
                iov.len() as libc::c_int,
                offset as off64_t,
            )
        };

        if res >= 0 {
            return Ok(res as usize);
        } else {
            let e = std::io::Error::last_os_error();
            // Retry if the IO is interrupted by signal.
            if e.kind() != ErrorKind::Interrupted {
                return Err(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_runtime::block_on;
    use vmm_sys_util::tempdir::TempDir;

    #[test]
    fn test_new_async_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.as_path().to_path_buf().join("test.txt");
        std::fs::write(&path, b"test").unwrap();

        let file = block_on(async { File::async_open(&path, false, false).await.unwrap() });
        assert!(file.as_raw_fd() >= 0);
        drop(file);
    }

    #[test]
    fn test_async_file_metadata() {
        let dir = TempDir::new().unwrap();
        let path = dir.as_path().to_path_buf();
        std::fs::write(path.join("test.txt"), b"test").unwrap();
        let file = block_on(async {
            File::async_open(path.join("test.txt"), false, false)
                .await
                .unwrap()
        });

        let md = file.metadata().unwrap();
        assert!(md.is_file());
        let md = file.metadata().unwrap();
        assert!(md.is_file());

        drop(file);
    }

    #[test]
    fn test_async_read_at() {
        let dir = TempDir::new().unwrap();
        let path = dir.as_path().to_path_buf();
        std::fs::write(path.join("test.txt"), b"test").unwrap();

        block_on(async {
            let file = File::async_open(path.join("test.txt"), false, false)
                .await
                .unwrap();

            let mut buffer = [0u8; 3];
            let buf = unsafe { FileVolatileBuf::new(&mut buffer) };
            let (res, buf) = file.async_read_at(buf, 0).await;
            assert_eq!(res.unwrap(), 3);
            assert_eq!(buf.len(), 3);
            let buf = unsafe { FileVolatileBuf::new(&mut buffer) };
            let (res, buf) = file.async_read_at(buf, 2).await;
            assert_eq!(res.unwrap(), 2);
            assert_eq!(buf.len(), 2);
        });
    }

    #[test]
    fn test_async_readv_at() {
        let dir = TempDir::new().unwrap();
        let path = dir.as_path().to_path_buf();
        std::fs::write(path.join("test.txt"), b"test").unwrap();

        block_on(async {
            let file = File::async_open(path.join("test.txt"), false, false)
                .await
                .unwrap();

            let mut buffer = [0u8; 3];
            let buf = unsafe { FileVolatileBuf::new(&mut buffer) };
            let mut buffer2 = [0u8; 3];
            let buf2 = unsafe { FileVolatileBuf::new(&mut buffer2) };
            let bufs = vec![buf, buf2];
            let (res, bufs) = file.async_readv_at(bufs, 0).await;

            assert_eq!(res.unwrap(), 4);
            assert_eq!(bufs[0].len(), 3);
            assert_eq!(bufs[1].len(), 1);
        });
    }

    #[test]
    fn test_async_write_at() {
        let dir = TempDir::new().unwrap();
        let path = dir.as_path().to_path_buf();

        block_on(async {
            let file = File::async_open(path.join("test.txt"), true, true)
                .await
                .unwrap();

            let buffer = b"test";
            let buf = unsafe {
                FileVolatileBuf::from_raw_ptr(
                    buffer.as_ptr() as *mut u8,
                    buffer.len(),
                    buffer.len(),
                )
            };
            let (res, buf) = file.async_write_at(buf, 0).await;
            assert_eq!(res.unwrap(), 4);
            assert_eq!(buf.len(), 4);

            let res = std::fs::read_to_string(path.join("test.txt")).unwrap();
            assert_eq!(&res, "test");
        });
    }

    #[test]
    fn test_async_writev_at() {
        let dir = TempDir::new().unwrap();
        let path = dir.as_path().to_path_buf();

        block_on(async {
            let file = File::async_open(path.join("test.txt"), true, true)
                .await
                .unwrap();

            let buffer = b"tes";
            let buf = unsafe {
                FileVolatileBuf::from_raw_ptr(
                    buffer.as_ptr() as *mut u8,
                    buffer.len(),
                    buffer.len(),
                )
            };
            let buffer2 = b"t";
            let buf2 = unsafe {
                FileVolatileBuf::from_raw_ptr(
                    buffer2.as_ptr() as *mut u8,
                    buffer2.len(),
                    buffer2.len(),
                )
            };
            let bufs = vec![buf, buf2];
            let (res, bufs) = file.async_writev_at(bufs, 0).await;

            assert_eq!(res.unwrap(), 4);
            assert_eq!(bufs[0].len(), 3);
            assert_eq!(bufs[1].len(), 1);

            let res = std::fs::read_to_string(path.join("test.txt")).unwrap();
            assert_eq!(&res, "test");
        });
    }

    #[test]
    fn test_async_try_clone() {
        let dir = TempDir::new().unwrap();
        let path = dir.as_path().to_path_buf();

        block_on(async {
            let file = File::async_open(path.join("test.txt"), true, true)
                .await
                .unwrap();

            let file2 = file.async_try_clone().await.unwrap();
            drop(file);

            let buffer = b"test";
            let buf = unsafe {
                FileVolatileBuf::from_raw_ptr(
                    buffer.as_ptr() as *mut u8,
                    buffer.len(),
                    buffer.len(),
                )
            };
            let (res, buf) = file2.async_write_at(buf, 0).await;
            assert_eq!(res.unwrap(), 4);
            assert_eq!(buf.len(), 4);
        });
    }
}
