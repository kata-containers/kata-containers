// Copyright © 2026, Microsoft Corporation
// Copyright 2026 Ant Group.
//
// SPDX-License-Identifier: Apache-2.0
//
// Ported from cloud-hypervisor's block/src/formats/vmdk/descriptor.rs.
// Adapted for dragonball's synchronous read-only VMDK backend.

//! VMDK descriptor text-file parser.
//!
//! Parses the ASCII/UTF-8 VMDK descriptor file that accompanies flat VMDK
//! images. Supports `monolithicFlat` and `twoGbMaxExtentFlat` create types.
//! Each extent must be of type `FLAT`.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// VMDK sector size (always 512 bytes per the spec).
pub(crate) const VMDK_SECTOR_SIZE: u64 = 512;

/// Access mode for a single extent, as declared in the descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExtentAccess {
    ReadWrite,
    ReadOnly,
    NoAccess,
}

/// One extent entry parsed from the descriptor's `# Extent description` section.
#[derive(Clone, Debug)]
pub(crate) struct ExtentHeader {
    pub access: ExtentAccess,
    /// Size of this extent in 512-byte sectors.
    pub size_in_sectors: u64,
    /// Always "FLAT" for the formats we support.
    #[allow(
        dead_code,
        reason = "VMDK spec field reserved for future sparse extent support"
    )]
    pub extent_type: String,
    /// Path to the extent backing file (relative or absolute).
    pub filename: String,
    /// Offset within the backing file, in sectors.
    pub offset_in_sectors: u64,
}

/// Parsed VMDK descriptor.
pub(crate) struct VmdkDescriptor {
    /// Parent directory of the descriptor file (used to resolve relative paths).
    pub base_path: String,
    /// List of extents in virtual-disk order.
    pub extents: Vec<ExtentHeader>,
}

// --- format-specific constants ---

const DESCRIPTOR_HEADER: &str = "# Disk DescriptorFile";
const DESCRIPTOR_EXTENTS: &str = "# Extent description";
const DESCRIPTOR_DDB: &str = "# The Disk Data Base";
const DESCRIPTOR_DDB_2: &str = "#DDB";

#[derive(Debug, PartialEq)]
enum CreateType {
    MonolithicFlat,
    TwoGbMaxExtentFlat,
    Unsupported,
}

// --- public API ---

impl VmdkDescriptor {
    /// Parse a VMDK descriptor from an already-open file handle and its path.
    pub fn new(file: &mut File, path: &Path) -> io::Result<Self> {
        let base_path = path
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or(".")
            .to_string();

        let content = read_descriptor(file)?;
        let lines: Vec<&str> = content.lines().collect();

        let header_end = parse_header(&lines)?;
        let extents = parse_extents(&lines, header_end)?;

        Ok(Self { base_path, extents })
    }
}

/// Quick check: do the first bytes of `data` look like a VMDK descriptor header?
#[allow(dead_code, reason = "reserved for runtime VMDK validation path")]
pub(crate) fn has_descriptor_header(data: &[u8]) -> bool {
    data.starts_with(DESCRIPTOR_HEADER.as_bytes())
}

/// Full validation: open the file, parse the descriptor, and confirm this is a
/// flat VMDK we can handle.
#[allow(dead_code, reason = "reserved for runtime VMDK validation path")]
pub(crate) fn is_flat_vmdk(f: &mut File) -> io::Result<bool> {
    let path = Path::new(""); // path not needed for validation
    match VmdkDescriptor::new(f, path) {
        Ok(d) => {
            // All extents must be FLAT and the create type must be recognized.
            // (create_type is already validated in parse_header)
            Ok(d.extents.iter().all(|e| e.extent_type == "FLAT"))
        }
        Err(_) => Ok(false),
    }
}

// --- internal helpers ---

fn read_descriptor(file: &mut File) -> io::Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    Ok(buf)
}

/// Parse the header section. Returns the index of the first line *after* the
/// header (i.e. where the extent description section starts or should start).
fn parse_header(lines: &[&str]) -> io::Result<usize> {
    if lines.is_empty() || lines[0] != DESCRIPTOR_HEADER {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing VMDK descriptor header",
        ));
    }

    let mut create_type = CreateType::Unsupported;

    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.starts_with('#') {
            // Section boundary: end of header.
            if create_type == CreateType::Unsupported {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "createType not found in VMDK header",
                ));
            }
            return Ok(i);
        }

        if let Some(value) = line.strip_prefix("createType=") {
            // Strip optional surrounding quotes.
            let value = value.trim_matches('"');
            create_type = match value {
                "monolithicFlat" => CreateType::MonolithicFlat,
                "twoGbMaxExtentFlat" => CreateType::TwoGbMaxExtentFlat,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("unsupported VMDK createType: {value}"),
                    ))
                }
            };
        }
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "unexpected end of VMDK header",
    ))
}

/// Parse the `# Extent description` section.
fn parse_extents(lines: &[&str], start: usize) -> io::Result<Vec<ExtentHeader>> {
    if start >= lines.len() || lines[start] != DESCRIPTOR_EXTENTS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing VMDK extent description section",
        ));
    }

    let mut extents = Vec::new();

    for line in lines.iter().skip(start + 1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with(DESCRIPTOR_DDB) || line.starts_with(DESCRIPTOR_DDB_2) {
            break;
        }
        if line.starts_with('#') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected comment in VMDK extent section: {line}"),
            ));
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("malformed VMDK extent line: {line}"),
            ));
        }

        let access = match parts[0] {
            "RW" => ExtentAccess::ReadWrite,
            "RDONLY" => ExtentAccess::ReadOnly,
            "NOACCESS" => ExtentAccess::NoAccess,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unknown VMDK extent access: {other}"),
                ))
            }
        };

        let size_in_sectors: u64 = parts[1].parse().map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("bad sector count: {e}"))
        })?;

        let extent_type = parts[2].to_string();

        // Filename is quoted; strip the surrounding double-quotes.
        let filename = parts[3].trim_matches('"').to_string();

        let offset_in_sectors: u64 = if parts.len() >= 5 {
            parts[4].parse().map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("bad extent offset: {e}"),
                )
            })?
        } else {
            0
        };

        extents.push(ExtentHeader {
            access,
            size_in_sectors,
            extent_type,
            filename,
            offset_in_sectors,
        });
    }

    if extents.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "VMDK descriptor has no extents",
        ));
    }

    Ok(extents)
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

    #[test]
    fn test_monolithic_flat_descriptor() {
        let (_tf, mut f) = write_temp(
            r#"# Disk DescriptorFile
version=1
CID=fffffffe
parentCID=ffffffff
createType="monolithicFlat"

# Extent description
RW 20480 FLAT "disk-flat.vmdk" 0

# The Disk Data Base
#DDB
"#,
        );
        let desc = VmdkDescriptor::new(&mut f, Path::new("/tmp/test.vmdk")).unwrap();
        assert_eq!(desc.extents.len(), 1);
        assert_eq!(desc.extents[0].access, ExtentAccess::ReadWrite);
        assert_eq!(desc.extents[0].size_in_sectors, 20480);
        assert_eq!(desc.extents[0].extent_type, "FLAT");
        assert_eq!(desc.extents[0].filename, "disk-flat.vmdk");
    }

    #[test]
    fn test_two_gb_max_extent_flat() {
        let (_tf, mut f) = write_temp(
            r#"# Disk DescriptorFile
version=1
CID=fffffffe
parentCID=ffffffff
createType="twoGbMaxExtentFlat"

# Extent description
RW 4192256 FLAT "disk-s001.vmdk" 0
RW 4192256 FLAT "disk-s002.vmdk" 0
RW 2097152 FLAT "disk-s003.vmdk" 0

# The Disk Data Base
#DDB
"#,
        );
        let desc = VmdkDescriptor::new(&mut f, Path::new("/tmp/test.vmdk")).unwrap();
        assert_eq!(desc.extents.len(), 3);
        assert_eq!(desc.extents[1].filename, "disk-s002.vmdk");
    }

    #[test]
    fn test_has_descriptor_header() {
        assert!(has_descriptor_header(b"# Disk DescriptorFile\nversion=1\n"));
        assert!(!has_descriptor_header(b"KDMV\x00\x00..."));
        assert!(!has_descriptor_header(b""));
    }

    #[test]
    fn test_is_flat_vmdk_valid() {
        let (_tf, mut f) = write_temp(
            r#"# Disk DescriptorFile
version=1
CID=fffffffe
parentCID=ffffffff
createType="monolithicFlat"

# Extent description
RW 1024 FLAT "disk.vmdk" 0

#DDB
"#,
        );
        assert!(is_flat_vmdk(&mut f).unwrap());
    }

    #[test]
    fn test_rdonly_extent() {
        let (_tf, mut f) = write_temp(
            r#"# Disk DescriptorFile
version=1
createType="monolithicFlat"

# Extent description
RDONLY 1024 FLAT "disk.vmdk" 0

#DDB
"#,
        );
        let desc = VmdkDescriptor::new(&mut f, Path::new("/tmp/test.vmdk")).unwrap();
        assert_eq!(desc.extents[0].access, ExtentAccess::ReadOnly);
    }
}
