// Copyright (C) 2021 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::os::unix::io::FromRawFd;
use std::path::Path;
use std::str::FromStr;

use nix::sys::memfd;
use vm_memory::{Address, FileOffset, GuestAddress, GuestUsize};

use crate::memory::MemorySourceType;
use crate::memory::MemorySourceType::MemFdShared;
use crate::AddressSpaceError;

/// Type of address space regions.
///
/// On physical machines, physical memory may have different properties, such as
/// volatile vs non-volatile, read-only vs read-write, non-executable vs executable etc.
/// On virtual machines, the concept of memory property may be extended to support better
/// cooperation between the hypervisor and the guest kernel. Here address space region type means
/// what the region will be used for by the guest OS, and different permissions and policies may
/// be applied to different address space regions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AddressSpaceRegionType {
    /// Normal memory accessible by CPUs and IO devices.
    DefaultMemory,
    /// MMIO address region for Devices.
    DeviceMemory,
    /// DAX address region for virtio-fs/virtio-pmem.
    DAXMemory,
}

/// Struct to maintain configuration information about a guest address region.
#[derive(Debug, Clone)]
pub struct AddressSpaceRegion {
    /// Type of address space regions.
    pub ty: AddressSpaceRegionType,
    /// Base address of the region in virtual machine's physical address space.
    pub base: GuestAddress,
    /// Size of the address space region.
    pub size: GuestUsize,
    /// Host NUMA node ids assigned to this region.
    pub host_numa_node_id: Option<u32>,

    /// File/offset tuple to back the memory allocation.
    file_offset: Option<FileOffset>,
    /// Mmap permission flags.
    perm_flags: i32,
    /// Mmap protection flags.
    prot_flags: i32,
    /// Hugepage madvise hint.
    ///
    /// It needs 'advise' or 'always' policy in host shmem config.
    is_hugepage: bool,
    /// Hotplug hint.
    is_hotplug: bool,
    /// Anonymous memory hint.
    ///
    /// It should be true for regions with the MADV_DONTFORK flag enabled.
    is_anon: bool,
}

#[allow(clippy::too_many_arguments)]
impl AddressSpaceRegion {
    /// Create an address space region with default configuration.
    pub fn new(ty: AddressSpaceRegionType, base: GuestAddress, size: GuestUsize) -> Self {
        AddressSpaceRegion {
            ty,
            base,
            size,
            host_numa_node_id: None,
            file_offset: None,
            perm_flags: libc::MAP_SHARED,
            prot_flags: libc::PROT_READ | libc::PROT_WRITE,
            is_hugepage: false,
            is_hotplug: false,
            is_anon: false,
        }
    }

    /// Create an address space region with all configurable information.
    ///
    /// # Arguments
    /// * `ty` - Type of the address region
    /// * `base` - Base address in VM to map content
    /// * `size` - Length of content to map
    /// * `numa_node_id` - Optional NUMA node id to allocate memory from
    /// * `file_offset` - Optional file descriptor and offset to map content from
    /// * `perm_flags` - mmap permission flags
    /// * `prot_flags` - mmap protection flags
    /// * `is_hotplug` - Whether it's a region for hotplug.
    pub fn build(
        ty: AddressSpaceRegionType,
        base: GuestAddress,
        size: GuestUsize,
        host_numa_node_id: Option<u32>,
        file_offset: Option<FileOffset>,
        perm_flags: i32,
        prot_flags: i32,
        is_hotplug: bool,
    ) -> Self {
        let mut region = Self::new(ty, base, size);

        region.set_host_numa_node_id(host_numa_node_id);
        region.set_file_offset(file_offset);
        region.set_perm_flags(perm_flags);
        region.set_prot_flags(prot_flags);
        if is_hotplug {
            region.set_hotplug();
        }

        region
    }

    /// Create an address space region to map memory into the virtual machine.
    ///
    /// # Arguments
    /// * `base` - Base address in VM to map content
    /// * `size` - Length of content to map
    /// * `numa_node_id` - Optional NUMA node id to allocate memory from
    /// * `mem_type` - Memory mapping from, 'shmem' or 'hugetlbfs'
    /// * `mem_file_path` - Memory file path
    /// * `mem_prealloc` - Whether to enable pre-allocation of guest memory
    /// * `is_hotplug` - Whether it's a region for hotplug.
    pub fn create_default_memory_region(
        base: GuestAddress,
        size: GuestUsize,
        numa_node_id: Option<u32>,
        mem_type: &str,
        mem_file_path: &str,
        mem_prealloc: bool,
        is_hotplug: bool,
    ) -> Result<AddressSpaceRegion, AddressSpaceError> {
        Self::create_memory_region(
            base,
            size,
            numa_node_id,
            mem_type,
            mem_file_path,
            mem_prealloc,
            libc::PROT_READ | libc::PROT_WRITE,
            is_hotplug,
        )
    }

    /// Create an address space region to map memory from memfd/hugetlbfs into the virtual machine.
    ///
    /// # Arguments
    /// * `base` - Base address in VM to map content
    /// * `size` - Length of content to map
    /// * `numa_node_id` - Optional NUMA node id to allocate memory from
    /// * `mem_type` - Memory mapping from, 'shmem' or 'hugetlbfs'
    /// * `mem_file_path` - Memory file path
    /// * `mem_prealloc` - Whether to enable pre-allocation of guest memory
    /// * `is_hotplug` - Whether it's a region for hotplug.
    /// * `prot_flags` - mmap protection flags
    pub fn create_memory_region(
        base: GuestAddress,
        size: GuestUsize,
        numa_node_id: Option<u32>,
        mem_type: &str,
        mem_file_path: &str,
        mem_prealloc: bool,
        prot_flags: i32,
        is_hotplug: bool,
    ) -> Result<AddressSpaceRegion, AddressSpaceError> {
        let perm_flags = if mem_prealloc {
            libc::MAP_SHARED | libc::MAP_POPULATE
        } else {
            libc::MAP_SHARED
        };
        let source_type = MemorySourceType::from_str(mem_type)
            .map_err(|_e| AddressSpaceError::InvalidMemorySourceType(mem_type.to_string()))?;
        let mut reg = match source_type {
            MemorySourceType::MemFdShared | MemorySourceType::MemFdOnHugeTlbFs => {
                let fn_str = if source_type == MemFdShared {
                    CString::new("shmem").expect("CString::new('shmem') failed")
                } else {
                    CString::new("hugeshmem").expect("CString::new('hugeshmem') failed")
                };
                let filename = fn_str.as_c_str();
                let fd = memfd::memfd_create(filename, memfd::MemFdCreateFlag::empty())
                    .map_err(AddressSpaceError::CreateMemFd)?;
                // Safe because we have just created the fd.
                let file: File = unsafe { File::from_raw_fd(fd) };
                file.set_len(size).map_err(AddressSpaceError::SetFileSize)?;
                Self::build(
                    AddressSpaceRegionType::DefaultMemory,
                    base,
                    size,
                    numa_node_id,
                    Some(FileOffset::new(file, 0)),
                    perm_flags,
                    prot_flags,
                    is_hotplug,
                )
            }
            MemorySourceType::MmapAnonymous | MemorySourceType::MmapAnonymousHugeTlbFs => {
                let mut perm_flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS;
                if mem_prealloc {
                    perm_flags |= libc::MAP_POPULATE
                }
                Self::build(
                    AddressSpaceRegionType::DefaultMemory,
                    base,
                    size,
                    numa_node_id,
                    None,
                    perm_flags,
                    prot_flags,
                    is_hotplug,
                )
            }
            MemorySourceType::FileOnHugeTlbFs => {
                let path = Path::new(mem_file_path);
                if let Some(parent_dir) = path.parent() {
                    // Ensure that the parent directory is existed for the mem file path.
                    std::fs::create_dir_all(parent_dir).map_err(AddressSpaceError::CreateDir)?;
                }
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(mem_file_path)
                    .map_err(AddressSpaceError::OpenFile)?;
                nix::unistd::unlink(mem_file_path).map_err(AddressSpaceError::UnlinkFile)?;
                file.set_len(size).map_err(AddressSpaceError::SetFileSize)?;
                let file_offset = FileOffset::new(file, 0);
                Self::build(
                    AddressSpaceRegionType::DefaultMemory,
                    base,
                    size,
                    numa_node_id,
                    Some(file_offset),
                    perm_flags,
                    prot_flags,
                    is_hotplug,
                )
            }
        };

        if source_type.is_hugepage() {
            reg.set_hugepage();
        }
        if source_type.is_mmap_anonymous() {
            reg.set_anonpage();
        }

        Ok(reg)
    }

    /// Create an address region for device MMIO.
    ///
    /// # Arguments
    /// * `base` - Base address in VM to map content
    /// * `size` - Length of content to map
    pub fn create_device_region(
        base: GuestAddress,
        size: GuestUsize,
    ) -> Result<AddressSpaceRegion, AddressSpaceError> {
        Ok(Self::build(
            AddressSpaceRegionType::DeviceMemory,
            base,
            size,
            None,
            None,
            0,
            0,
            false,
        ))
    }

    /// Get type of the address space region.
    pub fn region_type(&self) -> AddressSpaceRegionType {
        self.ty
    }

    /// Get size of region.
    pub fn len(&self) -> GuestUsize {
        self.size
    }

    /// Get the inclusive start physical address of the region.
    pub fn start_addr(&self) -> GuestAddress {
        self.base
    }

    /// Get the inclusive end physical address of the region.
    pub fn last_addr(&self) -> GuestAddress {
        debug_assert!(self.size > 0 && self.base.checked_add(self.size).is_some());
        GuestAddress(self.base.raw_value() + self.size - 1)
    }

    /// Get mmap permission flags of the address space region.
    pub fn perm_flags(&self) -> i32 {
        self.perm_flags
    }

    /// Set mmap permission flags for the address space region.
    pub fn set_perm_flags(&mut self, perm_flags: i32) {
        self.perm_flags = perm_flags;
    }

    /// Get mmap protection flags of the address space region.
    pub fn prot_flags(&self) -> i32 {
        self.prot_flags
    }

    /// Set mmap protection flags for the address space region.
    pub fn set_prot_flags(&mut self, prot_flags: i32) {
        self.prot_flags = prot_flags;
    }

    /// Get host_numa_node_id flags
    pub fn host_numa_node_id(&self) -> Option<u32> {
        self.host_numa_node_id
    }

    /// Set associated NUMA node ID to allocate memory from for this region.
    pub fn set_host_numa_node_id(&mut self, host_numa_node_id: Option<u32>) {
        self.host_numa_node_id = host_numa_node_id;
    }

    /// Check whether the address space region is backed by a memory file.
    pub fn has_file(&self) -> bool {
        self.file_offset.is_some()
    }

    /// Get optional file associated with the region.
    pub fn file_offset(&self) -> Option<&FileOffset> {
        self.file_offset.as_ref()
    }

    /// Set associated file/offset pair for the region.
    pub fn set_file_offset(&mut self, file_offset: Option<FileOffset>) {
        self.file_offset = file_offset;
    }

    /// Set the hotplug hint.
    pub fn set_hotplug(&mut self) {
        self.is_hotplug = true
    }

    /// Get the hotplug hint.
    pub fn is_hotplug(&self) -> bool {
        self.is_hotplug
    }

    /// Set hugepage hint for `madvise()`, only takes effect when the memory type is `shmem`.
    pub fn set_hugepage(&mut self) {
        self.is_hugepage = true
    }

    /// Get the hugepage hint.
    pub fn is_hugepage(&self) -> bool {
        self.is_hugepage
    }

    /// Set the anonymous memory hint.
    pub fn set_anonpage(&mut self) {
        self.is_anon = true
    }

    /// Get the anonymous memory hint.
    pub fn is_anonpage(&self) -> bool {
        self.is_anon
    }

    /// Check whether the address space region is valid.
    pub fn is_valid(&self) -> bool {
        self.size > 0 && self.base.checked_add(self.size).is_some()
    }

    /// Check whether the address space region intersects with another one.
    pub fn intersect_with(&self, other: &AddressSpaceRegion) -> bool {
        // Treat invalid address region as intersecting always
        let end1 = match self.base.checked_add(self.size) {
            Some(addr) => addr,
            None => return true,
        };
        let end2 = match other.base.checked_add(other.size) {
            Some(addr) => addr,
            None => return true,
        };

        !(end1 <= other.base || self.base >= end2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use vmm_sys_util::tempfile::TempFile;

    #[test]
    fn test_address_space_region_valid() {
        let reg1 = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0xFFFFFFFFFFFFF000),
            0x2000,
        );
        assert!(!reg1.is_valid());
        let reg1 = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0xFFFFFFFFFFFFF000),
            0x1000,
        );
        assert!(!reg1.is_valid());
        let reg1 = AddressSpaceRegion::new(
            AddressSpaceRegionType::DeviceMemory,
            GuestAddress(0xFFFFFFFFFFFFE000),
            0x1000,
        );
        assert!(reg1.is_valid());
        assert_eq!(reg1.start_addr(), GuestAddress(0xFFFFFFFFFFFFE000));
        assert_eq!(reg1.len(), 0x1000);
        assert!(!reg1.has_file());
        assert!(reg1.file_offset().is_none());
        assert_eq!(reg1.perm_flags(), libc::MAP_SHARED);
        assert_eq!(reg1.prot_flags(), libc::PROT_READ | libc::PROT_WRITE);
        assert_eq!(reg1.region_type(), AddressSpaceRegionType::DeviceMemory);

        let tmp_file = TempFile::new().unwrap();
        let mut f = tmp_file.into_file();
        let sample_buf = &[1, 2, 3, 4, 5];
        assert!(f.write_all(sample_buf).is_ok());
        let reg2 = AddressSpaceRegion::build(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x1000),
            0x1000,
            None,
            Some(FileOffset::new(f, 0x0)),
            0x5a,
            0x5a,
            false,
        );
        assert_eq!(reg2.region_type(), AddressSpaceRegionType::DefaultMemory);
        assert!(reg2.is_valid());
        assert_eq!(reg2.start_addr(), GuestAddress(0x1000));
        assert_eq!(reg2.len(), 0x1000);
        assert!(reg2.has_file());
        assert!(reg2.file_offset().is_some());
        assert_eq!(reg2.perm_flags(), 0x5a);
        assert_eq!(reg2.prot_flags(), 0x5a);
    }

    #[test]
    fn test_address_space_region_intersect() {
        let reg1 = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x1000),
            0x1000,
        );
        let reg2 = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x2000),
            0x1000,
        );
        let reg3 = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x1000),
            0x1001,
        );
        let reg4 = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0x1100),
            0x100,
        );
        let reg5 = AddressSpaceRegion::new(
            AddressSpaceRegionType::DefaultMemory,
            GuestAddress(0xFFFFFFFFFFFFF000),
            0x2000,
        );

        assert!(!reg1.intersect_with(&reg2));
        assert!(!reg2.intersect_with(&reg1));

        // intersect with self
        assert!(reg1.intersect_with(&reg1));

        // intersect with others
        assert!(reg3.intersect_with(&reg2));
        assert!(reg2.intersect_with(&reg3));
        assert!(reg1.intersect_with(&reg4));
        assert!(reg4.intersect_with(&reg1));
        assert!(reg1.intersect_with(&reg5));
        assert!(reg5.intersect_with(&reg1));
    }

    #[test]
    fn test_create_device_region() {
        let reg = AddressSpaceRegion::create_device_region(GuestAddress(0x10000), 0x1000).unwrap();
        assert_eq!(reg.region_type(), AddressSpaceRegionType::DeviceMemory);
        assert_eq!(reg.start_addr(), GuestAddress(0x10000));
        assert_eq!(reg.len(), 0x1000);
    }

    #[test]
    fn test_create_default_memory_region() {
        AddressSpaceRegion::create_default_memory_region(
            GuestAddress(0x100000),
            0x100000,
            None,
            "invalid",
            "invalid",
            false,
            false,
        )
        .unwrap_err();

        let reg = AddressSpaceRegion::create_default_memory_region(
            GuestAddress(0x100000),
            0x100000,
            None,
            "shmem",
            "",
            false,
            false,
        )
        .unwrap();
        assert_eq!(reg.region_type(), AddressSpaceRegionType::DefaultMemory);
        assert_eq!(reg.start_addr(), GuestAddress(0x100000));
        assert_eq!(reg.last_addr(), GuestAddress(0x1fffff));
        assert_eq!(reg.len(), 0x100000);
        assert!(reg.file_offset().is_some());

        let reg = AddressSpaceRegion::create_default_memory_region(
            GuestAddress(0x100000),
            0x100000,
            None,
            "hugeshmem",
            "",
            true,
            false,
        )
        .unwrap();
        assert_eq!(reg.region_type(), AddressSpaceRegionType::DefaultMemory);
        assert_eq!(reg.start_addr(), GuestAddress(0x100000));
        assert_eq!(reg.last_addr(), GuestAddress(0x1fffff));
        assert_eq!(reg.len(), 0x100000);
        assert!(reg.file_offset().is_some());

        let reg = AddressSpaceRegion::create_default_memory_region(
            GuestAddress(0x100000),
            0x100000,
            None,
            "mmap",
            "",
            true,
            false,
        )
        .unwrap();
        assert_eq!(reg.region_type(), AddressSpaceRegionType::DefaultMemory);
        assert_eq!(reg.start_addr(), GuestAddress(0x100000));
        assert_eq!(reg.last_addr(), GuestAddress(0x1fffff));
        assert_eq!(reg.len(), 0x100000);
        assert!(reg.file_offset().is_none());

        // TODO: test hugetlbfs
    }
}
