// Copyright (C) 2021-2022 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Provide data buffers to support [tokio] and [tokio-uring] based async io.
//!
//! The vm-memory v0.6.0 introduced support of dirty page tracking by using `Bitmap`, which adds a
//! generic type parameters to several APIs. That's a breaking change and  makes the rust compiler
//! fail to compile our code. So introduce [FileVolatileSlice] to mask out the `BitmapSlice`
//! generic type parameter. Dirty page tracking is handled at higher level in `IoBuffers`.
//!
//! The [tokio-uring] crates uses [io-uring] for actual IO operations. And the [io-uring] APIs
//! require passing ownership of buffers to the runtime. So [FileVolatileBuf] is introduced to
//! support [tokio-uring] based async io.
//!
//! [io-uring]: https://github.com/tokio-rs/io-uring
//! [tokio]: https://tokio.rs/
//! [tokio-uring]: https://github.com/tokio-rs/tokio-uring

use std::io::{IoSlice, IoSliceMut, Read, Write};
use std::marker::PhantomData;
use std::sync::atomic::Ordering;
use std::{error, fmt, slice};

use vm_memory::{
    bitmap::BitmapSlice, volatile_memory::Error as VError, AtomicAccess, Bytes, VolatileSlice,
};

/// Error codes related to buffer management.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum Error {
    /// `addr` is out of bounds of the volatile memory slice.
    OutOfBounds { addr: usize },
    /// Taking a slice at `base` with `offset` would overflow `usize`.
    Overflow { base: usize, offset: usize },
    /// The error of VolatileSlice.
    VolatileSlice(VError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::OutOfBounds { addr } => write!(f, "address 0x{:x} is out of bounds", addr),
            Error::Overflow { base, offset } => write!(
                f,
                "address 0x{:x} offset by 0x{:x} would overflow",
                base, offset
            ),
            Error::VolatileSlice(e) => write!(f, "{}", e),
        }
    }
}

impl error::Error for Error {}

/// An adapter structure to work around limitations of the `vm-memory` crate.
///
/// It solves the compilation failure by masking out the  [`vm_memory::BitmapSlice`] generic type
/// parameter of [`vm_memory::VolatileSlice`].
///
/// [`vm_memory::BitmapSlice`]: https://docs.rs/vm-memory/latest/vm_memory/bitmap/trait.BitmapSlice.html
/// [`vm_memory::VolatileSlice`]: https://docs.rs/vm-memory/latest/vm_memory/volatile_memory/struct.VolatileSlice.html
#[derive(Clone, Copy, Debug)]
pub struct FileVolatileSlice<'a> {
    addr: usize,
    size: usize,
    phantom: PhantomData<&'a u8>,
}

impl<'a> FileVolatileSlice<'a> {
    fn new(addr: *mut u8, size: usize) -> Self {
        Self {
            addr: addr as usize,
            size,
            phantom: PhantomData,
        }
    }

    /// Create a new instance of [`FileVolatileSlice`] from a raw pointer.
    ///
    /// # Safety
    /// To use this safely, the caller must guarantee that the memory at `addr` is `size` bytes long
    /// and is available for the duration of the lifetime of the new [FileVolatileSlice].
    /// The caller must also guarantee that all other users of the given chunk of memory are using
    /// volatile accesses.
    ///
    /// ### Example
    /// ```rust
    /// # use fuse_backend_rs::file_buf::FileVolatileSlice;
    /// # use vm_memory::bytes::Bytes;
    /// # use std::sync::atomic::Ordering;
    /// let mut buffer = [0u8; 1024];
    /// let s = unsafe { FileVolatileSlice::from_raw_ptr(buffer.as_mut_ptr(), buffer.len()) };
    ///
    /// {
    ///     let o: u32 = s.load(0x10, Ordering::Acquire).unwrap();
    ///     assert_eq!(o, 0);
    ///     s.store(1u8, 0x10, Ordering::Release).unwrap();
    ///
    ///     let s2 = s.as_volatile_slice();
    ///     let s3 = FileVolatileSlice::from_volatile_slice(&s2);
    ///     assert_eq!(s3.len(), 1024);
    /// }
    ///
    /// assert_eq!(buffer[0x10], 1);
    /// ```
    pub unsafe fn from_raw_ptr(addr: *mut u8, size: usize) -> Self {
        Self::new(addr, size)
    }

    /// Create a new instance of [`FileVolatileSlice`] from a mutable slice.
    ///
    /// # Safety
    /// The caller must guarantee that all other users of the given chunk of memory are using
    /// volatile accesses.
    pub unsafe fn from_mut_slice(buf: &'a mut [u8]) -> Self {
        Self::new(buf.as_mut_ptr(), buf.len())
    }

    /// Create a new [`FileVolatileSlice`] from [`vm_memory::VolatileSlice`] and strip off the
    /// [`vm_memory::BitmapSlice`].
    ///
    /// The caller needs to handle dirty page tracking for the data buffer.
    ///
    /// [`vm_memory::BitmapSlice`]: https://docs.rs/vm-memory/latest/vm_memory/bitmap/trait.BitmapSlice.html
    /// [`vm_memory::VolatileSlice`]: https://docs.rs/vm-memory/latest/vm_memory/volatile_memory/struct.VolatileSlice.html
    pub fn from_volatile_slice<S: BitmapSlice>(s: &VolatileSlice<'a, S>) -> Self {
        Self::new(s.as_ptr(), s.len())
    }

    /// Create a [`vm_memory::VolatileSlice`] from [FileVolatileSlice] without dirty page tracking.
    ///
    /// [`vm_memory::VolatileSlice`]: https://docs.rs/vm-memory/latest/vm_memory/volatile_memory/struct.VolatileSlice.html
    pub fn as_volatile_slice(&self) -> VolatileSlice<'a, ()> {
        unsafe { VolatileSlice::new(self.as_ptr(), self.len()) }
    }

    /// Borrow as a [FileVolatileSlice] object to temporarily elide the lifetime parameter.
    ///
    /// # Safety
    /// The [FileVolatileSlice] is borrowed without a lifetime parameter, so the caller must
    /// ensure that [FileVolatileBuf] doesn't out-live the borrowed [FileVolatileSlice] object.
    pub unsafe fn borrow_as_buf(&self, inited: bool) -> FileVolatileBuf {
        let size = if inited { self.size } else { 0 };

        FileVolatileBuf {
            addr: self.addr,
            size,
            cap: self.size,
        }
    }

    /// Return a pointer to the start of the slice.
    pub fn as_ptr(&self) -> *mut u8 {
        self.addr as *mut u8
    }

    /// Get the size of the slice.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Check if the slice is empty.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Return a subslice of this [FileVolatileSlice] starting at `offset`.
    pub fn offset(&self, count: usize) -> Result<Self, Error> {
        let new_addr = (self.addr as usize)
            .checked_add(count)
            .ok_or(Error::Overflow {
                base: self.addr as usize,
                offset: count,
            })?;
        let new_size = self
            .size
            .checked_sub(count)
            .ok_or(Error::OutOfBounds { addr: new_addr })?;
        Ok(Self::new(new_addr as *mut u8, new_size))
    }
}

impl<'a> Bytes<usize> for FileVolatileSlice<'a> {
    type E = VError;

    fn write(&self, buf: &[u8], addr: usize) -> Result<usize, Self::E> {
        VolatileSlice::write(&self.as_volatile_slice(), buf, addr)
    }

    fn read(&self, buf: &mut [u8], addr: usize) -> Result<usize, Self::E> {
        VolatileSlice::read(&self.as_volatile_slice(), buf, addr)
    }

    fn write_slice(&self, buf: &[u8], addr: usize) -> Result<(), Self::E> {
        VolatileSlice::write_slice(&self.as_volatile_slice(), buf, addr)
    }

    fn read_slice(&self, buf: &mut [u8], addr: usize) -> Result<(), Self::E> {
        VolatileSlice::write_slice(&self.as_volatile_slice(), buf, addr)
    }

    fn read_from<F>(&self, addr: usize, src: &mut F, count: usize) -> Result<usize, Self::E>
    where
        F: Read,
    {
        VolatileSlice::read_from(&self.as_volatile_slice(), addr, src, count)
    }

    fn read_exact_from<F>(&self, addr: usize, src: &mut F, count: usize) -> Result<(), Self::E>
    where
        F: Read,
    {
        VolatileSlice::read_exact_from(&self.as_volatile_slice(), addr, src, count)
    }

    fn write_to<F>(&self, addr: usize, dst: &mut F, count: usize) -> Result<usize, Self::E>
    where
        F: Write,
    {
        VolatileSlice::write_to(&self.as_volatile_slice(), addr, dst, count)
    }

    fn write_all_to<F>(&self, addr: usize, dst: &mut F, count: usize) -> Result<(), Self::E>
    where
        F: Write,
    {
        VolatileSlice::write_all_to(&self.as_volatile_slice(), addr, dst, count)
    }

    fn store<T: AtomicAccess>(&self, val: T, addr: usize, order: Ordering) -> Result<(), Self::E> {
        VolatileSlice::store(&self.as_volatile_slice(), val, addr, order)
    }

    fn load<T: AtomicAccess>(&self, addr: usize, order: Ordering) -> Result<T, Self::E> {
        VolatileSlice::load(&self.as_volatile_slice(), addr, order)
    }
}

/// An adapter structure to support `io-uring` based asynchronous IO.
///
/// The [tokio-uring] framework needs to take ownership of data buffers during asynchronous IO
/// operations. The [FileVolatileBuf] converts a referenced buffer to a buffer compatible with
/// the [tokio-uring] APIs.
///
/// # Safety
/// The buffer is borrowed without a lifetime parameter, so the caller must ensure that
/// the [FileVolatileBuf] object doesn't out-live the borrowed buffer. And during the lifetime
/// of the [FileVolatileBuf] object, the referenced buffer must be stable.
///
/// [tokio-uring]: https://github.com/tokio-rs/tokio-uring
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct FileVolatileBuf {
    addr: usize,
    size: usize,
    cap: usize,
}

impl FileVolatileBuf {
    /// Create a [FileVolatileBuf] object from a mutable slice, eliding the lifetime associated
    /// with the slice.
    ///
    /// # Safety
    /// The caller needs to guarantee that the returned `FileVolatileBuf` object doesn't out-live
    /// the referenced buffer. The caller must also guarantee that all other users of the given
    /// chunk of memory are using volatile accesses.
    pub unsafe fn new(buf: &mut [u8]) -> Self {
        Self {
            addr: buf.as_mut_ptr() as usize,
            size: 0,
            cap: buf.len(),
        }
    }

    /// Create a [FileVolatileBuf] object containing `size` bytes of initialized data from a mutable
    /// slice, eliding the lifetime associated with the slice.
    ///
    /// # Safety
    /// The caller needs to guarantee that the returned `FileVolatileBuf` object doesn't out-live
    /// the referenced buffer. The caller must also guarantee that all other users of the given
    /// chunk of memory are using volatile accesses.
    ///
    /// # Panic
    /// Panic if `size` is bigger than `buf.len()`.
    pub unsafe fn new_with_data(buf: &mut [u8], size: usize) -> Self {
        assert!(size <= buf.len());
        Self {
            addr: buf.as_mut_ptr() as usize,
            size,
            cap: buf.len(),
        }
    }

    /// Create a [FileVolatileBuf] object from a raw pointer.
    ///
    /// # Safety
    /// The caller needs to guarantee that the returned `FileVolatileBuf` object doesn't out-live
    /// the referenced buffer. The caller must also guarantee that all other users of the given
    /// chunk of memory are using volatile accesses.
    ///
    /// # Panic
    /// Panic if `size` is bigger than `cap`.
    pub unsafe fn from_raw_ptr(addr: *mut u8, size: usize, cap: usize) -> Self {
        assert!(size <= cap);
        Self {
            addr: addr as usize,
            size,
            cap,
        }
    }

    /// Generate an `IoSlice` object to read data from the buffer.
    pub fn io_slice(&self) -> IoSlice {
        let buf = unsafe { slice::from_raw_parts(self.addr as *const u8, self.size) };
        IoSlice::new(buf)
    }

    /// Generate an `IoSliceMut` object to write data into the buffer.
    pub fn io_slice_mut(&self) -> IoSliceMut {
        let buf = unsafe {
            let ptr = (self.addr as *mut u8).add(self.size);
            let sz = self.cap - self.size;
            slice::from_raw_parts_mut(ptr, sz)
        };

        IoSliceMut::new(buf)
    }

    /// Get capacity of the buffer.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Check whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Get size of initialized data in the buffer.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Set size of initialized data in the buffer.
    ///
    /// # Safety
    /// Caller needs to ensure size is less than or equal to `cap`.
    pub unsafe fn set_size(&mut self, size: usize) {
        if size <= self.cap {
            self.size = size;
        }
    }
}

#[cfg(all(feature = "async-io", target_os = "linux"))]
pub use crate::tokio_uring::buf::{IoBuf, IoBufMut, Slice};

#[cfg(all(feature = "async-io", target_os = "linux"))]
mod async_io {
    use super::*;

    unsafe impl crate::tokio_uring::buf::IoBuf for FileVolatileBuf {
        fn stable_ptr(&self) -> *const u8 {
            self.addr as *const u8
        }

        fn bytes_init(&self) -> usize {
            self.size
        }

        fn bytes_total(&self) -> usize {
            self.cap
        }
    }

    unsafe impl crate::tokio_uring::buf::IoBufMut for FileVolatileBuf {
        fn stable_mut_ptr(&mut self) -> *mut u8 {
            self.addr as *mut u8
        }

        unsafe fn set_init(&mut self, pos: usize) {
            self.set_size(pos)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::tokio_uring::buf::{IoBuf, IoBufMut};

        #[test]
        fn test_new_file_volatile_buf() {
            let mut buf = [0u8; 1024];
            let mut buf2 = unsafe { FileVolatileBuf::new(&mut buf) };
            assert_eq!(buf2.bytes_total(), 1024);
            assert_eq!(buf2.bytes_init(), 0);
            assert_eq!(buf2.stable_ptr(), buf.as_ptr());
            unsafe { *buf2.stable_mut_ptr() = b'a' };
            assert_eq!(buf[0], b'a');
        }

        #[test]
        fn test_file_volatile_slice_with_size() {
            let mut buf = [0u8; 1024];
            let mut buf2 = unsafe { FileVolatileBuf::new_with_data(&mut buf, 256) };

            assert_eq!(buf2.bytes_total(), 1024);
            assert_eq!(buf2.bytes_init(), 256);
            assert_eq!(buf2.stable_ptr(), buf.as_ptr());
            assert_eq!(buf2.stable_mut_ptr(), buf.as_mut_ptr());
            unsafe { buf2.set_init(512) };
            assert_eq!(buf2.bytes_init(), 512);
            unsafe { buf2.set_init(2048) };
            assert_eq!(buf2.bytes_init(), 512);
        }

        #[test]
        fn test_file_volatile_slice_io_slice() {
            let mut buf = [0u8; 1024];
            let buf2 = unsafe { FileVolatileBuf::new_with_data(&mut buf, 256) };

            let slice = buf2.io_slice_mut();
            assert_eq!(slice.len(), 768);
            assert_eq!(unsafe { buf2.stable_ptr().add(256) }, slice.as_ptr());

            let slice2 = buf2.io_slice();
            assert_eq!(slice2.len(), 256);
            assert_eq!(buf2.stable_ptr(), slice2.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_file_volatile_slice() {
        let mut buffer = [0u8; 1024];
        let s = unsafe { FileVolatileSlice::from_raw_ptr(buffer.as_mut_ptr(), buffer.len()) };

        let o: u32 = s.load(0x10, Ordering::Acquire).unwrap();
        assert_eq!(o, 0);
        s.store(1u8, 0x10, Ordering::Release).unwrap();

        let s2 = s.as_volatile_slice();
        let s3 = FileVolatileSlice::from_volatile_slice(&s2);
        assert_eq!(s3.len(), 1024);

        assert!(s3.offset(2048).is_err());

        assert_eq!(buffer[0x10], 1);
    }
}
