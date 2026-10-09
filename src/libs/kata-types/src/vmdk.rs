// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
//! VMDK (Virtual Machine Disk) types and utilities for structured
//! multi-extent flat disk layouts.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fmt::Write as _;

/// Maximum sectors in a single VMDK flat extent (2GB - 512 bytes).
pub const MAX_VMDK_EXTENT_SECTORS: u64 = 0x8000_0000 >> 9;

/// A single extent within a structured VMDK layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmdkExtent {
    /// Host-visible path to the flat backing file.
    pub path_on_host: String,
    /// Number of 512-byte sectors covered by this extent.
    pub sectors: u64,
    /// Byte offset within the backing file where this extent begins.
    pub file_offset: u64,
}

/// A structured multi-extent VMDK configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VmdkConfig {
    /// Ordered list of extents that make up the virtual disk.
    pub extents: Vec<VmdkExtent>,
}

impl VmdkConfig {
    /// Append a contiguous extent referencing a host file.
    pub fn push_extent(&mut self, path_on_host: &str, sectors: u64, file_offset: u64) {
        self.extents.push(VmdkExtent {
            path_on_host: path_on_host.to_string(),
            sectors,
            file_offset,
        });
    }

    /// Append chunked extents covering `total_sectors` without exceeding
    /// [`MAX_VMDK_EXTENT_SECTORS`] per chunk.
    pub fn push_extent_chunked(&mut self, path_on_host: &str, total_sectors: u64) {
        let mut remaining = total_sectors;
        let mut file_offset = 0;
        while remaining > 0 {
            let sectors = remaining.min(MAX_VMDK_EXTENT_SECTORS);
            self.push_extent(path_on_host, sectors, file_offset);
            file_offset += sectors;
            remaining -= sectors;
        }
    }

    /// Sum of all extent sectors, or [`None`] on overflow.
    pub fn total_sectors(&self) -> Option<u64> {
        self.extents
            .iter()
            .try_fold(0_u64, |total, extent| total.checked_add(extent.sectors))
    }
}

/// A structurally validated VMDK layout.
///
/// The required sector count is the greatest `file_offset + sectors` for each
/// unique backing path. Consumers use it to verify that a backing source is
/// large enough for the layout.
#[derive(Debug)]
pub struct VmdkLayout<'a> {
    extents: &'a [VmdkExtent],
    total_sectors: u64,
    required_sectors_by_path: HashMap<&'a str, u64>,
}

/// Validate a structured VMDK layout without accessing its backing sources.
pub fn validate_vmdk_layout(vmdk: &VmdkConfig) -> Result<VmdkLayout<'_>> {
    if vmdk.extents.is_empty() {
        return Err(anyhow!("VMDK contains no extents"));
    }

    let total_sectors = vmdk
        .total_sectors()
        .ok_or_else(|| anyhow!("VMDK total sector count overflow"))?;
    if total_sectors == 0 {
        return Err(anyhow!("VMDK contains no non-empty extents"));
    }

    let mut required_sectors_by_path: HashMap<&str, u64> = HashMap::new();
    for extent in &vmdk.extents {
        let required_sectors = extent
            .file_offset
            .checked_add(extent.sectors)
            .ok_or_else(|| anyhow!("VMDK extent sector count overflow"))?;
        required_sectors_by_path
            .entry(extent.path_on_host.as_str())
            .and_modify(|maximum| *maximum = (*maximum).max(required_sectors))
            .or_insert(required_sectors);
    }

    Ok(VmdkLayout {
        extents: &vmdk.extents,
        total_sectors,
        required_sectors_by_path,
    })
}

impl VmdkLayout<'_> {
    /// Return the number of sectors required from the source at `path`.
    ///
    /// A source may provide more than one extent, so this is the greatest
    /// `file_offset + sectors` among all extents referring to that path.
    pub fn required_sectors_for(&self, path: &str) -> Option<u64> {
        self.required_sectors_by_path.get(path).copied()
    }

    /// Render a descriptor using consumer-specific extent references.
    ///
    /// The resolver translates each backing extent into the reference written
    /// to the descriptor. QEMU, for example, resolves host paths to fdset paths,
    /// while a VMM that opens sources itself can use accessible host paths.
    pub fn render_descriptor_with<F>(&self, mut resolve_extent_reference: F) -> Result<String>
    where
        F: FnMut(&VmdkExtent) -> Result<String>,
    {
        let mut descriptor = String::new();
        writeln!(descriptor, "# Disk DescriptorFile")?;
        writeln!(descriptor, "version=1")?;
        writeln!(descriptor, "CID=fffffffe")?;
        writeln!(descriptor, "parentCID=ffffffff")?;
        writeln!(descriptor, "createType=\"twoGbMaxExtentFlat\"")?;
        writeln!(descriptor)?;
        writeln!(descriptor, "# Extent description")?;
        for extent in self.extents {
            let reference = resolve_extent_reference(extent)?;
            // VMDK quoted path fields have no escape mechanism. Reject values
            // that would terminate the field or inject descriptor lines.
            if reference.contains(['"', '\n', '\r']) {
                return Err(anyhow!(
                    "VMDK extent reference contains characters unsupported by the descriptor grammar"
                ));
            }
            writeln!(
                descriptor,
                "RW {} FLAT \"{}\" {}",
                extent.sectors, reference, extent.file_offset
            )?;
        }

        let cylinders = self.total_sectors.div_ceil(63 * 16);
        writeln!(descriptor)?;
        writeln!(descriptor, "# The Disk Data Base")?;
        writeln!(descriptor, "#DDB")?;
        writeln!(descriptor)?;
        writeln!(descriptor, "ddb.virtualHWVersion = \"4\"")?;
        writeln!(descriptor, "ddb.geometry.cylinders = \"{cylinders}\"")?;
        writeln!(descriptor, "ddb.geometry.heads = \"16\"")?;
        writeln!(descriptor, "ddb.geometry.sectors = \"63\"")?;
        writeln!(descriptor, "ddb.adapterType = \"ide\"")?;

        Ok(descriptor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_structured_vmdk_layout() {
        let mut config = VmdkConfig::default();
        config.push_extent("/images/first.raw", 8, 0);
        config.push_extent("/images/first.raw", 4, 8);
        config.push_extent("/images/second.raw", 16, 0);
        let references = HashMap::from([
            ("/images/first.raw", "/dev/fdset/1"),
            ("/images/second.raw", "/dev/fdset/2"),
        ]);

        let descriptor = validate_vmdk_layout(&config)
            .unwrap()
            .render_descriptor_with(|extent| {
                references
                    .get(extent.path_on_host.as_str())
                    .map(|reference| (*reference).to_string())
                    .ok_or_else(|| anyhow!("missing prepared VMDK extent {}", extent.path_on_host))
            })
            .unwrap();

        assert!(descriptor.contains("RW 8 FLAT \"/dev/fdset/1\" 0"));
        assert!(descriptor.contains("RW 4 FLAT \"/dev/fdset/1\" 8"));
        assert!(descriptor.contains("RW 16 FLAT \"/dev/fdset/2\" 0"));
        assert!(descriptor.contains("ddb.geometry.cylinders = \"1\""));
    }

    #[test]
    fn records_largest_requirement_for_each_extent_path() {
        let mut config = VmdkConfig::default();
        config.push_extent("/images/extent.raw", 1, 0);
        config.push_extent("/images/extent.raw", 2, 8);

        let layout = validate_vmdk_layout(&config).unwrap();
        assert_eq!(layout.required_sectors_for("/images/extent.raw"), Some(10));
    }

    #[test]
    fn rejects_invalid_resolved_extent_references() {
        let mut config = VmdkConfig::default();
        config.push_extent("/images/extent.raw", 1, 0);

        for reference in ["bad\"reference", "bad\nreference", "bad\rreference"] {
            let error = validate_vmdk_layout(&config)
                .unwrap()
                .render_descriptor_with(|_| Ok(reference.to_string()))
                .unwrap_err();
            assert!(error
                .to_string()
                .contains("unsupported by the descriptor grammar"));
        }
    }

    #[test]
    fn rejects_empty_and_overflowing_layouts() {
        let empty = VmdkConfig::default();
        assert!(validate_vmdk_layout(&empty)
            .unwrap_err()
            .to_string()
            .contains("contains no extents"));

        let mut overflowing = VmdkConfig::default();
        overflowing.push_extent("/images/extent.raw", 1, u64::MAX);
        assert!(validate_vmdk_layout(&overflowing)
            .unwrap_err()
            .to_string()
            .contains("sector count overflow"));
    }
}
