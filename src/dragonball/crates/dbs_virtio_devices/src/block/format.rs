// Copyright 2026 Ant Group.
//
// SPDX-License-Identifier: Apache-2.0

//! Disk image format detection for virtio-blk backends.
//!
//! Today only Raw and VMDK are recognized; everything that does not match the
//! VMDK heuristics is treated as Raw. The detector is *not* feature-gated:
//! `virtio-blk-vmdk` controls whether the runtime can *open* a VMDK image,
//! but the detector must always be able to recognize one so the backend
//! factory can return `Unsupported` instead of silently treating VMDK bytes
//! as raw sectors and feeding garbage to the guest.

use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;

use serde::{Deserialize, Serialize};

/// VMDK4 sparse file magic: bytes `K`, `D`, `M`, `V` at offset 0
/// (little-endian representation of `0x564d_444b`).
const VMDK_SPARSE_MAGIC: [u8; 4] = [0x4b, 0x44, 0x4d, 0x56];

/// First line of every flat VMDK descriptor file.
const VMDK_DESCRIPTOR_SIGNATURE: &[u8] = b"# Disk DescriptorFile";

/// The detected disk image format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiskImageFormat {
    /// Raw disk image (no recognized format header).
    Raw,
    /// VMDK disk image (sparse or descriptor-based).
    Vmdk,
}

/// Detect the format of a disk image by inspecting its header bytes.
/// Peek the first 512 bytes of `file` and apply two orthogonal checks:
/// 1. **VMDK4 sparse**: the first four bytes match `KDMV` (`0x564d444b`).
/// 2. **VMDK descriptor**: the file starts with `# Disk DescriptorFile`.
pub fn detect_disk_format(file: &File) -> io::Result<DiskImageFormat> {
    let mut buf = [0u8; 512];
    let len = file.read_at(&mut buf, 0)?;

    if len < 4 {
        return Ok(DiskImageFormat::Raw);
    }

    // Check VMDK4 sparse magic at offset 0.
    if buf[..4] == VMDK_SPARSE_MAGIC {
        return Ok(DiskImageFormat::Vmdk);
    }

    // Check for flat VMDK descriptor signature at offset 0.
    if buf.starts_with(VMDK_DESCRIPTOR_SIGNATURE) {
        return Ok(DiskImageFormat::Vmdk);
    }

    Ok(DiskImageFormat::Raw)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use vmm_sys_util::tempfile::TempFile;

    use super::*;

    #[test]
    fn test_detect_raw_format_empty_file() {
        let empty = TempFile::new().unwrap();
        let file = std::fs::File::open(empty.as_path()).unwrap();
        assert_eq!(detect_disk_format(&file).unwrap(), DiskImageFormat::Raw);
    }

    #[test]
    fn test_detect_raw_format_zero_filled() {
        let f = TempFile::new().unwrap();
        f.as_file().set_len(4096).unwrap();
        let file = std::fs::File::open(f.as_path()).unwrap();
        assert_eq!(detect_disk_format(&file).unwrap(), DiskImageFormat::Raw);
    }

    #[test]
    fn test_detect_raw_random_bytes() {
        let f = TempFile::new().unwrap();
        f.as_file().write_all(b"\x00\x01\x02\x03HELLO").unwrap();
        f.as_file().set_len(4096).unwrap();
        let file = std::fs::File::open(f.as_path()).unwrap();
        assert_eq!(detect_disk_format(&file).unwrap(), DiskImageFormat::Raw);
    }

    #[test]
    fn test_detect_vmdk_sparse_magic() {
        let f = TempFile::new().unwrap();
        f.as_file().write_all(&VMDK_SPARSE_MAGIC).unwrap();
        f.as_file().set_len(4096).unwrap();
        let file = std::fs::File::open(f.as_path()).unwrap();
        assert_eq!(detect_disk_format(&file).unwrap(), DiskImageFormat::Vmdk);
    }

    #[test]
    fn test_detect_vmdk_descriptor_header() {
        let f = TempFile::new().unwrap();
        f.as_file()
            .write_all(b"# Disk DescriptorFile\nversion=1\n")
            .unwrap();
        let file = std::fs::File::open(f.as_path()).unwrap();
        assert_eq!(detect_disk_format(&file).unwrap(), DiskImageFormat::Vmdk);
    }

    #[test]
    fn test_descriptor_without_header_is_raw() {
        let f = TempFile::new().unwrap();
        f.as_file()
            .write_all(b"version=1\ncreateType=\"monolithicFlat\"\n")
            .unwrap();
        let file = std::fs::File::open(f.as_path()).unwrap();
        // Without the canonical header, this is not recognized as VMDK.
        assert_eq!(detect_disk_format(&file).unwrap(), DiskImageFormat::Raw);
    }

    #[cfg(not(feature = "virtio-blk-vmdk"))]
    #[test]
    fn test_detect_recognizes_vmdk_even_without_feature() {
        let f = TempFile::new().unwrap();
        f.as_file().write_all(b"# Disk DescriptorFile\n").unwrap();
        let file = std::fs::File::open(f.as_path()).unwrap();
        assert_eq!(detect_disk_format(&file).unwrap(), DiskImageFormat::Vmdk);
    }
}
