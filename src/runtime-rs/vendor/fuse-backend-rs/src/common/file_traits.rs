// Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.

//! File extension traits to transfer data between File objects and [VolatileSlice][2] buffers.
//!
//! Fuse filesystem servers use normal memory buffers to transfer data from/to File objects.
//! For virtio-fs file servers, they need to transfer data between File objects and guest memory.
//! The guest memory could be accessed through [GuestMemory][1] or [VolatileSlice][2] objects.
//! And the [VolatileSlice][2] trait could also be used to access normal memory buffers too.
//! So several [VolatileSlice][2] based File extension traits are introduced to deal with both
//! guest memory and normal memory buffers.
//!
//! [1]: https://docs.rs/vm-memory/0.2.0/vm_memory/guest_memory/trait.GuestMemory.html
//! [2]: https://docs.rs/vm-memory/0.2.0/vm_memory/volatile_memory/struct.VolatileSlice.html

use std::fs::File;
use std::io::{Error, ErrorKind, Result};
use std::os::unix::io::AsRawFd;

use libc::{c_int, c_void, read, readv, size_t, write, writev};

use crate::file_buf::FileVolatileSlice;
use crate::{off64_t, pread64, preadv64, pwrite64, pwritev64};

/// A trait for setting the size of a file.
///
/// This is equivalent to File's `set_len` method, but wrapped in a trait so that it can be
/// implemented for other types.
pub trait FileSetLen {
    /// Set the size of this file.
    ///
    /// This is the moral equivalent of `ftruncate()`.
    fn set_len(&self, _len: u64) -> Result<()>;
}

impl FileSetLen for File {
    fn set_len(&self, len: u64) -> Result<()> {
        File::set_len(self, len)
    }
}

/// A trait similar to `Read` and `Write`, but uses [FileVolatileSlice] objects as data buffers.
pub trait FileReadWriteVolatile {
    /// Read bytes from this file into the given slice, returning the number of bytes read on
    /// success.
    fn read_volatile(&mut self, slice: FileVolatileSlice) -> Result<usize>;

    /// Like `read_volatile`, except it reads to a slice of buffers. Data is copied to fill each
    /// buffer in order, with the final buffer written to possibly being only partially filled. This
    /// method must behave as a single call to `read_volatile` with the buffers concatenated would.
    /// The default implementation calls `read_volatile` with either the first nonempty buffer
    /// provided, or returns `Ok(0)` if none exists.
    fn read_vectored_volatile(&mut self, bufs: &[FileVolatileSlice]) -> Result<usize> {
        bufs.iter()
            .find(|b| !b.is_empty())
            .map(|b| self.read_volatile(*b))
            .unwrap_or(Ok(0))
    }

    /// Reads bytes from this into the given slice until all bytes in the slice are written, or an
    /// error is returned.
    fn read_exact_volatile(&mut self, mut slice: FileVolatileSlice) -> Result<()> {
        while !slice.is_empty() {
            let bytes_read = self.read_volatile(slice)?;
            if bytes_read == 0 {
                return Err(Error::from(ErrorKind::UnexpectedEof));
            }
            // Will panic if read_volatile read more bytes than we gave it, which would be worthy of
            // a panic.
            slice = slice.offset(bytes_read).unwrap();
        }
        Ok(())
    }

    /// Write bytes from the slice to the given file, returning the number of bytes written on
    /// success.
    fn write_volatile(&mut self, slice: FileVolatileSlice) -> Result<usize>;

    /// Like `write_volatile`, except that it writes from a slice of buffers. Data is copied from
    /// each buffer in order, with the final buffer read from possibly being only partially
    /// consumed. This method must behave as a call to `write_volatile` with the buffers
    /// concatenated would. The default implementation calls `write_volatile` with either the first
    /// nonempty buffer provided, or returns `Ok(0)` if none exists.
    fn write_vectored_volatile(&mut self, bufs: &[FileVolatileSlice]) -> Result<usize> {
        bufs.iter()
            .find(|b| !b.is_empty())
            .map(|b| self.write_volatile(*b))
            .unwrap_or(Ok(0))
    }

    /// Write bytes from the slice to the given file until all the bytes from the slice have been
    /// written, or an error is returned.
    fn write_all_volatile(&mut self, mut slice: FileVolatileSlice) -> Result<()> {
        while !slice.is_empty() {
            let bytes_written = self.write_volatile(slice)?;
            if bytes_written == 0 {
                return Err(Error::from(ErrorKind::WriteZero));
            }
            // Will panic if write_volatile read more bytes than we gave it, which would be worthy
            // of a panic.
            slice = slice.offset(bytes_written).unwrap();
        }
        Ok(())
    }

    /// Reads bytes from this file at `offset` into the given slice, returning the number of bytes
    /// read on success.
    fn read_at_volatile(&mut self, slice: FileVolatileSlice, offset: u64) -> Result<usize>;

    /// Like `read_at_volatile`, except it reads to a slice of buffers. Data is copied to fill each
    /// buffer in order, with the final buffer written to possibly being only partially filled. This
    /// method must behave as a single call to `read_at_volatile` with the buffers concatenated
    /// would. The default implementation calls `read_at_volatile` with either the first nonempty
    /// buffer provided, or returns `Ok(0)` if none exists.
    fn read_vectored_at_volatile(
        &mut self,
        bufs: &[FileVolatileSlice],
        offset: u64,
    ) -> Result<usize> {
        if let Some(slice) = bufs.first() {
            self.read_at_volatile(*slice, offset)
        } else {
            Ok(0)
        }
    }

    /// Reads bytes from this file at `offset` into the given slice until all bytes in the slice are
    /// read, or an error is returned.
    fn read_exact_at_volatile(
        &mut self,
        mut slice: FileVolatileSlice,
        mut offset: u64,
    ) -> Result<()> {
        while !slice.is_empty() {
            match self.read_at_volatile(slice, offset) {
                Ok(0) => return Err(Error::from(ErrorKind::UnexpectedEof)),
                Ok(n) => {
                    // Will panic if read_at_volatile read more bytes than we gave it, which would
                    // be worthy of a panic.
                    slice = slice.offset(n).unwrap();
                    offset = offset.checked_add(n as u64).unwrap();
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Writes bytes from this file at `offset` into the given slice, returning the number of bytes
    /// written on success.
    fn write_at_volatile(&mut self, slice: FileVolatileSlice, offset: u64) -> Result<usize>;

    /// Like `write_at_at_volatile`, except that it writes from a slice of buffers. Data is copied
    /// from each buffer in order, with the final buffer read from possibly being only partially
    /// consumed. This method must behave as a call to `write_at_volatile` with the buffers
    /// concatenated would. The default implementation calls `write_at_volatile` with either the
    /// first nonempty buffer provided, or returns `Ok(0)` if none exists.
    fn write_vectored_at_volatile(
        &mut self,
        bufs: &[FileVolatileSlice],
        offset: u64,
    ) -> Result<usize> {
        if let Some(slice) = bufs.first() {
            self.write_at_volatile(*slice, offset)
        } else {
            Ok(0)
        }
    }

    /// Writes bytes from this file at `offset` into the given slice until all bytes in the slice
    /// are written, or an error is returned.
    fn write_all_at_volatile(
        &mut self,
        mut slice: FileVolatileSlice,
        mut offset: u64,
    ) -> Result<()> {
        while !slice.is_empty() {
            match self.write_at_volatile(slice, offset) {
                Ok(0) => return Err(Error::from(ErrorKind::WriteZero)),
                Ok(n) => {
                    // Will panic if write_at_volatile read more bytes than we gave it, which would
                    // be worthy of a panic.
                    slice = slice.offset(n).unwrap();
                    offset = offset.checked_add(n as u64).unwrap();
                }
                Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

impl<T: FileReadWriteVolatile + ?Sized> FileReadWriteVolatile for &mut T {
    fn read_volatile(&mut self, slice: FileVolatileSlice) -> Result<usize> {
        (**self).read_volatile(slice)
    }

    fn read_vectored_volatile(&mut self, bufs: &[FileVolatileSlice]) -> Result<usize> {
        (**self).read_vectored_volatile(bufs)
    }

    fn read_exact_volatile(&mut self, slice: FileVolatileSlice) -> Result<()> {
        (**self).read_exact_volatile(slice)
    }

    fn write_volatile(&mut self, slice: FileVolatileSlice) -> Result<usize> {
        (**self).write_volatile(slice)
    }

    fn write_vectored_volatile(&mut self, bufs: &[FileVolatileSlice]) -> Result<usize> {
        (**self).write_vectored_volatile(bufs)
    }

    fn write_all_volatile(&mut self, slice: FileVolatileSlice) -> Result<()> {
        (**self).write_all_volatile(slice)
    }

    fn read_at_volatile(&mut self, slice: FileVolatileSlice, offset: u64) -> Result<usize> {
        (**self).read_at_volatile(slice, offset)
    }

    fn read_vectored_at_volatile(
        &mut self,
        bufs: &[FileVolatileSlice],
        offset: u64,
    ) -> Result<usize> {
        (**self).read_vectored_at_volatile(bufs, offset)
    }

    fn read_exact_at_volatile(&mut self, slice: FileVolatileSlice, offset: u64) -> Result<()> {
        (**self).read_exact_at_volatile(slice, offset)
    }

    fn write_at_volatile(&mut self, slice: FileVolatileSlice, offset: u64) -> Result<usize> {
        (**self).write_at_volatile(slice, offset)
    }

    fn write_vectored_at_volatile(
        &mut self,
        bufs: &[FileVolatileSlice],
        offset: u64,
    ) -> Result<usize> {
        (**self).write_vectored_at_volatile(bufs, offset)
    }

    fn write_all_at_volatile(&mut self, slice: FileVolatileSlice, offset: u64) -> Result<()> {
        (**self).write_all_at_volatile(slice, offset)
    }
}

macro_rules! volatile_impl {
    ($ty:ty) => {
        impl FileReadWriteVolatile for $ty {
            fn read_volatile(&mut self, slice: FileVolatileSlice) -> Result<usize> {
                // Safe because only bytes inside the slice are accessed and the kernel is expected
                // to handle arbitrary memory for I/O.
                let ret =
                    unsafe { read(self.as_raw_fd(), slice.as_ptr() as *mut c_void, slice.len()) };

                if ret >= 0 {
                    Ok(ret as usize)
                } else {
                    Err(Error::last_os_error())
                }
            }

            fn read_vectored_volatile(&mut self, bufs: &[FileVolatileSlice]) -> Result<usize> {
                let iovecs: Vec<libc::iovec> = bufs
                    .iter()
                    .map(|s| libc::iovec {
                        iov_base: s.as_ptr() as *mut c_void,
                        iov_len: s.len() as size_t,
                    })
                    .collect();

                if iovecs.is_empty() {
                    return Ok(0);
                }

                // Safe because only bytes inside the buffers are accessed and the kernel is
                // expected to handle arbitrary memory for I/O.
                let ret = unsafe { readv(self.as_raw_fd(), &iovecs[0], iovecs.len() as c_int) };

                if ret >= 0 {
                    Ok(ret as usize)
                } else {
                    Err(Error::last_os_error())
                }
            }

            fn write_volatile(&mut self, slice: FileVolatileSlice) -> Result<usize> {
                // Safe because only bytes inside the slice are accessed and the kernel is expected
                // to handle arbitrary memory for I/O.
                let ret = unsafe {
                    write(
                        self.as_raw_fd(),
                        slice.as_ptr() as *const c_void,
                        slice.len(),
                    )
                };
                if ret >= 0 {
                    Ok(ret as usize)
                } else {
                    Err(Error::last_os_error())
                }
            }

            fn write_vectored_volatile(&mut self, bufs: &[FileVolatileSlice]) -> Result<usize> {
                let iovecs: Vec<libc::iovec> = bufs
                    .iter()
                    .map(|s| libc::iovec {
                        iov_base: s.as_ptr() as *mut c_void,
                        iov_len: s.len() as size_t,
                    })
                    .collect();

                if iovecs.is_empty() {
                    return Ok(0);
                }

                // Safe because only bytes inside the buffers are accessed and the kernel is
                // expected to handle arbitrary memory for I/O.
                let ret = unsafe { writev(self.as_raw_fd(), &iovecs[0], iovecs.len() as c_int) };
                if ret >= 0 {
                    Ok(ret as usize)
                } else {
                    Err(Error::last_os_error())
                }
            }

            fn read_at_volatile(&mut self, slice: FileVolatileSlice, offset: u64) -> Result<usize> {
                // Safe because only bytes inside the slice are accessed and the kernel is expected
                // to handle arbitrary memory for I/O.
                let ret = unsafe {
                    pread64(
                        self.as_raw_fd(),
                        slice.as_ptr() as *mut c_void,
                        slice.len(),
                        offset as off64_t,
                    )
                };

                if ret >= 0 {
                    Ok(ret as usize)
                } else {
                    Err(Error::last_os_error())
                }
            }

            fn read_vectored_at_volatile(
                &mut self,
                bufs: &[FileVolatileSlice],
                offset: u64,
            ) -> Result<usize> {
                let iovecs: Vec<libc::iovec> = bufs
                    .iter()
                    .map(|s| libc::iovec {
                        iov_base: s.as_ptr() as *mut c_void,
                        iov_len: s.len() as size_t,
                    })
                    .collect();

                if iovecs.is_empty() {
                    return Ok(0);
                }

                // Safe because only bytes inside the buffers are accessed and the kernel is
                // expected to handle arbitrary memory for I/O.
                let ret = unsafe {
                    preadv64(
                        self.as_raw_fd(),
                        &iovecs[0],
                        iovecs.len() as c_int,
                        offset as off64_t,
                    )
                };

                if ret >= 0 {
                    Ok(ret as usize)
                } else {
                    Err(Error::last_os_error())
                }
            }

            fn write_at_volatile(
                &mut self,
                slice: FileVolatileSlice,
                offset: u64,
            ) -> Result<usize> {
                // Safe because only bytes inside the slice are accessed and the kernel is expected
                // to handle arbitrary memory for I/O.
                let ret = unsafe {
                    pwrite64(
                        self.as_raw_fd(),
                        slice.as_ptr() as *const c_void,
                        slice.len(),
                        offset as off64_t,
                    )
                };

                if ret >= 0 {
                    Ok(ret as usize)
                } else {
                    Err(Error::last_os_error())
                }
            }

            fn write_vectored_at_volatile(
                &mut self,
                bufs: &[FileVolatileSlice],
                offset: u64,
            ) -> Result<usize> {
                let iovecs: Vec<libc::iovec> = bufs
                    .iter()
                    .map(|s| libc::iovec {
                        iov_base: s.as_ptr() as *mut c_void,
                        iov_len: s.len() as size_t,
                    })
                    .collect();

                if iovecs.is_empty() {
                    return Ok(0);
                }

                // Safe because only bytes inside the buffers are accessed and the kernel is
                // expected to handle arbitrary memory for I/O.
                let ret = unsafe {
                    pwritev64(
                        self.as_raw_fd(),
                        &iovecs[0],
                        iovecs.len() as c_int,
                        offset as off64_t,
                    )
                };
                if ret >= 0 {
                    Ok(ret as usize)
                } else {
                    Err(Error::last_os_error())
                }
            }
        }
    };
}

volatile_impl!(File);

#[cfg(all(target_os = "linux", feature = "async-io"))]
pub use async_io::AsyncFileReadWriteVolatile;

#[cfg(all(target_os = "linux", feature = "async-io"))]
mod async_io {
    use std::sync::Arc;

    use tokio::join;

    use super::*;
    use crate::async_file::File;
    use crate::file_buf::FileVolatileBuf;
    use crate::tokio_uring::buf::IoBuf;

    /// Extension of [FileReadWriteVolatile] to support io-uring based asynchronous IO.
    ///
    /// The asynchronous IO framework provided by [tokio-uring](https://docs.rs/tokio-uring/latest/tokio_uring/)
    /// needs to take ownership of data buffers during asynchronous IO operations.
    /// The [AsyncFileReadWriteVolatile] trait is designed to support io-uring based asynchronous IO.
    #[async_trait::async_trait(?Send)]
    pub trait AsyncFileReadWriteVolatile {
        /// Read bytes from this file at `offset` into the given slice in asynchronous mode.
        ///
        /// Return the number of bytes read on success.
        async fn async_read_at_volatile(
            &self,
            buf: FileVolatileBuf,
            offset: u64,
        ) -> (Result<usize>, FileVolatileBuf);

        /// Asynchronous version of [FileReadWriteVolatile::read_vectored_at_volatile], to read data
        /// into [FileVolatileSlice] buffers.
        ///
        /// Like `async_read_at_volatile()`, except it reads to a slice of buffers. Data is copied
        /// to fill each buffer in order, with the final buffer written to possibly being only
        /// partially filled. This method must behave as a single call to `read_at_volatile` with
        /// the buffers concatenated would.
        ///
        /// Returns `Ok(0)` if none exists.
        async fn async_read_vectored_at_volatile(
            &self,
            bufs: Vec<FileVolatileBuf>,
            offset: u64,
        ) -> (Result<usize>, Vec<FileVolatileBuf>);

        /// Asynchronous version of [FileReadWriteVolatile::write_at_volatile], to write
        /// data from a [FileVolatileSlice] buffer.
        async fn async_write_at_volatile(
            &self,
            buf: FileVolatileBuf,
            offset: u64,
        ) -> (Result<usize>, FileVolatileBuf);

        /// Asynchronous version of [FileReadWriteVolatile::write_vectored_at_volatile], to write
        /// data from [FileVolatileSlice] buffers.
        async fn async_write_vectored_at_volatile(
            &self,
            bufs: Vec<FileVolatileBuf>,
            offset: u64,
        ) -> (Result<usize>, Vec<FileVolatileBuf>);
    }

    #[async_trait::async_trait(?Send)]
    impl AsyncFileReadWriteVolatile for File {
        async fn async_read_at_volatile(
            &self,
            buf: FileVolatileBuf,
            offset: u64,
        ) -> (Result<usize>, FileVolatileBuf) {
            self.async_read_at(buf, offset).await
        }

        async fn async_read_vectored_at_volatile(
            &self,
            mut bufs: Vec<FileVolatileBuf>,
            mut offset: u64,
        ) -> (Result<usize>, Vec<FileVolatileBuf>) {
            if bufs.is_empty() {
                return (Ok(0), bufs);
            } else if bufs.len() == 1 {
                let (res, buf) = self.async_read_at_volatile(bufs[0], offset).await;
                bufs[0] = buf;
                return (res, bufs);
            }

            let mut count = 0;
            let mut pos = 0;
            while bufs.len() - pos >= 4 {
                let op1 = self.async_read_at_volatile(bufs[pos], offset);
                offset += bufs[pos].bytes_total() as u64;
                let op2 = self.async_read_at_volatile(bufs[pos + 1], offset);
                offset += bufs[pos + 1].bytes_total() as u64;
                let op3 = self.async_read_at_volatile(bufs[pos + 2], offset);
                offset += bufs[pos + 2].bytes_total() as u64;
                let op4 = self.async_read_at_volatile(bufs[pos + 3], offset);
                offset += bufs[pos + 3].bytes_total() as u64;
                let (res1, res2, res3, res4) = join!(op1, op2, op3, op4);

                match res1 {
                    (Err(e), _) => return (Err(e), bufs),
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos] = buf;
                        if cnt < bufs[pos].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                }
                match res2 {
                    (Err(e), _) => return (Err(e), bufs),
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 1] = buf;
                        if cnt < bufs[pos + 1].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                }
                match res3 {
                    (Err(e), _) => return (Err(e), bufs),
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 2] = buf;
                        if cnt < bufs[pos + 2].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                }
                match res4 {
                    (Err(e), _) => return (Err(e), bufs),
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 3] = buf;
                        if cnt < bufs[pos + 3].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                }
                pos += 4;
            }

            if bufs.len() - pos == 3 {
                let op1 = self.async_read_at_volatile(bufs[pos], offset);
                offset += bufs[pos].bytes_total() as u64;
                let op2 = self.async_read_at_volatile(bufs[pos + 1], offset);
                offset += bufs[pos + 1].bytes_total() as u64;
                let op3 = self.async_read_at_volatile(bufs[pos + 2], offset);
                let (res1, res2, res3) = join!(op1, op2, op3);

                match res1 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos] = buf;
                        if cnt < bufs[pos].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
                match res2 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 1] = buf;
                        if cnt < bufs[pos + 1].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
                match res3 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 2] = buf;
                        if cnt < bufs[pos + 2].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
            } else if bufs.len() - pos == 2 {
                let op1 = self.async_read_at_volatile(bufs[pos], offset);
                offset += bufs[pos].bytes_total() as u64;
                let op2 = self.async_read_at_volatile(bufs[pos + 1], offset);
                let (res1, res2) = join!(op1, op2);

                match res1 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos] = buf;
                        if cnt < bufs[pos].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
                match res2 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 1] = buf;
                        if cnt < bufs[pos + 1].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
            } else if bufs.len() - pos == 1 {
                let res1 = self.async_read_at_volatile(bufs[pos], offset).await;
                match res1 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos] = buf;
                        if cnt < bufs[pos].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
            }

            (Ok(count), bufs)
        }

        async fn async_write_at_volatile(
            &self,
            buf: FileVolatileBuf,
            offset: u64,
        ) -> (Result<usize>, FileVolatileBuf) {
            self.async_write_at(buf, offset).await
        }

        async fn async_write_vectored_at_volatile(
            &self,
            mut bufs: Vec<FileVolatileBuf>,
            mut offset: u64,
        ) -> (Result<usize>, Vec<FileVolatileBuf>) {
            if bufs.is_empty() {
                return (Ok(0), bufs);
            } else if bufs.len() == 1 {
                let (res, buf) = self.async_write_at_volatile(bufs[0], offset).await;
                bufs[0] = buf;
                return (res, bufs);
            }

            let mut count = 0;
            let mut pos = 0;
            while bufs.len() - pos >= 4 {
                let op1 = self.async_write_at_volatile(bufs[pos], offset);
                offset += bufs[pos].bytes_total() as u64;
                let op2 = self.async_write_at_volatile(bufs[pos + 1], offset);
                offset += bufs[pos + 1].bytes_total() as u64;
                let op3 = self.async_write_at_volatile(bufs[pos + 2], offset);
                offset += bufs[pos + 2].bytes_total() as u64;
                let op4 = self.async_write_at_volatile(bufs[pos + 3], offset);
                offset += bufs[pos + 3].bytes_total() as u64;
                let (res1, res2, res3, res4) = join!(op1, op2, op3, op4);

                match res1 {
                    (Err(e), _) => return (Err(e), bufs),
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos] = buf;
                        if cnt < bufs[pos].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                }
                match res2 {
                    (Err(e), _) => return (Err(e), bufs),
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 1] = buf;
                        if cnt < bufs[pos + 1].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                }
                match res3 {
                    (Err(e), _) => return (Err(e), bufs),
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 2] = buf;
                        if cnt < bufs[pos + 2].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                }
                match res4 {
                    (Err(e), _) => return (Err(e), bufs),
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 3] = buf;
                        if cnt < bufs[pos + 3].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                }
                pos += 4;
            }

            if bufs.len() - pos == 3 {
                let op1 = self.async_write_at_volatile(bufs[pos], offset);
                offset += bufs[pos].bytes_total() as u64;
                let op2 = self.async_write_at_volatile(bufs[pos + 1], offset);
                offset += bufs[pos + 1].bytes_total() as u64;
                let op3 = self.async_write_at_volatile(bufs[pos + 2], offset);
                let (res1, res2, res3) = join!(op1, op2, op3);

                match res1 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos] = buf;
                        if cnt < bufs[pos].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
                match res2 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 1] = buf;
                        if cnt < bufs[pos + 1].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
                match res3 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 2] = buf;
                        if cnt < bufs[pos + 2].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
            } else if bufs.len() - pos == 2 {
                let op1 = self.async_write_at_volatile(bufs[pos], offset);
                offset += bufs[pos].bytes_total() as u64;
                let op2 = self.async_write_at_volatile(bufs[pos + 1], offset);
                let (res1, res2) = join!(op1, op2);

                match res1 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos] = buf;
                        if cnt < bufs[pos].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
                match res2 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos + 1] = buf;
                        if cnt < bufs[pos + 1].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
            } else if bufs.len() - pos == 1 {
                let res1 = self.async_write_at_volatile(bufs[pos], offset).await;
                match res1 {
                    (Ok(cnt), buf) => {
                        count += cnt;
                        bufs[pos] = buf;
                        if cnt < bufs[pos].bytes_total() {
                            return (Ok(count), bufs);
                        }
                    }
                    (Err(e), _) => return (Err(e), bufs),
                }
            }

            (Ok(count), bufs)
        }
    }

    #[async_trait::async_trait(?Send)]
    impl<T: AsyncFileReadWriteVolatile + ?Sized> AsyncFileReadWriteVolatile for Arc<T> {
        async fn async_read_at_volatile(
            &self,
            buf: FileVolatileBuf,
            offset: u64,
        ) -> (Result<usize>, FileVolatileBuf) {
            self.async_read_at_volatile(buf, offset).await
        }

        async fn async_read_vectored_at_volatile(
            &self,
            bufs: Vec<FileVolatileBuf>,
            offset: u64,
        ) -> (Result<usize>, Vec<FileVolatileBuf>) {
            self.async_read_vectored_at_volatile(bufs, offset).await
        }

        async fn async_write_at_volatile(
            &self,
            buf: FileVolatileBuf,
            offset: u64,
        ) -> (Result<usize>, FileVolatileBuf) {
            self.async_write_at_volatile(buf, offset).await
        }

        async fn async_write_vectored_at_volatile(
            &self,
            bufs: Vec<FileVolatileBuf>,
            offset: u64,
        ) -> (Result<usize>, Vec<FileVolatileBuf>) {
            self.async_write_vectored_at_volatile(bufs, offset).await
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::async_runtime::block_on;
        use crate::file_buf::FileVolatileSlice;

        #[test]
        fn io_uring_async_read_at_volatile() {
            let tmpfile = vmm_sys_util::tempdir::TempDir::new().unwrap();
            let path = tmpfile.as_path().to_path_buf().join("test.txt");
            std::fs::write(&path, b"this is a test").unwrap();

            let mut buf = vec![0; 4096];
            block_on(async {
                let vslice = unsafe { FileVolatileSlice::from_mut_slice(&mut buf) };
                let vbuf = unsafe { vslice.borrow_as_buf(false) };
                let file = File::async_open(&path, false, false).await.unwrap();
                let (res, vbuf) = file.async_read_at_volatile(vbuf, 4).await;
                assert_eq!(res.unwrap(), 10);
                assert_eq!(vbuf.bytes_init(), 10);
            });
            assert_eq!(buf[0], b' ');
            assert_eq!(buf[9], b't');
        }

        #[test]
        fn io_uring_async_read_vectored_at_volatile() {
            let tmpfile = vmm_sys_util::tempdir::TempDir::new().unwrap();
            let path = tmpfile.as_path().to_path_buf().join("test.txt");
            std::fs::write(&path, b"this is a test").unwrap();

            let mut buf1 = vec![0; 4];
            let mut buf2 = vec![0; 4];

            block_on(async {
                let vslice1 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr(), buf1.len()) };
                let vslice2 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr(), buf2.len()) };
                let vbufs = vec![unsafe { vslice1.borrow_as_buf(false) }, unsafe {
                    vslice2.borrow_as_buf(false)
                }];
                let file = File::async_open(&path, false, false).await.unwrap();
                let (res, vbufs) = file.async_read_vectored_at_volatile(vbufs, 4).await;
                assert_eq!(res.unwrap(), 8);
                assert_eq!(vbufs.len(), 2);
            });
            assert_eq!(buf1[0], b' ');
            assert_eq!(buf2[3], b'e');

            let mut buf1 = vec![0; 1024];
            let mut buf2 = vec![0; 4];

            block_on(async {
                let vslice1 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr(), buf1.len()) };
                let vslice2 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr(), buf2.len()) };
                let vbufs = vec![unsafe { vslice1.borrow_as_buf(false) }, unsafe {
                    vslice2.borrow_as_buf(false)
                }];
                let file = File::async_open(&path, false, false).await.unwrap();
                let (res, vbufs) = file.async_read_vectored_at_volatile(vbufs, 4).await;
                assert_eq!(res.unwrap(), 10);
                assert_eq!(vbufs.len(), 2);
                assert_eq!(vbufs[0].bytes_init(), 10);
                assert_eq!(vbufs[1].bytes_init(), 0);
            });
            assert_eq!(buf1[0], b' ');
            assert_eq!(buf1[9], b't');

            block_on(async {
                let vslice1 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr(), buf1.len()) };
                let vbufs = vec![unsafe { vslice1.borrow_as_buf(false) }];
                let file = File::async_open(&path, false, false).await.unwrap();
                let (res, _vbufs) = file.async_read_vectored_at_volatile(vbufs, 14).await;
                assert_eq!(res.unwrap(), 0);
            });

            block_on(async {
                let vbufs = vec![];
                let file = File::async_open(&path, false, false).await.unwrap();
                let (res, _vbufs) = file.async_read_vectored_at_volatile(vbufs, 0).await;
                assert_eq!(res.unwrap(), 0);
            });

            block_on(async {
                let vslice1 = unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr(), 1) };
                let vslice2 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(1), 1) };
                let vslice3 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(2), 1) };
                let vslice4 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(3), 1) };
                let vslice5 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(4), 1) };
                let vslice6 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(5), 1) };
                let vslice7 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(6), 1) };
                let vbufs = vec![
                    unsafe { vslice1.borrow_as_buf(false) },
                    unsafe { vslice2.borrow_as_buf(false) },
                    unsafe { vslice3.borrow_as_buf(false) },
                    unsafe { vslice4.borrow_as_buf(false) },
                    unsafe { vslice5.borrow_as_buf(false) },
                    unsafe { vslice6.borrow_as_buf(false) },
                    unsafe { vslice7.borrow_as_buf(false) },
                ];
                let file = File::async_open(&path, false, false).await.unwrap();
                let (res, vbufs) = file.async_read_vectored_at_volatile(vbufs, 4).await;
                assert_eq!(res.unwrap(), 7);
                assert_eq!(vbufs.len(), 7);
                assert_eq!(buf1[0], b' ');
                assert_eq!(buf1[1], b'i');
                assert_eq!(buf1[2], b's');
                assert_eq!(buf1[3], b' ');
                assert_eq!(buf1[4], b'a');
                assert_eq!(buf1[5], b' ');
                assert_eq!(buf1[6], b't');
            });

            block_on(async {
                let vslice1 = unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr(), 1) };
                let vslice2 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(1), 1) };
                let vslice3 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(2), 1) };
                let vslice4 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(3), 1) };
                let vslice5 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(4), 1) };
                let vslice6 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(5), 1) };
                let vbufs = vec![
                    unsafe { vslice1.borrow_as_buf(false) },
                    unsafe { vslice2.borrow_as_buf(false) },
                    unsafe { vslice3.borrow_as_buf(false) },
                    unsafe { vslice4.borrow_as_buf(false) },
                    unsafe { vslice5.borrow_as_buf(false) },
                    unsafe { vslice6.borrow_as_buf(false) },
                ];
                let file = File::async_open(&path, false, false).await.unwrap();
                let (res, vbufs) = file.async_read_vectored_at_volatile(vbufs, 4).await;
                assert_eq!(res.unwrap(), 6);
                assert_eq!(vbufs.len(), 6);
            });

            block_on(async {
                let vslice1 = unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr(), 1) };
                let vslice2 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(1), 1) };
                let vslice3 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(2), 1) };
                let vslice4 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(3), 1) };
                let vslice5 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(4), 1) };
                let vbufs = vec![
                    unsafe { vslice1.borrow_as_buf(false) },
                    unsafe { vslice2.borrow_as_buf(false) },
                    unsafe { vslice3.borrow_as_buf(false) },
                    unsafe { vslice4.borrow_as_buf(false) },
                    unsafe { vslice5.borrow_as_buf(false) },
                ];
                let file = File::async_open(&path, false, false).await.unwrap();
                let (res, vbufs) = file.async_read_vectored_at_volatile(vbufs, 4).await;
                assert_eq!(res.unwrap(), 5);
                assert_eq!(vbufs.len(), 5);
            });

            block_on(async {
                let vslice1 = unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr(), 1) };
                let vslice2 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(1), 1) };
                let vslice3 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(2), 1) };
                let vslice4 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(3), 1) };
                let vbufs = vec![
                    unsafe { vslice1.borrow_as_buf(false) },
                    unsafe { vslice2.borrow_as_buf(false) },
                    unsafe { vslice3.borrow_as_buf(false) },
                    unsafe { vslice4.borrow_as_buf(false) },
                ];
                let file = File::async_open(&path, false, false).await.unwrap();
                let (res, vbufs) = file.async_read_vectored_at_volatile(vbufs, 4).await;
                assert_eq!(res.unwrap(), 4);
                assert_eq!(vbufs.len(), 4);
            });

            block_on(async {
                let vslice1 = unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr(), 1) };
                let vslice2 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(1), 1) };
                let vslice3 =
                    unsafe { FileVolatileSlice::from_raw_ptr(buf1.as_mut_ptr().add(2), 1) };
                let vbufs = vec![
                    unsafe { vslice1.borrow_as_buf(false) },
                    unsafe { vslice2.borrow_as_buf(false) },
                    unsafe { vslice3.borrow_as_buf(false) },
                ];
                let file = File::async_open(&path, false, false).await.unwrap();
                let (res, vbufs) = file.async_read_vectored_at_volatile(vbufs, 4).await;
                assert_eq!(res.unwrap(), 3);
                assert_eq!(vbufs.len(), 3);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom, Write};
    use vmm_sys_util::tempfile::TempFile;

    #[test]
    fn test_read_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let buf = [0xfu8; 32];
        file.write_all(&buf).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slice =
            unsafe { FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, buf2.len()) };
        assert_eq!(file.read_volatile(slice).unwrap(), 32);
        assert_eq!(buf, buf2);

        assert_eq!(file.read_volatile(slice).unwrap(), 0);
    }

    #[test]
    fn test_read_vectored_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let buf = [0xfu8; 32];
        file.write_all(&buf).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slices = unsafe {
            [
                FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, 16),
                FileVolatileSlice::from_raw_ptr((buf2.as_mut_ptr() as *mut u8).add(16), 16),
            ]
        };
        assert_eq!(file.read_vectored_volatile(&slices).unwrap(), 32);
        assert_eq!(buf, buf2);

        assert_eq!(file.read_vectored_volatile(&slices).unwrap(), 0);
    }

    #[test]
    fn test_read_exact_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let buf = [0xfu8; 32];
        file.write_all(&buf).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut buf2 = [0x0u8; 31];
        let slice =
            unsafe { FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, buf2.len()) };
        file.read_exact_volatile(slice).unwrap();
        assert_eq!(buf[..31], buf2);

        file.read_exact_volatile(slice).unwrap_err();
    }

    #[test]
    fn test_read_at_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let buf = [0xfu8; 32];
        file.write_all(&buf).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slice =
            unsafe { FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, buf2.len()) };
        assert_eq!(file.read_at_volatile(slice, 0).unwrap(), 32);
        assert_eq!(buf, buf2);

        assert_eq!(file.read_at_volatile(slice, 30).unwrap(), 2);
        assert_eq!(file.read_at_volatile(slice, 32).unwrap(), 0);
    }

    #[test]
    fn test_read_vectored_at_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let buf = [0xfu8; 32];
        file.write_all(&buf).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slices = unsafe {
            [
                FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, 16),
                FileVolatileSlice::from_raw_ptr((buf2.as_mut_ptr() as *mut u8).add(16), 16),
            ]
        };
        assert_eq!(file.read_vectored_at_volatile(&slices, 0).unwrap(), 32);
        assert_eq!(buf, buf2);

        assert_eq!(file.read_vectored_at_volatile(&slices, 30).unwrap(), 2);
        assert_eq!(file.read_vectored_at_volatile(&slices, 32).unwrap(), 0);
    }

    #[test]
    fn test_read_exact_at_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let buf = [0xfu8; 32];
        file.write_all(&buf).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slice =
            unsafe { FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, buf2.len()) };
        file.read_exact_at_volatile(slice, 0).unwrap();
        assert_eq!(buf, buf2);

        file.read_exact_at_volatile(slice, 30).unwrap_err();
        file.read_exact_at_volatile(slice, 32).unwrap_err();
    }

    #[test]
    fn test_write_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let mut buf = [0xfu8; 32];
        let slice1 =
            unsafe { FileVolatileSlice::from_raw_ptr(buf.as_mut_ptr() as *mut u8, buf.len()) };
        file.write_volatile(slice1).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slice =
            unsafe { FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, buf2.len()) };
        assert_eq!(file.read_volatile(slice).unwrap(), 32);
        assert_eq!(buf, buf2);

        assert_eq!(file.read_volatile(slice).unwrap(), 0);
    }

    #[test]
    fn test_write_vectored_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let mut buf = [0xfu8; 32];
        let slices1 = unsafe {
            [
                FileVolatileSlice::from_raw_ptr(buf.as_mut_ptr() as *mut u8, 16),
                FileVolatileSlice::from_raw_ptr((buf.as_mut_ptr() as *mut u8).add(16), 16),
            ]
        };
        file.write_vectored_volatile(&slices1).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slices = unsafe {
            [
                FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, 16),
                FileVolatileSlice::from_raw_ptr((buf2.as_mut_ptr() as *mut u8).add(16), 16),
            ]
        };
        assert_eq!(file.read_vectored_volatile(&slices).unwrap(), 32);
        assert_eq!(buf, buf2);

        assert_eq!(file.read_vectored_volatile(&slices).unwrap(), 0);
    }

    #[test]
    fn test_write_exact_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let mut buf = [0xfu8; 32];
        let slice1 =
            unsafe { FileVolatileSlice::from_raw_ptr(buf.as_mut_ptr() as *mut u8, buf.len()) };
        file.write_all_volatile(slice1).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slice =
            unsafe { FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, buf2.len()) };
        file.read_exact_volatile(slice).unwrap();
        assert_eq!(buf, buf2);

        file.read_exact_volatile(slice).unwrap_err();
    }

    #[test]
    fn test_write_at_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let mut buf = [0xfu8; 32];
        let slice1 =
            unsafe { FileVolatileSlice::from_raw_ptr(buf.as_mut_ptr() as *mut u8, buf.len()) };
        file.write_volatile(slice1).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slice =
            unsafe { FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, buf2.len()) };
        assert_eq!(file.read_at_volatile(slice, 0).unwrap(), 32);
        assert_eq!(buf, buf2);

        assert_eq!(file.read_at_volatile(slice, 30).unwrap(), 2);
        assert_eq!(file.read_at_volatile(slice, 32).unwrap(), 0);
    }

    #[test]
    fn test_write_vectored_at_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let mut buf = [0xfu8; 32];
        let slices1 = unsafe {
            [
                FileVolatileSlice::from_raw_ptr(buf.as_mut_ptr() as *mut u8, 16),
                FileVolatileSlice::from_raw_ptr((buf.as_mut_ptr() as *mut u8).add(16), 16),
            ]
        };
        file.write_vectored_volatile(&slices1).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slices = unsafe {
            [
                FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, 16),
                FileVolatileSlice::from_raw_ptr((buf2.as_mut_ptr() as *mut u8).add(16), 16),
            ]
        };
        assert_eq!(file.read_vectored_at_volatile(&slices, 0).unwrap(), 32);
        assert_eq!(buf, buf2);

        assert_eq!(file.read_vectored_at_volatile(&slices, 30).unwrap(), 2);
        assert_eq!(file.read_vectored_at_volatile(&slices, 32).unwrap(), 0);
    }

    #[test]
    fn test_write_exact_at_volatile() {
        let mut file = TempFile::new().unwrap().into_file();

        let mut buf = [0xfu8; 32];
        let slice1 =
            unsafe { FileVolatileSlice::from_raw_ptr(buf.as_mut_ptr() as *mut u8, buf.len()) };
        file.write_all_volatile(slice1).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();

        let mut buf2 = [0x0u8; 32];
        let slice =
            unsafe { FileVolatileSlice::from_raw_ptr(buf2.as_mut_ptr() as *mut u8, buf2.len()) };
        file.read_exact_at_volatile(slice, 0).unwrap();
        assert_eq!(buf, buf2);

        file.read_exact_at_volatile(slice, 30).unwrap_err();
        file.read_exact_at_volatile(slice, 32).unwrap_err();
    }
}
