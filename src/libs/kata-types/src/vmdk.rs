// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
//! VMDK (Virtual Machine Disk) types and utilities for structured
//! multi-extent flat disk layouts.

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
