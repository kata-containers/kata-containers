// Copyright (C) 2019 Alibaba Cloud Computing. All rights reserved.
//
// Portions Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Helper structure for working with mmaped memory regions in Unix.

use std::error;
use std::fmt;
use std::io;
use std::os::unix::io::AsRawFd;
use std::ptr::null_mut;
use std::result;

use crate::bitmap::{Bitmap, BS};
use crate::guest_memory::FileOffset;
use crate::mmap::{check_file_offset, AsSlice, NewBitmap};
use crate::volatile_memory::{self, compute_offset, VolatileMemory, VolatileSlice};

/// Error conditions that may arise when creating a new `MmapRegion` object.
#[derive(Debug)]
pub enum Error {
    /// The specified file offset and length cause overflow when added.
    InvalidOffsetLength,
    /// The specified pointer to the mapping is not page-aligned.
    InvalidPointer,
    /// The forbidden `MAP_FIXED` flag was specified.
    MapFixed,
    /// Mappings using the same fd overlap in terms of file offset and length.
    MappingOverlap,
    /// A mapping with offset + length > EOF was attempted.
    MappingPastEof,
    /// The `mmap` call returned an error.
    Mmap(io::Error),
    /// Seeking the end of the file returned an error.
    SeekEnd(io::Error),
    /// Seeking the start of the file returned an error.
    SeekStart(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> fmt::Result {
        match self {
            Error::InvalidOffsetLength => write!(
                f,
                "The specified file offset and length cause overflow when added"
            ),
            Error::InvalidPointer => write!(
                f,
                "The specified pointer to the mapping is not page-aligned",
            ),
            Error::MapFixed => write!(f, "The forbidden `MAP_FIXED` flag was specified"),
            Error::MappingOverlap => write!(
                f,
                "Mappings using the same fd overlap in terms of file offset and length"
            ),
            Error::MappingPastEof => write!(
                f,
                "The specified file offset and length is greater then file length"
            ),
            Error::Mmap(error) => write!(f, "{}", error),
            Error::SeekEnd(error) => write!(f, "Error seeking the end of the file: {}", error),
            Error::SeekStart(error) => write!(f, "Error seeking the start of the file: {}", error),
        }
    }
}

impl error::Error for Error {}

pub type Result<T> = result::Result<T, Error>;

/// A factory struct to build `MmapRegion` objects.
pub struct MmapRegionBuilder<B = ()> {
    size: usize,
    prot: i32,
    flags: i32,
    file_offset: Option<FileOffset>,
    raw_ptr: Option<*mut u8>,
    hugetlbfs: Option<bool>,
    bitmap: B,
}

impl<B: Bitmap + Default> MmapRegionBuilder<B> {
    /// Create a new `MmapRegionBuilder` using the default value for
    /// the inner `Bitmap` object.
    pub fn new(size: usize) -> Self {
        Self::new_with_bitmap(size, B::default())
    }
}

impl<B: Bitmap> MmapRegionBuilder<B> {
    /// Create a new `MmapRegionBuilder` using the provided `Bitmap` object.
    ///
    /// When instantiating the builder for a region that does not require dirty bitmap
    /// bitmap tracking functionality, we can specify a trivial `Bitmap` implementation
    /// such as `()`.
    pub fn new_with_bitmap(size: usize, bitmap: B) -> Self {
        MmapRegionBuilder {
            size,
            prot: 0,
            flags: libc::MAP_ANONYMOUS | libc::MAP_PRIVATE,
            file_offset: None,
            raw_ptr: None,
            hugetlbfs: None,
            bitmap,
        }
    }

    /// Create the `MmapRegion` object with the specified mmap memory protection flag `prot`.
    pub fn with_mmap_prot(mut self, prot: i32) -> Self {
        self.prot = prot;
        self
    }

    /// Create the `MmapRegion` object with the specified mmap `flags`.
    pub fn with_mmap_flags(mut self, flags: i32) -> Self {
        self.flags = flags;
        self
    }

    /// Create the `MmapRegion` object with the specified `file_offset`.
    pub fn with_file_offset(mut self, file_offset: FileOffset) -> Self {
        self.file_offset = Some(file_offset);
        self
    }

    /// Create the `MmapRegion` object with the specified `hugetlbfs` flag.
    pub fn with_hugetlbfs(mut self, hugetlbfs: bool) -> Self {
        self.hugetlbfs = Some(hugetlbfs);
        self
    }

    /// Create the `MmapRegion` object with pre-mmapped raw pointer.
    ///
    /// # Safety
    ///
    /// To use this safely, the caller must guarantee that `raw_addr` and `self.size` define a
    /// region within a valid mapping that is already present in the process.
    pub unsafe fn with_raw_mmap_pointer(mut self, raw_ptr: *mut u8) -> Self {
        self.raw_ptr = Some(raw_ptr);
        self
    }

    /// Build the `MmapRegion` object.
    pub fn build(self) -> Result<MmapRegion<B>> {
        if self.raw_ptr.is_some() {
            return self.build_raw();
        }

        // Forbid MAP_FIXED, as it doesn't make sense in this context, and is pretty dangerous
        // in general.
        if self.flags & libc::MAP_FIXED != 0 {
            return Err(Error::MapFixed);
        }

        let (fd, offset) = if let Some(ref f_off) = self.file_offset {
            check_file_offset(f_off, self.size)?;
            (f_off.file().as_raw_fd(), f_off.start())
        } else {
            (-1, 0)
        };

        // This is safe because we're not allowing MAP_FIXED, and invalid parameters cannot break
        // Rust safety guarantees (things may change if we're mapping /dev/mem or some wacky file).
        let addr = unsafe {
            libc::mmap(
                null_mut(),
                self.size,
                self.prot,
                self.flags,
                fd,
                offset as libc::off_t,
            )
        };

        if addr == libc::MAP_FAILED {
            return Err(Error::Mmap(io::Error::last_os_error()));
        }

        Ok(MmapRegion {
            addr: addr as *mut u8,
            size: self.size,
            bitmap: self.bitmap,
            file_offset: self.file_offset,
            prot: self.prot,
            flags: self.flags,
            owned: true,
            hugetlbfs: self.hugetlbfs,
        })
    }

    fn build_raw(self) -> Result<MmapRegion<B>> {
        // Safe because this call just returns the page size and doesn't have any side effects.
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        let addr = self.raw_ptr.unwrap();

        // Check that the pointer to the mapping is page-aligned.
        if (addr as usize) & (page_size - 1) != 0 {
            return Err(Error::InvalidPointer);
        }

        Ok(MmapRegion {
            addr,
            size: self.size,
            bitmap: self.bitmap,
            file_offset: self.file_offset,
            prot: self.prot,
            flags: self.flags,
            owned: false,
            hugetlbfs: self.hugetlbfs,
        })
    }
}

/// Helper structure for working with mmaped memory regions in Unix.
///
/// The structure is used for accessing the guest's physical memory by mmapping it into
/// the current process.
///
/// # Limitations
/// When running a 64-bit virtual machine on a 32-bit hypervisor, only part of the guest's
/// physical memory may be mapped into the current process due to the limited virtual address
/// space size of the process.
#[derive(Debug)]
pub struct MmapRegion<B = ()> {
    addr: *mut u8,
    size: usize,
    bitmap: B,
    file_offset: Option<FileOffset>,
    prot: i32,
    flags: i32,
    owned: bool,
    hugetlbfs: Option<bool>,
}

// Send and Sync aren't automatically inherited for the raw address pointer.
// Accessing that pointer is only done through the stateless interface which
// allows the object to be shared by multiple threads without a decrease in
// safety.
unsafe impl<B: Send> Send for MmapRegion<B> {}
unsafe impl<B: Sync> Sync for MmapRegion<B> {}

impl<B: NewBitmap> MmapRegion<B> {
    /// Creates a shared anonymous mapping of `size` bytes.
    ///
    /// # Arguments
    /// * `size` - The size of the memory region in bytes.
    pub fn new(size: usize) -> Result<Self> {
        MmapRegionBuilder::new_with_bitmap(size, B::with_len(size))
            .with_mmap_prot(libc::PROT_READ | libc::PROT_WRITE)
            .with_mmap_flags(libc::MAP_ANONYMOUS | libc::MAP_NORESERVE | libc::MAP_PRIVATE)
            .build()
    }

    /// Creates a shared file mapping of `size` bytes.
    ///
    /// # Arguments
    /// * `file_offset` - The mapping will be created at offset `file_offset.start` in the file
    ///                   referred to by `file_offset.file`.
    /// * `size` - The size of the memory region in bytes.
    pub fn from_file(file_offset: FileOffset, size: usize) -> Result<Self> {
        MmapRegionBuilder::new_with_bitmap(size, B::with_len(size))
            .with_file_offset(file_offset)
            .with_mmap_prot(libc::PROT_READ | libc::PROT_WRITE)
            .with_mmap_flags(libc::MAP_NORESERVE | libc::MAP_SHARED)
            .build()
    }

    /// Creates a mapping based on the provided arguments.
    ///
    /// # Arguments
    /// * `file_offset` - if provided, the method will create a file mapping at offset
    ///                   `file_offset.start` in the file referred to by `file_offset.file`.
    /// * `size` - The size of the memory region in bytes.
    /// * `prot` - The desired memory protection of the mapping.
    /// * `flags` - This argument determines whether updates to the mapping are visible to other
    ///             processes mapping the same region, and whether updates are carried through to
    ///             the underlying file.
    pub fn build(
        file_offset: Option<FileOffset>,
        size: usize,
        prot: i32,
        flags: i32,
    ) -> Result<Self> {
        let mut builder = MmapRegionBuilder::new_with_bitmap(size, B::with_len(size))
            .with_mmap_prot(prot)
            .with_mmap_flags(flags);
        if let Some(v) = file_offset {
            builder = builder.with_file_offset(v);
        }
        builder.build()
    }

    /// Creates a `MmapRegion` instance for an externally managed mapping.
    ///
    /// This method is intended to be used exclusively in situations in which the mapping backing
    /// the region is provided by an entity outside the control of the caller (e.g. the dynamic
    /// linker).
    ///
    /// # Arguments
    /// * `addr` - Pointer to the start of the mapping. Must be page-aligned.
    /// * `size` - The size of the memory region in bytes.
    /// * `prot` - Must correspond to the memory protection attributes of the existing mapping.
    /// * `flags` - Must correspond to the flags that were passed to `mmap` for the creation of
    ///             the existing mapping.
    ///
    /// # Safety
    ///
    /// To use this safely, the caller must guarantee that `addr` and `size` define a region within
    /// a valid mapping that is already present in the process.
    pub unsafe fn build_raw(addr: *mut u8, size: usize, prot: i32, flags: i32) -> Result<Self> {
        MmapRegionBuilder::new_with_bitmap(size, B::with_len(size))
            .with_raw_mmap_pointer(addr)
            .with_mmap_prot(prot)
            .with_mmap_flags(flags)
            .build()
    }
}

impl<B: Bitmap> MmapRegion<B> {
    /// Returns a pointer to the beginning of the memory region. Mutable accesses performed
    /// using the resulting pointer are not automatically accounted for by the dirty bitmap
    /// tracking functionality.
    ///
    /// Should only be used for passing this region to ioctls for setting guest memory.
    pub fn as_ptr(&self) -> *mut u8 {
        self.addr
    }

    /// Returns the size of this region.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns information regarding the offset into the file backing this region (if any).
    pub fn file_offset(&self) -> Option<&FileOffset> {
        self.file_offset.as_ref()
    }

    /// Returns the value of the `prot` parameter passed to `mmap` when mapping this region.
    pub fn prot(&self) -> i32 {
        self.prot
    }

    /// Returns the value of the `flags` parameter passed to `mmap` when mapping this region.
    pub fn flags(&self) -> i32 {
        self.flags
    }

    /// Returns `true` if the mapping is owned by this `MmapRegion` instance.
    pub fn owned(&self) -> bool {
        self.owned
    }

    /// Checks whether this region and `other` are backed by overlapping
    /// [`FileOffset`](struct.FileOffset.html) objects.
    ///
    /// This is mostly a sanity check available for convenience, as different file descriptors
    /// can alias the same file.
    pub fn fds_overlap<T: Bitmap>(&self, other: &MmapRegion<T>) -> bool {
        if let Some(f_off1) = self.file_offset() {
            if let Some(f_off2) = other.file_offset() {
                if f_off1.file().as_raw_fd() == f_off2.file().as_raw_fd() {
                    let s1 = f_off1.start();
                    let s2 = f_off2.start();
                    let l1 = self.len() as u64;
                    let l2 = other.len() as u64;

                    if s1 < s2 {
                        return s1 + l1 > s2;
                    } else {
                        return s2 + l2 > s1;
                    }
                }
            }
        }
        false
    }

    /// Set the hugetlbfs of the region
    pub fn set_hugetlbfs(&mut self, hugetlbfs: bool) {
        self.hugetlbfs = Some(hugetlbfs)
    }

    /// Returns `true` if the region is hugetlbfs
    pub fn is_hugetlbfs(&self) -> Option<bool> {
        self.hugetlbfs
    }

    /// Returns a reference to the inner bitmap object.
    pub fn bitmap(&self) -> &B {
        &self.bitmap
    }
}

impl<B> AsSlice for MmapRegion<B> {
    unsafe fn as_slice(&self) -> &[u8] {
        // This is safe because we mapped the area at addr ourselves, so this slice will not
        // overflow. However, it is possible to alias.
        std::slice::from_raw_parts(self.addr, self.size)
    }

    #[allow(clippy::mut_from_ref)]
    unsafe fn as_mut_slice(&self) -> &mut [u8] {
        // This is safe because we mapped the area at addr ourselves, so this slice will not
        // overflow. However, it is possible to alias.
        std::slice::from_raw_parts_mut(self.addr, self.size)
    }
}

impl<B: Bitmap> VolatileMemory for MmapRegion<B> {
    type B = B;

    fn len(&self) -> usize {
        self.size
    }

    fn get_slice(
        &self,
        offset: usize,
        count: usize,
    ) -> volatile_memory::Result<VolatileSlice<BS<B>>> {
        let end = compute_offset(offset, count)?;
        if end > self.size {
            return Err(volatile_memory::Error::OutOfBounds { addr: end });
        }

        // Safe because we checked that offset + count was within our range and we only ever hand
        // out volatile accessors.
        Ok(unsafe {
            VolatileSlice::with_bitmap(
                (self.addr as usize + offset) as *mut _,
                count,
                self.bitmap.slice_at(offset),
            )
        })
    }
}

impl<B> Drop for MmapRegion<B> {
    fn drop(&mut self) {
        // This is safe because we mmap the area at addr ourselves, and nobody
        // else is holding a reference to it.
        if self.owned {
            unsafe {
                libc::munmap(self.addr as *mut libc::c_void, self.size);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use std::slice;
    use std::sync::Arc;
    use vmm_sys_util::tempfile::TempFile;

    use crate::bitmap::AtomicBitmap;

    type MmapRegion = super::MmapRegion<()>;

    // Adding a helper method to extract the errno within an Error::Mmap(e), or return a
    // distinctive value when the error is represented by another variant.
    impl Error {
        pub fn raw_os_error(&self) -> i32 {
            match self {
                Error::Mmap(e) => e.raw_os_error().unwrap(),
                _ => std::i32::MIN,
            }
        }
    }

    #[test]
    fn test_mmap_region_new() {
        assert!(MmapRegion::new(0).is_err());

        let size = 4096;

        let r = MmapRegion::new(4096).unwrap();
        assert_eq!(r.size(), size);
        assert!(r.file_offset().is_none());
        assert_eq!(r.prot(), libc::PROT_READ | libc::PROT_WRITE);
        assert_eq!(
            r.flags(),
            libc::MAP_ANONYMOUS | libc::MAP_NORESERVE | libc::MAP_PRIVATE
        );
    }

    #[test]
    fn test_mmap_region_set_hugetlbfs() {
        assert!(MmapRegion::new(0).is_err());

        let size = 4096;

        let r = MmapRegion::new(size).unwrap();
        assert_eq!(r.size(), size);
        assert!(r.file_offset().is_none());
        assert_eq!(r.prot(), libc::PROT_READ | libc::PROT_WRITE);
        assert_eq!(
            r.flags(),
            libc::MAP_ANONYMOUS | libc::MAP_NORESERVE | libc::MAP_PRIVATE
        );
        assert_eq!(r.is_hugetlbfs(), None);

        let mut r = MmapRegion::new(size).unwrap();
        r.set_hugetlbfs(false);
        assert_eq!(r.size(), size);
        assert!(r.file_offset().is_none());
        assert_eq!(r.prot(), libc::PROT_READ | libc::PROT_WRITE);
        assert_eq!(
            r.flags(),
            libc::MAP_ANONYMOUS | libc::MAP_NORESERVE | libc::MAP_PRIVATE
        );
        assert_eq!(r.is_hugetlbfs(), Some(false));

        let mut r = MmapRegion::new(size).unwrap();
        r.set_hugetlbfs(true);
        assert_eq!(r.size(), size);
        assert!(r.file_offset().is_none());
        assert_eq!(r.prot(), libc::PROT_READ | libc::PROT_WRITE);
        assert_eq!(
            r.flags(),
            libc::MAP_ANONYMOUS | libc::MAP_NORESERVE | libc::MAP_PRIVATE
        );
        assert_eq!(r.is_hugetlbfs(), Some(true));
    }

    #[test]
    fn test_mmap_region_from_file() {
        let mut f = TempFile::new().unwrap().into_file();
        let offset: usize = 0;
        let buf1 = [1u8, 2, 3, 4, 5];

        f.write_all(buf1.as_ref()).unwrap();
        let r = MmapRegion::from_file(FileOffset::new(f, offset as u64), buf1.len()).unwrap();

        assert_eq!(r.size(), buf1.len() - offset);
        assert_eq!(r.file_offset().unwrap().start(), offset as u64);
        assert_eq!(r.prot(), libc::PROT_READ | libc::PROT_WRITE);
        assert_eq!(r.flags(), libc::MAP_NORESERVE | libc::MAP_SHARED);

        let buf2 = unsafe { slice::from_raw_parts(r.as_ptr(), buf1.len() - offset) };
        assert_eq!(&buf1[offset..], buf2);
    }

    #[test]
    fn test_mmap_region_build() {
        let a = Arc::new(TempFile::new().unwrap().into_file());

        let prot = libc::PROT_READ | libc::PROT_WRITE;
        let flags = libc::MAP_NORESERVE | libc::MAP_PRIVATE;
        let offset = 4096;
        let size = 1000;

        // Offset + size will overflow.
        let r = MmapRegion::build(
            Some(FileOffset::from_arc(a.clone(), std::u64::MAX)),
            size,
            prot,
            flags,
        );
        assert_eq!(format!("{:?}", r.unwrap_err()), "InvalidOffsetLength");

        // Offset + size is greater than the size of the file (which is 0 at this point).
        let r = MmapRegion::build(
            Some(FileOffset::from_arc(a.clone(), offset)),
            size,
            prot,
            flags,
        );
        assert_eq!(format!("{:?}", r.unwrap_err()), "MappingPastEof");

        // MAP_FIXED was specified among the flags.
        let r = MmapRegion::build(
            Some(FileOffset::from_arc(a.clone(), offset)),
            size,
            prot,
            flags | libc::MAP_FIXED,
        );
        assert_eq!(format!("{:?}", r.unwrap_err()), "MapFixed");

        // Let's resize the file.
        assert_eq!(unsafe { libc::ftruncate(a.as_raw_fd(), 1024 * 10) }, 0);

        // The offset is not properly aligned.
        let r = MmapRegion::build(
            Some(FileOffset::from_arc(a.clone(), offset - 1)),
            size,
            prot,
            flags,
        );
        assert_eq!(r.unwrap_err().raw_os_error(), libc::EINVAL);

        // The build should be successful now.
        let r =
            MmapRegion::build(Some(FileOffset::from_arc(a, offset)), size, prot, flags).unwrap();

        assert_eq!(r.size(), size);
        assert_eq!(r.file_offset().unwrap().start(), offset as u64);
        assert_eq!(r.prot(), libc::PROT_READ | libc::PROT_WRITE);
        assert_eq!(r.flags(), libc::MAP_NORESERVE | libc::MAP_PRIVATE);
        assert!(r.owned());

        let region_size = 0x10_0000;
        let bitmap = AtomicBitmap::new(region_size, 0x1000);
        let builder = MmapRegionBuilder::new_with_bitmap(region_size, bitmap)
            .with_hugetlbfs(true)
            .with_mmap_prot(libc::PROT_READ | libc::PROT_WRITE);
        assert_eq!(builder.size, region_size);
        assert_eq!(builder.hugetlbfs, Some(true));
        assert_eq!(builder.prot, libc::PROT_READ | libc::PROT_WRITE);

        crate::bitmap::tests::test_volatile_memory(&(builder.build().unwrap()));
    }

    #[test]
    fn test_mmap_region_build_raw() {
        let addr = 0;
        let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize };
        let prot = libc::PROT_READ | libc::PROT_WRITE;
        let flags = libc::MAP_NORESERVE | libc::MAP_PRIVATE;

        let r = unsafe { MmapRegion::build_raw((addr + 1) as *mut u8, size, prot, flags) };
        assert_eq!(format!("{:?}", r.unwrap_err()), "InvalidPointer");

        let r = unsafe { MmapRegion::build_raw(addr as *mut u8, size, prot, flags).unwrap() };

        assert_eq!(r.size(), size);
        assert_eq!(r.prot(), libc::PROT_READ | libc::PROT_WRITE);
        assert_eq!(r.flags(), libc::MAP_NORESERVE | libc::MAP_PRIVATE);
        assert!(!r.owned());
    }

    #[test]
    fn test_mmap_region_fds_overlap() {
        let a = Arc::new(TempFile::new().unwrap().into_file());
        assert_eq!(unsafe { libc::ftruncate(a.as_raw_fd(), 1024 * 10) }, 0);

        let r1 = MmapRegion::from_file(FileOffset::from_arc(a.clone(), 0), 4096).unwrap();
        let r2 = MmapRegion::from_file(FileOffset::from_arc(a.clone(), 4096), 4096).unwrap();
        assert!(!r1.fds_overlap(&r2));

        let r1 = MmapRegion::from_file(FileOffset::from_arc(a.clone(), 0), 5000).unwrap();
        assert!(r1.fds_overlap(&r2));

        let r2 = MmapRegion::from_file(FileOffset::from_arc(a, 0), 1000).unwrap();
        assert!(r1.fds_overlap(&r2));

        // Different files, so there's not overlap.
        let new_file = TempFile::new().unwrap().into_file();
        // Resize before mapping.
        assert_eq!(
            unsafe { libc::ftruncate(new_file.as_raw_fd(), 1024 * 10) },
            0
        );
        let r2 = MmapRegion::from_file(FileOffset::new(new_file, 0), 5000).unwrap();
        assert!(!r1.fds_overlap(&r2));

        // R2 is not file backed, so no overlap.
        let r2 = MmapRegion::new(5000).unwrap();
        assert!(!r1.fds_overlap(&r2));
    }

    #[test]
    fn test_dirty_tracking() {
        // Using the `crate` prefix because we aliased `MmapRegion` to `MmapRegion<()>` for
        // the rest of the unit tests above.
        let m = crate::MmapRegion::<AtomicBitmap>::new(0x1_0000).unwrap();
        crate::bitmap::tests::test_volatile_memory(&m);
    }
}
