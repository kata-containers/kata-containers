// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Structs to manage guest memory for virtual machines.
//!
//! The `vm-memory` crate only provides traits and structs to access normal guest memory,
//! it doesn't support special guest memory like virtio-fs/virtio-pmem DAX window etc.
//! So this crate provides `GuestMemoryManager` over `vm-memory` to provide uniform abstraction
//! for all guest memory.
//!
//! It also provides interfaces to coordinate guest memory hotplug events.

use std::str::FromStr;
use std::sync::Arc;
use vm_memory::{GuestAddressSpace, GuestMemoryAtomic, GuestMemoryLoadGuard, GuestMemoryMmap};

mod raw_region;
pub use raw_region::GuestRegionRaw;

mod hybrid;
pub use hybrid::{GuestMemoryHybrid, GuestRegionHybrid};

/// Type of source to allocate memory for virtual machines.
#[derive(Debug, Eq, PartialEq)]
pub enum MemorySourceType {
    /// File on HugeTlbFs.
    FileOnHugeTlbFs,
    /// mmap() without flag `MAP_HUGETLB`.
    MmapAnonymous,
    /// mmap() with flag `MAP_HUGETLB`.
    MmapAnonymousHugeTlbFs,
    /// memfd() without flag `MFD_HUGETLB`.
    MemFdShared,
    /// memfd() with flag `MFD_HUGETLB`.
    MemFdOnHugeTlbFs,
}

impl MemorySourceType {
    /// Check whether the memory source is huge page.
    pub fn is_hugepage(&self) -> bool {
        *self == Self::FileOnHugeTlbFs
            || *self == Self::MmapAnonymousHugeTlbFs
            || *self == Self::MemFdOnHugeTlbFs
    }

    /// Check whether the memory source is anonymous memory.
    pub fn is_mmap_anonymous(&self) -> bool {
        *self == Self::MmapAnonymous || *self == Self::MmapAnonymousHugeTlbFs
    }
}

impl FromStr for MemorySourceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "hugetlbfs" => Ok(MemorySourceType::FileOnHugeTlbFs),
            "memfd" => Ok(MemorySourceType::MemFdShared),
            "shmem" => Ok(MemorySourceType::MemFdShared),
            "hugememfd" => Ok(MemorySourceType::MemFdOnHugeTlbFs),
            "hugeshmem" => Ok(MemorySourceType::MemFdOnHugeTlbFs),
            "anon" => Ok(MemorySourceType::MmapAnonymous),
            "mmap" => Ok(MemorySourceType::MmapAnonymous),
            "hugeanon" => Ok(MemorySourceType::MmapAnonymousHugeTlbFs),
            "hugemmap" => Ok(MemorySourceType::MmapAnonymousHugeTlbFs),
            _ => Err(format!("unknown memory source type {s}")),
        }
    }
}

#[derive(Debug, Default)]
struct GuestMemoryHotplugManager {}

/// The `GuestMemoryManager` manages all guest memory for virtual machines.
///
/// The `GuestMemoryManager` fulfills several different responsibilities.
/// - First, it manages different types of guest memory, such as normal guest memory, virtio-fs
///   DAX window and virtio-pmem DAX window etc. Different clients may want to access different
///   types of memory. So the manager maintains two GuestMemory objects, one contains all guest
///   memory, the other contains only normal guest memory.
/// - Second, it coordinates memory/DAX window hotplug events, so clients may register hooks
///   to receive hotplug notifications.
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct GuestMemoryManager {
    default: GuestMemoryAtomic<GuestMemoryHybrid>,
    /// GuestMemory object hosts all guest memory.
    hybrid: GuestMemoryAtomic<GuestMemoryHybrid>,
    /// GuestMemory object for vIOMMU.
    iommu: GuestMemoryAtomic<GuestMemoryHybrid>,
    /// GuestMemory object hosts normal guest memory.
    normal: GuestMemoryAtomic<GuestMemoryMmap>,
    hotplug: Arc<GuestMemoryHotplugManager>,
}

impl GuestMemoryManager {
    /// Create a new instance of `GuestMemoryManager`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a reference to the normal `GuestMemory` object.
    pub fn get_normal_guest_memory(&self) -> &GuestMemoryAtomic<GuestMemoryMmap> {
        &self.normal
    }

    /// Try to downcast the `GuestAddressSpace` object to a `GuestMemoryManager` object.
    pub fn to_manager<AS: GuestAddressSpace>(_m: &AS) -> Option<&Self> {
        None
    }
}

impl Default for GuestMemoryManager {
    fn default() -> Self {
        let hybrid = GuestMemoryAtomic::new(GuestMemoryHybrid::new());
        let iommu = GuestMemoryAtomic::new(GuestMemoryHybrid::new());
        let normal = GuestMemoryAtomic::new(GuestMemoryMmap::new());
        // By default, it provides to the `GuestMemoryHybrid` object containing all guest memory.
        let default = hybrid.clone();

        GuestMemoryManager {
            default,
            hybrid,
            iommu,
            normal,
            hotplug: Arc::new(GuestMemoryHotplugManager::default()),
        }
    }
}

impl GuestAddressSpace for GuestMemoryManager {
    type M = GuestMemoryHybrid;
    type T = GuestMemoryLoadGuard<GuestMemoryHybrid>;

    fn memory(&self) -> Self::T {
        self.default.memory()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_source_type() {
        assert_eq!(
            MemorySourceType::from_str("hugetlbfs").unwrap(),
            MemorySourceType::FileOnHugeTlbFs
        );
        assert_eq!(
            MemorySourceType::from_str("memfd").unwrap(),
            MemorySourceType::MemFdShared
        );
        assert_eq!(
            MemorySourceType::from_str("shmem").unwrap(),
            MemorySourceType::MemFdShared
        );
        assert_eq!(
            MemorySourceType::from_str("hugememfd").unwrap(),
            MemorySourceType::MemFdOnHugeTlbFs
        );
        assert_eq!(
            MemorySourceType::from_str("hugeshmem").unwrap(),
            MemorySourceType::MemFdOnHugeTlbFs
        );
        assert_eq!(
            MemorySourceType::from_str("anon").unwrap(),
            MemorySourceType::MmapAnonymous
        );
        assert_eq!(
            MemorySourceType::from_str("mmap").unwrap(),
            MemorySourceType::MmapAnonymous
        );
        assert_eq!(
            MemorySourceType::from_str("hugeanon").unwrap(),
            MemorySourceType::MmapAnonymousHugeTlbFs
        );
        assert_eq!(
            MemorySourceType::from_str("hugemmap").unwrap(),
            MemorySourceType::MmapAnonymousHugeTlbFs
        );
        assert!(MemorySourceType::from_str("test").is_err());
    }

    #[ignore]
    #[test]
    fn test_to_manager() {
        let manager = GuestMemoryManager::new();
        let mgr = GuestMemoryManager::to_manager(&manager).unwrap();

        assert_eq!(&manager as *const _, mgr as *const _);
    }
}
