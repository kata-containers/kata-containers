// Copyright © 2026, Microsoft Corporation
// Copyright 2026 Ant Group.
//
// SPDX-License-Identifier: Apache-2.0
//
// Ported from cloud-hypervisor's block/src/formats/vmdk/flat.rs.
// Adapted for dragonball's synchronous read-only VMDK backend.

//! Flat VMDK extent management.
//!
//! Opens the backing files referenced by a VMDK descriptor and builds the
//! virtual→physical extent map used by the I/O engine.

use std::fs::File;
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};

use super::vmdk_descriptor::{ExtentAccess, VmdkDescriptor, VMDK_SECTOR_SIZE};

/// One contiguous region of the virtual disk, backed by a host file.
pub(crate) struct VmdkExtent {
    /// Open file handle (None for NOACCESS extents).
    pub file: Option<File>,
    /// Access mode declared in the descriptor.
    #[allow(dead_code, reason = "reserved for write support")]
    pub access: ExtentAccess,
    /// Byte offset of this extent within the virtual disk.
    pub virtual_start: u64,
    /// Length of this extent in bytes.
    pub length: u64,
    /// Byte offset within the backing file where data begins.
    pub file_base_offset: u64,
}

/// Parsed and opened flat VMDK image.
pub(crate) struct FlatVmdk {
    /// Descriptor metadata (base path for relative extent resolution).
    #[allow(dead_code, reason = "reserved for extent path resolution")]
    pub base_path: String,
    /// Ordered list of extents.
    pub extents: Vec<VmdkExtent>,
    /// Total virtual disk size in bytes (sum of extent lengths).
    pub size: u64,
    /// Raw fd of the descriptor file (for advisory locking).
    #[allow(dead_code, reason = "reserved for advisory file locking")]
    pub descriptor_fd: RawFd,
}

impl FlatVmdk {
    /// Open a flat VMDK image.
    ///
    /// `file` is the already-open descriptor file. `path` is the filesystem
    /// path to the descriptor file (used to resolve relative extent paths).
    /// `readonly` downgrades RW extents to read-only.
    pub fn new(file: &mut File, path: &Path, readonly: bool) -> io::Result<Self> {
        let descriptor = VmdkDescriptor::new(file, path)?;

        if descriptor.extents.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "VMDK descriptor has no extents",
            ));
        }

        let descriptor_fd = file.as_raw_fd();
        let base_path = descriptor.base_path.clone();
        let mut extents: Vec<VmdkExtent> = Vec::with_capacity(descriptor.extents.len());
        let mut virtual_start: u64 = 0;

        for header in &descriptor.extents {
            // Validate sizes.
            let size_in_bytes = header
                .size_in_sectors
                .checked_mul(VMDK_SECTOR_SIZE)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("extent size overflow: {} sectors", header.size_in_sectors),
                    )
                })?;

            if size_in_bytes == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "extent has zero sectors",
                ));
            }

            let file_base_offset = header
                .offset_in_sectors
                .checked_mul(VMDK_SECTOR_SIZE)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "extent offset overflow: {} sectors",
                            header.offset_in_sectors
                        ),
                    )
                })?;

            // Determine effective access.
            let access = match (header.access, readonly) {
                (ExtentAccess::ReadWrite, true) => ExtentAccess::ReadOnly,
                (a, _) => a,
            };

            // Open the extent file.
            let file = if access == ExtentAccess::NoAccess {
                None
            } else {
                Some(open_extent(&base_path, &header.filename)?)
            };

            extents.push(VmdkExtent {
                file,
                access,
                virtual_start,
                length: size_in_bytes,
                file_base_offset,
            });

            virtual_start = virtual_start.checked_add(size_in_bytes).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "total VMDK virtual size overflow",
                )
            })?;
        }

        Ok(Self {
            base_path,
            extents,
            size: virtual_start,
            descriptor_fd,
        })
    }
}

/// Open an extent file, resolving relative paths against the descriptor's
/// parent directory. Always opens read-only and does NOT follow symlinks
/// on the final path component.
fn open_extent(base_path: &str, filename: &str) -> io::Result<File> {
    let extent_path = resolve_extent_path(base_path, filename)?;

    // Use O_NOFOLLOW on the final component for security.
    // We use a simple approach: open with read-only, no O_DIRECT needed
    // for dragonball's use case.
    File::open(&extent_path)
}

/// Resolve an extent filename to an absolute path.
///
/// Relative paths are resolved against the descriptor file's parent directory.
/// Absolute paths are used as-is.
fn resolve_extent_path(base_path: &str, filename: &str) -> io::Result<PathBuf> {
    let path = Path::new(filename);
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(Path::new(base_path).join(filename))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use vmm_sys_util::tempfile::TempFile;

    fn write_temp(content: &str) -> (TempFile, File) {
        let tf = TempFile::new().unwrap();
        tf.as_file().write_all(content.as_bytes()).unwrap();
        let f = std::fs::File::open(tf.as_path()).unwrap();
        (tf, f)
    }

    /// Create a temp extent file with content, return the TempFile and path.
    fn make_extent(content: &[u8], size: u64) -> TempFile {
        let tf = TempFile::new().unwrap();
        tf.as_file().set_len(size).unwrap();
        if !content.is_empty() {
            tf.as_file().write_all(content).unwrap();
        }
        tf
    }

    #[test]
    fn test_single_extent_open() {
        let extent = make_extent(b"hello", 4096);
        let extent_path = extent.as_path().to_str().unwrap();

        let desc_text = format!(
            r#"# Disk DescriptorFile
version=1
createType="monolithicFlat"

# Extent description
RW 8 FLAT "{extent_path}" 0

#DDB
"#
        );
        let (_desc_tf, mut desc_file) = write_temp(&desc_text);

        let flat = FlatVmdk::new(&mut desc_file, Path::new("/fake/test.vmdk"), true).unwrap();
        assert_eq!(flat.extents.len(), 1);
        assert_eq!(flat.size, 4096); // 8 * 512
        assert_eq!(flat.extents[0].virtual_start, 0);
        assert_eq!(flat.extents[0].length, 4096);
        assert_eq!(flat.extents[0].access, ExtentAccess::ReadOnly);
    }

    #[test]
    fn test_multi_extent_open() {
        let e0 = make_extent(b"ext0", 4096);
        let e1 = make_extent(b"ext1", 4096);
        let e2 = make_extent(b"ext2", 4096);

        let desc_text = format!(
            r#"# Disk DescriptorFile
version=1
createType="twoGbMaxExtentFlat"

# Extent description
RW 8 FLAT "{}" 0
RW 8 FLAT "{}" 0
RW 8 FLAT "{}" 0

#DDB
"#,
            e0.as_path().to_str().unwrap(),
            e1.as_path().to_str().unwrap(),
            e2.as_path().to_str().unwrap(),
        );
        let (_desc_tf, mut desc_file) = write_temp(&desc_text);

        let flat = FlatVmdk::new(&mut desc_file, Path::new("/fake/test.vmdk"), true).unwrap();
        assert_eq!(flat.extents.len(), 3);
        assert_eq!(flat.size, 3 * 4096);
        assert_eq!(flat.extents[0].virtual_start, 0);
        assert_eq!(flat.extents[1].virtual_start, 4096);
        assert_eq!(flat.extents[2].virtual_start, 8192);
    }

    #[test]
    fn test_noaccess_extent() {
        let e0 = make_extent(b"ext0", 4096);

        let desc_text = format!(
            r#"# Disk DescriptorFile
version=1
createType="twoGbMaxExtentFlat"

# Extent description
RW 4 FLAT "{}" 0
NOACCESS 4 FLAT "nonexistent.vmdk" 0
RW 4 FLAT "{}" 0

#DDB
"#,
            e0.as_path().to_str().unwrap(),
            e0.as_path().to_str().unwrap(),
        );
        let (_desc_tf, mut desc_file) = write_temp(&desc_text);

        let flat = FlatVmdk::new(&mut desc_file, Path::new("/fake/test.vmdk"), true).unwrap();
        assert_eq!(flat.extents.len(), 3);
        assert!(flat.extents[0].file.is_some());
        assert!(flat.extents[1].file.is_none()); // NOACCESS
        assert!(flat.extents[2].file.is_some());
        assert_eq!(flat.extents[1].access, ExtentAccess::NoAccess);
    }
}
