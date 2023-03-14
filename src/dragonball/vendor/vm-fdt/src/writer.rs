// Copyright 2021 The Chromium OS Authors. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! This module writes Flattened Devicetree blobs as defined here:
//! <https://devicetree-specification.readthedocs.io/en/stable/flattened-format.html>

use std::cmp::{Ord, Ordering};
use std::collections::{BTreeMap, HashSet};
use std::convert::TryInto;
use std::ffi::CString;
use std::fmt;
use std::mem::size_of_val;

use crate::{
    FDT_BEGIN_NODE, FDT_END, FDT_END_NODE, FDT_MAGIC, FDT_PROP, NODE_NAME_MAX_LEN,
    PROPERTY_NAME_MAX_LEN,
};

#[derive(Debug, PartialEq)]
/// Errors associated with creating the Flattened Device Tree.
pub enum Error {
    /// Properties may not be added before beginning a node.
    PropertyBeforeBeginNode,
    /// Properties may not be added after a node has been ended.
    PropertyAfterEndNode,
    /// Property value size must fit in 32 bits.
    PropertyValueTooLarge,
    /// Total size must fit in 32 bits.
    TotalSizeTooLarge,
    /// Strings cannot contain NUL.
    InvalidString,
    /// Attempted to end a node that was not the most recent.
    OutOfOrderEndNode,
    /// Attempted to call finish without ending all nodes.
    UnclosedNode,
    /// Memory reservation is invalid.
    InvalidMemoryReservation,
    /// Memory reservations are overlapping.
    OverlappingMemoryReservations,
    /// Invalid node name.
    InvalidNodeName,
    /// Invalid property name.
    InvalidPropertyName,
    /// Node depth exceeds FDT_MAX_NODE_DEPTH
    NodeDepthTooLarge,
    /// Duplicate phandle property
    DuplicatePhandle,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::PropertyBeforeBeginNode => {
                write!(f, "Properties may not be added before beginning a node")
            }
            Error::PropertyAfterEndNode => {
                write!(f, "Properties may not be added after a node has been ended")
            }
            Error::PropertyValueTooLarge => write!(f, "Property value size must fit in 32 bits"),
            Error::TotalSizeTooLarge => write!(f, "Total size must fit in 32 bits"),
            Error::InvalidString => write!(f, "Strings cannot contain NUL"),
            Error::OutOfOrderEndNode => {
                write!(f, "Attempted to end a node that was not the most recent")
            }
            Error::UnclosedNode => write!(f, "Attempted to call finish without ending all nodes"),
            Error::InvalidMemoryReservation => write!(f, "Memory reservation is invalid"),
            Error::OverlappingMemoryReservations => {
                write!(f, "Memory reservations are overlapping")
            }
            Error::InvalidNodeName => write!(f, "Invalid node name"),
            Error::InvalidPropertyName => write!(f, "Invalid property name"),
            Error::NodeDepthTooLarge => write!(f, "Node depth exceeds FDT_MAX_NODE_DEPTH"),
            Error::DuplicatePhandle => write!(f, "Duplicate phandle value"),
        }
    }
}

impl std::error::Error for Error {}

/// Result of a FDT writer operation.
pub type Result<T> = std::result::Result<T, Error>;

const FDT_HEADER_SIZE: usize = 40;
const FDT_VERSION: u32 = 17;
const FDT_LAST_COMP_VERSION: u32 = 16;
/// The same max depth as in the Linux kernel.
const FDT_MAX_NODE_DEPTH: usize = 64;

/// Interface for writing a Flattened Devicetree (FDT) and emitting a Devicetree Blob (DTB).
#[derive(Debug)]
pub struct FdtWriter {
    data: Vec<u8>,
    off_mem_rsvmap: u32,
    off_dt_struct: u32,
    strings: Vec<u8>,
    string_offsets: BTreeMap<CString, u32>,
    node_depth: usize,
    node_ended: bool,
    boot_cpuid_phys: u32,
    // The set is used to track the uniqueness of phandle values as required by the spec
    // https://devicetree-specification.readthedocs.io/en/stable/devicetree-basics.html#phandle
    phandles: HashSet<u32>,
}

/// Reserved physical memory region.
///
/// This represents an area of physical memory reserved by the firmware and unusable by the OS.
/// For example, this could be used to preserve bootloader code or data used at runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FdtReserveEntry {
    address: u64,
    size: u64,
}

impl FdtReserveEntry {
    /// Create a memory reservation for the FDT.
    ///
    /// # Arguments
    ///
    /// * address: Physical address of the beginning of the reserved region.
    /// * size: Size of the reserved region in bytes.
    pub fn new(address: u64, size: u64) -> Result<Self> {
        if address.checked_add(size).is_none() || size == 0 {
            return Err(Error::InvalidMemoryReservation);
        }

        Ok(FdtReserveEntry { address, size })
    }
}

impl Ord for FdtReserveEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.address.cmp(&other.address)
    }
}

impl PartialOrd for FdtReserveEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.address.partial_cmp(&other.address)
    }
}

// Returns true if there are any overlapping memory reservations.
fn check_overlapping(mem_reservations: &[FdtReserveEntry]) -> Result<()> {
    let mut mem_rsvmap_copy = mem_reservations.to_vec();
    mem_rsvmap_copy.sort();
    let overlapping = mem_rsvmap_copy.windows(2).any(|w| {
        // The following add cannot overflow because we can only have
        // valid FdtReserveEntry (as per the constructor of the type).
        w[0].address + w[0].size > w[1].address
    });

    if overlapping {
        return Err(Error::OverlappingMemoryReservations);
    }

    Ok(())
}

// Check if `name` is a valid node name in the form "node-name@unit-address".
// https://devicetree-specification.readthedocs.io/en/stable/devicetree-basics.html#node-name-requirements
fn node_name_valid(name: &str) -> bool {
    // Special case: allow empty node names.
    // This is technically not allowed by the spec, but it seems to be accepted in practice.
    if name.is_empty() {
        return true;
    }

    let mut parts = name.split('@');

    let node_name = parts.next().unwrap(); // split() always returns at least one part
    let unit_address = parts.next();

    if unit_address.is_some() && parts.next().is_some() {
        // Node names should only contain one '@'.
        return false;
    }

    if node_name.is_empty() || node_name.len() > NODE_NAME_MAX_LEN {
        return false;
    }

    if !node_name.starts_with(node_name_valid_first_char) {
        return false;
    }

    if node_name.contains(|c: char| !node_name_valid_char(c)) {
        return false;
    }

    if let Some(unit_address) = unit_address {
        if unit_address.contains(|c: char| !node_name_valid_char(c)) {
            return false;
        }
    }

    true
}

fn node_name_valid_char(c: char) -> bool {
    matches!(c, '0'..='9' | 'a'..='z' | 'A'..='Z' | ',' | '.' | '_' | '+' | '-')
}

fn node_name_valid_first_char(c: char) -> bool {
    matches!(c, 'a'..='z' | 'A'..='Z')
}

// Check if `name` is a valid property name.
// https://devicetree-specification.readthedocs.io/en/stable/devicetree-basics.html#property-names
fn property_name_valid(name: &str) -> bool {
    if name.is_empty() || name.len() > PROPERTY_NAME_MAX_LEN {
        return false;
    }

    if name.contains(|c: char| !property_name_valid_char(c)) {
        return false;
    }

    true
}

fn property_name_valid_char(c: char) -> bool {
    matches!(c, '0'..='9' | 'a'..='z' | 'A'..='Z' | ',' | '.' | '_' | '+' | '?' | '#' | '-')
}

/// Handle to an open node created by `FdtWriter::begin_node`.
///
/// This must be passed back to `FdtWriter::end_node` to close the nodes.
/// Nodes must be closed in reverse order as they were opened, matching the nesting structure
/// of the devicetree.
#[derive(Debug)]
pub struct FdtWriterNode {
    depth: usize,
}

impl FdtWriter {
    /// Create a new Flattened Devicetree writer instance.
    pub fn new() -> Result<Self> {
        FdtWriter::new_with_mem_reserv(&[])
    }

    /// Create a new Flattened Devicetree writer instance.
    ///
    /// # Arguments
    ///
    /// `mem_reservations` - reserved physical memory regions to list in the FDT header.
    pub fn new_with_mem_reserv(mem_reservations: &[FdtReserveEntry]) -> Result<Self> {
        let data = vec![0u8; FDT_HEADER_SIZE]; // Reserve space for header.

        let mut fdt = FdtWriter {
            data,
            off_mem_rsvmap: 0,
            off_dt_struct: 0,
            strings: Vec::new(),
            string_offsets: BTreeMap::new(),
            node_depth: 0,
            node_ended: false,
            boot_cpuid_phys: 0,
            phandles: HashSet::new(),
        };

        fdt.align(8);
        // This conversion cannot fail since the size of the header is fixed.
        fdt.off_mem_rsvmap = fdt.data.len() as u32;

        check_overlapping(mem_reservations)?;
        fdt.write_mem_rsvmap(mem_reservations);

        fdt.align(4);
        fdt.off_dt_struct = fdt
            .data
            .len()
            .try_into()
            .map_err(|_| Error::TotalSizeTooLarge)?;

        Ok(fdt)
    }

    fn write_mem_rsvmap(&mut self, mem_reservations: &[FdtReserveEntry]) {
        for rsv in mem_reservations {
            self.append_u64(rsv.address);
            self.append_u64(rsv.size);
        }

        self.append_u64(0);
        self.append_u64(0);
    }

    /// Set the `boot_cpuid_phys` field of the devicetree header.
    ///
    /// # Example
    ///
    /// ```rust
    /// use vm_fdt::{Error, FdtWriter};
    ///
    /// fn create_fdt() -> Result<Vec<u8>, Error> {
    ///     let mut fdt = FdtWriter::new()?;
    ///     fdt.set_boot_cpuid_phys(0x12345678);
    ///     // ... add other nodes & properties
    ///     fdt.finish()
    /// }
    ///
    /// # let dtb = create_fdt().unwrap();
    /// ```
    pub fn set_boot_cpuid_phys(&mut self, boot_cpuid_phys: u32) {
        self.boot_cpuid_phys = boot_cpuid_phys;
    }

    // Append `num_bytes` padding bytes (0x00).
    fn pad(&mut self, num_bytes: usize) {
        self.data.extend(std::iter::repeat(0).take(num_bytes));
    }

    // Append padding bytes (0x00) until the length of data is a multiple of `alignment`.
    fn align(&mut self, alignment: usize) {
        let offset = self.data.len() % alignment;
        if offset != 0 {
            self.pad(alignment - offset);
        }
    }

    // Rewrite the value of a big-endian u32 within data.
    fn update_u32(&mut self, offset: usize, val: u32) {
        // Safe to use `+ 4` since we are calling this function with small values, and it's a
        // private function.
        let data_slice = &mut self.data[offset..offset + 4];
        data_slice.copy_from_slice(&val.to_be_bytes());
    }

    fn append_u32(&mut self, val: u32) {
        self.data.extend_from_slice(&val.to_be_bytes());
    }

    fn append_u64(&mut self, val: u64) {
        self.data.extend_from_slice(&val.to_be_bytes());
    }

    /// Open a new FDT node.
    ///
    /// The node must be closed using `end_node`.
    ///
    /// # Arguments
    ///
    /// `name` - name of the node; must not contain any NUL bytes.
    pub fn begin_node(&mut self, name: &str) -> Result<FdtWriterNode> {
        if self.node_depth >= FDT_MAX_NODE_DEPTH {
            return Err(Error::NodeDepthTooLarge);
        }

        let name_cstr = CString::new(name).map_err(|_| Error::InvalidString)?;
        // The unit adddress part of the node name, if present, is not fully validated
        // since the exact requirements depend on the bus mapping.
        // https://devicetree-specification.readthedocs.io/en/stable/devicetree-basics.html#node-name-requirements
        if !node_name_valid(name) {
            return Err(Error::InvalidNodeName);
        }
        self.append_u32(FDT_BEGIN_NODE);
        self.data.extend(name_cstr.to_bytes_with_nul());
        self.align(4);
        // This can not overflow due to the `if` at the beginning of the function
        // where the current depth is checked against FDT_MAX_NODE_DEPTH.
        self.node_depth += 1;
        self.node_ended = false;
        Ok(FdtWriterNode {
            depth: self.node_depth,
        })
    }

    /// Close a node previously opened with `begin_node`.
    pub fn end_node(&mut self, node: FdtWriterNode) -> Result<()> {
        if node.depth != self.node_depth {
            return Err(Error::OutOfOrderEndNode);
        }

        self.append_u32(FDT_END_NODE);
        // This can not underflow. The above `if` makes sure there is at least one open node
        // (node_depth >= 1).
        self.node_depth -= 1;
        self.node_ended = true;
        Ok(())
    }

    // Find an existing instance of a string `s`, or add it to the strings block.
    // Returns the offset into the strings block.
    fn intern_string(&mut self, s: CString) -> Result<u32> {
        if let Some(off) = self.string_offsets.get(&s) {
            Ok(*off)
        } else {
            let off = self
                .strings
                .len()
                .try_into()
                .map_err(|_| Error::TotalSizeTooLarge)?;
            self.strings.extend_from_slice(s.to_bytes_with_nul());
            self.string_offsets.insert(s, off);
            Ok(off)
        }
    }

    /// Write a property.
    ///
    /// # Arguments
    ///
    /// `name` - name of the property; must not contain any NUL bytes.
    /// `val` - value of the property (raw byte array).
    pub fn property(&mut self, name: &str, val: &[u8]) -> Result<()> {
        if self.node_ended {
            return Err(Error::PropertyAfterEndNode);
        }

        if self.node_depth == 0 {
            return Err(Error::PropertyBeforeBeginNode);
        }

        let name_cstr = CString::new(name).map_err(|_| Error::InvalidString)?;

        if !property_name_valid(name) {
            return Err(Error::InvalidPropertyName);
        }

        let len = val
            .len()
            .try_into()
            .map_err(|_| Error::PropertyValueTooLarge)?;

        let nameoff = self.intern_string(name_cstr)?;
        self.append_u32(FDT_PROP);
        self.append_u32(len);
        self.append_u32(nameoff);
        self.data.extend_from_slice(val);
        self.align(4);
        Ok(())
    }

    /// Write an empty property.
    pub fn property_null(&mut self, name: &str) -> Result<()> {
        self.property(name, &[])
    }

    /// Write a string property.
    pub fn property_string(&mut self, name: &str, val: &str) -> Result<()> {
        let cstr_value = CString::new(val).map_err(|_| Error::InvalidString)?;
        self.property(name, cstr_value.to_bytes_with_nul())
    }

    /// Write a stringlist property.
    pub fn property_string_list(&mut self, name: &str, values: Vec<String>) -> Result<()> {
        let mut bytes = Vec::new();
        for s in values {
            let cstr = CString::new(s).map_err(|_| Error::InvalidString)?;
            bytes.extend_from_slice(cstr.to_bytes_with_nul());
        }
        self.property(name, &bytes)
    }

    /// Write a 32-bit unsigned integer property.
    pub fn property_u32(&mut self, name: &str, val: u32) -> Result<()> {
        self.property(name, &val.to_be_bytes())
    }

    /// Write a 64-bit unsigned integer property.
    pub fn property_u64(&mut self, name: &str, val: u64) -> Result<()> {
        self.property(name, &val.to_be_bytes())
    }

    /// Write a property containing an array of 32-bit unsigned integers.
    pub fn property_array_u32(&mut self, name: &str, cells: &[u32]) -> Result<()> {
        let mut arr = Vec::with_capacity(size_of_val(cells));
        for &c in cells {
            arr.extend(&c.to_be_bytes());
        }
        self.property(name, &arr)
    }

    /// Write a property containing an array of 64-bit unsigned integers.
    pub fn property_array_u64(&mut self, name: &str, cells: &[u64]) -> Result<()> {
        let mut arr = Vec::with_capacity(size_of_val(cells));
        for &c in cells {
            arr.extend(&c.to_be_bytes());
        }
        self.property(name, &arr)
    }

    /// Write a [`phandle`](https://devicetree-specification.readthedocs.io/en/stable/devicetree-basics.html?#phandle)
    /// property. The value is checked for uniqueness within the FDT. In the case of a duplicate
    /// [`Error::DuplicatePhandle`] is returned.
    pub fn property_phandle(&mut self, val: u32) -> Result<()> {
        if !self.phandles.insert(val) {
            return Err(Error::DuplicatePhandle);
        }
        self.property("phandle", &val.to_be_bytes())
    }

    /// Finish writing the Devicetree Blob (DTB).
    ///
    /// Returns the DTB as a vector of bytes, consuming the `FdtWriter`.
    pub fn finish(mut self) -> Result<Vec<u8>> {
        if self.node_depth > 0 {
            return Err(Error::UnclosedNode);
        }

        self.append_u32(FDT_END);
        let size_dt_plus_header: u32 = self
            .data
            .len()
            .try_into()
            .map_err(|_| Error::TotalSizeTooLarge)?;
        // The following operation cannot fail because the total size of data
        // also includes the offset, and we checked that `size_dt_plus_header`
        // does not wrap around when converted to an u32.
        let size_dt_struct = size_dt_plus_header - self.off_dt_struct;

        let off_dt_strings = self
            .data
            .len()
            .try_into()
            .map_err(|_| Error::TotalSizeTooLarge)?;
        let size_dt_strings = self
            .strings
            .len()
            .try_into()
            .map_err(|_| Error::TotalSizeTooLarge)?;

        let totalsize = self
            .data
            .len()
            .checked_add(self.strings.len())
            .ok_or(Error::TotalSizeTooLarge)?;
        let totalsize = totalsize.try_into().map_err(|_| Error::TotalSizeTooLarge)?;

        // Finalize the header.
        self.update_u32(0, FDT_MAGIC);
        self.update_u32(4, totalsize);
        self.update_u32(2 * 4, self.off_dt_struct);
        self.update_u32(3 * 4, off_dt_strings);
        self.update_u32(4 * 4, self.off_mem_rsvmap);
        self.update_u32(5 * 4, FDT_VERSION);
        self.update_u32(6 * 4, FDT_LAST_COMP_VERSION);
        self.update_u32(7 * 4, self.boot_cpuid_phys);
        self.update_u32(8 * 4, size_dt_strings);
        self.update_u32(9 * 4, size_dt_struct);

        // Add the strings block.
        self.data.append(&mut self.strings);

        Ok(self.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let mut fdt = FdtWriter::new().unwrap();
        let root_node = fdt.begin_node("").unwrap();
        fdt.end_node(root_node).unwrap();
        let actual_fdt = fdt.finish().unwrap();
        let expected_fdt = vec![
            0xd0, 0x0d, 0xfe, 0xed, // 0000: magic (0xd00dfeed)
            0x00, 0x00, 0x00, 0x48, // 0004: totalsize (0x48)
            0x00, 0x00, 0x00, 0x38, // 0008: off_dt_struct (0x38)
            0x00, 0x00, 0x00, 0x48, // 000C: off_dt_strings (0x48)
            0x00, 0x00, 0x00, 0x28, // 0010: off_mem_rsvmap (0x28)
            0x00, 0x00, 0x00, 0x11, // 0014: version (0x11 = 17)
            0x00, 0x00, 0x00, 0x10, // 0018: last_comp_version (0x10 = 16)
            0x00, 0x00, 0x00, 0x00, // 001C: boot_cpuid_phys (0)
            0x00, 0x00, 0x00, 0x00, // 0020: size_dt_strings (0)
            0x00, 0x00, 0x00, 0x10, // 0024: size_dt_struct (0x10)
            0x00, 0x00, 0x00, 0x00, // 0028: rsvmap terminator (address = 0 high)
            0x00, 0x00, 0x00, 0x00, // 002C: rsvmap terminator (address = 0 low)
            0x00, 0x00, 0x00, 0x00, // 0030: rsvmap terminator (size = 0 high)
            0x00, 0x00, 0x00, 0x00, // 0034: rsvmap terminator (size = 0 low)
            0x00, 0x00, 0x00, 0x01, // 0038: FDT_BEGIN_NODE
            0x00, 0x00, 0x00, 0x00, // 003C: node name ("") + padding
            0x00, 0x00, 0x00, 0x02, // 0040: FDT_END_NODE
            0x00, 0x00, 0x00, 0x09, // 0044: FDT_END
        ];
        assert_eq!(expected_fdt, actual_fdt);
    }

    #[test]
    fn reservemap() {
        let mut fdt = FdtWriter::new_with_mem_reserv(&[
            FdtReserveEntry::new(0x12345678AABBCCDD, 0x1234).unwrap(),
            FdtReserveEntry::new(0x1020304050607080, 0x5678).unwrap(),
        ])
        .unwrap();
        let root_node = fdt.begin_node("").unwrap();
        fdt.end_node(root_node).unwrap();
        let actual_fdt = fdt.finish().unwrap();
        let expected_fdt = vec![
            0xd0, 0x0d, 0xfe, 0xed, // 0000: magic (0xd00dfeed)
            0x00, 0x00, 0x00, 0x68, // 0004: totalsize (0x68)
            0x00, 0x00, 0x00, 0x58, // 0008: off_dt_struct (0x58)
            0x00, 0x00, 0x00, 0x68, // 000C: off_dt_strings (0x68)
            0x00, 0x00, 0x00, 0x28, // 0010: off_mem_rsvmap (0x28)
            0x00, 0x00, 0x00, 0x11, // 0014: version (0x11 = 17)
            0x00, 0x00, 0x00, 0x10, // 0018: last_comp_version (0x10 = 16)
            0x00, 0x00, 0x00, 0x00, // 001C: boot_cpuid_phys (0)
            0x00, 0x00, 0x00, 0x00, // 0020: size_dt_strings (0)
            0x00, 0x00, 0x00, 0x10, // 0024: size_dt_struct (0x10)
            0x12, 0x34, 0x56, 0x78, // 0028: rsvmap entry 0 address high
            0xAA, 0xBB, 0xCC, 0xDD, // 002C: rsvmap entry 0 address low
            0x00, 0x00, 0x00, 0x00, // 0030: rsvmap entry 0 size high
            0x00, 0x00, 0x12, 0x34, // 0034: rsvmap entry 0 size low
            0x10, 0x20, 0x30, 0x40, // 0038: rsvmap entry 1 address high
            0x50, 0x60, 0x70, 0x80, // 003C: rsvmap entry 1 address low
            0x00, 0x00, 0x00, 0x00, // 0040: rsvmap entry 1 size high
            0x00, 0x00, 0x56, 0x78, // 0044: rsvmap entry 1 size low
            0x00, 0x00, 0x00, 0x00, // 0048: rsvmap terminator (address = 0 high)
            0x00, 0x00, 0x00, 0x00, // 004C: rsvmap terminator (address = 0 low)
            0x00, 0x00, 0x00, 0x00, // 0050: rsvmap terminator (size = 0 high)
            0x00, 0x00, 0x00, 0x00, // 0054: rsvmap terminator (size = 0 low)
            0x00, 0x00, 0x00, 0x01, // 0058: FDT_BEGIN_NODE
            0x00, 0x00, 0x00, 0x00, // 005C: node name ("") + padding
            0x00, 0x00, 0x00, 0x02, // 0060: FDT_END_NODE
            0x00, 0x00, 0x00, 0x09, // 0064: FDT_END
        ];
        assert_eq!(expected_fdt, actual_fdt);
    }

    #[test]
    fn prop_null() {
        let mut fdt = FdtWriter::new().unwrap();
        let root_node = fdt.begin_node("").unwrap();
        fdt.property_null("null").unwrap();
        fdt.end_node(root_node).unwrap();
        let actual_fdt = fdt.finish().unwrap();
        let expected_fdt = vec![
            0xd0, 0x0d, 0xfe, 0xed, // 0000: magic (0xd00dfeed)
            0x00, 0x00, 0x00, 0x59, // 0004: totalsize (0x59)
            0x00, 0x00, 0x00, 0x38, // 0008: off_dt_struct (0x38)
            0x00, 0x00, 0x00, 0x54, // 000C: off_dt_strings (0x54)
            0x00, 0x00, 0x00, 0x28, // 0010: off_mem_rsvmap (0x28)
            0x00, 0x00, 0x00, 0x11, // 0014: version (0x11 = 17)
            0x00, 0x00, 0x00, 0x10, // 0018: last_comp_version (0x10 = 16)
            0x00, 0x00, 0x00, 0x00, // 001C: boot_cpuid_phys (0)
            0x00, 0x00, 0x00, 0x05, // 0020: size_dt_strings (0x05)
            0x00, 0x00, 0x00, 0x1c, // 0024: size_dt_struct (0x1C)
            0x00, 0x00, 0x00, 0x00, // 0028: rsvmap terminator (address = 0 high)
            0x00, 0x00, 0x00, 0x00, // 002C: rsvmap terminator (address = 0 low)
            0x00, 0x00, 0x00, 0x00, // 0030: rsvmap terminator (size = 0 high)
            0x00, 0x00, 0x00, 0x00, // 0034: rsvmap terminator (size = 0 low)
            0x00, 0x00, 0x00, 0x01, // 0038: FDT_BEGIN_NODE
            0x00, 0x00, 0x00, 0x00, // 003C: node name ("") + padding
            0x00, 0x00, 0x00, 0x03, // 0040: FDT_PROP
            0x00, 0x00, 0x00, 0x00, // 0044: prop len (0)
            0x00, 0x00, 0x00, 0x00, // 0048: prop nameoff (0)
            0x00, 0x00, 0x00, 0x02, // 004C: FDT_END_NODE
            0x00, 0x00, 0x00, 0x09, // 0050: FDT_END
            b'n', b'u', b'l', b'l', 0x00, // 0054: strings block
        ];
        assert_eq!(expected_fdt, actual_fdt);
    }

    #[test]
    fn prop_u32() {
        let mut fdt = FdtWriter::new().unwrap();
        let root_node = fdt.begin_node("").unwrap();
        fdt.property_u32("u32", 0x12345678).unwrap();
        fdt.end_node(root_node).unwrap();
        let actual_fdt = fdt.finish().unwrap();
        let expected_fdt = vec![
            0xd0, 0x0d, 0xfe, 0xed, // 0000: magic (0xd00dfeed)
            0x00, 0x00, 0x00, 0x5c, // 0004: totalsize (0x5C)
            0x00, 0x00, 0x00, 0x38, // 0008: off_dt_struct (0x38)
            0x00, 0x00, 0x00, 0x58, // 000C: off_dt_strings (0x58)
            0x00, 0x00, 0x00, 0x28, // 0010: off_mem_rsvmap (0x28)
            0x00, 0x00, 0x00, 0x11, // 0014: version (0x11 = 17)
            0x00, 0x00, 0x00, 0x10, // 0018: last_comp_version (0x10 = 16)
            0x00, 0x00, 0x00, 0x00, // 001C: boot_cpuid_phys (0)
            0x00, 0x00, 0x00, 0x04, // 0020: size_dt_strings (0x04)
            0x00, 0x00, 0x00, 0x20, // 0024: size_dt_struct (0x20)
            0x00, 0x00, 0x00, 0x00, // 0028: rsvmap terminator (address = 0 high)
            0x00, 0x00, 0x00, 0x00, // 002C: rsvmap terminator (address = 0 low)
            0x00, 0x00, 0x00, 0x00, // 0030: rsvmap terminator (size = 0 high)
            0x00, 0x00, 0x00, 0x00, // 0034: rsvmap terminator (size = 0 low)
            0x00, 0x00, 0x00, 0x01, // 0038: FDT_BEGIN_NODE
            0x00, 0x00, 0x00, 0x00, // 003C: node name ("") + padding
            0x00, 0x00, 0x00, 0x03, // 0040: FDT_PROP
            0x00, 0x00, 0x00, 0x04, // 0044: prop len (4)
            0x00, 0x00, 0x00, 0x00, // 0048: prop nameoff (0)
            0x12, 0x34, 0x56, 0x78, // 004C: prop u32 value (0x12345678)
            0x00, 0x00, 0x00, 0x02, // 0050: FDT_END_NODE
            0x00, 0x00, 0x00, 0x09, // 0054: FDT_END
            b'u', b'3', b'2', 0x00, // 0058: strings block
        ];
        assert_eq!(expected_fdt, actual_fdt);
    }

    #[test]
    fn all_props() {
        let mut fdt = FdtWriter::new().unwrap();
        let root_node = fdt.begin_node("").unwrap();
        fdt.property_null("null").unwrap();
        fdt.property_u32("u32", 0x12345678).unwrap();
        fdt.property_u64("u64", 0x1234567887654321).unwrap();
        fdt.property_string("str", "hello").unwrap();
        fdt.property_string_list("strlst", vec!["hi".into(), "bye".into()])
            .unwrap();
        fdt.property_array_u32("arru32", &[0x12345678, 0xAABBCCDD])
            .unwrap();
        fdt.property_array_u64("arru64", &[0x1234567887654321])
            .unwrap();
        fdt.end_node(root_node).unwrap();
        let actual_fdt = fdt.finish().unwrap();
        let expected_fdt = vec![
            0xd0, 0x0d, 0xfe, 0xed, // 0000: magic (0xd00dfeed)
            0x00, 0x00, 0x00, 0xee, // 0004: totalsize (0xEE)
            0x00, 0x00, 0x00, 0x38, // 0008: off_dt_struct (0x38)
            0x00, 0x00, 0x00, 0xc8, // 000C: off_dt_strings (0xC8)
            0x00, 0x00, 0x00, 0x28, // 0010: off_mem_rsvmap (0x28)
            0x00, 0x00, 0x00, 0x11, // 0014: version (0x11 = 17)
            0x00, 0x00, 0x00, 0x10, // 0018: last_comp_version (0x10 = 16)
            0x00, 0x00, 0x00, 0x00, // 001C: boot_cpuid_phys (0)
            0x00, 0x00, 0x00, 0x26, // 0020: size_dt_strings (0x26)
            0x00, 0x00, 0x00, 0x90, // 0024: size_dt_struct (0x90)
            0x00, 0x00, 0x00, 0x00, // 0028: rsvmap terminator (address = 0 high)
            0x00, 0x00, 0x00, 0x00, // 002C: rsvmap terminator (address = 0 low)
            0x00, 0x00, 0x00, 0x00, // 0030: rsvmap terminator (size = 0 high)
            0x00, 0x00, 0x00, 0x00, // 0034: rsvmap terminator (size = 0 low)
            0x00, 0x00, 0x00, 0x01, // 0038: FDT_BEGIN_NODE
            0x00, 0x00, 0x00, 0x00, // 003C: node name ("") + padding
            0x00, 0x00, 0x00, 0x03, // 0040: FDT_PROP (null)
            0x00, 0x00, 0x00, 0x00, // 0044: prop len (0)
            0x00, 0x00, 0x00, 0x00, // 0048: prop nameoff (0)
            0x00, 0x00, 0x00, 0x03, // 004C: FDT_PROP (u32)
            0x00, 0x00, 0x00, 0x04, // 0050: prop len (4)
            0x00, 0x00, 0x00, 0x05, // 0054: prop nameoff (0x05)
            0x12, 0x34, 0x56, 0x78, // 0058: prop u32 value (0x12345678)
            0x00, 0x00, 0x00, 0x03, // 005C: FDT_PROP (u64)
            0x00, 0x00, 0x00, 0x08, // 0060: prop len (8)
            0x00, 0x00, 0x00, 0x09, // 0064: prop nameoff (0x09)
            0x12, 0x34, 0x56, 0x78, // 0068: prop u64 value high (0x12345678)
            0x87, 0x65, 0x43, 0x21, // 006C: prop u64 value low (0x87654321)
            0x00, 0x00, 0x00, 0x03, // 0070: FDT_PROP (string)
            0x00, 0x00, 0x00, 0x06, // 0074: prop len (6)
            0x00, 0x00, 0x00, 0x0D, // 0078: prop nameoff (0x0D)
            b'h', b'e', b'l', b'l', // 007C: prop str value ("hello") + padding
            b'o', 0x00, 0x00, 0x00, // 0080: "o\0" + padding
            0x00, 0x00, 0x00, 0x03, // 0084: FDT_PROP (string list)
            0x00, 0x00, 0x00, 0x07, // 0088: prop len (7)
            0x00, 0x00, 0x00, 0x11, // 008C: prop nameoff (0x11)
            b'h', b'i', 0x00, b'b', // 0090: prop value ("hi", "bye")
            b'y', b'e', 0x00, 0x00, // 0094: "ye\0" + padding
            0x00, 0x00, 0x00, 0x03, // 0098: FDT_PROP (u32 array)
            0x00, 0x00, 0x00, 0x08, // 009C: prop len (8)
            0x00, 0x00, 0x00, 0x18, // 00A0: prop nameoff (0x18)
            0x12, 0x34, 0x56, 0x78, // 00A4: prop value 0
            0xAA, 0xBB, 0xCC, 0xDD, // 00A8: prop value 1
            0x00, 0x00, 0x00, 0x03, // 00AC: FDT_PROP (u64 array)
            0x00, 0x00, 0x00, 0x08, // 00B0: prop len (8)
            0x00, 0x00, 0x00, 0x1f, // 00B4: prop nameoff (0x1F)
            0x12, 0x34, 0x56, 0x78, // 00B8: prop u64 value 0 high
            0x87, 0x65, 0x43, 0x21, // 00BC: prop u64 value 0 low
            0x00, 0x00, 0x00, 0x02, // 00C0: FDT_END_NODE
            0x00, 0x00, 0x00, 0x09, // 00C4: FDT_END
            b'n', b'u', b'l', b'l', 0x00, // 00C8: strings + 0x00: "null""
            b'u', b'3', b'2', 0x00, // 00CD: strings + 0x05: "u32"
            b'u', b'6', b'4', 0x00, // 00D1: strings + 0x09: "u64"
            b's', b't', b'r', 0x00, // 00D5: strings + 0x0D: "str"
            b's', b't', b'r', b'l', b's', b't', 0x00, // 00D9: strings + 0x11: "strlst"
            b'a', b'r', b'r', b'u', b'3', b'2', 0x00, // 00E0: strings + 0x18: "arru32"
            b'a', b'r', b'r', b'u', b'6', b'4', 0x00, // 00E7: strings + 0x1F: "arru64"
        ];
        assert_eq!(expected_fdt, actual_fdt);
    }

    #[test]
    fn property_before_begin_node() {
        let mut fdt = FdtWriter::new().unwrap();
        // Test that adding a property at the beginning of the FDT blob does not work.
        assert_eq!(
            fdt.property_string("invalid", "property").unwrap_err(),
            Error::PropertyBeforeBeginNode
        );

        // Test that adding a property after the end node does not work.
        let node = fdt.begin_node("root").unwrap();
        fdt.end_node(node).unwrap();
        assert_eq!(
            fdt.property_string("invalid", "property").unwrap_err(),
            Error::PropertyAfterEndNode
        );
    }

    #[test]
    fn nested_nodes() {
        let mut fdt = FdtWriter::new().unwrap();
        let root_node = fdt.begin_node("").unwrap();
        fdt.property_u32("abc", 0x13579024).unwrap();
        let nested_node = fdt.begin_node("nested").unwrap();
        fdt.property_u32("def", 0x12121212).unwrap();
        fdt.end_node(nested_node).unwrap();
        fdt.end_node(root_node).unwrap();
        let actual_fdt = fdt.finish().unwrap();
        let expected_fdt = vec![
            0xd0, 0x0d, 0xfe, 0xed, // 0000: magic (0xd00dfeed)
            0x00, 0x00, 0x00, 0x80, // 0004: totalsize (0x80)
            0x00, 0x00, 0x00, 0x38, // 0008: off_dt_struct (0x38)
            0x00, 0x00, 0x00, 0x78, // 000C: off_dt_strings (0x78)
            0x00, 0x00, 0x00, 0x28, // 0010: off_mem_rsvmap (0x28)
            0x00, 0x00, 0x00, 0x11, // 0014: version (0x11 = 17)
            0x00, 0x00, 0x00, 0x10, // 0018: last_comp_version (0x10 = 16)
            0x00, 0x00, 0x00, 0x00, // 001C: boot_cpuid_phys (0)
            0x00, 0x00, 0x00, 0x08, // 0020: size_dt_strings (0x08)
            0x00, 0x00, 0x00, 0x40, // 0024: size_dt_struct (0x40)
            0x00, 0x00, 0x00, 0x00, // 0028: rsvmap terminator (address = 0 high)
            0x00, 0x00, 0x00, 0x00, // 002C: rsvmap terminator (address = 0 low)
            0x00, 0x00, 0x00, 0x00, // 0030: rsvmap terminator (size = 0 high)
            0x00, 0x00, 0x00, 0x00, // 0034: rsvmap terminator (size = 0 low)
            0x00, 0x00, 0x00, 0x01, // 0038: FDT_BEGIN_NODE
            0x00, 0x00, 0x00, 0x00, // 003C: node name ("") + padding
            0x00, 0x00, 0x00, 0x03, // 0040: FDT_PROP
            0x00, 0x00, 0x00, 0x04, // 0044: prop len (4)
            0x00, 0x00, 0x00, 0x00, // 0048: prop nameoff (0x00)
            0x13, 0x57, 0x90, 0x24, // 004C: prop u32 value (0x13579024)
            0x00, 0x00, 0x00, 0x01, // 0050: FDT_BEGIN_NODE
            b'n', b'e', b's', b't', // 0054: Node name ("nested")
            b'e', b'd', 0x00, 0x00, // 0058: "ed\0" + pad
            0x00, 0x00, 0x00, 0x03, // 005C: FDT_PROP
            0x00, 0x00, 0x00, 0x04, // 0060: prop len (4)
            0x00, 0x00, 0x00, 0x04, // 0064: prop nameoff (0x04)
            0x12, 0x12, 0x12, 0x12, // 0068: prop u32 value (0x12121212)
            0x00, 0x00, 0x00, 0x02, // 006C: FDT_END_NODE ("nested")
            0x00, 0x00, 0x00, 0x02, // 0070: FDT_END_NODE ("")
            0x00, 0x00, 0x00, 0x09, // 0074: FDT_END
            b'a', b'b', b'c', 0x00, // 0078: strings + 0x00: "abc"
            b'd', b'e', b'f', 0x00, // 007C: strings + 0x04: "def"
        ];
        assert_eq!(expected_fdt, actual_fdt);
    }

    #[test]
    fn prop_name_string_reuse() {
        let mut fdt = FdtWriter::new().unwrap();
        let root_node = fdt.begin_node("").unwrap();
        fdt.property_u32("abc", 0x13579024).unwrap();
        let nested_node = fdt.begin_node("nested").unwrap();
        fdt.property_u32("def", 0x12121212).unwrap();
        fdt.property_u32("abc", 0x12121212).unwrap(); // This should reuse the "abc" string.
        fdt.end_node(nested_node).unwrap();
        fdt.end_node(root_node).unwrap();
        let actual_fdt = fdt.finish().unwrap();
        let expected_fdt = vec![
            0xd0, 0x0d, 0xfe, 0xed, // 0000: magic (0xd00dfeed)
            0x00, 0x00, 0x00, 0x90, // 0004: totalsize (0x90)
            0x00, 0x00, 0x00, 0x38, // 0008: off_dt_struct (0x38)
            0x00, 0x00, 0x00, 0x88, // 000C: off_dt_strings (0x88)
            0x00, 0x00, 0x00, 0x28, // 0010: off_mem_rsvmap (0x28)
            0x00, 0x00, 0x00, 0x11, // 0014: version (0x11 = 17)
            0x00, 0x00, 0x00, 0x10, // 0018: last_comp_version (0x10 = 16)
            0x00, 0x00, 0x00, 0x00, // 001C: boot_cpuid_phys (0)
            0x00, 0x00, 0x00, 0x08, // 0020: size_dt_strings (0x08)
            0x00, 0x00, 0x00, 0x50, // 0024: size_dt_struct (0x50)
            0x00, 0x00, 0x00, 0x00, // 0028: rsvmap terminator (address = 0 high)
            0x00, 0x00, 0x00, 0x00, // 002C: rsvmap terminator (address = 0 low)
            0x00, 0x00, 0x00, 0x00, // 0030: rsvmap terminator (size = 0 high)
            0x00, 0x00, 0x00, 0x00, // 0034: rsvmap terminator (size = 0 low)
            0x00, 0x00, 0x00, 0x01, // 0038: FDT_BEGIN_NODE
            0x00, 0x00, 0x00, 0x00, // 003C: node name ("") + padding
            0x00, 0x00, 0x00, 0x03, // 0040: FDT_PROP
            0x00, 0x00, 0x00, 0x04, // 0044: prop len (4)
            0x00, 0x00, 0x00, 0x00, // 0048: prop nameoff (0x00)
            0x13, 0x57, 0x90, 0x24, // 004C: prop u32 value (0x13579024)
            0x00, 0x00, 0x00, 0x01, // 0050: FDT_BEGIN_NODE
            b'n', b'e', b's', b't', // 0054: Node name ("nested")
            b'e', b'd', 0x00, 0x00, // 0058: "ed\0" + pad
            0x00, 0x00, 0x00, 0x03, // 005C: FDT_PROP
            0x00, 0x00, 0x00, 0x04, // 0060: prop len (4)
            0x00, 0x00, 0x00, 0x04, // 0064: prop nameoff (0x04)
            0x12, 0x12, 0x12, 0x12, // 0068: prop u32 value (0x12121212)
            0x00, 0x00, 0x00, 0x03, // 006C: FDT_PROP
            0x00, 0x00, 0x00, 0x04, // 0070: prop len (4)
            0x00, 0x00, 0x00, 0x00, // 0074: prop nameoff (0x00 - reuse)
            0x12, 0x12, 0x12, 0x12, // 0078: prop u32 value (0x12121212)
            0x00, 0x00, 0x00, 0x02, // 007C: FDT_END_NODE ("nested")
            0x00, 0x00, 0x00, 0x02, // 0080: FDT_END_NODE ("")
            0x00, 0x00, 0x00, 0x09, // 0084: FDT_END
            b'a', b'b', b'c', 0x00, // 0088: strings + 0x00: "abc"
            b'd', b'e', b'f', 0x00, // 008C: strings + 0x04: "def"
        ];
        assert_eq!(expected_fdt, actual_fdt);
    }

    #[test]
    fn boot_cpuid() {
        let mut fdt = FdtWriter::new().unwrap();
        fdt.set_boot_cpuid_phys(0x12345678);
        let root_node = fdt.begin_node("").unwrap();
        fdt.end_node(root_node).unwrap();
        let actual_fdt = fdt.finish().unwrap();
        let expected_fdt = vec![
            0xd0, 0x0d, 0xfe, 0xed, // 0000: magic (0xd00dfeed)
            0x00, 0x00, 0x00, 0x48, // 0004: totalsize (0x48)
            0x00, 0x00, 0x00, 0x38, // 0008: off_dt_struct (0x38)
            0x00, 0x00, 0x00, 0x48, // 000C: off_dt_strings (0x48)
            0x00, 0x00, 0x00, 0x28, // 0010: off_mem_rsvmap (0x28)
            0x00, 0x00, 0x00, 0x11, // 0014: version (0x11 = 17)
            0x00, 0x00, 0x00, 0x10, // 0018: last_comp_version (0x10 = 16)
            0x12, 0x34, 0x56, 0x78, // 001C: boot_cpuid_phys (0x12345678)
            0x00, 0x00, 0x00, 0x00, // 0020: size_dt_strings (0)
            0x00, 0x00, 0x00, 0x10, // 0024: size_dt_struct (0x10)
            0x00, 0x00, 0x00, 0x00, // 0028: rsvmap terminator (address = 0 high)
            0x00, 0x00, 0x00, 0x00, // 002C: rsvmap terminator (address = 0 low)
            0x00, 0x00, 0x00, 0x00, // 0030: rsvmap terminator (size = 0 high)
            0x00, 0x00, 0x00, 0x00, // 0034: rsvmap terminator (size = 0 low)
            0x00, 0x00, 0x00, 0x01, // 0038: FDT_BEGIN_NODE
            0x00, 0x00, 0x00, 0x00, // 003C: node name ("") + padding
            0x00, 0x00, 0x00, 0x02, // 0040: FDT_END_NODE
            0x00, 0x00, 0x00, 0x09, // 0044: FDT_END
        ];
        assert_eq!(expected_fdt, actual_fdt);
    }

    #[test]
    fn invalid_node_name_nul() {
        let mut fdt = FdtWriter::new().unwrap();
        fdt.begin_node("root").unwrap();
        assert_eq!(
            fdt.begin_node("abc\0def").unwrap_err(),
            Error::InvalidString
        );
    }

    #[test]
    fn invalid_prop_name_nul() {
        let mut fdt = FdtWriter::new().unwrap();
        fdt.begin_node("root").unwrap();
        assert_eq!(
            fdt.property_u32("abc\0def", 0).unwrap_err(),
            Error::InvalidString
        );
    }

    #[test]
    fn invalid_prop_string_value_nul() {
        let mut fdt = FdtWriter::new().unwrap();
        fdt.begin_node("root").unwrap();
        assert_eq!(
            fdt.property_string("mystr", "abc\0def").unwrap_err(),
            Error::InvalidString
        );
    }

    #[test]
    fn invalid_prop_string_list_value_nul() {
        let mut fdt = FdtWriter::new().unwrap();
        let strs = vec!["test".into(), "abc\0def".into()];
        assert_eq!(
            fdt.property_string_list("mystr", strs).unwrap_err(),
            Error::InvalidString
        );
    }

    #[test]
    fn invalid_prop_after_end_node() {
        let mut fdt = FdtWriter::new().unwrap();
        let _root_node = fdt.begin_node("").unwrap();
        fdt.property_u32("ok_prop", 1234).unwrap();
        let nested_node = fdt.begin_node("mynode").unwrap();
        fdt.property_u32("ok_nested_prop", 5678).unwrap();
        fdt.end_node(nested_node).unwrap();
        assert_eq!(
            fdt.property_u32("bad_prop_after_end_node", 1357)
                .unwrap_err(),
            Error::PropertyAfterEndNode
        );
    }

    #[test]
    fn invalid_end_node_out_of_order() {
        let mut fdt = FdtWriter::new().unwrap();
        let root_node = fdt.begin_node("").unwrap();
        fdt.property_u32("ok_prop", 1234).unwrap();
        let _nested_node = fdt.begin_node("mynode").unwrap();
        assert_eq!(
            fdt.end_node(root_node).unwrap_err(),
            Error::OutOfOrderEndNode
        );
    }

    #[test]
    fn invalid_finish_while_node_open() {
        let mut fdt = FdtWriter::new().unwrap();
        let _root_node = fdt.begin_node("").unwrap();
        fdt.property_u32("ok_prop", 1234).unwrap();
        let _nested_node = fdt.begin_node("mynode").unwrap();
        fdt.property_u32("ok_nested_prop", 5678).unwrap();
        assert_eq!(fdt.finish().unwrap_err(), Error::UnclosedNode);
    }

    #[test]
    #[cfg(feature = "long_running_test")]
    fn test_overflow_subtract() {
        let overflow_size = u32::MAX / std::mem::size_of::<FdtReserveEntry>() as u32 - 3;
        let too_large_mem_reserve: Vec<FdtReserveEntry> = (0..overflow_size)
            .map(|i| FdtReserveEntry::new(u64::from(i) * 2, 1).unwrap())
            .collect();
        let mut fdt = FdtWriter::new_with_mem_reserv(&too_large_mem_reserve).unwrap();
        let root_node = fdt.begin_node("").unwrap();
        fdt.end_node(root_node).unwrap();
        assert_eq!(fdt.finish().unwrap_err(), Error::TotalSizeTooLarge);
    }

    #[test]
    fn test_invalid_mem_reservations() {
        // Test that we cannot create an invalid FDT reserve entry where the
        // end address of the region would not fit in an u64.
        assert_eq!(
            FdtReserveEntry::new(0x1, u64::MAX).unwrap_err(),
            Error::InvalidMemoryReservation
        );

        // Test that we cannot have a memory reservation with size 0.
        assert_eq!(
            FdtReserveEntry::new(0x1, 0).unwrap_err(),
            Error::InvalidMemoryReservation
        );
    }

    #[test]
    fn test_cmp_mem_reservations() {
        // Test that just the address is taken into consideration when comparing to `FdtReserveEntry`.
        assert_eq!(
            FdtReserveEntry::new(0x1, 10)
                .unwrap()
                .cmp(&FdtReserveEntry::new(0x1, 11).unwrap()),
            Ordering::Equal
        );
        assert_eq!(
            FdtReserveEntry::new(0x1, 10)
                .unwrap()
                .cmp(&FdtReserveEntry::new(0x2, 10).unwrap()),
            Ordering::Less
        );
        assert_eq!(
            FdtReserveEntry::new(0x1, 10)
                .unwrap()
                .cmp(&FdtReserveEntry::new(0x0, 10).unwrap()),
            Ordering::Greater
        );
    }

    #[test]
    fn test_overlapping_mem_reservations() {
        // Check that regions that overlap return an error on new.
        // Check overlapping by one.
        let overlapping = [
            FdtReserveEntry::new(0x3, 1).unwrap(), // this overlaps with
            FdtReserveEntry::new(0x0, 1).unwrap(),
            FdtReserveEntry::new(0x2, 2).unwrap(), // this one.
        ];
        let fdt = FdtWriter::new_with_mem_reserv(&overlapping);
        assert_eq!(fdt.unwrap_err(), Error::OverlappingMemoryReservations);

        // Check a larger overlap.
        let overlapping = [
            FdtReserveEntry::new(0x100, 100).unwrap(),
            FdtReserveEntry::new(0x50, 300).unwrap(),
        ];
        let fdt = FdtWriter::new_with_mem_reserv(&overlapping);
        assert_eq!(fdt.unwrap_err(), Error::OverlappingMemoryReservations);
    }

    #[test]
    fn test_off_by_one_mem_rsv() {
        // This test is for making sure we do not introduce off by one errors
        // in the memory reservations checks.
        let non_overlapping = [
            FdtReserveEntry::new(0x0, 1).unwrap(),
            FdtReserveEntry::new(0x1, 1).unwrap(),
            FdtReserveEntry::new(0x2, 2).unwrap(),
        ];

        assert!(FdtWriter::new_with_mem_reserv(&non_overlapping).is_ok());
    }

    #[test]
    fn test_node_name_valid() {
        assert!(node_name_valid("abcdef"));
        assert!(node_name_valid("abcdef@1000"));
        assert!(node_name_valid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(node_name_valid("azAZ09,._+-"));
        assert!(node_name_valid("Abcd"));

        assert!(node_name_valid(""));

        // Name missing.
        assert!(!node_name_valid("@1000"));

        // Name too long.
        assert!(!node_name_valid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(!node_name_valid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa@1234"));

        // Name contains invalid characters.
        assert!(!node_name_valid("abc#def"));
        assert!(!node_name_valid("abc/def"));

        // Name begins with non-alphabetic character.
        assert!(!node_name_valid("1abc"));

        // Unit address contains invalid characters.
        assert!(!node_name_valid("abcdef@1000#"));

        // More than one '@'.
        assert!(!node_name_valid("abc@1000@def"));
    }

    #[test]
    fn test_property_name_valid() {
        assert!(property_name_valid("abcdef"));
        assert!(property_name_valid("01234"));
        assert!(property_name_valid("azAZ09,._+?#-"));
        assert!(property_name_valid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));

        // Name contains invalid characters.
        assert!(!property_name_valid("abc!def"));
        assert!(!property_name_valid("abc@1234"));

        // Name too long.
        assert!(!property_name_valid("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    }

    #[test]
    fn depth_overflow() {
        let mut fdt = FdtWriter::new().unwrap();
        for _ in 1..=FDT_MAX_NODE_DEPTH {
            fdt.begin_node("test").unwrap();
        }
        assert_eq!(
            fdt.begin_node("test").unwrap_err(),
            Error::NodeDepthTooLarge
        );
    }

    #[test]
    fn unique_phandles() {
        let mut fdt = FdtWriter::new().unwrap();
        let root_node = fdt.begin_node("root").unwrap();

        let prim_node = fdt.begin_node("phandle-1").unwrap();
        fdt.property_phandle(1).unwrap();
        fdt.end_node(prim_node).unwrap();

        let prim_node = fdt.begin_node("phandle-2").unwrap();
        fdt.property_phandle(2).unwrap();
        fdt.end_node(prim_node).unwrap();

        fdt.end_node(root_node).unwrap();
        fdt.finish().unwrap();
    }

    #[test]
    fn duplicate_phandles() {
        let mut fdt = FdtWriter::new().unwrap();
        let _root_node = fdt.begin_node("root").unwrap();

        let prim_node = fdt.begin_node("phandle-1").unwrap();
        fdt.property_phandle(1).unwrap();
        fdt.end_node(prim_node).unwrap();

        let _sec_node = fdt.begin_node("phandle-2").unwrap();
        assert_eq!(
            fdt.property_phandle(1).unwrap_err(),
            Error::DuplicatePhandle
        );
    }
}
