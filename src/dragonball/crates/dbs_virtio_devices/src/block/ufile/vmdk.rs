// Copyright 2026 Ant Group.
//
// SPDX-License-Identifier: Apache-2.0
//
// Ported from the snapshot-dev imago-based implementation.
// Replaced imago dependency with a self-contained flat VMDK implementation
// derived from cloud-hypervisor's VMDK support.

//! Read-only VMDK backend for virtio-blk.
//!
//! Uses a self-contained flat VMDK implementation (no imago dependency).
//! This backend slots into the existing `Ufile` async-shaped API: each guest
//! request is served *synchronously* inside `io_read_submit`, the completion
//! status is pushed into an internal queue, and an internal `EventFd` is poked
//! so the IO thread's epoll loop wakes up and drains via `io_complete`.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::linux::fs::MetadataExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;
use std::sync::Mutex;

use log::warn;
use virtio_bindings::bindings::virtio_blk::{VIRTIO_BLK_S_IOERR, VIRTIO_BLK_S_OK};
use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};

use super::vmdk_engine::FlatVmdkSync;
use super::vmdk_flat::FlatVmdk;
use super::{IoDataDesc, Ufile};

/// A read-only VMDK-backed `Ufile`.
pub struct VmdkFile {
    /// Synchronous accessor (handles multi-extent translation).
    access: FlatVmdkSync,
    /// Logical (virtual) disk size in bytes.
    capacity: u64,
    /// Cursor for the `Read + Seek` fallback path used by `Request::execute`.
    cursor: u64,
    /// Stable device id derived from the descriptor file's inode triple.
    device_id: String,
    /// Eventfd used as the doorbell. Written once per completed submit; the IO
    /// thread's epoll loop polls it under `END_IO_EVENT`.
    complete_evtfd: EventFd,
    /// Completion queue drained by `io_complete`. `(user_data, status)` pairs.
    pending: Mutex<VecDeque<(u16, u32)>>,
}

// SAFETY: FlatVmdkSync contains only Vec<VmdkExtent> and u64 (both Send).
// The underlying std::fs::File handles within VmdkExtent are accessed from a
// single IO thread. The Ufile is owned exclusively by a single virtio-blk IO
// thread for its entire lifetime.
unsafe impl Send for VmdkFile {}

impl std::fmt::Debug for VmdkFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VmdkFile")
            .field("capacity", &self.capacity)
            .field("cursor", &self.cursor)
            .field("device_id", &self.device_id)
            .finish_non_exhaustive()
    }
}

impl VmdkFile {
    /// Open a VMDK image from the given file handle and path.
    ///
    /// Only flat VMDK (monolithicFlat, twoGbMaxExtentFlat) is supported.
    /// The image is always opened read-only.
    pub fn new(file: File, path: &Path) -> io::Result<Self> {
        let device_id = {
            let m = file.metadata()?;
            format!("{}{}{}", m.st_dev(), m.st_rdev(), m.st_ino())
        };

        // Open the flat VMDK using our custom implementation.
        let mut file_clone = file
            .try_clone()
            .map_err(|e| io::Error::other(format!("vmdk: failed to clone descriptor file: {e}")))?;

        let flat = FlatVmdk::new(&mut file_clone, path, true /* readonly */)
            .map_err(|e| io::Error::other(format!("vmdk open: {e}")))?;

        let capacity = flat.size;
        let access = FlatVmdkSync::new(flat.extents, flat.size);

        let complete_evtfd = EventFd::new(EFD_NONBLOCK)?;

        Ok(Self {
            access,
            capacity,
            cursor: 0,
            device_id,
            complete_evtfd,
            pending: Mutex::new(VecDeque::new()),
        })
    }

    /// Push a completion entry and ring the doorbell. If the eventfd write
    /// fails, pop the entry back so the queue does not retain a phantom
    /// completion that the caller already saw as `Err` and will not drain.
    fn enqueue(&self, user_data: u16, status: u32) -> io::Result<()> {
        let mut g = self.pending.lock().unwrap();
        g.push_back((user_data, status));
        if let Err(e) = self.complete_evtfd.write(1) {
            g.pop_back();
            return Err(e);
        }
        Ok(())
    }
}

impl Read for VmdkFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Fallback path used by `Request::execute` if `io_read_submit` returns
        // an error. Honor the std::io::Read contract: clamp to remaining
        // bytes and return the actual length read.
        let remaining = self.capacity.saturating_sub(self.cursor);
        if remaining == 0 || buf.is_empty() {
            return Ok(0);
        }
        let n = std::cmp::min(buf.len() as u64, remaining) as usize;
        let bytes = self.access.read(&mut buf[..n], self.cursor)?;
        self.cursor = self.cursor.saturating_add(bytes as u64);
        Ok(bytes)
    }
}

impl Write for VmdkFile {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "VMDK images are mounted read-only by Dragonball",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        // Flat VMDK has no metadata to flush; reads don't dirty anything.
        Ok(())
    }
}

impl Seek for VmdkFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // CRITICAL: `Block::new` (block/device.rs) calls `seek(End, 0)` on
        // disk_images[0] to derive guest-visible capacity, so this method must
        // return the *virtual* VMDK size, not the descriptor file size.
        let signed_new = match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(n) => (self.capacity as i64)
                .checked_add(n)
                .ok_or_else(|| io::Error::other("vmdk seek end overflow"))?,
            SeekFrom::Current(n) => (self.cursor as i64)
                .checked_add(n)
                .ok_or_else(|| io::Error::other("vmdk seek cur overflow"))?,
        };
        if signed_new < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vmdk seek before start of file",
            ));
        }
        self.cursor = signed_new as u64;
        Ok(self.cursor)
    }
}

impl Ufile for VmdkFile {
    fn get_capacity(&self) -> u64 {
        self.capacity
    }

    fn get_max_size(&self) -> u32 {
        0x10_0000
    }

    fn get_device_id(&self) -> io::Result<String> {
        Ok(self.device_id.clone())
    }

    fn get_data_evt_fd(&self) -> RawFd {
        self.complete_evtfd.as_raw_fd()
    }

    fn io_read_submit(
        &mut self,
        offset: i64,
        iovecs: &mut Vec<IoDataDesc>,
        user_data: u16,
    ) -> io::Result<usize> {
        let mut cur = offset as u64;
        let mut status = VIRTIO_BLK_S_OK;

        for iov in iovecs.iter() {
            // SAFETY: `iov.data_addr` is a host pointer obtained from
            // `GuestMemory::get_host_address(GuestAddress(_))` against the
            // active `GuestMemoryMmap`, which validates the entire range
            // belongs to a mapped region.
            let buf =
                unsafe { std::slice::from_raw_parts_mut(iov.data_addr as *mut u8, iov.data_len) };

            match self.access.read(&mut *buf, cur) {
                Ok(bytes) => {
                    if bytes < iov.data_len {
                        // Partial read: zero-fill the rest.
                        buf[bytes..].fill(0);
                    }
                }
                Err(e) => {
                    warn!(
                        "virtio-blk vmdk: read off=0x{:x} len={} failed: {}",
                        cur, iov.data_len, e
                    );
                    status = VIRTIO_BLK_S_IOERR;
                    break;
                }
            }

            cur = match cur.checked_add(iov.data_len as u64) {
                Some(v) => v,
                None => {
                    warn!(
                        "virtio-blk vmdk: read offset overflow at off=0x{:x} len={}",
                        cur, iov.data_len
                    );
                    status = VIRTIO_BLK_S_IOERR;
                    break;
                }
            };
        }

        self.enqueue(user_data, status)?;
        Ok(1)
    }

    fn io_write_submit(
        &mut self,
        _offset: i64,
        _iovecs: &mut Vec<IoDataDesc>,
        user_data: u16,
    ) -> io::Result<usize> {
        self.enqueue(user_data, VIRTIO_BLK_S_IOERR)?;
        Ok(1)
    }

    fn punch_hole(&mut self, _offset: u64, _length: u64) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "VMDK images are read-only by Dragonball",
        ))
    }

    fn io_complete(&mut self) -> io::Result<Vec<(u16, u32)>> {
        let _ = self.complete_evtfd.read();
        let mut g = self.pending.lock().unwrap();
        Ok(g.drain(..).collect())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use vmm_sys_util::tempfile::TempFile;

    use super::*;

    /// Build an on-disk descriptor + flat extent VMDK pair and return the
    /// (descriptor_file, extent_file) tuple so callers can keep both alive.
    fn create_test_vmdk(content: &[u8], extent_size: u64) -> (TempFile, TempFile) {
        let extent_file = TempFile::new().unwrap();
        extent_file.as_file().set_len(extent_size).unwrap();
        extent_file.as_file().write_all(content).unwrap();

        let extent_path = extent_file.as_path().to_str().unwrap().to_string();
        let extent_sectors = extent_size / 512;

        let descriptor_file = TempFile::new().unwrap();
        let descriptor = format!(
            r#"# Disk DescriptorFile
version=1
CID=fffffffe
parentCID=ffffffff
createType="monolithicFlat"

# Extent description
RW {extent_sectors} FLAT "{extent_path}" 0

# The Disk Data Base
#DDB
"#
        );
        descriptor_file
            .as_file()
            .write_all(descriptor.as_bytes())
            .unwrap();

        (descriptor_file, extent_file)
    }

    #[test]
    fn test_open_and_capacity() {
        let extent_size: u64 = 1024 * 1024;
        let (descriptor, _extent) = create_test_vmdk(b"Hello VMDK from Dragonball!", extent_size);

        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();
        assert_eq!(vmdk.get_capacity(), extent_size);
        assert_eq!(vmdk.get_max_size(), 0x10_0000);
        assert!(vmdk.get_device_id().is_ok());
    }

    #[test]
    fn test_seek_end_returns_virtual_size() {
        let extent_size: u64 = 2 * 1024 * 1024;
        let (descriptor, _extent) = create_test_vmdk(b"abc", extent_size);
        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();
        let pos = vmdk.seek(SeekFrom::End(0)).unwrap();
        assert_eq!(pos, extent_size);
    }

    #[test]
    fn test_sync_read_matches_content() {
        let extent_size: u64 = 1024 * 1024;
        let payload = b"Hello VMDK from Dragonball!";
        let (descriptor, _extent) = create_test_vmdk(payload, extent_size);

        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();
        vmdk.seek(SeekFrom::Start(0)).unwrap();

        let mut buf = vec![0u8; 512];
        let n = vmdk.read(&mut buf).unwrap();
        assert_eq!(n, 512);
        assert_eq!(&buf[..payload.len()], payload);
    }

    #[test]
    fn test_write_returns_permission_denied() {
        let extent_size: u64 = 1024 * 1024;
        let (descriptor, _extent) = create_test_vmdk(b"x", extent_size);
        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();
        let err = vmdk.write(&[0u8; 16]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn test_io_read_submit_writes_into_host_buffer() {
        let extent_size: u64 = 1024 * 1024;
        let payload = b"Hello VMDK from Dragonball!";
        let (descriptor, _extent) = create_test_vmdk(payload, extent_size);
        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();

        let mut buf = vec![0u8; 512];
        let mut iovecs = vec![IoDataDesc {
            data_addr: buf.as_mut_ptr() as u64,
            data_len: buf.len(),
        }];
        let submitted = vmdk.io_read_submit(0, &mut iovecs, 0x42).unwrap();
        assert_eq!(submitted, 1);

        let completions = vmdk.io_complete().unwrap();
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0], (0x42u16, VIRTIO_BLK_S_OK));
        assert_eq!(&buf[..payload.len()], payload);

        let again = vmdk.io_complete().unwrap();
        assert!(again.is_empty());
    }

    #[test]
    fn test_io_write_submit_reports_ioerr() {
        let extent_size: u64 = 1024 * 1024;
        let (descriptor, _extent) = create_test_vmdk(b"x", extent_size);
        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();

        let mut iovecs = vec![IoDataDesc {
            data_addr: 0,
            data_len: 0,
        }];
        let submitted = vmdk.io_write_submit(0, &mut iovecs, 0x77).unwrap();
        assert_eq!(submitted, 1);

        let completions = vmdk.io_complete().unwrap();
        assert_eq!(completions, vec![(0x77u16, VIRTIO_BLK_S_IOERR)]);
    }

    #[test]
    fn test_io_complete_drains_all_pending() {
        let extent_size: u64 = 1024 * 1024;
        let (descriptor, _extent) = create_test_vmdk(b"x", extent_size);
        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();

        let mut buf1 = vec![0u8; 16];
        let mut buf2 = vec![0u8; 16];

        let mut iov1 = vec![IoDataDesc {
            data_addr: buf1.as_mut_ptr() as u64,
            data_len: buf1.len(),
        }];
        let mut iov2 = vec![IoDataDesc {
            data_addr: buf2.as_mut_ptr() as u64,
            data_len: buf2.len(),
        }];

        vmdk.io_read_submit(0, &mut iov1, 1).unwrap();
        vmdk.io_read_submit(0, &mut iov2, 2).unwrap();

        let completions = vmdk.io_complete().unwrap();
        assert_eq!(completions.len(), 2);
        assert_eq!(completions[0].0, 1);
        assert_eq!(completions[1].0, 2);
    }

    // ----- multi-extent VMDK tests (NOW PASSING with custom implementation) -----

    use std::os::unix::fs::FileExt;

    struct ExtentSpec {
        body: Vec<u8>,
        size_bytes: u64,
    }

    fn create_multi_extent_vmdk(extents: &[ExtentSpec]) -> (TempFile, Vec<TempFile>, u64) {
        let mut extent_files: Vec<TempFile> = Vec::with_capacity(extents.len());
        let mut total_sectors: u64 = 0;
        let mut extent_lines = String::new();

        for spec in extents {
            assert_eq!(
                spec.size_bytes % 512,
                0,
                "extent size must be sector-aligned"
            );
            let f = TempFile::new().unwrap();
            f.as_file().set_len(spec.size_bytes).unwrap();
            if !spec.body.is_empty() {
                f.as_file().write_at(&spec.body, 0).unwrap();
                let mut probe = [0u8; 1];
                f.as_file().read_at(&mut probe, 0).unwrap();
                assert_eq!(probe[0], spec.body[0], "backing-file write_at failed");
            }

            let path = f.as_path().to_str().unwrap().to_string();
            let sectors = spec.size_bytes / 512;
            total_sectors += sectors;
            extent_lines.push_str(&format!("RW {sectors} FLAT \"{path}\" 0\n"));
            extent_files.push(f);
        }

        let descriptor_file = TempFile::new().unwrap();
        let descriptor = format!(
            r#"# Disk DescriptorFile
version=1
CID=fffffffe
parentCID=ffffffff
createType="twoGbMaxExtentFlat"

# Extent description
{extent_lines}
# The Disk Data Base
#DDB
ddb.virtualHWVersion = "4"
"#
        );
        descriptor_file
            .as_file()
            .write_at(descriptor.as_bytes(), 0)
            .unwrap();

        (descriptor_file, extent_files, total_sectors * 512)
    }

    #[test]
    fn test_two_extent_read_spans_boundary() {
        let ext_size: u64 = 4096;
        let mut e0 = vec![0u8; ext_size as usize];
        let mut e1 = vec![0u8; ext_size as usize];
        e0[0] = 0xA0;
        e0[(ext_size - 1) as usize] = 0xA1;
        e1[0] = 0xB0;
        e1[(ext_size - 1) as usize] = 0xB1;

        let extents = vec![
            ExtentSpec {
                body: e0,
                size_bytes: ext_size,
            },
            ExtentSpec {
                body: e1,
                size_bytes: ext_size,
            },
        ];
        let (descriptor, _extents, total) = create_multi_extent_vmdk(&extents);
        assert_eq!(total, 2 * ext_size);

        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();
        assert_eq!(vmdk.get_capacity(), total);

        let mut buf = vec![0u8; total as usize];
        let mut iovecs = vec![IoDataDesc {
            data_addr: buf.as_mut_ptr() as u64,
            data_len: buf.len(),
        }];
        vmdk.io_read_submit(0, &mut iovecs, 0xAB).unwrap();
        assert_eq!(
            vmdk.io_complete().unwrap(),
            vec![(0xABu16, VIRTIO_BLK_S_OK)]
        );
        assert_eq!(buf[0], 0xA0, "extent 0 first byte");
        assert_eq!(buf[(ext_size - 1) as usize], 0xA1, "extent 0 last byte");
        assert_eq!(buf[ext_size as usize], 0xB0, "extent 1 first byte");
        assert_eq!(buf[(2 * ext_size - 1) as usize], 0xB1, "extent 1 last byte");
    }

    #[test]
    fn test_two_extent_read_starts_at_boundary() {
        let ext_size: u64 = 4096;
        let mut e0 = vec![0u8; ext_size as usize];
        let mut e1 = vec![0u8; ext_size as usize];
        e0[0] = 0xC0;
        e1[0] = 0xC1;

        let extents = vec![
            ExtentSpec {
                body: e0,
                size_bytes: ext_size,
            },
            ExtentSpec {
                body: e1,
                size_bytes: ext_size,
            },
        ];
        let (descriptor, _extents, _total) = create_multi_extent_vmdk(&extents);
        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();

        let mut buf = vec![0xFFu8; ext_size as usize];
        let mut iovecs = vec![IoDataDesc {
            data_addr: buf.as_mut_ptr() as u64,
            data_len: buf.len(),
        }];
        vmdk.io_read_submit(ext_size as i64, &mut iovecs, 0xCD)
            .unwrap();
        assert_eq!(
            vmdk.io_complete().unwrap(),
            vec![(0xCDu16, VIRTIO_BLK_S_OK)]
        );
        assert_eq!(buf[0], 0xC1, "extent 1 marker at offset == boundary");
    }

    #[test]
    fn test_multi_extent_capacity_sums() {
        let extents = vec![
            ExtentSpec {
                body: b"first".to_vec(),
                size_bytes: 64 * 1024,
            },
            ExtentSpec {
                body: b"second".to_vec(),
                size_bytes: 256 * 1024,
            },
            ExtentSpec {
                body: b"third".to_vec(),
                size_bytes: 128 * 1024,
            },
        ];
        let (descriptor, _extents, total) = create_multi_extent_vmdk(&extents);
        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();
        assert_eq!(vmdk.get_capacity(), total);
        assert_eq!(total, 448 * 1024);
    }

    #[test]
    fn test_multi_extent_read_spans_boundaries() {
        let ext_size: u64 = 4096;
        let mut e0 = vec![0u8; ext_size as usize];
        let mut e1 = vec![0u8; ext_size as usize];
        let mut e2 = vec![0u8; ext_size as usize];
        e0[(ext_size - 1) as usize] = 0xAA;
        e1[(ext_size - 1) as usize] = 0xBB;
        e2[(ext_size - 1) as usize] = 0xCC;
        e0[0] = 0x11;
        e1[0] = 0x22;
        e2[0] = 0x33;

        let extents = vec![
            ExtentSpec {
                body: e0,
                size_bytes: ext_size,
            },
            ExtentSpec {
                body: e1,
                size_bytes: ext_size,
            },
            ExtentSpec {
                body: e2,
                size_bytes: ext_size,
            },
        ];
        let (descriptor, _extents, total) = create_multi_extent_vmdk(&extents);
        assert_eq!(total, 3 * ext_size);

        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();
        assert_eq!(vmdk.get_capacity(), total);

        let mut buf = vec![0u8; total as usize];
        let mut iovecs = vec![IoDataDesc {
            data_addr: buf.as_mut_ptr() as u64,
            data_len: buf.len(),
        }];
        let submitted = vmdk.io_read_submit(0, &mut iovecs, 0xABCD).unwrap();
        assert_eq!(submitted, 1);
        let completions = vmdk.io_complete().unwrap();
        assert_eq!(completions, vec![(0xABCDu16, VIRTIO_BLK_S_OK)]);

        assert_eq!(buf[0], 0x11, "first byte of extent 0");
        assert_eq!(buf[(ext_size - 1) as usize], 0xAA, "last byte of extent 0");
        assert_eq!(buf[ext_size as usize], 0x22, "first byte of extent 1");
        assert_eq!(
            buf[(2 * ext_size - 1) as usize],
            0xBB,
            "last byte of extent 1"
        );
        assert_eq!(buf[(2 * ext_size) as usize], 0x33, "first byte of extent 2");
        assert_eq!(
            buf[(3 * ext_size - 1) as usize],
            0xCC,
            "last byte of extent 2"
        );
    }

    #[test]
    fn test_multi_extent_read_offset_into_second_extent() {
        let ext_size: u64 = 4096;
        let mut e0 = vec![0u8; ext_size as usize];
        let mut e1 = vec![0u8; ext_size as usize];
        let mut e2 = vec![0u8; ext_size as usize];
        e0[0] = 0xE0;
        e1[0] = 0xE1;
        e2[0] = 0xE2;

        let extents = vec![
            ExtentSpec {
                body: e0,
                size_bytes: ext_size,
            },
            ExtentSpec {
                body: e1,
                size_bytes: ext_size,
            },
            ExtentSpec {
                body: e2,
                size_bytes: ext_size,
            },
        ];
        let (descriptor, _extents, _total) = create_multi_extent_vmdk(&extents);
        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();

        let mut buf = vec![0xFFu8; (2 * ext_size) as usize];
        let mut iovecs = vec![IoDataDesc {
            data_addr: buf.as_mut_ptr() as u64,
            data_len: buf.len(),
        }];
        vmdk.io_read_submit(ext_size as i64, &mut iovecs, 1)
            .unwrap();
        let completions = vmdk.io_complete().unwrap();
        assert_eq!(completions, vec![(1u16, VIRTIO_BLK_S_OK)]);
        assert_eq!(buf[0], 0xE1, "extent 1 marker at relative offset 0");
        assert_eq!(
            buf[ext_size as usize], 0xE2,
            "extent 2 marker relative ext_size"
        );
    }

    #[test]
    fn test_gpt_shape_head_pad_layer_capacity() {
        let head_size: u64 = 1 * 1024 * 1024;
        let pad_size: u64 = 512 * 1024;
        let layer_size: u64 = 4 * 1024 * 1024;

        let mut head_body = vec![0u8; head_size as usize];
        head_body[512..512 + 8].copy_from_slice(b"EFI PART");

        let extents = vec![
            ExtentSpec {
                body: head_body,
                size_bytes: head_size,
            },
            ExtentSpec {
                body: vec![],
                size_bytes: pad_size,
            },
            ExtentSpec {
                body: vec![0x77u8; 16],
                size_bytes: layer_size,
            },
        ];
        let (descriptor, _extents, total) = create_multi_extent_vmdk(&extents);
        assert_eq!(total, head_size + pad_size + layer_size);

        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();
        assert_eq!(vmdk.get_capacity(), total);
    }

    #[test]
    fn test_gpt_shape_read_crosses_pad_into_layer() {
        let head_size: u64 = 1024 * 1024;
        let pad_size: u64 = 4096;
        let layer_size: u64 = 64 * 1024;

        let mut layer_body = vec![0u8; layer_size as usize];
        layer_body[0] = 0x9A;
        layer_body[1] = 0x9B;

        let extents = vec![
            ExtentSpec {
                body: vec![],
                size_bytes: head_size,
            },
            ExtentSpec {
                body: vec![],
                size_bytes: pad_size,
            },
            ExtentSpec {
                body: layer_body,
                size_bytes: layer_size,
            },
        ];
        let (descriptor, _extents, _total) = create_multi_extent_vmdk(&extents);
        let file = std::fs::File::open(descriptor.as_path()).unwrap();
        let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();

        let read_off = (head_size + pad_size / 2) as i64;
        let mut buf = vec![0xFFu8; 8 * 1024];
        let mut iovecs = vec![IoDataDesc {
            data_addr: buf.as_mut_ptr() as u64,
            data_len: buf.len(),
        }];
        vmdk.io_read_submit(read_off, &mut iovecs, 7).unwrap();
        let completions = vmdk.io_complete().unwrap();
        assert_eq!(completions, vec![(7u16, VIRTIO_BLK_S_OK)]);

        let pad_tail_len = (pad_size / 2) as usize;
        assert!(
            buf[..pad_tail_len].iter().all(|b| *b == 0),
            "pad tail should be zero"
        );
        assert_eq!(buf[pad_tail_len], 0x9A, "layer marker[0] at boundary");
        assert_eq!(buf[pad_tail_len + 1], 0x9B, "layer marker[1] past boundary");
    }
    /// E2E tests for realistic Kata workloads using the multi-extent helpers.
    mod e2e {
        use super::*;
        use std::os::unix::fs::FileExt;

        /// Kata EROFS fsmerge mode: 2-layer image.
        #[test]
        fn e2e_fsmerge_two_layer() {
            let ext_size: u64 = 64 * 1024;
            let mut e0 = vec![0u8; ext_size as usize];
            let mut e1 = vec![0u8; ext_size as usize];
            e0[0..4].copy_from_slice(b"\xe2\xe1\xf5\xd6"); // EROFS magic
            e0[1024] = 0x42;
            e1[0..4].copy_from_slice(b"\xe2\xe1\xf5\xd6");
            e1[2048] = 0x43;

            let extents = vec![
                ExtentSpec {
                    body: e0,
                    size_bytes: ext_size,
                },
                ExtentSpec {
                    body: e1,
                    size_bytes: ext_size,
                },
            ];
            let (descriptor, _extents, total) = create_multi_extent_vmdk(&extents);
            assert_eq!(total, 2 * ext_size);

            let file = std::fs::File::open(descriptor.as_path()).unwrap();
            let mut vmdk = VmdkFile::new(file, descriptor.as_path()).unwrap();
            assert_eq!(vmdk.get_capacity(), total);

            let mut buf = vec![0u8; 4096];
            vmdk.seek(SeekFrom::Start(0)).unwrap();
            let n = vmdk.read(&mut buf).unwrap();
            assert_eq!(n, 4096);
            assert_eq!(&buf[0..4], b"\xe2\xe1\xf5\xd6");
            assert_eq!(buf[1024], 0x42);

            // Read across layer boundary
            let cross_off = ext_size - 2048;
            let mut buf = vec![0u8; 4096];
            vmdk.seek(SeekFrom::Start(cross_off)).unwrap();
            let n = vmdk.read(&mut buf).unwrap();
            assert_eq!(n, 4096);
            assert_eq!(
                &buf[2048..2052],
                b"\xe2\xe1\xf5\xd6",
                "layer1 EROFS magic at boundary"
            );
        }

        /// Kata EROFS GPT mode: head + pad + layer.
        #[test]
        fn e2e_gpt_mode_head_pad_layer() {
            let head_size: u64 = 1024 * 1024;
            let pad_size: u64 = 4096;
            let layer_size: u64 = 128 * 1024;

            let mut head_body = vec![0u8; head_size as usize];
            head_body[510] = 0x55;
            head_body[511] = 0xAA;
            head_body[512..520].copy_from_slice(b"EFI PART");

            let mut layer_body = vec![0u8; layer_size as usize];
            layer_body[0..4].copy_from_slice(b"\xe2\xe1\xf5\xd6");
            layer_body[128] = 0x7F;

            let extents = vec![
                ExtentSpec {
                    body: head_body,
                    size_bytes: head_size,
                },
                ExtentSpec {
                    body: vec![],
                    size_bytes: pad_size,
                },
                ExtentSpec {
                    body: layer_body,
                    size_bytes: layer_size,
                },
            ];
            let (descriptor, _extents, total) = create_multi_extent_vmdk(&extents);
            assert_eq!(total, head_size + pad_size + layer_size);

            let mut vmdk = VmdkFile::new(
                std::fs::File::open(descriptor.as_path()).unwrap(),
                descriptor.as_path(),
            )
            .unwrap();
            assert_eq!(vmdk.get_capacity(), total);

            // Verify GPT header
            let mut buf = vec![0u8; 1024];
            vmdk.seek(SeekFrom::Start(0)).unwrap();
            vmdk.read(&mut buf).unwrap();
            assert_eq!(buf[510], 0x55);
            assert_eq!(buf[511], 0xAA);
            assert_eq!(&buf[512..520], b"EFI PART");

            // Read across pad->layer boundary
            let read_start = head_size + pad_size / 2;
            let mut buf = vec![0u8; 8192];
            vmdk.seek(SeekFrom::Start(read_start)).unwrap();
            let n = vmdk.read(&mut buf).unwrap();
            assert_eq!(n, 8192);

            let pad_half = (pad_size / 2) as usize;
            assert!(
                buf[..pad_half].iter().all(|b| *b == 0),
                "pad region must be zero"
            );
            assert_eq!(
                &buf[pad_half..pad_half + 4],
                b"\xe2\xe1\xf5\xd6",
                "EROFS magic at pad->layer boundary"
            );
            assert_eq!(buf[pad_half + 128], 0x7F, "layer feature flags");
        }

        /// Large-scale read across 5 extents.
        #[test]
        fn e2e_five_extent_large_read() {
            let ext_size: u64 = 64 * 1024;
            let num_extents = 5;
            let mut specs = Vec::new();
            for i in 0..num_extents {
                let mut body = vec![0u8; ext_size as usize];
                body[0] = 0xA0 + i as u8;
                body[(ext_size - 1) as usize] = 0xF0 + i as u8;
                specs.push(ExtentSpec {
                    body,
                    size_bytes: ext_size,
                });
            }

            let (descriptor, _extents, total) = create_multi_extent_vmdk(&specs);
            let mut vmdk = VmdkFile::new(
                std::fs::File::open(descriptor.as_path()).unwrap(),
                descriptor.as_path(),
            )
            .unwrap();
            assert_eq!(vmdk.get_capacity(), total);

            let total_usize = total as usize;
            let mut buf = vec![0u8; total_usize];
            vmdk.seek(SeekFrom::Start(0)).unwrap();
            let n = vmdk.read(&mut buf).unwrap();
            assert_eq!(n, total_usize);

            for i in 0..num_extents {
                let base = i as usize * ext_size as usize;
                assert_eq!(buf[base], 0xA0 + i as u8, "extent start marker");
                assert_eq!(
                    buf[base + ext_size as usize - 1],
                    0xF0 + i as u8,
                    "extent end marker"
                );
            }
        }

        /// Read truncated at end of disk.
        #[test]
        fn e2e_read_past_end_truncates() {
            let ext_size: u64 = 4096;
            let (descriptor, _extents, _total) = create_multi_extent_vmdk(&[ExtentSpec {
                body: vec![0x42u8; ext_size as usize],
                size_bytes: ext_size,
            }]);

            let mut vmdk = VmdkFile::new(
                std::fs::File::open(descriptor.as_path()).unwrap(),
                descriptor.as_path(),
            )
            .unwrap();

            let mut buf = vec![0u8; 512];
            vmdk.seek(SeekFrom::Start(4000)).unwrap();
            let n = vmdk.read(&mut buf).unwrap();
            assert_eq!(n, 96);
            assert!(buf.iter().take(96).all(|b| *b == 0x42));

            let mut buf = vec![0u8; 128];
            vmdk.seek(SeekFrom::Start(ext_size)).unwrap();
            let n = vmdk.read(&mut buf).unwrap();
            assert_eq!(n, 0);
        }

        /// Stress test: many small reads across extent boundaries.
        #[test]
        fn e2e_many_boundary_crossing_reads() {
            let ext_count = 6;
            let ext_size: u64 = 4096;
            let read_size: usize = 1024;

            let mut specs = Vec::new();
            for i in 0..ext_count {
                let mut body = vec![(i as u8).wrapping_mul(17); ext_size as usize];
                body[0] = 0x80 | i as u8;
                specs.push(ExtentSpec {
                    body,
                    size_bytes: ext_size,
                });
            }

            let (descriptor, _extents, total) = create_multi_extent_vmdk(&specs);
            let mut vmdk = VmdkFile::new(
                std::fs::File::open(descriptor.as_path()).unwrap(),
                descriptor.as_path(),
            )
            .unwrap();
            assert_eq!(vmdk.get_capacity(), total);

            for step in 0..50 {
                let max_off = total - read_size as u64;
                let offset = (step as u64 * 512) % max_off;
                let mut buf = vec![0xFFu8; read_size];
                vmdk.seek(SeekFrom::Start(offset)).unwrap();
                let n = vmdk.read(&mut buf).unwrap();
                assert_eq!(n, read_size, "step read at offset");
                let first_byte_is_populated = buf[0] != 0xFF;
                assert!(first_byte_is_populated, "buf should not be 0xFF");
            }
        }
    }
}
