// Copyright (C) 2020 Alibaba Cloud. All rights reserved.
//
// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause

//! FUSE/Virtiofs transport drivers to receive requests from/send reply to Fuse/Virtiofs clients.
//!
//! Originally a FUSE server communicates with the FUSE driver through the device `/dev/fuse`,
//! and the communication protocol is called as FUSE protocol. Later the FUSE protocol is extended
//! to support Virtio-fs device. So there are two transport layers supported:
//! - fusedev: communicate with the FUSE driver through `/dev/fuse`
//! - virtiofs: communicate with the virtiofsd on host side by using virtio descriptors.

use std::collections::VecDeque;
use std::io::{self, IoSlice, Read};
use std::marker::PhantomData;
use std::mem::{size_of, MaybeUninit};
use std::ptr::copy_nonoverlapping;
use std::{cmp, fmt};

use lazy_static::lazy_static;
use libc::{sysconf, _SC_PAGESIZE};
use vm_memory::{ByteValued, VolatileSlice};

#[cfg(feature = "async-io")]
use crate::file_buf::FileVolatileBuf;
use crate::file_buf::FileVolatileSlice;
#[cfg(feature = "async-io")]
use crate::file_traits::AsyncFileReadWriteVolatile;
use crate::file_traits::FileReadWriteVolatile;
use crate::BitmapSlice;

mod fs_cache_req_handler;
#[cfg(feature = "fusedev")]
mod fusedev;
#[cfg(feature = "virtiofs")]
mod virtiofs;

pub use self::fs_cache_req_handler::FsCacheReqHandler;
#[cfg(feature = "fusedev")]
pub use self::fusedev::{FuseBuf, FuseChannel, FuseDevWriter, FuseSession};
#[cfg(feature = "virtiofs")]
pub use self::virtiofs::VirtioFsWriter;

/// Transport layer specific error codes.
#[derive(Debug)]
pub enum Error {
    /// Virtio queue descriptor chain overflows.
    DescriptorChainOverflow,
    /// Failed to find memory region for guest physical address.
    FindMemoryRegion,
    /// Invalid virtio queue descriptor chain.
    InvalidChain,
    /// Invalid paramater.
    InvalidParameter,
    /// Generic IO error.
    IoError(io::Error),
    /// Out of bounds when splitting VolatileSplice.
    SplitOutOfBounds(usize),
    /// Failed to access volatile memory.
    VolatileMemoryError(vm_memory::VolatileMemoryError),
    #[cfg(feature = "fusedev")]
    /// Session errors
    SessionFailure(String),
    #[cfg(feature = "virtiofs")]
    /// Failed to access guest memory.
    GuestMemoryError(vm_memory::GuestMemoryError),
    #[cfg(feature = "virtiofs")]
    /// Invalid Indirect Virtio descriptors.
    ConvertIndirectDescriptor(virtio_queue::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match self {
            DescriptorChainOverflow => write!(
                f,
                "the combined length of all the buffers in a `DescriptorChain` would overflow"
            ),
            FindMemoryRegion => write!(f, "no memory region for this address range"),
            InvalidChain => write!(f, "invalid descriptor chain"),
            InvalidParameter => write!(f, "invalid parameter"),
            IoError(e) => write!(f, "descriptor I/O error: {}", e),
            SplitOutOfBounds(off) => write!(f, "`DescriptorChain` split is out of bounds: {}", off),
            VolatileMemoryError(e) => write!(f, "volatile memory error: {}", e),

            #[cfg(feature = "fusedev")]
            SessionFailure(e) => write!(f, "fuse session failure: {}", e),

            #[cfg(feature = "virtiofs")]
            ConvertIndirectDescriptor(e) => write!(f, "invalid indirect descriptor: {}", e),
            #[cfg(feature = "virtiofs")]
            GuestMemoryError(e) => write!(f, "descriptor guest memory error: {}", e),
        }
    }
}

/// Specialized version of [std::result::Result] for transport layer operations.
pub type Result<T> = std::result::Result<T, Error>;

impl std::error::Error for Error {}

#[derive(Clone)]
struct IoBuffers<'a, S> {
    buffers: VecDeque<VolatileSlice<'a, S>>,
    bytes_consumed: usize,
}

impl<S: BitmapSlice> Default for IoBuffers<'_, S> {
    fn default() -> Self {
        IoBuffers {
            buffers: VecDeque::new(),
            bytes_consumed: 0,
        }
    }
}

impl<S: BitmapSlice> IoBuffers<'_, S> {
    fn available_bytes(&self) -> usize {
        // This is guaranteed not to overflow because the total length of the chain
        // is checked during all creations of `IoBuffers` (see
        // `Reader::new()` and `Writer::new()`).
        self.buffers
            .iter()
            .fold(0usize, |count, buf| count + buf.len() as usize)
    }

    fn bytes_consumed(&self) -> usize {
        self.bytes_consumed
    }

    fn allocate_file_volatile_slice(&self, count: usize) -> Vec<FileVolatileSlice> {
        let mut rem = count;
        let mut bufs: Vec<FileVolatileSlice> = Vec::with_capacity(self.buffers.len());

        for buf in &self.buffers {
            if rem == 0 {
                break;
            }

            // If buffer contains more data than `rem`, truncate buffer to `rem`, otherwise
            // more data is written out and causes data corruption.
            let local_buf = if buf.len() > rem {
                // Safe because we just check rem < buf.len()
                FileVolatileSlice::from_volatile_slice(&buf.subslice(0, rem).unwrap())
            } else {
                FileVolatileSlice::from_volatile_slice(buf)
            };
            bufs.push(local_buf);

            // Don't need check_sub() as we just made sure rem >= local_buf.len()
            rem -= local_buf.len() as usize;
        }

        bufs
    }

    #[cfg(feature = "async-io")]
    unsafe fn prepare_io_buf(&self, count: usize) -> Vec<FileVolatileBuf> {
        let mut rem = count;
        let mut bufs = Vec::with_capacity(self.buffers.len());

        for buf in &self.buffers {
            if rem == 0 {
                break;
            }

            // If buffer contains more data than `rem`, truncate buffer to `rem`, otherwise
            // more data is written out and causes data corruption.
            let local_buf = if buf.len() > rem {
                // Safe because we just check rem < buf.len()
                buf.subslice(0, rem).unwrap()
            } else {
                buf.clone()
            };
            // Safe because we just change the interface to access underlying buffers.
            bufs.push(FileVolatileBuf::from_raw_ptr(
                local_buf.as_ptr(),
                local_buf.len(),
                local_buf.len(),
            ));

            // Don't need check_sub() as we just made sure rem >= local_buf.len()
            rem -= local_buf.len() as usize;
        }

        bufs
    }

    #[cfg(all(feature = "async-io", feature = "virtiofs"))]
    unsafe fn prepare_mut_io_buf(&self, count: usize) -> Vec<FileVolatileBuf> {
        let mut rem = count;
        let mut bufs = Vec::with_capacity(self.buffers.len());

        for buf in &self.buffers {
            if rem == 0 {
                break;
            }

            // If buffer contains more data than `rem`, truncate buffer to `rem`, otherwise
            // more data is written out and causes data corruption.
            let local_buf = if buf.len() > rem {
                // Safe because we just check rem < buf.len()
                buf.subslice(0, rem).unwrap()
            } else {
                buf.clone()
            };
            bufs.push(FileVolatileBuf::from_raw_ptr(
                local_buf.as_ptr(),
                0,
                local_buf.len(),
            ));

            // Don't need check_sub() as we just made sure rem >= local_buf.len()
            rem -= local_buf.len() as usize;
        }

        bufs
    }

    fn mark_dirty(&self, count: usize) {
        let mut rem = count;

        for buf in &self.buffers {
            if rem == 0 {
                break;
            }

            // If buffer contains more data than `rem`, truncate buffer to `rem`, otherwise
            // more data is written out and causes data corruption.
            let local_buf = if buf.len() > rem {
                // Safe because we just check rem < buf.len()
                buf.subslice(0, rem).unwrap()
            } else {
                buf.clone()
            };
            local_buf.bitmap().mark_dirty(0, local_buf.len());

            // Don't need check_sub() as we just made sure rem >= local_buf.len()
            rem -= local_buf.len() as usize;
        }
    }

    fn mark_used(&mut self, bytes_consumed: usize) -> io::Result<()> {
        // This can happen if a driver tricks a device into reading/writing more data than
        // fits in a `usize`.
        let total_bytes_consumed =
            self.bytes_consumed
                .checked_add(bytes_consumed)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, Error::DescriptorChainOverflow)
                })?;

        let mut rem = bytes_consumed;
        while let Some(buf) = self.buffers.pop_front() {
            if rem < buf.len() {
                // Split the slice and push the remainder back into the buffer list. Safe because we
                // know that `rem` is not out of bounds due to the check and we checked the bounds
                // on `buf` when we added it to the buffer list.
                self.buffers.push_front(buf.offset(rem).unwrap());
                break;
            }

            // No need for checked math because we know that `buf.size() <= rem`.
            rem -= buf.len();
        }

        self.bytes_consumed = total_bytes_consumed;

        Ok(())
    }

    /// Consumes at most `count` bytes from the `DescriptorChain`. Callers must provide a function
    /// that takes a `&[FileVolatileSlice]` and returns the total number of bytes consumed. This
    /// function guarantees that the combined length of all the slices in the `&[FileVolatileSlice]` is
    /// less than or equal to `count`. `mark_dirty` is used for tracing dirty pages.
    ///
    /// # Errors
    ///
    /// If the provided function returns any error then no bytes are consumed from the buffer and
    /// the error is returned to the caller.
    fn consume<F>(&mut self, mark_dirty: bool, count: usize, f: F) -> io::Result<usize>
    where
        F: FnOnce(&[FileVolatileSlice]) -> io::Result<usize>,
    {
        let bufs = self.allocate_file_volatile_slice(count);
        if bufs.is_empty() {
            Ok(0)
        } else {
            let bytes_consumed = f(&*bufs)?;
            if mark_dirty {
                self.mark_dirty(bytes_consumed);
            }
            self.mark_used(bytes_consumed)?;
            Ok(bytes_consumed)
        }
    }

    fn consume_for_read<F>(&mut self, count: usize, f: F) -> io::Result<usize>
    where
        F: FnOnce(&[FileVolatileSlice]) -> io::Result<usize>,
    {
        self.consume(false, count, f)
    }

    fn split_at(&mut self, offset: usize) -> Result<Self> {
        let mut rem = offset;
        let pos = self.buffers.iter().position(|buf| {
            if rem < buf.len() {
                true
            } else {
                rem -= buf.len();
                false
            }
        });

        if let Some(at) = pos {
            let mut other = self.buffers.split_off(at);

            if rem > 0 {
                // There must be at least one element in `other` because we checked
                // its `size` value in the call to `position` above.
                let front = other.pop_front().expect("empty VecDeque after split");
                self.buffers
                    .push_back(front.subslice(0, rem).map_err(Error::VolatileMemoryError)?);
                other.push_front(front.offset(rem).map_err(Error::VolatileMemoryError)?);
            }

            Ok(IoBuffers {
                buffers: other,
                bytes_consumed: 0,
            })
        } else if rem == 0 {
            Ok(IoBuffers {
                buffers: VecDeque::new(),
                bytes_consumed: 0,
            })
        } else {
            Err(Error::SplitOutOfBounds(offset))
        }
    }
}

/// Reader to access FUSE requests from the transport layer data buffers.
///
/// Note that virtio spec requires driver to place any device-writable
/// descriptors after any device-readable descriptors (2.6.4.2 in Virtio Spec v1.1).
/// Reader will skip iterating over descriptor chain when first writable
/// descriptor is encountered.
#[derive(Clone)]
pub struct Reader<'a, S = ()> {
    buffers: IoBuffers<'a, S>,
}

impl<S: BitmapSlice> Default for Reader<'_, S> {
    fn default() -> Self {
        Reader {
            buffers: IoBuffers::default(),
        }
    }
}

impl<S: BitmapSlice> Reader<'_, S> {
    /// Reads an object from the descriptor chain buffer.
    pub fn read_obj<T: ByteValued>(&mut self) -> io::Result<T> {
        let mut obj = MaybeUninit::<T>::uninit();

        // Safe because `MaybeUninit` guarantees that the pointer is valid for
        // `size_of::<T>()` bytes.
        let buf = unsafe {
            ::std::slice::from_raw_parts_mut(obj.as_mut_ptr() as *mut u8, size_of::<T>())
        };

        self.read_exact(buf)?;

        // Safe because any type that implements `ByteValued` can be considered initialized
        // even if it is filled with random data.
        Ok(unsafe { obj.assume_init() })
    }

    /// Reads data from the descriptor chain buffer into a file descriptor.
    /// Returns the number of bytes read from the descriptor chain buffer.
    /// The number of bytes read can be less than `count` if there isn't
    /// enough data in the descriptor chain buffer.
    pub fn read_to<F: FileReadWriteVolatile>(
        &mut self,
        mut dst: F,
        count: usize,
    ) -> io::Result<usize> {
        self.buffers
            .consume_for_read(count, |bufs| dst.write_vectored_volatile(bufs))
    }

    /// Reads data from the descriptor chain buffer into a File at offset `off`.
    /// Returns the number of bytes read from the descriptor chain buffer.
    /// The number of bytes read can be less than `count` if there isn't
    /// enough data in the descriptor chain buffer.
    pub fn read_to_at<F: FileReadWriteVolatile>(
        &mut self,
        mut dst: F,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        self.buffers
            .consume_for_read(count, |bufs| dst.write_vectored_at_volatile(bufs, off))
    }

    /// Reads exactly size of data from the descriptor chain buffer into a file descriptor.
    pub fn read_exact_to<F: FileReadWriteVolatile>(
        &mut self,
        mut dst: F,
        mut count: usize,
    ) -> io::Result<()> {
        while count > 0 {
            match self.read_to(&mut dst, count) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "failed to fill whole buffer",
                    ))
                }
                Ok(n) => count -= n,
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    /// Returns number of bytes available for reading.
    ///
    /// May return an error if the combined lengths of all the buffers in the DescriptorChain
    /// would cause an integer overflow.
    pub fn available_bytes(&self) -> usize {
        self.buffers.available_bytes()
    }

    /// Returns number of bytes already read from the descriptor chain buffer.
    pub fn bytes_read(&self) -> usize {
        self.buffers.bytes_consumed()
    }

    /// Splits this `Reader` into two at the given offset in the `DescriptorChain` buffer.
    /// After the split, `self` will be able to read up to `offset` bytes while the returned
    /// `Reader` can read up to `available_bytes() - offset` bytes.  Returns an error if
    /// `offset > self.available_bytes()`.
    pub fn split_at(&mut self, offset: usize) -> Result<Self> {
        self.buffers
            .split_at(offset)
            .map(|buffers| Reader { buffers })
    }
}

impl<S: BitmapSlice> io::Read for Reader<'_, S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.buffers.consume_for_read(buf.len(), |bufs| {
            let mut rem = buf;
            let mut total = 0;
            for buf in bufs {
                let copy_len = cmp::min(rem.len(), buf.len());

                // Safe because we have already verified that `buf` points to valid memory.
                unsafe {
                    copy_nonoverlapping(buf.as_ptr() as *const u8, rem.as_mut_ptr(), copy_len);
                }
                rem = &mut rem[copy_len..];
                total += copy_len;
            }
            Ok(total)
        })
    }
}

#[cfg(feature = "async-io")]
mod async_io {
    use super::*;

    impl<'a, S: BitmapSlice> Reader<'a, S> {
        /// Read data from the data buffer into a File at offset `off` in asynchronous mode.
        ///
        /// Return the number of bytes read from the data buffer. The number of bytes read can
        /// be less than `count` if there isn't enough data in the buffer.
        pub async fn async_read_to_at<F: AsyncFileReadWriteVolatile>(
            &mut self,
            dst: &F,
            count: usize,
            off: u64,
        ) -> io::Result<usize> {
            // Safe because `bufs` doesn't out-live `self`.
            let bufs = unsafe { self.buffers.prepare_io_buf(count) };
            if bufs.is_empty() {
                Ok(0)
            } else {
                let (res, _) = dst.async_write_vectored_at_volatile(bufs, off).await;
                match res {
                    Ok(cnt) => {
                        self.buffers.mark_used(cnt)?;
                        Ok(cnt)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }
}

/// Writer to send reply message to '/dev/fuse` or virtiofs queue.
pub enum Writer<'a, S: BitmapSlice = ()> {
    #[cfg(feature = "fusedev")]
    /// Writer for FuseDev transport driver.
    FuseDev(FuseDevWriter<'a, S>),
    #[cfg(feature = "virtiofs")]
    /// Writer for virtiofs transport driver.
    VirtioFs(VirtioFsWriter<'a, S>),
    /// Writer for Noop transport driver.
    Noop(PhantomData<&'a S>),
}

impl<'a, S: BitmapSlice> Writer<'a, S> {
    /// Write data to the descriptor chain buffer from a File at offset `off`.
    ///
    /// Return the number of bytes written to the descriptor chain buffer.
    pub fn write_from_at<F: FileReadWriteVolatile>(
        &mut self,
        src: F,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.write_from_at(src, count, off),
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.write_from_at(src, count, off),
            _ => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    /// Split this `Writer` into two at the given offset in the `DescriptorChain` buffer.
    ///
    /// After the split, `self` will be able to write up to `offset` bytes while the returned
    /// `Writer` can write up to `available_bytes() - offset` bytes.  Return an error if
    /// `offset > self.available_bytes()`.
    pub fn split_at(&mut self, offset: usize) -> Result<Self> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.split_at(offset).map(|w| w.into()),
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.split_at(offset).map(|w| w.into()),
            _ => Err(Error::InvalidParameter),
        }
    }

    /// Return number of bytes available for writing.
    ///
    /// May return an error if the combined lengths of all the buffers in the DescriptorChain would
    /// cause an overflow.
    pub fn available_bytes(&self) -> usize {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.available_bytes(),
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.available_bytes(),
            _ => 0,
        }
    }

    /// Return number of bytes already written to the descriptor chain buffer.
    pub fn bytes_written(&self) -> usize {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.bytes_written(),
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.bytes_written(),
            _ => 0,
        }
    }

    /// Commit all internal buffers of self and others
    pub fn commit(&mut self, other: Option<&Self>) -> io::Result<usize> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.commit(other),
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.commit(other),
            _ => Ok(0),
        }
    }
}

impl<'a, S: BitmapSlice> io::Write for Writer<'a, S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.write(buf),
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.write(buf),
            _ => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.write_vectored(bufs),
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.write_vectored(bufs),
            _ => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.flush(),
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.flush(),
            _ => Ok(()),
        }
    }
}

#[cfg(feature = "async-io")]
impl<'a, S: BitmapSlice> Writer<'a, S> {
    /// Write data from a buffer into this writer in asynchronous mode.
    pub async fn async_write(&mut self, data: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.async_write(data).await,
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.async_write(data).await,
            _ => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    /// Write data from two buffers into this writer in asynchronous mode.
    pub async fn async_write2(&mut self, data: &[u8], data2: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.async_write2(data, data2).await,
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.async_write2(data, data2).await,
            _ => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    /// Write data from three buffers into this writer in asynchronous mode.
    pub async fn async_write3(
        &mut self,
        data: &[u8],
        data2: &[u8],
        data3: &[u8],
    ) -> io::Result<usize> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.async_write3(data, data2, data3).await,
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.async_write3(data, data2, data3).await,
            _ => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    /// Attempt to write an entire buffer into this writer in asynchronous mode.
    pub async fn async_write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.async_write_all(buf).await,
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.async_write_all(buf).await,
            _ => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    /// Asynchronously write data to the descriptor chain buffer from a File at offset `off`.
    ///
    /// Return the number of bytes written to the descriptor chain buffer.
    pub async fn async_write_from_at<F: AsyncFileReadWriteVolatile>(
        &mut self,
        src: &F,
        count: usize,
        off: u64,
    ) -> io::Result<usize> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.async_write_from_at(src, count, off).await,
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.async_write_from_at(src, count, off).await,
            _ => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    /// Commit all internal buffers of self and others
    pub async fn async_commit(&mut self, other: Option<&Writer<'a, S>>) -> io::Result<usize> {
        match self {
            #[cfg(feature = "fusedev")]
            Writer::FuseDev(w) => w.async_commit(other).await,
            #[cfg(feature = "virtiofs")]
            Writer::VirtioFs(w) => w.async_commit(other).await,
            _ => Err(std::io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }
}

#[cfg(feature = "fusedev")]
impl<'a, S: BitmapSlice> From<FuseDevWriter<'a, S>> for Writer<'a, S> {
    fn from(w: FuseDevWriter<'a, S>) -> Self {
        Writer::FuseDev(w)
    }
}

#[cfg(feature = "virtiofs")]
impl<'a, S: BitmapSlice> From<VirtioFsWriter<'a, S>> for Writer<'a, S> {
    fn from(w: VirtioFsWriter<'a, S>) -> Self {
        Writer::VirtioFs(w)
    }
}

lazy_static! {
    static ref PAGESIZE: usize = unsafe { sysconf(_SC_PAGESIZE) as usize };
}

/// Safe wrapper for `sysconf(_SC_PAGESIZE)`.
#[inline(always)]
pub fn pagesize() -> usize {
    *PAGESIZE
}

#[cfg(test)]
mod tests {
    use crate::transport::IoBuffers;
    use std::collections::VecDeque;
    use vm_memory::{
        bitmap::{AtomicBitmap, Bitmap},
        VolatileSlice,
    };

    #[test]
    fn test_io_buffers() {
        let mut buf1 = vec![0x0u8; 16];
        let mut buf2 = vec![0x0u8; 16];
        let mut bufs = VecDeque::new();
        unsafe {
            bufs.push_back(VolatileSlice::new(buf1.as_mut_ptr(), buf1.len()));
            bufs.push_back(VolatileSlice::new(buf2.as_mut_ptr(), buf2.len()));
        }
        let mut buffers = IoBuffers {
            buffers: bufs,
            bytes_consumed: 0,
        };

        assert_eq!(buffers.available_bytes(), 32);
        assert_eq!(buffers.bytes_consumed(), 0);

        assert_eq!(
            buffers.consume_for_read(2, |buf| Ok(buf[0].len())).unwrap(),
            2
        );
        assert_eq!(buffers.available_bytes(), 30);
        assert_eq!(buffers.bytes_consumed(), 2);

        let mut buffers2 = buffers.split_at(10).unwrap();
        assert_eq!(buffers.available_bytes(), 10);
        assert_eq!(buffers.bytes_consumed(), 2);
        assert_eq!(buffers2.available_bytes(), 20);
        assert_eq!(buffers2.bytes_consumed(), 0);

        assert_eq!(
            buffers2
                .consume_for_read(10, |buf| Ok(buf[0].len() + buf[1].len()))
                .unwrap(),
            10
        );
        assert_eq!(
            buffers2
                .consume_for_read(20, |buf| Ok(buf[0].len()))
                .unwrap(),
            10
        );

        let _buffers3 = buffers2.split_at(0).unwrap();
        assert!(buffers2.split_at(1).is_err());
    }

    #[test]
    fn test_mark_dirty() {
        let mut buf1 = vec![0x0u8; 16];
        let bitmap1 = AtomicBitmap::new(16, 2);

        assert_eq!(bitmap1.len(), 8);
        for i in 0..8 {
            assert_eq!(bitmap1.is_bit_set(i), false);
        }

        let mut buf2 = vec![0x0u8; 16];
        let bitmap2 = AtomicBitmap::new(16, 2);
        let mut bufs = VecDeque::new();

        unsafe {
            bufs.push_back(VolatileSlice::with_bitmap(
                buf1.as_mut_ptr(),
                buf1.len(),
                bitmap1.slice_at(0),
            ));
            bufs.push_back(VolatileSlice::with_bitmap(
                buf2.as_mut_ptr(),
                buf2.len(),
                bitmap2.slice_at(0),
            ));
        }
        let mut buffers = IoBuffers {
            buffers: bufs,
            bytes_consumed: 0,
        };

        assert_eq!(buffers.available_bytes(), 32);
        assert_eq!(buffers.bytes_consumed(), 0);

        assert_eq!(
            buffers.consume_for_read(8, |buf| Ok(buf[0].len())).unwrap(),
            8
        );

        assert_eq!(buffers.available_bytes(), 24);
        assert_eq!(buffers.bytes_consumed(), 8);

        for i in 0..8 {
            assert_eq!(bitmap1.is_bit_set(i), false);
        }

        assert_eq!(
            buffers
                .consume(true, 16, |buf| Ok(buf[0].len() + buf[1].len()))
                .unwrap(),
            16
        );
        assert_eq!(buffers.available_bytes(), 8);
        assert_eq!(buffers.bytes_consumed(), 24);
        for i in 0..8 {
            if i >= 4 {
                assert_eq!(bitmap1.is_bit_set(i), true);
                continue;
            } else {
                assert_eq!(bitmap1.is_bit_set(i), false);
            }
        }
        for i in 0..8 {
            if i < 4 {
                assert_eq!(bitmap2.is_bit_set(i), true);
            } else {
                assert_eq!(bitmap2.is_bit_set(i), false);
            }
        }
    }
}
