// Portions Copyright 2019 Red Hat, Inc.
//
// Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRT-PARTY file.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Types for volatile access to memory.
//!
//! Two of the core rules for safe rust is no data races and no aliased mutable references.
//! `VolatileRef` and `VolatileSlice`, along with types that produce those which implement
//! `VolatileMemory`, allow us to sidestep that rule by wrapping pointers that absolutely have to be
//! accessed volatile. Some systems really do need to operate on shared memory and can't have the
//! compiler reordering or eliding access because it has no visibility into what other systems are
//! doing with that hunk of memory.
//!
//! For the purposes of maintaining safety, volatile memory has some rules of its own:
//! 1. No references or slices to volatile memory (`&` or `&mut`).
//! 2. Access should always been done with a volatile read or write.
//! The First rule is because having references of any kind to memory considered volatile would
//! violate pointer aliasing. The second is because unvolatile accesses are inherently undefined if
//! done concurrently without synchronization. With volatile access we know that the compiler has
//! not reordered or elided the access.

use std::cmp::min;
use std::convert::TryFrom;
use std::error;
use std::fmt;
use std::io::{self, Read, Write};
use std::marker::PhantomData;
use std::mem::{align_of, size_of};
use std::ptr::copy;
use std::ptr::{read_volatile, write_volatile};
use std::result;
use std::slice::{from_raw_parts, from_raw_parts_mut};
use std::sync::atomic::Ordering;
use std::usize;

use crate::atomic_integer::AtomicInteger;
use crate::bitmap::{Bitmap, BitmapSlice, BS};
use crate::{AtomicAccess, ByteValued, Bytes};

use copy_slice_impl::copy_slice;

/// `VolatileMemory` related errors.
#[allow(missing_docs)]
#[derive(Debug)]
pub enum Error {
    /// `addr` is out of bounds of the volatile memory slice.
    OutOfBounds { addr: usize },
    /// Taking a slice at `base` with `offset` would overflow `usize`.
    Overflow { base: usize, offset: usize },
    /// Taking a slice whose size overflows `usize`.
    TooBig { nelements: usize, size: usize },
    /// Trying to obtain a misaligned reference.
    Misaligned { addr: usize, alignment: usize },
    /// Writing to memory failed
    IOError(io::Error),
    /// Incomplete read or write
    PartialBuffer { expected: usize, completed: usize },
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
            Error::TooBig { nelements, size } => write!(
                f,
                "{:?} elements of size {:?} would overflow a usize",
                nelements, size
            ),
            Error::Misaligned { addr, alignment } => {
                write!(f, "address 0x{:x} is not aligned to {:?}", addr, alignment)
            }
            Error::IOError(error) => write!(f, "{}", error),
            Error::PartialBuffer {
                expected,
                completed,
            } => write!(
                f,
                "only used {} bytes in {} long buffer",
                completed, expected
            ),
        }
    }
}

impl error::Error for Error {}

/// Result of volatile memory operations.
pub type Result<T> = result::Result<T, Error>;

/// Convenience function for computing `base + offset`.
///
/// # Errors
///
/// Returns [`Err(Error::Overflow)`](enum.Error.html#variant.Overflow) in case `base + offset`
/// exceeds `usize::MAX`.
///
/// # Examples
///
/// ```
/// # use vm_memory::volatile_memory::compute_offset;
/// #
/// assert_eq!(108, compute_offset(100, 8).unwrap());
/// assert!(compute_offset(std::usize::MAX, 6).is_err());
/// ```
pub fn compute_offset(base: usize, offset: usize) -> Result<usize> {
    match base.checked_add(offset) {
        None => Err(Error::Overflow { base, offset }),
        Some(m) => Ok(m),
    }
}

/// Types that support raw volatile access to their data.
pub trait VolatileMemory {
    /// Type used for dirty memory tracking.
    type B: Bitmap;

    /// Gets the size of this slice.
    fn len(&self) -> usize;

    /// Check whether the region is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a [`VolatileSlice`](struct.VolatileSlice.html) of `count` bytes starting at
    /// `offset`.
    fn get_slice(&self, offset: usize, count: usize) -> Result<VolatileSlice<BS<Self::B>>>;

    /// Gets a slice of memory for the entire region that supports volatile access.
    fn as_volatile_slice(&self) -> VolatileSlice<BS<Self::B>> {
        self.get_slice(0, self.len()).unwrap()
    }

    /// Gets a `VolatileRef` at `offset`.
    fn get_ref<T: ByteValued>(&self, offset: usize) -> Result<VolatileRef<T, BS<Self::B>>> {
        let slice = self.get_slice(offset, size_of::<T>())?;
        unsafe {
            // This is safe because the pointer is range-checked by get_slice, and
            // the lifetime is the same as self.
            Ok(VolatileRef::with_bitmap(slice.addr, slice.bitmap))
        }
    }

    /// Returns a [`VolatileArrayRef`](struct.VolatileArrayRef.html) of `n` elements starting at
    /// `offset`.
    fn get_array_ref<T: ByteValued>(
        &self,
        offset: usize,
        n: usize,
    ) -> Result<VolatileArrayRef<T, BS<Self::B>>> {
        // Use isize to avoid problems with ptr::offset and ptr::add down the line.
        let nbytes = isize::try_from(n)
            .ok()
            .and_then(|n| n.checked_mul(size_of::<T>() as isize))
            .ok_or(Error::TooBig {
                nelements: n,
                size: size_of::<T>(),
            })?;
        let slice = self.get_slice(offset, nbytes as usize)?;
        unsafe {
            // This is safe because the pointer is range-checked by get_slice, and
            // the lifetime is the same as self.
            Ok(VolatileArrayRef::with_bitmap(slice.addr, n, slice.bitmap))
        }
    }

    /// Returns a reference to an instance of `T` at `offset`.
    ///
    /// # Safety
    /// To use this safely, the caller must guarantee that there are no other
    /// users of the given chunk of memory for the lifetime of the result.
    ///
    /// # Errors
    ///
    /// If the resulting pointer is not aligned, this method will return an
    /// [`Error`](enum.Error.html).
    unsafe fn aligned_as_ref<T: ByteValued>(&self, offset: usize) -> Result<&T> {
        let slice = self.get_slice(offset, size_of::<T>())?;
        slice.check_alignment(align_of::<T>())?;
        Ok(&*(slice.addr as *const T))
    }

    /// Returns a mutable reference to an instance of `T` at `offset`. Mutable accesses performed
    /// using the resulting reference are not automatically accounted for by the dirty bitmap
    /// tracking functionality.
    ///
    /// # Safety
    ///
    /// To use this safely, the caller must guarantee that there are no other
    /// users of the given chunk of memory for the lifetime of the result.
    ///
    /// # Errors
    ///
    /// If the resulting pointer is not aligned, this method will return an
    /// [`Error`](enum.Error.html).
    unsafe fn aligned_as_mut<T: ByteValued>(&self, offset: usize) -> Result<&mut T> {
        let slice = self.get_slice(offset, size_of::<T>())?;
        slice.check_alignment(align_of::<T>())?;

        Ok(&mut *(slice.addr as *mut T))
    }

    /// Returns a reference to an instance of `T` at `offset`. Mutable accesses performed
    /// using the resulting reference are not automatically accounted for by the dirty bitmap
    /// tracking functionality.
    ///
    /// # Errors
    ///
    /// If the resulting pointer is not aligned, this method will return an
    /// [`Error`](enum.Error.html).
    fn get_atomic_ref<T: AtomicInteger>(&self, offset: usize) -> Result<&T> {
        let slice = self.get_slice(offset, size_of::<T>())?;
        slice.check_alignment(align_of::<T>())?;

        unsafe {
            // This is safe because the pointer is range-checked by get_slice, and
            // the lifetime is the same as self.
            Ok(&*(slice.addr as *const T))
        }
    }

    /// Returns the sum of `base` and `offset` if the resulting address is valid.
    fn compute_end_offset(&self, base: usize, offset: usize) -> Result<usize> {
        let mem_end = compute_offset(base, offset)?;
        if mem_end > self.len() {
            return Err(Error::OutOfBounds { addr: mem_end });
        }
        Ok(mem_end)
    }
}

impl<'a> VolatileMemory for &'a mut [u8] {
    type B = ();

    fn len(&self) -> usize {
        <[u8]>::len(self)
    }

    fn get_slice(&self, offset: usize, count: usize) -> Result<VolatileSlice<()>> {
        let _ = self.compute_end_offset(offset, count)?;
        unsafe {
            // This is safe because the pointer is range-checked by compute_end_offset, and
            // the lifetime is the same as the original slice.
            Ok(VolatileSlice::new(
                (self.as_ptr() as usize + offset) as *mut _,
                count,
            ))
        }
    }
}

#[repr(C, packed)]
struct Packed<T>(T);

/// A slice of raw memory that supports volatile access.
#[derive(Clone, Copy, Debug)]
pub struct VolatileSlice<'a, B = ()> {
    addr: *mut u8,
    size: usize,
    bitmap: B,
    phantom: PhantomData<&'a u8>,
}

impl<'a> VolatileSlice<'a, ()> {
    /// Creates a slice of raw memory that must support volatile access.
    ///
    /// # Safety
    ///
    /// To use this safely, the caller must guarantee that the memory at `addr` is `size` bytes long
    /// and is available for the duration of the lifetime of the new `VolatileSlice`. The caller
    /// must also guarantee that all other users of the given chunk of memory are using volatile
    /// accesses.
    pub unsafe fn new(addr: *mut u8, size: usize) -> VolatileSlice<'a> {
        Self::with_bitmap(addr, size, ())
    }
}

impl<'a, B: BitmapSlice> VolatileSlice<'a, B> {
    /// Creates a slice of raw memory that must support volatile access, and uses the provided
    /// `bitmap` object for dirty page tracking.
    ///
    /// # Safety
    ///
    /// To use this safely, the caller must guarantee that the memory at `addr` is `size` bytes long
    /// and is available for the duration of the lifetime of the new `VolatileSlice`. The caller
    /// must also guarantee that all other users of the given chunk of memory are using volatile
    /// accesses.
    pub unsafe fn with_bitmap(addr: *mut u8, size: usize, bitmap: B) -> VolatileSlice<'a, B> {
        VolatileSlice {
            addr,
            size,
            bitmap,
            phantom: PhantomData,
        }
    }

    /// Returns a pointer to the beginning of the slice. Mutable accesses performed
    /// using the resulting pointer are not automatically accounted for by the dirty bitmap
    /// tracking functionality.
    pub fn as_ptr(&self) -> *mut u8 {
        self.addr
    }

    /// Gets the size of this slice.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Checks if the slice is empty.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Borrows the inner `BitmapSlice`.
    pub fn bitmap(&self) -> &B {
        &self.bitmap
    }

    /// Divides one slice into two at an index.
    ///
    /// # Example
    ///
    /// ```
    /// # use vm_memory::VolatileMemory;
    /// #
    /// # // Create a buffer
    /// # let mut mem = [0u8; 32];
    /// # let mem_ref = &mut mem[..];
    /// #
    /// # // Get a `VolatileSlice` from the buffer
    /// let vslice = mem_ref
    ///     .get_slice(0, 32)
    ///     .expect("Could not get VolatileSlice");
    ///
    /// let (start, end) = vslice.split_at(8).expect("Could not split VolatileSlice");
    /// assert_eq!(8, start.len());
    /// assert_eq!(24, end.len());
    /// ```
    pub fn split_at(&self, mid: usize) -> Result<(Self, Self)> {
        let end = self.offset(mid)?;
        let start = unsafe {
            // safe because self.offset() already checked the bounds
            VolatileSlice::with_bitmap(self.addr, mid, self.bitmap.clone())
        };

        Ok((start, end))
    }

    /// Returns a subslice of this [`VolatileSlice`](struct.VolatileSlice.html) starting at
    /// `offset` with `count` length.
    ///
    /// The returned subslice is a copy of this slice with the address increased by `offset` bytes
    /// and the size set to `count` bytes.
    pub fn subslice(&self, offset: usize, count: usize) -> Result<Self> {
        let mem_end = compute_offset(offset, count)?;
        if mem_end > self.len() {
            return Err(Error::OutOfBounds { addr: mem_end });
        }
        unsafe {
            // This is safe because the pointer is range-checked by compute_end_offset, and
            // the lifetime is the same as the original slice.
            Ok(VolatileSlice::with_bitmap(
                (self.as_ptr() as usize + offset) as *mut u8,
                count,
                self.bitmap.slice_at(offset),
            ))
        }
    }

    /// Returns a subslice of this [`VolatileSlice`](struct.VolatileSlice.html) starting at
    /// `offset`.
    ///
    /// The returned subslice is a copy of this slice with the address increased by `count` bytes
    /// and the size reduced by `count` bytes.
    pub fn offset(&self, count: usize) -> Result<VolatileSlice<'a, B>> {
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
        unsafe {
            // Safe because the memory has the same lifetime and points to a subset of the
            // memory of the original slice.
            Ok(VolatileSlice::with_bitmap(
                new_addr as *mut u8,
                new_size,
                self.bitmap.slice_at(count),
            ))
        }
    }

    /// Copies as many elements of type `T` as possible from this slice to `buf`.
    ///
    /// Copies `self.len()` or `buf.len()` times the size of `T` bytes, whichever is smaller,
    /// to `buf`. The copy happens from smallest to largest address in `T` sized chunks
    /// using volatile reads.
    ///
    /// # Examples
    ///
    /// ```
    /// # use vm_memory::VolatileMemory;
    /// #
    /// let mut mem = [0u8; 32];
    /// let mem_ref = &mut mem[..];
    /// let vslice = mem_ref
    ///     .get_slice(0, 32)
    ///     .expect("Could not get VolatileSlice");
    /// let mut buf = [5u8; 16];
    /// let res = vslice.copy_to(&mut buf[..]);
    ///
    /// assert_eq!(16, res);
    /// for &v in &buf[..] {
    ///     assert_eq!(v, 0);
    /// }
    /// ```
    pub fn copy_to<T>(&self, buf: &mut [T]) -> usize
    where
        T: ByteValued,
    {
        // A fast path for u8/i8
        if size_of::<T>() == 1 {
            // It is safe because the pointers are range-checked when the slices are created,
            // and they never escape the VolatileSlices.
            let source = unsafe { self.as_slice() };
            // Safe because `T` is a one-byte data structure.
            let dst = unsafe { from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, buf.len()) };
            copy_slice(dst, source)
        } else {
            let count = self.size / size_of::<T>();
            let source = self.get_array_ref::<T>(0, count).unwrap();
            source.copy_to(buf)
        }
    }

    /// Copies as many bytes as possible from this slice to the provided `slice`.
    ///
    /// The copies happen in an undefined order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use vm_memory::VolatileMemory;
    /// #
    /// # // Create a buffer
    /// # let mut mem = [0u8; 32];
    /// # let mem_ref = &mut mem[..];
    /// #
    /// # // Get a `VolatileSlice` from the buffer
    /// # let vslice = mem_ref.get_slice(0, 32)
    /// #    .expect("Could not get VolatileSlice");
    /// #
    /// vslice.copy_to_volatile_slice(
    ///     vslice
    ///         .get_slice(16, 16)
    ///         .expect("Could not get VolatileSlice"),
    /// );
    /// ```
    pub fn copy_to_volatile_slice<S: BitmapSlice>(&self, slice: VolatileSlice<S>) {
        unsafe {
            // Safe because the pointers are range-checked when the slices
            // are created, and they never escape the VolatileSlices.
            // FIXME: ... however, is it really okay to mix non-volatile
            // operations such as copy with read_volatile and write_volatile?
            let count = min(self.size, slice.size);
            copy(self.addr, slice.addr, count);
            slice.bitmap.mark_dirty(0, count);
        }
    }

    /// Copies as many elements of type `T` as possible from `buf` to this slice.
    ///
    /// The copy happens from smallest to largest address in `T` sized chunks using volatile writes.
    ///
    /// # Examples
    ///
    /// ```
    /// # use vm_memory::VolatileMemory;
    /// #
    /// let mut mem = [0u8; 32];
    /// let mem_ref = &mut mem[..];
    /// let vslice = mem_ref
    ///     .get_slice(0, 32)
    ///     .expect("Could not get VolatileSlice");
    ///
    /// let buf = [5u8; 64];
    /// vslice.copy_from(&buf[..]);
    ///
    /// for i in 0..4 {
    ///     let val = vslice
    ///         .get_ref::<u32>(i * 4)
    ///         .expect("Could not get value")
    ///         .load();
    ///     assert_eq!(val, 0x05050505);
    /// }
    /// ```
    pub fn copy_from<T>(&self, buf: &[T])
    where
        T: ByteValued,
    {
        // A fast path for u8/i8
        if size_of::<T>() == 1 {
            // It is safe because the pointers are range-checked when the slices are created,
            // and they never escape the VolatileSlices.
            let dst = unsafe { self.as_mut_slice() };
            // Safe because `T` is a one-byte data structure.
            let src = unsafe { from_raw_parts(buf.as_ptr() as *const u8, buf.len()) };
            let count = copy_slice(dst, src);
            self.bitmap.mark_dirty(0, count * size_of::<T>());
        } else {
            let count = self.size / size_of::<T>();
            // It's ok to use unwrap here because `count` was computed based on the current
            // length of `self`.
            let dest = self.get_array_ref::<T>(0, count).unwrap();

            // No need to explicitly call `mark_dirty` after this call because
            // `VolatileArrayRef::copy_from` already takes care of that.
            dest.copy_from(buf);
        };
    }

    /// Returns a slice corresponding to the data in the underlying memory.
    ///
    /// # Safety
    ///
    /// This function is private and only used for the read/write functions. It is not valid in
    /// general to take slices of volatile memory.
    unsafe fn as_slice(&self) -> &[u8] {
        from_raw_parts(self.addr, self.size)
    }

    /// Returns a mutable slice corresponding to the data in the underlying memory. Writes to the
    /// slice have to be tracked manually using the handle returned by `VolatileSlice::bitmap`.
    ///
    /// # Safety
    ///
    /// This function is private and only used for the read/write functions. It is not valid in
    /// general to take slices of volatile memory. Mutable accesses performed through the returned
    /// slice are not visible to the dirty bitmap tracking functionality, and must be manually
    /// recorded using the associated bitmap object.
    #[allow(clippy::mut_from_ref)]
    unsafe fn as_mut_slice(&self) -> &mut [u8] {
        from_raw_parts_mut(self.addr, self.size)
    }

    /// Checks if the current slice is aligned at `alignment` bytes.
    fn check_alignment(&self, alignment: usize) -> Result<()> {
        // Check that the desired alignment is a power of two.
        debug_assert!((alignment & (alignment - 1)) == 0);
        if ((self.addr as usize) & (alignment - 1)) != 0 {
            return Err(Error::Misaligned {
                addr: self.addr as usize,
                alignment,
            });
        }
        Ok(())
    }
}

impl<B: BitmapSlice> Bytes<usize> for VolatileSlice<'_, B> {
    type E = Error;

    /// # Examples
    /// * Write a slice of size 5 at offset 1020 of a 1024-byte `VolatileSlice`.
    ///
    /// ```
    /// # use vm_memory::{Bytes, VolatileMemory};
    /// #
    /// let mut mem = [0u8; 1024];
    /// let mut mem_ref = &mut mem[..];
    /// let vslice = mem_ref.as_volatile_slice();
    /// let res = vslice.write(&[1, 2, 3, 4, 5], 1020);
    ///
    /// assert!(res.is_ok());
    /// assert_eq!(res.unwrap(), 4);
    /// ```
    fn write(&self, buf: &[u8], addr: usize) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        if addr >= self.size {
            return Err(Error::OutOfBounds { addr });
        }

        // Guest memory can't strictly be modeled as a slice because it is
        // volatile.  Writing to it with what is essentially a fancy memcpy
        // won't hurt anything as long as we get the bounds checks right.
        let slice = unsafe { self.as_mut_slice() }.split_at_mut(addr).1;

        let count = copy_slice(slice, buf);
        self.bitmap.mark_dirty(addr, count);
        Ok(count)
    }

    /// # Examples
    /// * Read a slice of size 16 at offset 1010 of a 1024-byte `VolatileSlice`.
    ///
    /// ```
    /// # use vm_memory::{Bytes, VolatileMemory};
    /// #
    /// let mut mem = [0u8; 1024];
    /// let mut mem_ref = &mut mem[..];
    /// let vslice = mem_ref.as_volatile_slice();
    /// let buf = &mut [0u8; 16];
    /// let res = vslice.read(buf, 1010);
    ///
    /// assert!(res.is_ok());
    /// assert_eq!(res.unwrap(), 14);
    /// ```
    fn read(&self, buf: &mut [u8], addr: usize) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        if addr >= self.size {
            return Err(Error::OutOfBounds { addr });
        }

        // Guest memory can't strictly be modeled as a slice because it is
        // volatile.  Writing to it with what is essentially a fancy memcpy
        // won't hurt anything as long as we get the bounds checks right.
        let slice = unsafe { self.as_slice() }.split_at(addr).1;
        Ok(copy_slice(buf, slice))
    }

    /// # Examples
    /// * Write a slice at offset 256.
    ///
    /// ```
    /// # use vm_memory::{Bytes, VolatileMemory};
    /// #
    /// # // Create a buffer
    /// # let mut mem = [0u8; 1024];
    /// # let mut mem_ref = &mut mem[..];
    /// #
    /// # // Get a `VolatileSlice` from the buffer
    /// # let vslice = mem_ref.as_volatile_slice();
    /// #
    /// let res = vslice.write_slice(&[1, 2, 3, 4, 5], 256);
    ///
    /// assert!(res.is_ok());
    /// assert_eq!(res.unwrap(), ());
    /// ```
    fn write_slice(&self, buf: &[u8], addr: usize) -> Result<()> {
        // `mark_dirty` called within `self.write`.
        let len = self.write(buf, addr)?;
        if len != buf.len() {
            return Err(Error::PartialBuffer {
                expected: buf.len(),
                completed: len,
            });
        }
        Ok(())
    }

    /// # Examples
    /// * Read a slice of size 16 at offset 256.
    ///
    /// ```
    /// # use vm_memory::{Bytes, VolatileMemory};
    /// #
    /// # // Create a buffer
    /// # let mut mem = [0u8; 1024];
    /// # let mut mem_ref = &mut mem[..];
    /// #
    /// # // Get a `VolatileSlice` from the buffer
    /// # let vslice = mem_ref.as_volatile_slice();
    /// #
    /// let buf = &mut [0u8; 16];
    /// let res = vslice.read_slice(buf, 256);
    ///
    /// assert!(res.is_ok());
    /// ```
    fn read_slice(&self, buf: &mut [u8], addr: usize) -> Result<()> {
        let len = self.read(buf, addr)?;
        if len != buf.len() {
            return Err(Error::PartialBuffer {
                expected: buf.len(),
                completed: len,
            });
        }
        Ok(())
    }

    /// # Examples
    ///
    /// * Read bytes from /dev/urandom
    ///
    /// ```
    /// # use vm_memory::{Bytes, VolatileMemory};
    /// # use std::fs::File;
    /// # use std::path::Path;
    /// #
    /// # if cfg!(unix) {
    /// # let mut mem = [0u8; 1024];
    /// # let mut mem_ref = &mut mem[..];
    /// # let vslice = mem_ref.as_volatile_slice();
    /// let mut file = File::open(Path::new("/dev/urandom")).expect("Could not open /dev/urandom");
    ///
    /// vslice
    ///     .read_from(32, &mut file, 128)
    ///     .expect("Could not read bytes from file into VolatileSlice");
    ///
    /// let rand_val: u32 = vslice
    ///     .read_obj(40)
    ///     .expect("Could not read value from VolatileSlice");
    /// # }
    /// ```
    fn read_from<F>(&self, addr: usize, src: &mut F, count: usize) -> Result<usize>
    where
        F: Read,
    {
        let end = self.compute_end_offset(addr, count)?;
        let bytes_read = unsafe {
            // It is safe to overwrite the volatile memory. Accessing the guest
            // memory as a mutable slice is OK because nothing assumes another
            // thread won't change what is loaded.
            let dst = &mut self.as_mut_slice()[addr..end];
            loop {
                match src.read(dst) {
                    Ok(n) => break n,
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(Error::IOError(e)),
                }
            }
        };

        self.bitmap.mark_dirty(addr, bytes_read);
        Ok(bytes_read)
    }

    /// # Examples
    ///
    /// * Read bytes from /dev/urandom
    ///
    /// ```
    /// # use vm_memory::{Bytes, VolatileMemory};
    /// # use std::fs::File;
    /// # use std::path::Path;
    /// #
    /// # if cfg!(unix) {
    /// # let mut mem = [0u8; 1024];
    /// # let mut mem_ref = &mut mem[..];
    /// # let vslice = mem_ref.as_volatile_slice();
    /// let mut file = File::open(Path::new("/dev/urandom")).expect("Could not open /dev/urandom");
    ///
    /// vslice
    ///     .read_exact_from(32, &mut file, 128)
    ///     .expect("Could not read bytes from file into VolatileSlice");
    ///
    /// let rand_val: u32 = vslice
    ///     .read_obj(40)
    ///     .expect("Could not read value from VolatileSlice");
    /// # }
    /// ```
    fn read_exact_from<F>(&self, addr: usize, src: &mut F, count: usize) -> Result<()>
    where
        F: Read,
    {
        let end = self.compute_end_offset(addr, count)?;

        // It is safe to overwrite the volatile memory. Accessing the guest memory as a mutable
        // slice is OK because nothing assumes another thread won't change what is loaded. We also
        // manually update the dirty bitmap below.
        let dst = unsafe { &mut self.as_mut_slice()[addr..end] };

        let result = src.read_exact(dst).map_err(Error::IOError);
        self.bitmap.mark_dirty(addr, count);
        result
    }

    /// # Examples
    ///
    /// * Write 128 bytes to /dev/null
    ///
    /// ```
    /// # use vm_memory::{Bytes, VolatileMemory};
    /// # use std::fs::OpenOptions;
    /// # use std::path::Path;
    /// #
    /// # if cfg!(unix) {
    /// # let mut mem = [0u8; 1024];
    /// # let mut mem_ref = &mut mem[..];
    /// # let vslice = mem_ref.as_volatile_slice();
    /// let mut file = OpenOptions::new()
    ///     .write(true)
    ///     .open("/dev/null")
    ///     .expect("Could not open /dev/null");
    ///
    /// vslice
    ///     .write_to(32, &mut file, 128)
    ///     .expect("Could not write value from VolatileSlice to /dev/null");
    /// # }
    /// ```
    fn write_to<F>(&self, addr: usize, dst: &mut F, count: usize) -> Result<usize>
    where
        F: Write,
    {
        let end = self.compute_end_offset(addr, count)?;
        unsafe {
            // It is safe to read from volatile memory. Accessing the guest
            // memory as a slice is OK because nothing assumes another thread
            // won't change what is loaded.
            let src = &self.as_slice()[addr..end];
            loop {
                match dst.write(src) {
                    Ok(n) => break Ok(n),
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => break Err(Error::IOError(e)),
                }
            }
        }
    }

    /// # Examples
    ///
    /// * Write 128 bytes to /dev/null
    ///
    /// ```
    /// # use vm_memory::{Bytes, VolatileMemory};
    /// # use std::fs::OpenOptions;
    /// # use std::path::Path;
    /// #
    /// # if cfg!(unix) {
    /// # let mut mem = [0u8; 1024];
    /// # let mut mem_ref = &mut mem[..];
    /// # let vslice = mem_ref.as_volatile_slice();
    /// let mut file = OpenOptions::new()
    ///     .write(true)
    ///     .open("/dev/null")
    ///     .expect("Could not open /dev/null");
    ///
    /// vslice
    ///     .write_all_to(32, &mut file, 128)
    ///     .expect("Could not write value from VolatileSlice to /dev/null");
    /// # }
    /// ```
    fn write_all_to<F>(&self, addr: usize, dst: &mut F, count: usize) -> Result<()>
    where
        F: Write,
    {
        let end = self.compute_end_offset(addr, count)?;
        unsafe {
            // It is safe to read from volatile memory. Accessing the guest
            // memory as a slice is OK because nothing assumes another thread
            // won't change what is loaded.
            let src = &self.as_slice()[addr..end];
            dst.write_all(src).map_err(Error::IOError)?;
        }
        Ok(())
    }

    fn store<T: AtomicAccess>(&self, val: T, addr: usize, order: Ordering) -> Result<()> {
        self.get_atomic_ref::<T::A>(addr).map(|r| {
            r.store(val.into(), order);
            self.bitmap.mark_dirty(addr, size_of::<T>())
        })
    }

    fn load<T: AtomicAccess>(&self, addr: usize, order: Ordering) -> Result<T> {
        self.get_atomic_ref::<T::A>(addr)
            .map(|r| r.load(order).into())
    }
}

impl<B: BitmapSlice> VolatileMemory for VolatileSlice<'_, B> {
    type B = B;

    fn len(&self) -> usize {
        self.size
    }

    fn get_slice(&self, offset: usize, count: usize) -> Result<VolatileSlice<B>> {
        let _ = self.compute_end_offset(offset, count)?;
        Ok(unsafe {
            // This is safe because the pointer is range-checked by compute_end_offset, and
            // the lifetime is the same as self.
            VolatileSlice::with_bitmap(
                (self.addr as usize + offset) as *mut u8,
                count,
                self.bitmap.slice_at(offset),
            )
        })
    }
}

/// A memory location that supports volatile access to an instance of `T`.
///
/// # Examples
///
/// ```
/// # use vm_memory::VolatileRef;
/// #
/// let mut v = 5u32;
/// let v_ref = unsafe { VolatileRef::new(&mut v as *mut u32 as *mut u8) };
///
/// assert_eq!(v, 5);
/// assert_eq!(v_ref.load(), 5);
/// v_ref.store(500);
/// assert_eq!(v, 500);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct VolatileRef<'a, T, B = ()> {
    addr: *mut Packed<T>,
    bitmap: B,
    phantom: PhantomData<&'a T>,
}

impl<'a, T> VolatileRef<'a, T, ()>
where
    T: ByteValued,
{
    /// Creates a [`VolatileRef`](struct.VolatileRef.html) to an instance of `T`.
    ///
    /// # Safety
    ///
    /// To use this safely, the caller must guarantee that the memory at `addr` is big enough for a
    /// `T` and is available for the duration of the lifetime of the new `VolatileRef`. The caller
    /// must also guarantee that all other users of the given chunk of memory are using volatile
    /// accesses.
    pub unsafe fn new(addr: *mut u8) -> Self {
        Self::with_bitmap(addr, ())
    }
}

#[allow(clippy::len_without_is_empty)]
impl<'a, T, B> VolatileRef<'a, T, B>
where
    T: ByteValued,
    B: BitmapSlice,
{
    /// Creates a [`VolatileRef`](struct.VolatileRef.html) to an instance of `T`, using the
    /// provided `bitmap` object for dirty page tracking.
    ///
    /// # Safety
    ///
    /// To use this safely, the caller must guarantee that the memory at `addr` is big enough for a
    /// `T` and is available for the duration of the lifetime of the new `VolatileRef`. The caller
    /// must also guarantee that all other users of the given chunk of memory are using volatile
    /// accesses.
    pub unsafe fn with_bitmap(addr: *mut u8, bitmap: B) -> Self {
        VolatileRef {
            addr: addr as *mut Packed<T>,
            bitmap,
            phantom: PhantomData,
        }
    }

    /// Returns a pointer to the underlying memory. Mutable accesses performed
    /// using the resulting pointer are not automatically accounted for by the dirty bitmap
    /// tracking functionality.
    pub fn as_ptr(&self) -> *mut u8 {
        self.addr as *mut u8
    }

    /// Gets the size of the referenced type `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::mem::size_of;
    /// # use vm_memory::VolatileRef;
    /// #
    /// let v_ref = unsafe { VolatileRef::<u32>::new(0 as *mut _) };
    /// assert_eq!(v_ref.len(), size_of::<u32>() as usize);
    /// ```
    pub fn len(&self) -> usize {
        size_of::<T>()
    }

    /// Borrows the inner `BitmapSlice`.
    pub fn bitmap(&self) -> &B {
        &self.bitmap
    }

    /// Does a volatile write of the value `v` to the address of this ref.
    #[inline(always)]
    pub fn store(&self, v: T) {
        unsafe { write_volatile(self.addr, Packed::<T>(v)) };
        self.bitmap.mark_dirty(0, size_of::<T>())
    }

    /// Does a volatile read of the value at the address of this ref.
    #[inline(always)]
    pub fn load(&self) -> T {
        // For the purposes of demonstrating why read_volatile is necessary, try replacing the code
        // in this function with the commented code below and running `cargo test --release`.
        // unsafe { *(self.addr as *const T) }
        unsafe { read_volatile(self.addr).0 }
    }

    /// Converts this to a [`VolatileSlice`](struct.VolatileSlice.html) with the same size and
    /// address.
    pub fn to_slice(&self) -> VolatileSlice<'a, B> {
        unsafe {
            VolatileSlice::with_bitmap(self.addr as *mut u8, size_of::<T>(), self.bitmap.clone())
        }
    }
}

/// A memory location that supports volatile access to an array of elements of type `T`.
///
/// # Examples
///
/// ```
/// # use vm_memory::VolatileArrayRef;
/// #
/// let mut v = [5u32; 1];
/// let v_ref = unsafe { VolatileArrayRef::new(&mut v[0] as *mut u32 as *mut u8, v.len()) };
///
/// assert_eq!(v[0], 5);
/// assert_eq!(v_ref.load(0), 5);
/// v_ref.store(0, 500);
/// assert_eq!(v[0], 500);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct VolatileArrayRef<'a, T, B = ()> {
    addr: *mut u8,
    nelem: usize,
    bitmap: B,
    phantom: PhantomData<&'a T>,
}

impl<'a, T> VolatileArrayRef<'a, T>
where
    T: ByteValued,
{
    /// Creates a [`VolatileArrayRef`](struct.VolatileArrayRef.html) to an array of elements of
    /// type `T`.
    ///
    /// # Safety
    ///
    /// To use this safely, the caller must guarantee that the memory at `addr` is big enough for
    /// `nelem` values of type `T` and is available for the duration of the lifetime of the new
    /// `VolatileRef`. The caller must also guarantee that all other users of the given chunk of
    /// memory are using volatile accesses.
    pub unsafe fn new(addr: *mut u8, nelem: usize) -> Self {
        Self::with_bitmap(addr, nelem, ())
    }
}

impl<'a, T, B> VolatileArrayRef<'a, T, B>
where
    T: ByteValued,
    B: BitmapSlice,
{
    /// Creates a [`VolatileArrayRef`](struct.VolatileArrayRef.html) to an array of elements of
    /// type `T`, using the provided `bitmap` object for dirty page tracking.
    ///
    /// # Safety
    ///
    /// To use this safely, the caller must guarantee that the memory at `addr` is big enough for
    /// `nelem` values of type `T` and is available for the duration of the lifetime of the new
    /// `VolatileRef`. The caller must also guarantee that all other users of the given chunk of
    /// memory are using volatile accesses.
    pub unsafe fn with_bitmap(addr: *mut u8, nelem: usize, bitmap: B) -> Self {
        VolatileArrayRef {
            addr,
            nelem,
            bitmap,
            phantom: PhantomData,
        }
    }

    /// Returns `true` if this array is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// # use vm_memory::VolatileArrayRef;
    /// #
    /// let v_array = unsafe { VolatileArrayRef::<u32>::new(0 as *mut _, 0) };
    /// assert!(v_array.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.nelem == 0
    }

    /// Returns the number of elements in the array.
    ///
    /// # Examples
    ///
    /// ```
    /// # use vm_memory::VolatileArrayRef;
    /// #
    /// # let v_array = unsafe { VolatileArrayRef::<u32>::new(0 as *mut _, 1) };
    /// assert_eq!(v_array.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.nelem
    }

    /// Returns the size of `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::mem::size_of;
    /// # use vm_memory::VolatileArrayRef;
    /// #
    /// let v_ref = unsafe { VolatileArrayRef::<u32>::new(0 as *mut _, 0) };
    /// assert_eq!(v_ref.element_size(), size_of::<u32>() as usize);
    /// ```
    pub fn element_size(&self) -> usize {
        size_of::<T>()
    }

    /// Returns a pointer to the underlying memory. Mutable accesses performed
    /// using the resulting pointer are not automatically accounted for by the dirty bitmap
    /// tracking functionality.
    pub fn as_ptr(&self) -> *mut u8 {
        self.addr
    }

    /// Borrows the inner `BitmapSlice`.
    pub fn bitmap(&self) -> &B {
        &self.bitmap
    }

    /// Converts this to a `VolatileSlice` with the same size and address.
    pub fn to_slice(&self) -> VolatileSlice<'a, B> {
        unsafe {
            VolatileSlice::with_bitmap(
                self.addr,
                self.nelem * self.element_size(),
                self.bitmap.clone(),
            )
        }
    }

    /// Does a volatile read of the element at `index`.
    ///
    /// # Panics
    ///
    /// Panics if `index` is less than the number of elements of the array to which `&self` points.
    pub fn ref_at(&self, index: usize) -> VolatileRef<'a, T, B> {
        assert!(index < self.nelem);
        // Safe because the memory has the same lifetime and points to a subset of the
        // memory of the VolatileArrayRef.
        unsafe {
            // byteofs must fit in an isize as it was checked in get_array_ref.
            let byteofs = (self.element_size() * index) as isize;
            let ptr = self.as_ptr().offset(byteofs);
            VolatileRef::with_bitmap(ptr, self.bitmap.slice_at(byteofs as usize))
        }
    }

    /// Does a volatile read of the element at `index`.
    pub fn load(&self, index: usize) -> T {
        self.ref_at(index).load()
    }

    /// Does a volatile write of the element at `index`.
    pub fn store(&self, index: usize, value: T) {
        // The `VolatileRef::store` call below implements the required dirty bitmap tracking logic,
        // so no need to do that in this method as well.
        self.ref_at(index).store(value)
    }

    /// Copies as many elements of type `T` as possible from this array to `buf`.
    ///
    /// Copies `self.len()` or `buf.len()` times the size of `T` bytes, whichever is smaller,
    /// to `buf`. The copy happens from smallest to largest address in `T` sized chunks
    /// using volatile reads.
    ///
    /// # Examples
    ///
    /// ```
    /// # use vm_memory::VolatileArrayRef;
    /// #
    /// let mut v = [0u8; 32];
    /// let v_ref = unsafe { VolatileArrayRef::new(&mut v[0] as *mut u8, v.len()) };
    ///
    /// let mut buf = [5u8; 16];
    /// v_ref.copy_to(&mut buf[..]);
    /// for &v in &buf[..] {
    ///     assert_eq!(v, 0);
    /// }
    /// ```
    pub fn copy_to(&self, buf: &mut [T]) -> usize {
        // A fast path for u8/i8
        if size_of::<T>() == 1 {
            let source = self.to_slice();
            // It is safe because the pointers are range-checked when the slices are created,
            // and they never escape the VolatileSlices.
            let src = unsafe { source.as_slice() };
            // Safe because `T` is a one-byte data structure.
            let dst = unsafe { from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, buf.len()) };
            return copy_slice(dst, src);
        }

        let mut addr = self.addr;
        let mut i = 0;
        for v in buf.iter_mut().take(self.len()) {
            unsafe {
                // read_volatile is safe because the pointers are range-checked when
                // the slices are created, and they never escape the VolatileSlices.
                // ptr::add is safe because get_array_ref() validated that
                // size_of::<T>() * self.len() fits in an isize.
                *v = read_volatile(addr as *const Packed<T>).0;
                addr = addr.add(self.element_size());
            };
            i += 1;
        }
        i
    }

    /// Copies as many bytes as possible from this slice to the provided `slice`.
    ///
    /// The copies happen in an undefined order.
    ///
    /// # Examples
    ///
    /// ```
    /// # use vm_memory::VolatileArrayRef;
    /// #
    /// let mut v = [0u8; 32];
    /// let v_ref = unsafe { VolatileArrayRef::<u8>::new(&mut v[0] as *mut u8, v.len()) };
    /// let mut buf = [5u8; 16];
    /// let v_ref2 = unsafe { VolatileArrayRef::<u8>::new(&mut buf[0] as *mut u8, buf.len()) };
    ///
    /// v_ref.copy_to_volatile_slice(v_ref2.to_slice());
    /// for &v in &buf[..] {
    ///     assert_eq!(v, 0);
    /// }
    /// ```
    pub fn copy_to_volatile_slice<S: BitmapSlice>(&self, slice: VolatileSlice<S>) {
        unsafe {
            // Safe because the pointers are range-checked when the slices
            // are created, and they never escape the VolatileSlices.
            // FIXME: ... however, is it really okay to mix non-volatile
            // operations such as copy with read_volatile and write_volatile?
            let count = min(self.len() * self.element_size(), slice.size);
            copy(self.addr, slice.addr, count);
            slice.bitmap.mark_dirty(0, count);
        }
    }

    /// Copies as many elements of type `T` as possible from `buf` to this slice.
    ///
    /// Copies `self.len()` or `buf.len()` times the size of `T` bytes, whichever is smaller,
    /// to this slice's memory. The copy happens from smallest to largest address in
    /// `T` sized chunks using volatile writes.
    ///
    /// # Examples
    ///
    /// ```
    /// # use vm_memory::VolatileArrayRef;
    /// #
    /// let mut v = [0u8; 32];
    /// let v_ref = unsafe { VolatileArrayRef::<u8>::new(&mut v[0] as *mut u8, v.len()) };
    ///
    /// let buf = [5u8; 64];
    /// v_ref.copy_from(&buf[..]);
    /// for &val in &v[..] {
    ///     assert_eq!(5u8, val);
    /// }
    /// ```
    pub fn copy_from(&self, buf: &[T]) {
        // A fast path for u8/i8
        if size_of::<T>() == 1 {
            // It is safe because the pointers are range-checked when the slices are created,
            // and they never escape the VolatileSlices.
            let destination = self.to_slice();
            let dst = unsafe { destination.as_mut_slice() };
            // Safe because `T` is a one-byte data structure.
            let src = unsafe { from_raw_parts(buf.as_ptr() as *const u8, buf.len()) };
            let count = copy_slice(dst, src);
            self.bitmap.mark_dirty(0, count);
        } else {
            let mut addr = self.addr;
            for &v in buf.iter().take(self.len()) {
                unsafe {
                    // write_volatile is safe because the pointers are range-checked when
                    // the slices are created, and they never escape the VolatileSlices.
                    // ptr::add is safe because get_array_ref() validated that
                    // size_of::<T>() * self.len() fits in an isize.
                    write_volatile(addr as *mut Packed<T>, Packed::<T>(v));
                    addr = addr.add(self.element_size());
                }
            }

            self.bitmap
                .mark_dirty(0, addr as usize - self.addr as usize)
        }
    }
}

impl<'a, B: BitmapSlice> From<VolatileSlice<'a, B>> for VolatileArrayRef<'a, u8, B> {
    fn from(slice: VolatileSlice<'a, B>) -> Self {
        // Safe because the result has the same lifetime and points to the same
        // memory as the incoming VolatileSlice.
        unsafe { VolatileArrayRef::with_bitmap(slice.as_ptr(), slice.len(), slice.bitmap) }
    }
}

// Return the largest value that `addr` is aligned to. Forcing this function to return 1 will
// cause test_non_atomic_access to fail.
fn alignment(addr: usize) -> usize {
    // Rust is silly and does not let me write addr & -addr.
    addr & (!addr + 1)
}

mod copy_slice_impl {
    use super::*;

    // Has the same safety requirements as `read_volatile` + `write_volatile`, namely:
    // - `src_addr` and `dst_addr` must be valid for reads/writes.
    // - `src_addr` and `dst_addr` must be properly aligned with respect to `align`.
    // - `src_addr` must point to a properly initialized value, which is true here because
    //   we're only using integer primitives.
    unsafe fn copy_single(align: usize, src_addr: usize, dst_addr: usize) {
        match align {
            8 => write_volatile(dst_addr as *mut u64, read_volatile(src_addr as *const u64)),
            4 => write_volatile(dst_addr as *mut u32, read_volatile(src_addr as *const u32)),
            2 => write_volatile(dst_addr as *mut u16, read_volatile(src_addr as *const u16)),
            1 => write_volatile(dst_addr as *mut u8, read_volatile(src_addr as *const u8)),
            _ => unreachable!(),
        }
    }

    fn copy_slice_volatile(dst: &mut [u8], src: &[u8]) -> usize {
        let total = min(src.len(), dst.len());
        let mut left = total;

        let mut src_addr = src.as_ptr() as usize;
        let mut dst_addr = dst.as_ptr() as usize;
        let align = min(alignment(src_addr), alignment(dst_addr));

        let mut copy_aligned_slice = |min_align| {
            while align >= min_align && left >= min_align {
                // Safe because we check alignment beforehand, the memory areas are valid for
                // reads/writes, and the source always contains a valid value.
                unsafe { copy_single(min_align, src_addr, dst_addr) };
                src_addr += min_align;
                dst_addr += min_align;
                left -= min_align;
            }
        };

        if size_of::<usize>() > 4 {
            copy_aligned_slice(8);
        }
        copy_aligned_slice(4);
        copy_aligned_slice(2);
        copy_aligned_slice(1);

        total
    }

    pub(super) fn copy_slice(dst: &mut [u8], src: &[u8]) -> usize {
        let total = min(src.len(), dst.len());
        if total <= size_of::<usize>() {
            copy_slice_volatile(dst, src);
        } else {
            dst[..total].copy_from_slice(&src[..total]);
        }

        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;
    use std::mem::size_of_val;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread::spawn;

    use matches::assert_matches;
    use vmm_sys_util::tempfile::TempFile;

    use crate::bitmap::tests::{
        check_range, range_is_clean, range_is_dirty, test_bytes, test_volatile_memory,
    };
    use crate::bitmap::{AtomicBitmap, RefSlice};

    #[derive(Clone)]
    struct VecMem {
        mem: Arc<[u8]>,
    }

    impl VecMem {
        fn new(size: usize) -> VecMem {
            VecMem {
                mem: vec![0; size].into(),
            }
        }
    }

    impl VolatileMemory for VecMem {
        type B = ();

        fn len(&self) -> usize {
            self.mem.len()
        }

        fn get_slice(&self, offset: usize, count: usize) -> Result<VolatileSlice<()>> {
            let _ = self.compute_end_offset(offset, count)?;
            Ok(unsafe {
                VolatileSlice::new((self.mem.as_ptr() as usize + offset) as *mut _, count)
            })
        }
    }

    #[test]
    fn test_display_error() {
        assert_eq!(
            format!("{}", Error::OutOfBounds { addr: 0x10 }),
            "address 0x10 is out of bounds"
        );

        assert_eq!(
            format!(
                "{}",
                Error::Overflow {
                    base: 0x0,
                    offset: 0x10
                }
            ),
            "address 0x0 offset by 0x10 would overflow"
        );

        assert_eq!(
            format!(
                "{}",
                Error::TooBig {
                    nelements: 100_000,
                    size: 1_000_000_000
                }
            ),
            "100000 elements of size 1000000000 would overflow a usize"
        );

        assert_eq!(
            format!(
                "{}",
                Error::Misaligned {
                    addr: 0x4,
                    alignment: 8
                }
            ),
            "address 0x4 is not aligned to 8"
        );

        assert_eq!(
            format!(
                "{}",
                Error::PartialBuffer {
                    expected: 100,
                    completed: 90
                }
            ),
            "only used 90 bytes in 100 long buffer"
        );
    }

    #[test]
    fn misaligned_ref() {
        let mut a = [0u8; 3];
        let a_ref = &mut a[..];
        unsafe {
            assert!(
                a_ref.aligned_as_ref::<u16>(0).is_err() ^ a_ref.aligned_as_ref::<u16>(1).is_err()
            );
            assert!(
                a_ref.aligned_as_mut::<u16>(0).is_err() ^ a_ref.aligned_as_mut::<u16>(1).is_err()
            );
        }
    }

    #[test]
    fn atomic_store() {
        let mut a = [0usize; 1];
        {
            let a_ref = unsafe {
                VolatileSlice::new(&mut a[0] as *mut usize as *mut u8, size_of::<usize>())
            };
            let atomic = a_ref.get_atomic_ref::<AtomicUsize>(0).unwrap();
            atomic.store(2usize, Ordering::Relaxed)
        }
        assert_eq!(a[0], 2);
    }

    #[test]
    fn atomic_load() {
        let mut a = [5usize; 1];
        {
            let a_ref = unsafe {
                VolatileSlice::new(&mut a[0] as *mut usize as *mut u8,
                                   size_of::<usize>())
            };
            let atomic = {
                let atomic = a_ref.get_atomic_ref::<AtomicUsize>(0).unwrap();
                assert_eq!(atomic.load(Ordering::Relaxed), 5usize);
                atomic
            };
            // To make sure we can take the atomic out of the scope we made it in:
            atomic.load(Ordering::Relaxed);
            // but not too far:
            // atomicu8
        } //.load(std::sync::atomic::Ordering::Relaxed)
        ;
    }

    #[test]
    fn misaligned_atomic() {
        let mut a = [5usize, 5usize];
        let a_ref =
            unsafe { VolatileSlice::new(&mut a[0] as *mut usize as *mut u8, size_of::<usize>()) };
        assert!(a_ref.get_atomic_ref::<AtomicUsize>(0).is_ok());
        assert!(a_ref.get_atomic_ref::<AtomicUsize>(1).is_err());
    }

    #[test]
    fn ref_store() {
        let mut a = [0u8; 1];
        {
            let a_ref = &mut a[..];
            let v_ref = a_ref.get_ref(0).unwrap();
            v_ref.store(2u8);
        }
        assert_eq!(a[0], 2);
    }

    #[test]
    fn ref_load() {
        let mut a = [5u8; 1];
        {
            let a_ref = &mut a[..];
            let c = {
                let v_ref = a_ref.get_ref::<u8>(0).unwrap();
                assert_eq!(v_ref.load(), 5u8);
                v_ref
            };
            // To make sure we can take a v_ref out of the scope we made it in:
            c.load();
            // but not too far:
            // c
        } //.load()
        ;
    }

    #[test]
    fn ref_to_slice() {
        let mut a = [1u8; 5];
        let a_ref = &mut a[..];
        let v_ref = a_ref.get_ref(1).unwrap();
        v_ref.store(0x1234_5678u32);
        let ref_slice = v_ref.to_slice();
        assert_eq!(v_ref.as_ptr() as usize, ref_slice.as_ptr() as usize);
        assert_eq!(v_ref.len(), ref_slice.len());
        assert!(!ref_slice.is_empty());
    }

    #[test]
    fn observe_mutate() {
        let a = VecMem::new(1);
        let a_clone = a.clone();
        let v_ref = a.get_ref::<u8>(0).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let barrier1 = barrier.clone();

        v_ref.store(99);
        spawn(move || {
            barrier1.wait();
            let clone_v_ref = a_clone.get_ref::<u8>(0).unwrap();
            clone_v_ref.store(0);
            barrier1.wait();
        });

        assert_eq!(v_ref.load(), 99);
        barrier.wait();
        barrier.wait();
        assert_eq!(v_ref.load(), 0);
    }

    #[test]
    fn mem_is_empty() {
        let a = VecMem::new(100);
        assert!(!a.is_empty());

        let a = VecMem::new(0);
        assert!(a.is_empty());
    }

    #[test]
    fn slice_len() {
        let mem = VecMem::new(100);
        let slice = mem.get_slice(0, 27).unwrap();
        assert_eq!(slice.len(), 27);
        assert!(!slice.is_empty());

        let slice = mem.get_slice(34, 27).unwrap();
        assert_eq!(slice.len(), 27);
        assert!(!slice.is_empty());

        let slice = slice.get_slice(20, 5).unwrap();
        assert_eq!(slice.len(), 5);
        assert!(!slice.is_empty());

        let slice = mem.get_slice(34, 0).unwrap();
        assert!(slice.is_empty());
    }

    #[test]
    fn slice_subslice() {
        let mem = VecMem::new(100);
        let slice = mem.get_slice(0, 100).unwrap();
        assert!(slice.write(&[1; 80], 10).is_ok());

        assert!(slice.subslice(0, 0).is_ok());
        assert!(slice.subslice(0, 101).is_err());

        assert!(slice.subslice(99, 0).is_ok());
        assert!(slice.subslice(99, 1).is_ok());
        assert!(slice.subslice(99, 2).is_err());

        assert!(slice.subslice(100, 0).is_ok());
        assert!(slice.subslice(100, 1).is_err());

        assert!(slice.subslice(101, 0).is_err());
        assert!(slice.subslice(101, 1).is_err());

        assert!(slice.subslice(std::usize::MAX, 2).is_err());
        assert!(slice.subslice(2, std::usize::MAX).is_err());

        let maybe_offset_slice = slice.subslice(10, 80);
        assert!(maybe_offset_slice.is_ok());
        let offset_slice = maybe_offset_slice.unwrap();
        assert_eq!(offset_slice.len(), 80);

        let mut buf = [0; 80];
        assert!(offset_slice.read(&mut buf, 0).is_ok());
        assert_eq!(&buf[0..80], &[1; 80][0..80]);
    }

    #[test]
    fn slice_offset() {
        let mem = VecMem::new(100);
        let slice = mem.get_slice(0, 100).unwrap();
        assert!(slice.write(&[1; 80], 10).is_ok());

        assert!(slice.offset(101).is_err());

        let maybe_offset_slice = slice.offset(10);
        assert!(maybe_offset_slice.is_ok());
        let offset_slice = maybe_offset_slice.unwrap();
        assert_eq!(offset_slice.len(), 90);
        let mut buf = [0; 90];
        assert!(offset_slice.read(&mut buf, 0).is_ok());
        assert_eq!(&buf[0..80], &[1; 80][0..80]);
        assert_eq!(&buf[80..90], &[0; 10][0..10]);
    }

    #[test]
    fn slice_copy_to_u8() {
        let mut a = [2u8, 4, 6, 8, 10];
        let mut b = [0u8; 4];
        let mut c = [0u8; 6];
        let a_ref = &mut a[..];
        let v_ref = a_ref.get_slice(0, a_ref.len()).unwrap();
        v_ref.copy_to(&mut b[..]);
        v_ref.copy_to(&mut c[..]);
        assert_eq!(b[0..4], a_ref[0..4]);
        assert_eq!(c[0..5], a_ref[0..5]);
    }

    #[test]
    fn slice_copy_to_u16() {
        let mut a = [0x01u16, 0x2, 0x03, 0x4, 0x5];
        let mut b = [0u16; 4];
        let mut c = [0u16; 6];
        let a_ref = &mut a[..];
        let v_ref = unsafe { VolatileSlice::new(a_ref.as_mut_ptr() as *mut u8, 9) };

        v_ref.copy_to(&mut b[..]);
        v_ref.copy_to(&mut c[..]);
        assert_eq!(b[0..4], a_ref[0..4]);
        assert_eq!(c[0..4], a_ref[0..4]);
        assert_eq!(c[4], 0);
    }

    #[test]
    fn slice_copy_from_u8() {
        let a = [2u8, 4, 6, 8, 10];
        let mut b = [0u8; 4];
        let mut c = [0u8; 6];
        let b_ref = &mut b[..];
        let v_ref = b_ref.get_slice(0, b_ref.len()).unwrap();
        v_ref.copy_from(&a[..]);
        assert_eq!(b_ref[0..4], a[0..4]);

        let c_ref = &mut c[..];
        let v_ref = c_ref.get_slice(0, c_ref.len()).unwrap();
        v_ref.copy_from(&a[..]);
        assert_eq!(c_ref[0..5], a[0..5]);
    }

    #[test]
    fn slice_copy_from_u16() {
        let a = [2u16, 4, 6, 8, 10];
        let mut b = [0u16; 4];
        let mut c = [0u16; 6];
        let b_ref = &mut b[..];
        let v_ref = unsafe { VolatileSlice::new(b_ref.as_mut_ptr() as *mut u8, 8) };
        v_ref.copy_from(&a[..]);
        assert_eq!(b_ref[0..4], a[0..4]);

        let c_ref = &mut c[..];
        let v_ref = unsafe { VolatileSlice::new(c_ref.as_mut_ptr() as *mut u8, 9) };
        v_ref.copy_from(&a[..]);
        assert_eq!(c_ref[0..4], a[0..4]);
        assert_eq!(c_ref[4], 0);
    }

    #[test]
    fn slice_copy_to_volatile_slice() {
        let mut a = [2u8, 4, 6, 8, 10];
        let a_ref = &mut a[..];
        let a_slice = a_ref.get_slice(0, a_ref.len()).unwrap();

        let mut b = [0u8; 4];
        let b_ref = &mut b[..];
        let b_slice = b_ref.get_slice(0, b_ref.len()).unwrap();

        a_slice.copy_to_volatile_slice(b_slice);
        assert_eq!(b, [2, 4, 6, 8]);
    }

    #[test]
    fn slice_overflow_error() {
        use std::usize::MAX;
        let a = VecMem::new(1);
        let res = a.get_slice(MAX, 1).unwrap_err();
        assert_matches!(
            res,
            Error::Overflow {
                base: MAX,
                offset: 1,
            }
        );
    }

    #[test]
    fn slice_oob_error() {
        let a = VecMem::new(100);
        a.get_slice(50, 50).unwrap();
        let res = a.get_slice(55, 50).unwrap_err();
        assert_matches!(res, Error::OutOfBounds { addr: 105 });
    }

    #[test]
    fn ref_overflow_error() {
        use std::usize::MAX;
        let a = VecMem::new(1);
        let res = a.get_ref::<u8>(MAX).unwrap_err();
        assert_matches!(
            res,
            Error::Overflow {
                base: MAX,
                offset: 1,
            }
        );
    }

    #[test]
    fn ref_oob_error() {
        let a = VecMem::new(100);
        a.get_ref::<u8>(99).unwrap();
        let res = a.get_ref::<u16>(99).unwrap_err();
        assert_matches!(res, Error::OutOfBounds { addr: 101 });
    }

    #[test]
    fn ref_oob_too_large() {
        let a = VecMem::new(3);
        let res = a.get_ref::<u32>(0).unwrap_err();
        assert_matches!(res, Error::OutOfBounds { addr: 4 });
    }

    #[test]
    fn slice_store() {
        let a = VecMem::new(5);
        let s = a.as_volatile_slice();
        let r = a.get_ref(2).unwrap();
        r.store(9u16);
        assert_eq!(s.read_obj::<u16>(2).unwrap(), 9);
    }

    #[test]
    fn test_write_past_end() {
        let a = VecMem::new(5);
        let s = a.as_volatile_slice();
        let res = s.write(&[1, 2, 3, 4, 5, 6], 0);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), 5);
    }

    #[test]
    fn slice_read_and_write() {
        let a = VecMem::new(5);
        let s = a.as_volatile_slice();
        let sample_buf = [1, 2, 3];
        assert!(s.write(&sample_buf, 5).is_err());
        assert!(s.write(&sample_buf, 2).is_ok());
        let mut buf = [0u8; 3];
        assert!(s.read(&mut buf, 5).is_err());
        assert!(s.read_slice(&mut buf, 2).is_ok());
        assert_eq!(buf, sample_buf);

        // Writing an empty buffer at the end of the volatile slice works.
        assert_eq!(s.write(&[], 100).unwrap(), 0);
        let buf: &mut [u8] = &mut [];
        assert_eq!(s.read(buf, 4).unwrap(), 0);

        // Check that reading and writing an empty buffer does not yield an error.
        let empty_mem = VecMem::new(0);
        let empty = empty_mem.as_volatile_slice();
        assert_eq!(empty.write(&[], 1).unwrap(), 0);
        assert_eq!(empty.read(buf, 1).unwrap(), 0);
    }

    #[test]
    fn obj_read_and_write() {
        let a = VecMem::new(5);
        let s = a.as_volatile_slice();
        assert!(s.write_obj(55u16, 4).is_err());
        assert!(s.write_obj(55u16, core::usize::MAX).is_err());
        assert!(s.write_obj(55u16, 2).is_ok());
        assert_eq!(s.read_obj::<u16>(2).unwrap(), 55u16);
        assert!(s.read_obj::<u16>(4).is_err());
        assert!(s.read_obj::<u16>(core::usize::MAX).is_err());
    }

    #[test]
    fn mem_read_and_write() {
        let a = VecMem::new(5);
        let s = a.as_volatile_slice();
        assert!(s.write_obj(!0u32, 1).is_ok());
        let mut file = if cfg!(unix) {
            File::open(Path::new("/dev/zero")).unwrap()
        } else {
            File::open(Path::new("c:\\Windows\\system32\\ntoskrnl.exe")).unwrap()
        };
        assert!(s.read_exact_from(2, &mut file, size_of::<u32>()).is_err());
        assert!(s
            .read_exact_from(core::usize::MAX, &mut file, size_of::<u32>())
            .is_err());

        assert!(s.read_exact_from(1, &mut file, size_of::<u32>()).is_ok());

        let mut f = TempFile::new().unwrap().into_file();
        assert!(s.read_exact_from(1, &mut f, size_of::<u32>()).is_err());
        format!("{:?}", s.read_exact_from(1, &mut f, size_of::<u32>()));

        let value = s.read_obj::<u32>(1).unwrap();
        if cfg!(unix) {
            assert_eq!(value, 0);
        } else {
            assert_eq!(value, 0x0090_5a4d);
        }

        let mut sink = Vec::new();
        assert!(s.write_all_to(1, &mut sink, size_of::<u32>()).is_ok());
        assert!(s.write_all_to(2, &mut sink, size_of::<u32>()).is_err());
        assert!(s
            .write_all_to(core::usize::MAX, &mut sink, size_of::<u32>())
            .is_err());
        format!("{:?}", s.write_all_to(2, &mut sink, size_of::<u32>()));
        if cfg!(unix) {
            assert_eq!(sink, vec![0; size_of::<u32>()]);
        } else {
            assert_eq!(sink, vec![0x4d, 0x5a, 0x90, 0x00]);
        };
    }

    #[test]
    fn unaligned_read_and_write() {
        let a = VecMem::new(7);
        let s = a.as_volatile_slice();
        let sample_buf: [u8; 7] = [1, 2, 0xAA, 0xAA, 0xAA, 0xAA, 4];
        assert!(s.write_slice(&sample_buf, 0).is_ok());
        let r = a.get_ref::<u32>(2).unwrap();
        assert_eq!(r.load(), 0xAAAA_AAAA);

        r.store(0x5555_5555);
        let sample_buf: [u8; 7] = [1, 2, 0x55, 0x55, 0x55, 0x55, 4];
        let mut buf: [u8; 7] = Default::default();
        assert!(s.read_slice(&mut buf, 0).is_ok());
        assert_eq!(buf, sample_buf);
    }

    #[test]
    fn ref_array_from_slice() {
        let mut a = [2, 4, 6, 8, 10];
        let a_vec = a.to_vec();
        let a_ref = &mut a[..];
        let a_slice = a_ref.get_slice(0, a_ref.len()).unwrap();
        let a_array_ref: VolatileArrayRef<u8, ()> = a_slice.into();
        for (i, entry) in a_vec.iter().enumerate() {
            assert_eq!(&a_array_ref.load(i), entry);
        }
    }

    #[test]
    fn ref_array_store() {
        let mut a = [0u8; 5];
        {
            let a_ref = &mut a[..];
            let v_ref = a_ref.get_array_ref(1, 4).unwrap();
            v_ref.store(1, 2u8);
            v_ref.store(2, 4u8);
            v_ref.store(3, 6u8);
        }
        let expected = [2u8, 4u8, 6u8];
        assert_eq!(a[2..=4], expected);
    }

    #[test]
    fn ref_array_load() {
        let mut a = [0, 0, 2, 3, 10];
        {
            let a_ref = &mut a[..];
            let c = {
                let v_ref = a_ref.get_array_ref::<u8>(1, 4).unwrap();
                assert_eq!(v_ref.load(1), 2u8);
                assert_eq!(v_ref.load(2), 3u8);
                assert_eq!(v_ref.load(3), 10u8);
                v_ref
            };
            // To make sure we can take a v_ref out of the scope we made it in:
            c.load(0);
            // but not too far:
            // c
        } //.load()
        ;
    }

    #[test]
    fn ref_array_overflow() {
        let mut a = [0, 0, 2, 3, 10];
        let a_ref = &mut a[..];
        let res = a_ref.get_array_ref::<u32>(4, usize::MAX).unwrap_err();
        assert_matches!(
            res,
            Error::TooBig {
                nelements: usize::MAX,
                size: 4,
            }
        );
    }

    #[test]
    fn alignment() {
        let a = [0u8; 64];
        let a = &a[a.as_ptr().align_offset(32)] as *const u8 as usize;
        assert!(super::alignment(a) >= 32);
        assert_eq!(super::alignment(a + 9), 1);
        assert_eq!(super::alignment(a + 30), 2);
        assert_eq!(super::alignment(a + 12), 4);
        assert_eq!(super::alignment(a + 8), 8);
    }

    #[test]
    fn test_atomic_accesses() {
        let a = VecMem::new(0x1000);
        let s = a.as_volatile_slice();

        crate::bytes::tests::check_atomic_accesses(s, 0, 0x1000);
    }

    #[test]
    fn split_at() {
        let mut mem = [0u8; 32];
        let mem_ref = &mut mem[..];
        let vslice = mem_ref.get_slice(0, 32).unwrap();
        let (start, end) = vslice.split_at(8).unwrap();
        assert_eq!(start.len(), 8);
        assert_eq!(end.len(), 24);
        let (start, end) = vslice.split_at(0).unwrap();
        assert_eq!(start.len(), 0);
        assert_eq!(end.len(), 32);
        let (start, end) = vslice.split_at(31).unwrap();
        assert_eq!(start.len(), 31);
        assert_eq!(end.len(), 1);
        let (start, end) = vslice.split_at(32).unwrap();
        assert_eq!(start.len(), 32);
        assert_eq!(end.len(), 0);
        let err = vslice.split_at(33).unwrap_err();
        assert_matches!(err, Error::OutOfBounds { addr: _ })
    }

    #[test]
    fn test_volatile_slice_dirty_tracking() {
        let val = 123u64;
        let dirty_offset = 0x1000;
        let dirty_len = size_of_val(&val);
        let page_size = 0x1000;

        let mut buf = vec![0u8; 0x10000];

        // Invoke the `Bytes` test helper function.
        {
            let bitmap = AtomicBitmap::new(buf.len(), page_size);
            let slice = unsafe {
                VolatileSlice::with_bitmap(buf.as_mut_ptr(), buf.len(), bitmap.slice_at(0))
            };

            test_bytes(
                &slice,
                |s: &VolatileSlice<RefSlice<AtomicBitmap>>,
                 start: usize,
                 len: usize,
                 clean: bool| { check_range(s.bitmap(), start, len, clean) },
                |offset| offset,
                0x1000,
            );
        }

        // Invoke the `VolatileMemory` test helper function.
        {
            let bitmap = AtomicBitmap::new(buf.len(), page_size);
            let slice = unsafe {
                VolatileSlice::with_bitmap(buf.as_mut_ptr(), buf.len(), bitmap.slice_at(0))
            };
            test_volatile_memory(&slice);
        }

        let bitmap = AtomicBitmap::new(buf.len(), page_size);
        let slice =
            unsafe { VolatileSlice::with_bitmap(buf.as_mut_ptr(), buf.len(), bitmap.slice_at(0)) };

        let bitmap2 = AtomicBitmap::new(buf.len(), page_size);
        let slice2 =
            unsafe { VolatileSlice::with_bitmap(buf.as_mut_ptr(), buf.len(), bitmap2.slice_at(0)) };

        let bitmap3 = AtomicBitmap::new(buf.len(), page_size);
        let slice3 =
            unsafe { VolatileSlice::with_bitmap(buf.as_mut_ptr(), buf.len(), bitmap3.slice_at(0)) };

        assert!(range_is_clean(slice.bitmap(), 0, slice.len()));
        assert!(range_is_clean(slice2.bitmap(), 0, slice2.len()));

        slice.write_obj(val, dirty_offset).unwrap();
        assert!(range_is_dirty(slice.bitmap(), dirty_offset, dirty_len));

        slice.copy_to_volatile_slice(slice2);
        assert!(range_is_dirty(slice2.bitmap(), 0, slice2.len()));

        {
            let (s1, s2) = slice.split_at(dirty_offset).unwrap();
            assert!(range_is_clean(s1.bitmap(), 0, s1.len()));
            assert!(range_is_dirty(s2.bitmap(), 0, dirty_len));
        }

        {
            let s = slice.subslice(dirty_offset, dirty_len).unwrap();
            assert!(range_is_dirty(s.bitmap(), 0, s.len()));
        }

        {
            let s = slice.offset(dirty_offset).unwrap();
            assert!(range_is_dirty(s.bitmap(), 0, dirty_len));
        }

        // Test `copy_from` for size_of::<T> == 1.
        {
            let buf = vec![1u8; dirty_offset];

            assert!(range_is_clean(slice.bitmap(), 0, dirty_offset));
            slice.copy_from(&buf);
            assert!(range_is_dirty(slice.bitmap(), 0, dirty_offset));
        }

        // Test `copy_from` for size_of::<T> > 1.
        {
            let val = 1u32;
            let buf = vec![val; dirty_offset / size_of_val(&val)];

            assert!(range_is_clean(slice3.bitmap(), 0, dirty_offset));
            slice3.copy_from(&buf);
            assert!(range_is_dirty(slice3.bitmap(), 0, dirty_offset));
        }
    }

    #[test]
    fn test_volatile_ref_dirty_tracking() {
        let val = 123u64;
        let mut buf = vec![val];
        let page_size = 0x1000;

        let bitmap = AtomicBitmap::new(size_of_val(&val), page_size);
        let vref =
            unsafe { VolatileRef::with_bitmap(buf.as_mut_ptr() as *mut u8, bitmap.slice_at(0)) };

        assert!(range_is_clean(vref.bitmap(), 0, vref.len()));
        vref.store(val);
        assert!(range_is_dirty(vref.bitmap(), 0, vref.len()));
    }

    fn test_volatile_array_ref_copy_from_tracking<T>(buf: &mut [T], index: usize, page_size: usize)
    where
        T: ByteValued + From<u8>,
    {
        let bitmap = AtomicBitmap::new(buf.len() * size_of::<T>(), page_size);
        let arr = unsafe {
            VolatileArrayRef::with_bitmap(
                buf.as_mut_ptr() as *mut u8,
                index + 1,
                bitmap.slice_at(0),
            )
        };

        let val = T::from(123);
        let copy_buf = vec![val; index + 1];

        assert!(range_is_clean(arr.bitmap(), 0, arr.len() * size_of::<T>()));
        arr.copy_from(copy_buf.as_slice());
        assert!(range_is_dirty(arr.bitmap(), 0, buf.len() * size_of::<T>()));
    }

    #[test]
    fn test_volatile_array_ref_dirty_tracking() {
        let val = 123u64;
        let dirty_len = size_of_val(&val);
        let index = 0x1000;
        let dirty_offset = dirty_len * index;
        let page_size = 0x1000;

        let mut buf = vec![0u64; index + 1];
        let mut byte_buf = vec![0u8; index + 1];

        // Test `ref_at`.
        {
            let bitmap = AtomicBitmap::new(buf.len() * size_of_val(&val), page_size);
            let arr = unsafe {
                VolatileArrayRef::with_bitmap(
                    buf.as_mut_ptr() as *mut u8,
                    index + 1,
                    bitmap.slice_at(0),
                )
            };

            assert!(range_is_clean(arr.bitmap(), 0, arr.len() * dirty_len));
            arr.ref_at(index).store(val);
            assert!(range_is_dirty(arr.bitmap(), dirty_offset, dirty_len));
        }

        // Test `store`.
        {
            let bitmap = AtomicBitmap::new(buf.len() * size_of_val(&val), page_size);
            let arr = unsafe {
                VolatileArrayRef::with_bitmap(
                    buf.as_mut_ptr() as *mut u8,
                    index + 1,
                    bitmap.slice_at(0),
                )
            };

            let slice = arr.to_slice();
            assert!(range_is_clean(slice.bitmap(), 0, slice.len()));
            arr.store(index, val);
            assert!(range_is_dirty(slice.bitmap(), dirty_offset, dirty_len));
        }

        // Test `copy_from` when size_of::<T>() == 1.
        test_volatile_array_ref_copy_from_tracking(&mut byte_buf, index, page_size);
        // Test `copy_from` when size_of::<T>() > 1.
        test_volatile_array_ref_copy_from_tracking(&mut buf, index, page_size);
    }
}
