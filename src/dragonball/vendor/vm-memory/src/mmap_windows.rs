// Copyright (C) 2019 CrowdStrike, Inc. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Helper structure for working with mmaped memory regions in Windows.

use std;
use std::io;
use std::os::windows::io::{AsRawHandle, RawHandle};
use std::ptr::{null, null_mut};

use libc::{c_void, size_t};

use winapi::um::errhandlingapi::GetLastError;

use crate::bitmap::{Bitmap, BS};
use crate::guest_memory::FileOffset;
use crate::mmap::{AsSlice, NewBitmap};
use crate::volatile_memory::{self, compute_offset, VolatileMemory, VolatileSlice};

#[allow(non_snake_case)]
#[link(name = "kernel32")]
extern "stdcall" {
    pub fn VirtualAlloc(
        lpAddress: *mut c_void,
        dwSize: size_t,
        flAllocationType: u32,
        flProtect: u32,
    ) -> *mut c_void;

    pub fn VirtualFree(lpAddress: *mut c_void, dwSize: size_t, dwFreeType: u32) -> u32;

    pub fn CreateFileMappingA(
        hFile: RawHandle,                       // HANDLE
        lpFileMappingAttributes: *const c_void, // LPSECURITY_ATTRIBUTES
        flProtect: u32,                         // DWORD
        dwMaximumSizeHigh: u32,                 // DWORD
        dwMaximumSizeLow: u32,                  // DWORD
        lpName: *const u8,                      // LPCSTR
    ) -> RawHandle; // HANDLE

    pub fn MapViewOfFile(
        hFileMappingObject: RawHandle,
        dwDesiredAccess: u32,
        dwFileOffsetHigh: u32,
        dwFileOffsetLow: u32,
        dwNumberOfBytesToMap: size_t,
    ) -> *mut c_void;

    pub fn CloseHandle(hObject: RawHandle) -> u32; // BOOL
}

const MM_HIGHEST_VAD_ADDRESS: u64 = 0x000007FFFFFDFFFF;

const MEM_COMMIT: u32 = 0x00001000;
const MEM_RELEASE: u32 = 0x00008000;
const FILE_MAP_ALL_ACCESS: u32 = 0xf001f;
const PAGE_READWRITE: u32 = 0x04;

pub const MAP_FAILED: *mut c_void = 0 as *mut c_void;
pub const INVALID_HANDLE_VALUE: RawHandle = (-1isize) as RawHandle;
#[allow(dead_code)]
pub const ERROR_INVALID_PARAMETER: i32 = 87;

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
pub struct MmapRegion<B> {
    addr: *mut u8,
    size: usize,
    bitmap: B,
    file_offset: Option<FileOffset>,
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
    pub fn new(size: usize) -> io::Result<Self> {
        if (size == 0) || (size > MM_HIGHEST_VAD_ADDRESS as usize) {
            return Err(io::Error::from_raw_os_error(libc::EINVAL));
        }
        // This is safe because we are creating an anonymous mapping in a place not already used by
        // any other area in this process.
        let addr = unsafe { VirtualAlloc(0 as *mut c_void, size, MEM_COMMIT, PAGE_READWRITE) };
        if addr == MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            addr: addr as *mut u8,
            size,
            bitmap: B::with_len(size),
            file_offset: None,
        })
    }

    /// Creates a shared file mapping of `size` bytes.
    ///
    /// # Arguments
    /// * `file_offset` - The mapping will be created at offset `file_offset.start` in the file
    ///                   referred to by `file_offset.file`.
    /// * `size` - The size of the memory region in bytes.
    pub fn from_file(file_offset: FileOffset, size: usize) -> io::Result<Self> {
        let handle = file_offset.file().as_raw_handle();
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::from_raw_os_error(libc::EBADF));
        }

        let mapping = unsafe {
            CreateFileMappingA(
                handle,
                null(),
                PAGE_READWRITE,
                (size >> 32) as u32,
                size as u32,
                null(),
            )
        };
        if mapping == 0 as RawHandle {
            return Err(io::Error::last_os_error());
        }

        let offset = file_offset.start();

        // This is safe because we are creating a mapping in a place not already used by any other
        // area in this process.
        let addr = unsafe {
            MapViewOfFile(
                mapping,
                FILE_MAP_ALL_ACCESS,
                (offset >> 32) as u32,
                offset as u32,
                size,
            )
        };

        unsafe {
            CloseHandle(mapping);
        }

        if addr == null_mut() {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            addr: addr as *mut u8,
            size,
            bitmap: B::with_len(size),
            file_offset: Some(file_offset),
        })
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
    ) -> volatile_memory::Result<VolatileSlice<BS<Self::B>>> {
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
        // Note that the size must be set to 0 when using MEM_RELEASE,
        // otherwise the function fails.
        unsafe {
            let ret_val = VirtualFree(self.addr as *mut libc::c_void, 0, MEM_RELEASE);
            if ret_val == 0 {
                let err = GetLastError();
                // We can't use any fancy logger here, yet we want to
                // pin point memory leaks.
                println!(
                    "WARNING: Could not deallocate mmap region. \
                     Address: {:?}. Size: {}. Error: {}",
                    self.addr, self.size, err
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::windows::io::FromRawHandle;

    use crate::bitmap::AtomicBitmap;
    use crate::guest_memory::FileOffset;
    use crate::mmap_windows::INVALID_HANDLE_VALUE;

    type MmapRegion = super::MmapRegion<()>;

    #[test]
    fn map_invalid_handle() {
        let file = unsafe { std::fs::File::from_raw_handle(INVALID_HANDLE_VALUE) };
        let file_offset = FileOffset::new(file, 0);
        let e = MmapRegion::from_file(file_offset, 1024).unwrap_err();
        assert_eq!(e.raw_os_error(), Some(libc::EBADF));
    }

    #[test]
    fn test_dirty_tracking() {
        // Using the `crate` prefix because we aliased `MmapRegion` to `MmapRegion<()>` for
        // the rest of the unit tests above.
        let m = crate::MmapRegion::<AtomicBitmap>::new(0x1_0000).unwrap();
        crate::bitmap::tests::test_volatile_memory(&m);
    }
}
