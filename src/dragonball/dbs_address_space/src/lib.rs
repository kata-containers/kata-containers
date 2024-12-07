// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

#![deny(missing_docs)]

//! Traits and Structs to manage guest physical address space for virtual machines.
//!
//! The [vm-memory](https://crates.io/crates/vm-memory) implements mechanisms to manage and access
//! guest memory resident in guest physical address space. In addition to guest memory, there may
//! be other type of devices resident in the same guest physical address space.
//!
//! The `dbs-address-space` crate provides traits and structs to manage the guest physical address
//! space for virtual machines, and mechanisms to coordinate all the devices resident in the
//! guest physical address space.

use vm_memory::GuestUsize;

mod address_space;
pub use self::address_space::{AddressSpace, AddressSpaceBase};

mod layout;
pub use layout::{AddressSpaceLayout, USABLE_END};

mod memory;
pub use memory::{GuestMemoryHybrid, GuestMemoryManager, GuestRegionHybrid, GuestRegionRaw};

mod numa;
pub use self::numa::{NumaIdTable, NumaNode, NumaNodeInfo, MPOL_MF_MOVE, MPOL_PREFERRED};

mod region;
pub use region::{AddressSpaceRegion, AddressSpaceRegionType};

/// Errors associated with virtual machine address space management.
#[derive(Debug, thiserror::Error)]
pub enum AddressSpaceError {
    /// Invalid address space region type.
    #[error("invalid address space region type")]
    InvalidRegionType,

    /// Invalid address range.
    #[error("invalid address space region (0x{0:x}, 0x{1:x})")]
    InvalidAddressRange(u64, GuestUsize),

    /// Invalid guest memory source type.
    #[error("invalid memory source type {0}")]
    InvalidMemorySourceType(String),

    /// Failed to create memfd to map anonymous memory.
    #[error("can not create memfd to map anonymous memory")]
    CreateMemFd(#[source] nix::Error),

    /// Failed to open memory file.
    #[error("can not open memory file")]
    OpenFile(#[source] std::io::Error),

    /// Failed to create directory.
    #[error("can not create directory")]
    CreateDir(#[source] std::io::Error),

    /// Failed to set size for memory file.
    #[error("can not set size for memory file")]
    SetFileSize(#[source] std::io::Error),

    /// Failed to unlink memory file.
    #[error("can not unlink memory file")]
    UnlinkFile(#[source] nix::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code() {
        let e = AddressSpaceError::InvalidRegionType;

        assert_eq!(format!("{e}"), "invalid address space region type");
        assert_eq!(format!("{e:?}"), "InvalidRegionType");
        assert_eq!(
            format!(
                "{}",
                AddressSpaceError::InvalidMemorySourceType("test".to_string())
            ),
            "invalid memory source type test"
        );
    }
}
