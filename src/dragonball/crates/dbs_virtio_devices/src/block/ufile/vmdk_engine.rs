// Copyright © 2026, Microsoft Corporation
// Copyright 2026 Ant Group.
//
// SPDX-License-Identifier: Apache-2.0
//
// Ported from cloud-hypervisor's block/src/formats/vmdk/engine_sync.rs.
// Adapted for dragonball's synchronous read-only VMDK backend.

//! Synchronous VMDK I/O engine.
//!
//! Translates virtual disk offsets into extent file offsets and performs
//! reads, correctly handling requests that span extent boundaries.

use std::io;
use std::os::unix::fs::FileExt;

use super::vmdk_flat::VmdkExtent;

/// Synchronous read-only I/O engine for flat VMDK images.
pub(crate) struct FlatVmdkSync {
    /// Extent list (shared, read-only after construction).
    extents: Vec<VmdkExtent>,
    /// Total virtual disk size in bytes.
    size: u64,
}

impl FlatVmdkSync {
    /// Create a new I/O engine.
    pub fn new(extents: Vec<VmdkExtent>, size: u64) -> Self {
        Self { extents, size }
    }

    /// Total virtual disk size.
    #[allow(dead_code, reason = "public API for external callers")]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Read from the virtual disk at `offset`, filling `buf`.
    ///
    /// Returns the number of bytes read (may be less than buf.len() if the
    /// read would go past the end of the virtual disk, or 0 if the offset
    /// is at/past the end).
    pub fn read(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        if offset >= self.size || buf.is_empty() {
            return Ok(0);
        }

        let end = std::cmp::min(offset + buf.len() as u64, self.size);
        let total = (end - offset) as usize;

        // Find the starting extent.
        let start_extent_idx = match self.extent_at(offset) {
            Some(idx) => idx,
            None => return Ok(0),
        };

        // Check if the entire read fits within a single extent.
        let start_extent = &self.extents[start_extent_idx];
        let extent_end = start_extent.virtual_start + start_extent.length;

        if end <= extent_end {
            // Fast path: single extent.
            self.single_extent_read(start_extent, buf, offset, total)
        } else {
            // Slow path: spans multiple extents.
            self.spanning_read(buf, offset, total, start_extent_idx)
        }
    }

    /// Find which extent contains the given virtual offset.
    /// Returns None if the offset is out of bounds.
    fn extent_at(&self, offset: u64) -> Option<usize> {
        if offset >= self.size {
            return None;
        }

        // Binary search for the extent containing `offset`.
        // An extent contains offset when virtual_start <= offset < virtual_start + length.
        let result = self.extents.binary_search_by(|extent| {
            if offset < extent.virtual_start {
                std::cmp::Ordering::Greater // extent is too far right, search left
            } else if offset >= extent.virtual_start + extent.length {
                std::cmp::Ordering::Less // extent is too far left, search right
            } else {
                std::cmp::Ordering::Equal // found it
            }
        });

        result.ok()
    }

    /// Fast path: entire read fits in a single extent.
    fn single_extent_read(
        &self,
        extent: &VmdkExtent,
        buf: &mut [u8],
        offset: u64,
        total: usize,
    ) -> io::Result<usize> {
        let file_offset = offset - extent.virtual_start + extent.file_base_offset;

        match &extent.file {
            Some(file) => {
                file.read_at(&mut buf[..total], file_offset)?;
                Ok(total)
            }
            None => {
                // NOACCESS extent: return zeroes.
                buf[..total].fill(0);
                Ok(total)
            }
        }
    }

    /// Slow path: read spans multiple extents.
    fn spanning_read(
        &self,
        buf: &mut [u8],
        offset: u64,
        total: usize,
        start_extent_idx: usize,
    ) -> io::Result<usize> {
        let mut bytes_read: usize = 0;
        let mut remaining = total;
        let mut cur_offset = offset;
        let mut ext_idx = start_extent_idx;

        while remaining > 0 && ext_idx < self.extents.len() {
            let extent = &self.extents[ext_idx];
            let extent_end = extent.virtual_start + extent.length;

            // How many bytes can we read from this extent?
            let seg_len = std::cmp::min(remaining as u64, extent_end - cur_offset) as usize;
            if seg_len == 0 {
                ext_idx += 1;
                continue;
            }

            let file_offset = cur_offset - extent.virtual_start + extent.file_base_offset;

            match &extent.file {
                Some(file) => {
                    file.read_at(&mut buf[bytes_read..bytes_read + seg_len], file_offset)?;
                }
                None => {
                    // NOACCESS extent: zero-fill.
                    buf[bytes_read..bytes_read + seg_len].fill(0);
                }
            }

            bytes_read += seg_len;
            remaining -= seg_len;
            cur_offset += seg_len as u64;
            ext_idx += 1;
        }

        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use vmm_sys_util::tempfile::TempFile;

    fn make_file(content: &[u8]) -> TempFile {
        let tf = TempFile::new().unwrap();
        tf.as_file().write_all(content).unwrap();
        tf
    }

    /// Build a FlatVmdkSync from a list of (content, size_bytes) pairs.
    fn build_sync(extent_specs: &[(&[u8], u64)]) -> FlatVmdkSync {
        let mut extents = Vec::new();
        let mut virtual_start: u64 = 0;
        for &(content, size) in extent_specs {
            let tf = make_file(content);
            let file = std::fs::File::open(tf.as_path()).unwrap();
            // Leak the TempFile to keep the fd alive.
            std::mem::forget(tf);
            extents.push(VmdkExtent {
                file: Some(file),
                access: super::super::vmdk_descriptor::ExtentAccess::ReadOnly,
                virtual_start,
                length: size,
                file_base_offset: 0,
            });
            virtual_start += size;
        }
        FlatVmdkSync::new(extents, virtual_start)
    }

    #[test]
    fn test_extent_at_single() {
        let sync = build_sync(&[(b"hello", 4096)]);
        assert_eq!(sync.extent_at(0), Some(0));
        assert_eq!(sync.extent_at(100), Some(0));
        assert_eq!(sync.extent_at(4095), Some(0));
        assert_eq!(sync.extent_at(4096), None);
    }

    #[test]
    fn test_extent_at_multi() {
        let sync = build_sync(&[(b"a", 4096), (b"b", 4096), (b"c", 4096)]);
        assert_eq!(sync.extent_at(0), Some(0));
        assert_eq!(sync.extent_at(4095), Some(0));
        assert_eq!(sync.extent_at(4096), Some(1)); // boundary
        assert_eq!(sync.extent_at(8191), Some(1));
        assert_eq!(sync.extent_at(8192), Some(2));
        assert_eq!(sync.extent_at(12287), Some(2));
        assert_eq!(sync.extent_at(12288), None);
    }

    #[test]
    fn test_single_extent_read() {
        let mut body = vec![0u8; 4096];
        body[0] = 0xA0;
        body[4095] = 0xA1;
        let sync = build_sync(&[(&body, 4096)]);

        let mut buf = vec![0u8; 4096];
        let n = sync.read(&mut buf, 0).unwrap();
        assert_eq!(n, 4096);
        assert_eq!(buf[0], 0xA0);
        assert_eq!(buf[4095], 0xA1);
    }

    #[test]
    fn test_read_partial_past_end() {
        let sync = build_sync(&[(b"hello", 4096)]);
        let mut buf = vec![0u8; 4096];
        let n = sync.read(&mut buf, 4000).unwrap();
        assert_eq!(n, 96); // 4096 - 4000
    }

    #[test]
    fn test_spanning_read_two_extents() {
        let ext_size: u64 = 4096;
        let mut e0 = vec![0u8; ext_size as usize];
        let mut e1 = vec![0u8; ext_size as usize];
        e0[0] = 0xAA;
        e0[(ext_size - 1) as usize] = 0xAB;
        e1[0] = 0xBA;
        e1[(ext_size - 1) as usize] = 0xBB;

        let sync = build_sync(&[(&e0, ext_size), (&e1, ext_size)]);

        let mut buf = vec![0u8; (2 * ext_size) as usize];
        let n = sync.read(&mut buf, 0).unwrap();
        assert_eq!(n, 2 * ext_size as usize);
        assert_eq!(buf[0], 0xAA);
        assert_eq!(buf[(ext_size - 1) as usize], 0xAB);
        assert_eq!(buf[ext_size as usize], 0xBA);
        assert_eq!(buf[(2 * ext_size - 1) as usize], 0xBB);
    }

    #[test]
    fn test_spanning_read_three_extents() {
        // This is the crucial test that would FAIL with imago 0.2.3.
        let ext_size: u64 = 4096;
        let mut e0 = vec![0u8; ext_size as usize];
        let mut e1 = vec![0u8; ext_size as usize];
        let mut e2 = vec![0u8; ext_size as usize];
        e0[0] = 0x11;
        e0[(ext_size - 1) as usize] = 0xAA;
        e1[0] = 0x22;
        e1[(ext_size - 1) as usize] = 0xBB;
        e2[0] = 0x33;
        e2[(ext_size - 1) as usize] = 0xCC;

        let sync = build_sync(&[(&e0, ext_size), (&e1, ext_size), (&e2, ext_size)]);

        let mut buf = vec![0u8; (3 * ext_size) as usize];
        let n = sync.read(&mut buf, 0).unwrap();
        assert_eq!(n, 3 * ext_size as usize);

        // Verify sentinels across all boundaries.
        assert_eq!(buf[0], 0x11, "extent 0 first byte");
        assert_eq!(buf[(ext_size - 1) as usize], 0xAA, "extent 0 last byte");
        assert_eq!(buf[ext_size as usize], 0x22, "extent 1 first byte");
        assert_eq!(buf[(2 * ext_size - 1) as usize], 0xBB, "extent 1 last byte");
        assert_eq!(buf[(2 * ext_size) as usize], 0x33, "extent 2 first byte");
        assert_eq!(buf[(3 * ext_size - 1) as usize], 0xCC, "extent 2 last byte");
    }

    #[test]
    fn test_spanning_read_cross_boundary_partial() {
        let ext_size: u64 = 4096;
        let mut e0 = vec![0u8; ext_size as usize];
        let mut e1 = vec![0u8; ext_size as usize];
        let mut e2 = vec![0u8; ext_size as usize];
        e0[0] = 0xE0;
        e1[0] = 0xE1;
        e2[0] = 0xE2;

        let sync = build_sync(&[(&e0, ext_size), (&e1, ext_size), (&e2, ext_size)]);

        // Read from middle of extent 1, spanning into extent 2.
        let half = (ext_size / 2) as usize;
        let len = ext_size as usize; // from extent 1 mid to extent 2 mid
        let mut buf = vec![0xFFu8; len];
        let n = sync.read(&mut buf, ext_size + half as u64).unwrap();
        assert_eq!(n, len);

        // First half of buf is from extent 1 (second half)
        assert_eq!(buf[half], 0xE2, "extent 2 marker at boundary");
        // First byte of extent 2
        assert_eq!(
            buf[ext_size as usize - half],
            0xE2,
            "extent 2 marker at start"
        );
    }

    #[test]
    fn test_read_past_end_returns_empty() {
        let sync = build_sync(&[(b"hello", 4096)]);
        let mut buf = vec![0u8; 100];
        let n = sync.read(&mut buf, 4096).unwrap();
        assert_eq!(n, 0);
    }
}
