// Copyright 2021 Ant Group. All rights reserved.
// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io::{Result, Write};
use std::os::unix::io::AsRawFd;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use nydus_utils::div_round_up;

use crate::utils::readahead;

pub(crate) const MAGIC1: u32 = 0x424D_4150;
pub(crate) const MAGIC2: u32 = 0x434D_4150;
pub(crate) const MAGIC_ALL_READY: u32 = 0x4D4D_4150;
pub(crate) const HEADER_SIZE: usize = 4096;
pub(crate) const HEADER_RESERVED_SIZE: usize = HEADER_SIZE - 16;

/// The blob chunk map file header, 4096 bytes.
#[repr(C)]
pub(crate) struct Header {
    /// PersistMap magic number
    pub magic: u32,
    pub version: u32,
    pub magic2: u32,
    pub all_ready: u32,
    pub reserved: [u8; HEADER_RESERVED_SIZE],
}

impl Header {
    pub fn as_slice(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const Header as *const u8,
                std::mem::size_of::<Header>(),
            )
        }
    }
}

pub(crate) struct PersistMap {
    pub count: u32,
    pub size: usize,
    pub base: *const u8,
    pub not_ready_count: AtomicU32,
}

impl PersistMap {
    pub fn open(filename: &str, chunk_count: u32, create: bool, persist: bool) -> Result<Self> {
        if chunk_count == 0 {
            return Err(einval!("chunk count should be greater than 0"));
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(create)
            .create(create)
            .truncate(!persist)
            .open(filename)
            .map_err(|err| {
                einval!(format!(
                    "failed to open/create blob chunk_map file {:?}: {:?}",
                    filename, err
                ))
            })?;

        let file_size = file.metadata()?.len();
        let bitmap_size = div_round_up(chunk_count as u64, 8u64);
        let expected_size = HEADER_SIZE as u64 + bitmap_size;
        let mut new_content = false;

        if file_size == 0 {
            if !create {
                return Err(enoent!());
            }

            new_content = true;
            Self::write_header(&mut file, expected_size)?;
        } else if file_size != expected_size {
            // File size doesn't match, it's too risky to accept the chunk state file. Fallback to
            // always mark chunk data as not ready.
            warn!("blob chunk_map file may be corrupted: {:?}", filename);
            return Err(einval!(format!("chunk_map file {:?} is invalid", filename)));
        }

        let fd = file.as_raw_fd();
        let base = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                expected_size as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if base == libc::MAP_FAILED {
            return Err(last_error!("failed to mmap blob chunk_map"));
        } else if base.is_null() {
            return Err(ebadf!("failed to mmap blob chunk_map"));
        }

        let header = unsafe { &mut *(base as *mut Header) };
        if header.magic != MAGIC1 {
            if !create {
                return Err(enoent!());
            }

            // There's race window between "file.set_len()" and "file.write(&header)". If that
            // happens, all file content should be zero. Detect the race window and write out
            // header again to fix it.
            let content =
                unsafe { std::slice::from_raw_parts(base as *const u8, expected_size as usize) };
            for c in content {
                if *c != 0 {
                    return Err(einval!(format!(
                        "invalid blob chunk_map file header: {:?}",
                        filename
                    )));
                }
            }

            new_content = true;
            Self::write_header(&mut file, expected_size)?;
        }

        let mut not_ready_count = chunk_count;
        if header.version >= 1 {
            if header.magic2 != MAGIC2 {
                return Err(einval!(format!(
                    "invalid blob chunk_map file header: {:?}",
                    filename
                )));
            }
            if header.all_ready == MAGIC_ALL_READY {
                not_ready_count = 0;
            } else if new_content {
                not_ready_count = chunk_count;
            } else {
                let mut ready_count = 0;
                for idx in HEADER_SIZE..expected_size as usize {
                    let current = unsafe { &*(base.add(idx) as *const AtomicU8) };
                    let val = current.load(Ordering::Acquire);
                    ready_count += val.count_ones() as u32;
                }

                if ready_count >= chunk_count {
                    header.all_ready = MAGIC_ALL_READY;
                    let _ = file.sync_all();
                    not_ready_count = 0;
                } else {
                    not_ready_count = chunk_count - ready_count;
                }
            }
        }

        readahead(fd, 0, expected_size);
        if !persist {
            let _ = std::fs::remove_file(filename);
        }

        Ok(Self {
            count: chunk_count,
            size: expected_size as usize,
            base: base as *const u8,
            not_ready_count: AtomicU32::new(not_ready_count),
        })
    }

    fn write_header(file: &mut File, size: u64) -> Result<()> {
        let header = Header {
            magic: MAGIC1,
            version: 1,
            magic2: MAGIC2,
            all_ready: 0,
            reserved: [0x0u8; HEADER_RESERVED_SIZE],
        };

        // Set file size to expected value and sync to disk.
        file.set_len(size)?;
        file.sync_all()?;
        // write file header and sync to disk.
        file.write_all(header.as_slice())?;
        file.sync_all()?;

        Ok(())
    }

    #[inline]
    pub fn validate_index(&self, idx: u32) -> Result<u32> {
        if idx < self.count {
            Ok(idx)
        } else {
            Err(einval!(format!(
                "chunk index {} exceeds chunk count {}",
                idx, self.count
            )))
        }
    }

    #[inline]
    fn read_u8(&self, idx: u32) -> u8 {
        let start = HEADER_SIZE + (idx as usize >> 3);
        let current = unsafe { &*(self.base.add(start) as *const AtomicU8) };

        current.load(Ordering::Acquire)
    }

    #[inline]
    fn write_u8(&self, idx: u32, current: u8) -> bool {
        let mask = Self::index_to_mask(idx);
        let expected = current | mask;
        let start = HEADER_SIZE + (idx as usize >> 3);
        let atomic_value = unsafe { &*(self.base.add(start) as *const AtomicU8) };

        atomic_value
            .compare_exchange(current, expected, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }

    #[inline]
    fn index_to_mask(index: u32) -> u8 {
        let pos = 8 - ((index & 0b111) + 1);
        1 << pos
    }

    #[inline]
    pub fn is_chunk_ready(&self, index: u32) -> (bool, u8) {
        let mask = Self::index_to_mask(index);
        let current = self.read_u8(index);
        let ready = current & mask == mask;

        (ready, current)
    }

    pub fn set_chunk_ready(&self, index: u32) -> Result<()> {
        let index = self.validate_index(index)?;

        // Loop to atomically update the state bit corresponding to the chunk index.
        loop {
            let (ready, current) = self.is_chunk_ready(index);
            if ready {
                break;
            }

            if self.write_u8(index, current) {
                if self.not_ready_count.fetch_sub(1, Ordering::AcqRel) == 1 {
                    self.mark_all_ready();
                }
                break;
            }
        }

        Ok(())
    }

    fn mark_all_ready(&self) {
        let base = self.base as *const c_void as *mut c_void;
        unsafe {
            if libc::msync(base, self.size, libc::MS_SYNC) == 0 {
                let header = &mut *(self.base as *mut Header);
                header.all_ready = MAGIC_ALL_READY;
                let _ = libc::msync(base, HEADER_SIZE, libc::MS_SYNC);
            }
        }
    }

    #[inline]
    pub fn is_range_all_ready(&self) -> bool {
        self.not_ready_count.load(Ordering::Acquire) == 0
    }
}

impl Drop for PersistMap {
    fn drop(&mut self) {
        if !self.base.is_null() {
            unsafe { libc::munmap(self.base as *mut libc::c_void, self.size) };
            self.base = std::ptr::null();
        }
    }
}

unsafe impl Send for PersistMap {}

unsafe impl Sync for PersistMap {}
