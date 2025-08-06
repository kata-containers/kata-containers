// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use lazy_static::lazy_static;

use crate::{AddressSpaceRegion, AddressSpaceRegionType};

// Max retry times for reading /proc
const PROC_READ_RETRY: u64 = 5;

lazy_static! {
    /// Upper bound of host memory.
    pub static ref USABLE_END: u64 = {
        for _ in 0..PROC_READ_RETRY {
            if let Ok(buf) = std::fs::read("/proc/meminfo") {
                let content = String::from_utf8_lossy(&buf);
                for line in content.lines() {
                    if line.starts_with("MemTotal:") {
                        if let Some(end) = line.find(" kB") {
                            if let Ok(size) = line[9..end].trim().parse::<u64>() {
                                return (size << 10) - 1;
                            }
                        }
                    }
                }
            }
        }
        panic!("Exceed max retry times. Cannot get total mem size from /proc/meminfo");
    };
}

/// Address space layout configuration.
///
/// The layout configuration must guarantee that `mem_start` <= `mem_end` <= `phys_end`.
/// Non-memory region should be arranged into the range [mem_end, phys_end).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressSpaceLayout {
    /// end of guest physical address
    pub phys_end: u64,
    /// start of guest memory address
    pub mem_start: u64,
    /// end of guest memory address
    pub mem_end: u64,
    /// end of usable memory address
    pub usable_end: u64,
}

impl AddressSpaceLayout {
    /// Create a new instance of `AddressSpaceLayout`.
    pub fn new(phys_end: u64, mem_start: u64, mem_end: u64) -> Self {
        AddressSpaceLayout {
            phys_end,
            mem_start,
            mem_end,
            usable_end: *USABLE_END,
        }
    }

    /// Check whether an region is valid with the constraints of the layout.
    pub fn is_region_valid(&self, region: &AddressSpaceRegion) -> bool {
        let region_end = match region.base.0.checked_add(region.size) {
            None => return false,
            Some(v) => v,
        };

        match region.ty {
            AddressSpaceRegionType::DefaultMemory => {
                if region.base.0 < self.mem_start || region_end > self.mem_end {
                    return false;
                }
            }
            AddressSpaceRegionType::DeviceMemory | AddressSpaceRegionType::DAXMemory => {
                if region.base.0 < self.mem_end || region_end > self.phys_end {
                    return false;
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vm_memory::GuestAddress;

    #[test]
    fn test_is_region_valid() {
        let layout = AddressSpaceLayout::new(0x1_0000_0000, 0x1000_0000, 0x2000_0000);

        let region = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x0),
            0x1_0000,
        );
        assert!(!layout.is_region_valid(&region));
        let region = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x2000_0000),
            0x1_0000,
        );
        assert!(!layout.is_region_valid(&region));
        let region = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x1_0000),
            0x2000_0000,
        );
        assert!(!layout.is_region_valid(&region));
        let region = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(u64::MAX),
            0x1_0000_0000,
        );
        assert!(!layout.is_region_valid(&region));
        let region = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x1000_0000),
            0x1_0000,
        );
        assert!(layout.is_region_valid(&region));

        let region = AddressSpaceRegion::new(
            AddressSpaceRegionType::DeviceMemory,
            GuestAddress(0x1000_0000),
            0x1_0000,
        );
        assert!(!layout.is_region_valid(&region));
        let region = AddressSpaceRegion::new(
            AddressSpaceRegionType::DeviceMemory,
            GuestAddress(0x1_0000_0000),
            0x1_0000,
        );
        assert!(!layout.is_region_valid(&region));
        let region = AddressSpaceRegion::new(
            AddressSpaceRegionType::DeviceMemory,
            GuestAddress(0x1_0000),
            0x1_0000_0000,
        );
        assert!(!layout.is_region_valid(&region));
        let region = AddressSpaceRegion::new(
            AddressSpaceRegionType::DeviceMemory,
            GuestAddress(u64::MAX),
            0x1_0000_0000,
        );
        assert!(!layout.is_region_valid(&region));
        let region = AddressSpaceRegion::new(
            AddressSpaceRegionType::DeviceMemory,
            GuestAddress(0x8000_0000),
            0x1_0000,
        );
        assert!(layout.is_region_valid(&region));
    }
}
