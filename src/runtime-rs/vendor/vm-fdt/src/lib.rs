// Copyright 2021 The Chromium OS Authors. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

#![deny(missing_docs)]

//! This crate provides the ability to manipulate Flattened Devicetree blobs.
//!
//! # Example
//!
//! In the following example we create an FDT blob with a root node, and 2
//! children nodes. More details about this example are available in the readme.
//!
//! ```rust
//! use vm_fdt::{Error, FdtWriter};
//!
//! fn create_fdt() -> Result<Vec<u8>, Error> {
//!     let mut fdt = FdtWriter::new()?;
//!
//!     let root_node = fdt.begin_node("root")?;
//!     fdt.property_string("compatible", "linux,dummy-virt")?;
//!     fdt.property_u32("#address-cells", 0x2)?;
//!     fdt.property_u32("#size-cells", 0x2)?;
//!
//!     let chosen_node = fdt.begin_node("chosen")?;
//!     fdt.property_u32("linux,pci-probe-only", 1)?;
//!     fdt.property_string("bootargs", "panic=-1 console=hvc0")?;
//!     fdt.end_node(chosen_node)?;
//!
//!     let memory_node = fdt.begin_node("memory")?;
//!     fdt.property_string("device_type", "memory")?;
//!     fdt.end_node(memory_node)?;
//!
//!     fdt.end_node(root_node)?;
//!
//!     fdt.finish()
//! }
//!
//! # let dtb = create_fdt().unwrap();
//! ```
//!
//! By default the FDT does not have any memory reservations. If the user
//! needs to add memory reservations as well, then a different constructor
//! can be used as follows:
//!
//! ```rust
//! use vm_fdt::{Error, FdtReserveEntry, FdtWriter};
//!
//! fn create_fdt() -> Result<Vec<u8>, Error> {
//!     let mut fdt = FdtWriter::new_with_mem_reserv(&[
//!         FdtReserveEntry::new(0x12345678AABBCCDD, 0x1234).unwrap(),
//!         FdtReserveEntry::new(0x1020304050607080, 0x5678).unwrap(),
//!     ])?;
//!     let root_node = fdt.begin_node("root")?;
//!     // ... add other nodes & properties
//!     fdt.end_node(root_node)?;
//!
//!     fdt.finish()
//! }
//!
//! # let dtb = create_fdt().unwrap();
//! ```
//!
//! The [`phandle`](https://devicetree-specification.readthedocs.io/en/stable/devicetree-basics.html?#phandle)
//! property should be set using [`FdtWriter::property_phandle`],
//! so that the value is checked for uniqueness within the devicetree.
//!
//! ```rust
//! use vm_fdt::{Error, FdtWriter};
//!
//! fn create_fdt() -> Result<Vec<u8>, Error> {
//!     let mut fdt = FdtWriter::new()?;
//!
//!     let root_node = fdt.begin_node("root")?;
//!     fdt.property_phandle(1)?;
//!
//!     fdt.end_node(root_node)?;
//!
//!     fdt.finish()
//! }
//!
//! # let dtb = create_fdt().unwrap();
//! ```

mod writer;

pub use writer::Result as FdtWriterResult;
pub use writer::{Error, FdtReserveEntry, FdtWriter, FdtWriterNode};

/// Magic number used in the FDT header.
const FDT_MAGIC: u32 = 0xd00dfeed;

const FDT_BEGIN_NODE: u32 = 0x00000001;
const FDT_END_NODE: u32 = 0x00000002;
const FDT_PROP: u32 = 0x00000003;
const FDT_END: u32 = 0x00000009;

const NODE_NAME_MAX_LEN: usize = 31;
const PROPERTY_NAME_MAX_LEN: usize = 31;
