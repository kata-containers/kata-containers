// Copyright (C) 2019-2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Address space abstraction to manage virtual machine's physical address space.
//!
//! The AddressSpace abstraction is introduced to manage virtual machine's physical address space.
//! The regions in virtual machine's physical address space may be used to:
//! 1) map guest virtual memory
//! 2) map MMIO ranges for emulated virtual devices, such as virtio-fs DAX window.
//! 3) map MMIO ranges for pass-through devices, such as PCI device BARs.
//! 4) map MMIO ranges for to vCPU, such as local APIC.
//! 5) not used/available
//!
//! A related abstraction, vm_memory::GuestMemory, is used to access guest virtual memory only.
//! In other words, AddressSpace is the resource owner, and GuestMemory is an accessor for guest
//! virtual memory.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use dbs_address_space::{
    AddressSpace, AddressSpaceError, AddressSpaceLayout, AddressSpaceRegion,
    AddressSpaceRegionType, NumaNode, NumaNodeInfo, MPOL_MF_MOVE, MPOL_PREFERRED,
};
use dbs_allocator::Constraint;
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::VmFd;
use log::{debug, error, info, warn};
use nix::sys::mman;
use nix::unistd::dup;
#[cfg(feature = "atomic-guest-memory")]
use vm_memory::GuestMemoryAtomic;
use vm_memory::{
    address::Address, FileOffset, GuestAddress, GuestAddressSpace, GuestMemoryMmap,
    GuestMemoryRegion, GuestRegionMmap, GuestUsize, MemoryRegionAddress, MmapRegion,
};

use crate::resource_manager::ResourceManager;
use crate::vm::NumaRegionInfo;

#[cfg(not(feature = "atomic-guest-memory"))]
/// Concrete GuestAddressSpace type used by the VMM.
pub type GuestAddressSpaceImpl = Arc<GuestMemoryMmap>;

#[cfg(feature = "atomic-guest-memory")]
/// Concrete GuestAddressSpace type used by the VMM.
pub type GuestAddressSpaceImpl = GuestMemoryAtomic<GuestMemoryMmap>;

/// Concrete GuestMemory type used by the VMM.
pub type GuestMemoryImpl = <Arc<vm_memory::GuestMemoryMmap> as GuestAddressSpace>::M;
/// Concrete GuestRegion type used by the VMM.
pub type GuestRegionImpl = GuestRegionMmap;

// Maximum number of working threads for memory pre-allocation.
const MAX_PRE_ALLOC_THREAD: u64 = 16;

// Control the actual number of pre-allocating threads. After several performance tests, we decide to use one thread to do pre-allocating for every 4G memory.
const PRE_ALLOC_GRANULARITY: u64 = 32;

// We don't have plan to support mainframe computer and only focus on PC servers.
// 64 as max nodes should be enough for now.
const MAX_NODE: u32 = 64;

// We will split the memory region if it conflicts with the MMIO hole.
// But if the space below the MMIO hole is smaller than the MINIMAL_SPLIT_SPACE, we won't split the memory region in order to enhance performance.
const MINIMAL_SPLIT_SPACE: u64 = 128 << 20;

/// Errors associated with virtual machine address space management.
#[derive(Debug, thiserror::Error)]
pub enum AddressManagerError {
    /// Invalid address space operation.
    #[error("invalid address space operation")]
    InvalidOperation,

    /// Invalid address range.
    #[error("invalid address space region (0x{0:x}, 0x{1:x})")]
    InvalidAddressRange(u64, GuestUsize),

    /// No available mem address.
    #[error("no available mem address")]
    NoAvailableMemAddress,

    /// No available kvm slotse.
    #[error("no available kvm slots")]
    NoAvailableKvmSlot,

    /// Address manager failed to create memfd to map anonymous memory.
    #[error("address manager failed to create memfd to map anonymous memory")]
    CreateMemFd(#[source] nix::Error),

    /// Address manager failed to open memory file.
    #[error("address manager failed to open memory file")]
    OpenFile(#[source] std::io::Error),

    /// Memory file provided is invalid due to empty file path, non-existent file path and other possible mistakes.
    #[error("memory file provided to address manager {0} is invalid")]
    FileInvalid(String),

    /// Memory file provided is invalid due to empty memory type
    #[error("memory type provided to address manager {0} is invalid")]
    TypeInvalid(String),

    /// Failed to set size for memory file.
    #[error("address manager failed to set size for memory file")]
    SetFileSize(#[source] std::io::Error),

    /// Failed to unlink memory file.
    #[error("address manager failed to unlink memory file")]
    UnlinkFile(#[source] nix::Error),

    /// Failed to duplicate fd of memory file.
    #[error("address manager failed to duplicate memory file descriptor")]
    DupFd(#[source] nix::Error),

    /// Failure in accessing the memory located at some address.
    #[error("address manager failed to access guest memory located at 0x{0:x}")]
    AccessGuestMemory(u64, #[source] vm_memory::mmap::Error),

    /// Failed to create GuestMemory
    #[error("address manager failed to create guest memory object")]
    CreateGuestMemory(#[source] vm_memory::Error),

    /// Failure in initializing guest memory.
    #[error("address manager failed to initialize guest memory")]
    GuestMemoryNotInitialized,

    /// Failed to mmap() guest memory
    #[error("address manager failed to mmap() guest memory into current process")]
    MmapGuestMemory(#[source] vm_memory::mmap::MmapRegionError),

    /// Failed to set KVM memory slot.
    #[error("address manager failed to configure KVM memory slot")]
    KvmSetMemorySlot(#[source] kvm_ioctls::Error),

    /// Failed to set madvise on AddressSpaceRegion
    #[error("address manager failed to set madvice() on guest memory region")]
    Madvise(#[source] nix::Error),

    /// join threads fail
    #[error("address manager failed to join threads")]
    JoinFail,

    /// Failed to create Address Space Region
    #[error("address manager failed to create Address Space Region {0}")]
    CreateAddressSpaceRegion(#[source] AddressSpaceError),
}

type Result<T> = std::result::Result<T, AddressManagerError>;

/// Parameters to configure address space creation operations.
pub struct AddressSpaceMgrBuilder<'a> {
    mem_type: &'a str,
    mem_file: &'a str,
    mem_index: u32,
    mem_suffix: bool,
    mem_prealloc: bool,
    dirty_page_logging: bool,
    vmfd: Option<Arc<VmFd>>,
}

impl<'a> AddressSpaceMgrBuilder<'a> {
    /// Create a new [`AddressSpaceMgrBuilder`] object.
    pub fn new(mem_type: &'a str, mem_file: &'a str) -> Result<Self> {
        if mem_type.is_empty() {
            return Err(AddressManagerError::TypeInvalid(mem_type.to_string()));
        }
        Ok(AddressSpaceMgrBuilder {
            mem_type,
            mem_file,
            mem_index: 0,
            mem_suffix: true,
            mem_prealloc: false,
            dirty_page_logging: false,
            vmfd: None,
        })
    }

    /// Enable/disable adding numbered suffix to memory file path.
    /// This feature could be useful to generate hugetlbfs files with number suffix. (e.g. shmem0, shmem1)
    pub fn toggle_file_suffix(&mut self, enabled: bool) {
        self.mem_suffix = enabled;
    }

    /// Enable/disable memory pre-allocation.
    /// Enable this feature could improve performance stability at the start of workload by avoiding page fault.
    /// Disable this feature may influence performance stability but the cpu resource consumption and start-up time will decrease.
    pub fn toggle_prealloc(&mut self, prealloc: bool) {
        self.mem_prealloc = prealloc;
    }

    /// Enable/disable KVM dirty page logging.
    pub fn toggle_dirty_page_logging(&mut self, logging: bool) {
        self.dirty_page_logging = logging;
    }

    /// Set KVM [`VmFd`] handle to configure memory slots.
    pub fn set_kvm_vm_fd(&mut self, vmfd: Arc<VmFd>) -> Option<Arc<VmFd>> {
        let mut existing_vmfd = None;
        if self.vmfd.is_some() {
            existing_vmfd = self.vmfd.clone();
        }
        self.vmfd = Some(vmfd);
        existing_vmfd
    }

    /// Build a ['AddressSpaceMgr'] using the configured parameters.
    pub fn build(
        self,
        res_mgr: &ResourceManager,
        numa_region_infos: &[NumaRegionInfo],
    ) -> Result<AddressSpaceMgr> {
        let mut mgr = AddressSpaceMgr::default();
        mgr.create_address_space(res_mgr, numa_region_infos, self)?;
        Ok(mgr)
    }

    fn get_next_mem_file(&mut self) -> String {
        if self.mem_suffix {
            let path = format!("{}{}", self.mem_file, self.mem_index);
            self.mem_index += 1;
            path
        } else {
            self.mem_file.to_string()
        }
    }
}

/// Struct to manage virtual machine's physical address space.
pub struct AddressSpaceMgr {
    address_space: Option<AddressSpace>,
    vm_as: Option<GuestAddressSpaceImpl>,
    base_to_slot: Arc<Mutex<HashMap<u64, u32>>>,
    prealloc_handlers: Vec<thread::JoinHandle<()>>,
    prealloc_exit: Arc<AtomicBool>,
    numa_nodes: BTreeMap<u32, NumaNode>,
}

impl AddressSpaceMgr {
    /// Query address space manager is initialized or not
    pub fn is_initialized(&self) -> bool {
        self.address_space.is_some()
    }

    /// Gets address space.
    pub fn address_space(&self) -> Option<&AddressSpace> {
        self.address_space.as_ref()
    }

    /// Get the guest memory.
    pub fn vm_memory(&self) -> Option<<GuestAddressSpaceImpl as GuestAddressSpace>::T> {
        self.get_vm_as().map(|m| m.memory())
    }

    /// Create the address space for a virtual machine.
    ///
    /// This method is designed to be called when starting up a virtual machine instead of at
    /// runtime, so it's expected the virtual machine will be tore down and no strict error recover.
    pub fn create_address_space(
        &mut self,
        res_mgr: &ResourceManager,
        numa_region_infos: &[NumaRegionInfo],
        mut param: AddressSpaceMgrBuilder,
    ) -> Result<()> {
        let mut regions = Vec::new();
        let mut start_addr = dbs_boot::layout::GUEST_MEM_START;

        // Create address space regions.
        for info in numa_region_infos.iter() {
            info!("numa_region_info {:?}", info);
            // convert size_in_mib to bytes
            let size = info
                .size
                .checked_shl(20)
                .ok_or(AddressManagerError::InvalidOperation)?;

            // Guest memory does not intersect with the MMIO hole.
            // TODO: make it work for ARM (issue #4307)
            if start_addr > dbs_boot::layout::MMIO_LOW_END
                || start_addr + size <= dbs_boot::layout::MMIO_LOW_START
            {
                let region = self.create_region(start_addr, size, info, &mut param)?;
                regions.push(region);
                start_addr = start_addr
                    .checked_add(size)
                    .ok_or(AddressManagerError::InvalidOperation)?;
            } else {
                // Add guest memory below the MMIO hole, avoid splitting the memory region
                // if the available address region is small than MINIMAL_SPLIT_SPACE MiB.
                let mut below_size = dbs_boot::layout::MMIO_LOW_START
                    .checked_sub(start_addr)
                    .ok_or(AddressManagerError::InvalidOperation)?;
                if below_size < (MINIMAL_SPLIT_SPACE) {
                    below_size = 0;
                } else {
                    let region = self.create_region(start_addr, below_size, info, &mut param)?;
                    regions.push(region);
                }

                // Add guest memory above the MMIO hole
                let above_start = dbs_boot::layout::MMIO_LOW_END + 1;
                let above_size = size
                    .checked_sub(below_size)
                    .ok_or(AddressManagerError::InvalidOperation)?;
                let region = self.create_region(above_start, above_size, info, &mut param)?;
                regions.push(region);
                start_addr = above_start
                    .checked_add(above_size)
                    .ok_or(AddressManagerError::InvalidOperation)?;
            }
        }

        // Create GuestMemory object
        let mut vm_memory = GuestMemoryMmap::new();
        for reg in regions.iter() {
            // Allocate used guest memory addresses.
            // These addresses are statically allocated, resource allocation/update should not fail.
            let constraint = Constraint::new(reg.len())
                .min(reg.start_addr().raw_value())
                .max(reg.last_addr().raw_value());
            let _key = res_mgr
                .allocate_mem_address(&constraint)
                .ok_or(AddressManagerError::NoAvailableMemAddress)?;
            let mmap_reg = self.create_mmap_region(reg.clone())?;

            vm_memory = vm_memory
                .insert_region(mmap_reg.clone())
                .map_err(AddressManagerError::CreateGuestMemory)?;
            self.map_to_kvm(res_mgr, &param, reg, mmap_reg)?;
        }

        #[cfg(feature = "atomic-guest-memory")]
        {
            self.vm_as = Some(AddressSpace::convert_into_vm_as(vm_memory));
        }
        #[cfg(not(feature = "atomic-guest-memory"))]
        {
            self.vm_as = Some(Arc::new(vm_memory));
        }

        let layout = AddressSpaceLayout::new(
            *dbs_boot::layout::GUEST_PHYS_END,
            dbs_boot::layout::GUEST_MEM_START,
            *dbs_boot::layout::GUEST_MEM_END,
        );
        self.address_space = Some(AddressSpace::from_regions(regions, layout));

        Ok(())
    }

    // size unit: Byte
    fn create_region(
        &mut self,
        start_addr: u64,
        size_bytes: u64,
        info: &NumaRegionInfo,
        param: &mut AddressSpaceMgrBuilder,
    ) -> Result<Arc<AddressSpaceRegion>> {
        let mem_file_path = param.get_next_mem_file();
        let region = AddressSpaceRegion::create_default_memory_region(
            GuestAddress(start_addr),
            size_bytes,
            info.host_numa_node_id,
            param.mem_type,
            &mem_file_path,
            param.mem_prealloc,
            false,
        )
        .map_err(AddressManagerError::CreateAddressSpaceRegion)?;
        let region = Arc::new(region);

        self.insert_into_numa_nodes(
            &region,
            info.guest_numa_node_id.unwrap_or(0),
            &info.vcpu_ids,
        );
        info!(
            "create new region: guest addr 0x{:x}-0x{:x} size {}",
            start_addr,
            start_addr + size_bytes,
            size_bytes
        );

        Ok(region)
    }

    fn map_to_kvm(
        &mut self,
        res_mgr: &ResourceManager,
        param: &AddressSpaceMgrBuilder,
        reg: &Arc<AddressSpaceRegion>,
        mmap_reg: Arc<GuestRegionImpl>,
    ) -> Result<()> {
        // Build mapping between GPA <-> HVA, by adding kvm memory slot.
        let slot = res_mgr
            .allocate_kvm_mem_slot(1, None)
            .ok_or(AddressManagerError::NoAvailableKvmSlot)?;

        if let Some(vmfd) = param.vmfd.as_ref() {
            let host_addr = mmap_reg
                .get_host_address(MemoryRegionAddress(0))
                .map_err(|_e| AddressManagerError::InvalidOperation)?;
            let flags = 0u32;

            let mem_region = kvm_userspace_memory_region {
                slot,
                guest_phys_addr: reg.start_addr().raw_value(),
                memory_size: reg.len(),
                userspace_addr: host_addr as u64,
                flags,
            };

            info!(
                "VM: guest memory region {:x} starts at {:x?}",
                reg.start_addr().raw_value(),
                host_addr
            );
            // Safe because the guest regions are guaranteed not to overlap.
            unsafe { vmfd.set_user_memory_region(mem_region) }
                .map_err(AddressManagerError::KvmSetMemorySlot)?;
        }

        self.base_to_slot
            .lock()
            .unwrap()
            .insert(reg.start_addr().raw_value(), slot);

        Ok(())
    }

    /// Mmap the address space region into current process.
    pub fn create_mmap_region(
        &mut self,
        region: Arc<AddressSpaceRegion>,
    ) -> Result<Arc<GuestRegionImpl>> {
        // Special check for 32bit host with 64bit virtual machines.
        if region.len() > usize::MAX as u64 {
            return Err(AddressManagerError::InvalidAddressRange(
                region.start_addr().raw_value(),
                region.len(),
            ));
        }
        // The device MMIO regions may not be backed by memory files, so refuse to mmap them.
        if region.region_type() == AddressSpaceRegionType::DeviceMemory {
            return Err(AddressManagerError::InvalidOperation);
        }

        // The GuestRegionMmap/MmapRegion will take ownership of the FileOffset object,
        // so we have to duplicate the fd here. It's really a dirty design.
        let file_offset = match region.file_offset().as_ref() {
            Some(fo) => {
                let fd = dup(fo.file().as_raw_fd()).map_err(AddressManagerError::DupFd)?;
                // Safe because we have just duplicated the raw fd.
                let file = unsafe { File::from_raw_fd(fd) };
                let file_offset = FileOffset::new(file, fo.start());
                Some(file_offset)
            }
            None => None,
        };
        let perm_flags = if (region.perm_flags() & libc::MAP_POPULATE) != 0 && region.is_hugepage()
        {
            // mmap(MAP_POPULATE) conflicts with madive(MADV_HUGEPAGE) because mmap(MAP_POPULATE)
            // will pre-fault in all memory with normal pages before madive(MADV_HUGEPAGE) gets
            // called. So remove the MAP_POPULATE flag and memory will be faulted in by working
            // threads.
            region.perm_flags() & (!libc::MAP_POPULATE)
        } else {
            region.perm_flags()
        };
        let mmap_reg = MmapRegion::build(
            file_offset,
            region.len() as usize,
            libc::PROT_READ | libc::PROT_WRITE,
            perm_flags,
        )
        .map_err(AddressManagerError::MmapGuestMemory)?;

        if region.is_anonpage() {
            self.configure_anon_mem(&mmap_reg)?;
        }
        if let Some(node_id) = region.host_numa_node_id() {
            self.configure_numa(&mmap_reg, node_id)?;
        }
        if region.is_hugepage() {
            self.configure_thp_and_prealloc(&region, &mmap_reg)?;
        }

        let reg = GuestRegionImpl::new(mmap_reg, region.start_addr())
            .map_err(AddressManagerError::CreateGuestMemory)?;
        Ok(Arc::new(reg))
    }

    fn configure_anon_mem(&self, mmap_reg: &MmapRegion) -> Result<()> {
        unsafe {
            mman::madvise(
                mmap_reg.as_ptr() as *mut libc::c_void,
                mmap_reg.size(),
                mman::MmapAdvise::MADV_DONTFORK,
            )
        }
        .map_err(AddressManagerError::Madvise)
    }

    fn configure_numa(&self, mmap_reg: &MmapRegion, node_id: u32) -> Result<()> {
        let nodemask = 1_u64
            .checked_shl(node_id)
            .ok_or(AddressManagerError::InvalidOperation)?;
        let res = unsafe {
            libc::syscall(
                libc::SYS_mbind,
                mmap_reg.as_ptr() as *mut libc::c_void,
                mmap_reg.size(),
                MPOL_PREFERRED,
                &nodemask as *const u64,
                MAX_NODE,
                MPOL_MF_MOVE,
            )
        };
        if res < 0 {
            warn!(
                "failed to mbind memory to host_numa_node_id {}: this may affect performance",
                node_id
            );
        }
        Ok(())
    }

    // We set Transparent Huge Page (THP) through mmap to increase performance.
    // In order to reduce the impact of page fault on performance, we start several threads (up to MAX_PRE_ALLOC_THREAD) to touch every 4k page of the memory region to manually do memory pre-allocation.
    // The reason why we don't use mmap to enable THP and pre-alloction is that THP setting won't take effect in this operation (tested in kernel 4.9)
    fn configure_thp_and_prealloc(
        &mut self,
        region: &Arc<AddressSpaceRegion>,
        mmap_reg: &MmapRegion,
    ) -> Result<()> {
        debug!(
            "Setting MADV_HUGEPAGE on AddressSpaceRegion addr {:x?} len {:x?}",
            mmap_reg.as_ptr(),
            mmap_reg.size()
        );

        // Safe because we just create the MmapRegion
        unsafe {
            mman::madvise(
                mmap_reg.as_ptr() as *mut libc::c_void,
                mmap_reg.size(),
                mman::MmapAdvise::MADV_HUGEPAGE,
            )
        }
        .map_err(AddressManagerError::Madvise)?;

        if region.perm_flags() & libc::MAP_POPULATE > 0 {
            // Touch every 4k page to trigger allocation. The step is 4K instead of 2M to ensure
            // pre-allocation when running out of huge pages.
            const PAGE_SIZE: u64 = 4096;
            const PAGE_SHIFT: u32 = 12;
            let addr = mmap_reg.as_ptr() as u64;
            // Here we use >> PAGE_SHIFT to calculate how many 4K pages in the memory region.
            let npage = (mmap_reg.size() as u64) >> PAGE_SHIFT;

            let mut touch_thread = ((mmap_reg.size() as u64) >> PRE_ALLOC_GRANULARITY) + 1;
            if touch_thread > MAX_PRE_ALLOC_THREAD {
                touch_thread = MAX_PRE_ALLOC_THREAD;
            }

            let per_npage = npage / touch_thread;
            for n in 0..touch_thread {
                let start_npage = per_npage * n;
                let end_npage = if n == (touch_thread - 1) {
                    npage
                } else {
                    per_npage * (n + 1)
                };
                let mut per_addr = addr + (start_npage * PAGE_SIZE);
                let should_stop = self.prealloc_exit.clone();

                let handler = thread::Builder::new()
                    .name("PreallocThread".to_string())
                    .spawn(move || {
                        info!("PreallocThread start start_npage: {:?}, end_npage: {:?}, per_addr: {:?}, thread_number: {:?}",
                              start_npage, end_npage, per_addr, touch_thread );
                        for _ in start_npage..end_npage {
                            if should_stop.load(Ordering::Acquire) {
                                info!("PreallocThread stop start_npage: {:?}, end_npage: {:?}, per_addr: {:?}, thread_number: {:?}",
                                      start_npage, end_npage, per_addr, touch_thread);
                                break;
                            }

                            // Reading from a THP page may be served by the zero page, so only
                            // write operation could ensure THP memory allocation. So use
                            // the compare_exchange(old_val, old_val) trick to trigger allocation.
                            let addr_ptr = per_addr as *mut u8;
                            let read_byte = unsafe { std::ptr::read_volatile(addr_ptr) };
                            let atomic_u8 : &AtomicU8 = unsafe {&*(addr_ptr as *mut AtomicU8)};
                            let _ = atomic_u8.compare_exchange(read_byte, read_byte, Ordering::SeqCst, Ordering::SeqCst);
                            per_addr += PAGE_SIZE;
                        }

                        info!("PreallocThread done start_npage: {:?}, end_npage: {:?}, per_addr: {:?}, thread_number: {:?}",
                              start_npage, end_npage, per_addr, touch_thread );
                    });

                match handler {
                    Err(e) => error!(
                        "Failed to create working thread for async pre-allocation, {:?}. This may affect performance stability at the start of the workload.",
                        e
                    ),
                    Ok(hdl) => self.prealloc_handlers.push(hdl),
                }
            }
        }

        Ok(())
    }

    /// Get the address space object
    pub fn get_address_space(&self) -> Option<&AddressSpace> {
        self.address_space.as_ref()
    }

    /// Get the default guest memory object, which will be used to access virtual machine's default
    /// guest memory.
    pub fn get_vm_as(&self) -> Option<&GuestAddressSpaceImpl> {
        self.vm_as.as_ref()
    }

    /// Get the base to slot map
    pub fn get_base_to_slot_map(&self) -> Arc<Mutex<HashMap<u64, u32>>> {
        self.base_to_slot.clone()
    }

    /// get numa nodes infos from address space manager.
    pub fn get_numa_nodes(&self) -> &BTreeMap<u32, NumaNode> {
        &self.numa_nodes
    }

    /// add cpu and memory numa informations to BtreeMap
    fn insert_into_numa_nodes(
        &mut self,
        region: &Arc<AddressSpaceRegion>,
        guest_numa_node_id: u32,
        vcpu_ids: &[u32],
    ) {
        let node = self.numa_nodes.entry(guest_numa_node_id).or_default();
        node.add_info(&NumaNodeInfo {
            base: region.start_addr(),
            size: region.len(),
        });
        node.add_vcpu_ids(vcpu_ids);
    }

    /// get address space layout from address space manager.
    pub fn get_layout(&self) -> Result<AddressSpaceLayout> {
        self.address_space
            .as_ref()
            .map(|v| v.layout())
            .ok_or(AddressManagerError::GuestMemoryNotInitialized)
    }

    /// Wait for the pre-allocation working threads to finish work.
    ///
    /// Force all working threads to exit if `stop` is true.
    pub fn wait_prealloc(&mut self, stop: bool) -> Result<()> {
        if stop {
            self.prealloc_exit.store(true, Ordering::Release);
        }
        while let Some(handlers) = self.prealloc_handlers.pop() {
            if let Err(e) = handlers.join() {
                error!("wait_prealloc join fail {:?}", e);
                return Err(AddressManagerError::JoinFail);
            }
        }
        Ok(())
    }
}

impl Default for AddressSpaceMgr {
    /// Create a new empty AddressSpaceMgr
    fn default() -> Self {
        AddressSpaceMgr {
            address_space: None,
            vm_as: None,
            base_to_slot: Arc::new(Mutex::new(HashMap::new())),
            prealloc_handlers: Vec::new(),
            prealloc_exit: Arc::new(AtomicBool::new(false)),
            numa_nodes: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use dbs_boot::layout::GUEST_MEM_START;
    use std::ops::Deref;

    use vm_memory::{Bytes, GuestAddressSpace, GuestMemory, GuestMemoryRegion};
    use vmm_sys_util::tempfile::TempFile;

    use super::*;

    #[test]
    fn test_create_address_space() {
        let res_mgr = ResourceManager::new(None);
        let mem_size = 128 << 20;
        let numa_region_infos = vec![NumaRegionInfo {
            size: mem_size >> 20,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids: vec![1, 2],
        }];
        let builder = AddressSpaceMgrBuilder::new("shmem", "").unwrap();
        let as_mgr = builder.build(&res_mgr, &numa_region_infos).unwrap();
        let vm_as = as_mgr.get_vm_as().unwrap();
        let guard = vm_as.memory();
        let gmem = guard.deref();
        assert_eq!(gmem.num_regions(), 1);

        let reg = gmem
            .find_region(GuestAddress(GUEST_MEM_START + mem_size - 1))
            .unwrap();
        assert_eq!(reg.start_addr(), GuestAddress(GUEST_MEM_START));
        assert_eq!(reg.len(), mem_size);
        assert!(gmem
            .find_region(GuestAddress(GUEST_MEM_START + mem_size))
            .is_none());
        assert!(reg.file_offset().is_some());

        let buf = [0x1u8, 0x2u8, 0x3u8, 0x4u8, 0x5u8];
        gmem.write_slice(&buf, GuestAddress(GUEST_MEM_START))
            .unwrap();

        // Update middle of mapped memory region
        let mut val = 0xa5u8;
        gmem.write_obj(val, GuestAddress(GUEST_MEM_START + 0x1))
            .unwrap();
        val = gmem.read_obj(GuestAddress(GUEST_MEM_START + 0x1)).unwrap();
        assert_eq!(val, 0xa5);
        val = gmem.read_obj(GuestAddress(GUEST_MEM_START)).unwrap();
        assert_eq!(val, 1);
        val = gmem.read_obj(GuestAddress(GUEST_MEM_START + 0x2)).unwrap();
        assert_eq!(val, 3);
        val = gmem.read_obj(GuestAddress(GUEST_MEM_START + 0x5)).unwrap();
        assert_eq!(val, 0);

        // Read ahead of mapped memory region
        assert!(gmem
            .read_obj::<u8>(GuestAddress(GUEST_MEM_START + mem_size))
            .is_err());

        let res_mgr = ResourceManager::new(None);
        let mem_size = dbs_boot::layout::MMIO_LOW_START + (1 << 30);
        let numa_region_infos = vec![NumaRegionInfo {
            size: mem_size >> 20,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids: vec![1, 2],
        }];
        let builder = AddressSpaceMgrBuilder::new("shmem", "").unwrap();
        let as_mgr = builder.build(&res_mgr, &numa_region_infos).unwrap();
        let vm_as = as_mgr.get_vm_as().unwrap();
        let guard = vm_as.memory();
        let gmem = guard.deref();
        #[cfg(target_arch = "x86_64")]
        assert_eq!(gmem.num_regions(), 2);
        #[cfg(target_arch = "aarch64")]
        assert_eq!(gmem.num_regions(), 1);

        // Test dropping GuestMemoryMmap object releases all resources.
        for _ in 0..10000 {
            let res_mgr = ResourceManager::new(None);
            let mem_size = 1 << 20;
            let numa_region_infos = vec![NumaRegionInfo {
                size: mem_size >> 20,
                host_numa_node_id: None,
                guest_numa_node_id: Some(0),
                vcpu_ids: vec![1, 2],
            }];
            let builder = AddressSpaceMgrBuilder::new("shmem", "").unwrap();
            let _as_mgr = builder.build(&res_mgr, &numa_region_infos).unwrap();
        }
        let file = TempFile::new().unwrap().into_file();
        let fd = file.as_raw_fd();
        // fd should be small enough if there's no leaking of fds.
        assert!(fd < 1000);
    }

    #[test]
    fn test_address_space_mgr_get_boundary() {
        let layout = AddressSpaceLayout::new(
            *dbs_boot::layout::GUEST_PHYS_END,
            dbs_boot::layout::GUEST_MEM_START,
            *dbs_boot::layout::GUEST_MEM_END,
        );
        let res_mgr = ResourceManager::new(None);
        let mem_size = 128 << 20;
        let numa_region_infos = vec![NumaRegionInfo {
            size: mem_size >> 20,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids: vec![1, 2],
        }];
        let builder = AddressSpaceMgrBuilder::new("shmem", "").unwrap();
        let as_mgr = builder.build(&res_mgr, &numa_region_infos).unwrap();
        assert_eq!(as_mgr.get_layout().unwrap(), layout);
    }

    #[test]
    fn test_address_space_mgr_get_numa_nodes() {
        let res_mgr = ResourceManager::new(None);
        let mem_size = 128 << 20;
        let cpu_vec = vec![1, 2];
        let numa_region_infos = vec![NumaRegionInfo {
            size: mem_size >> 20,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids: cpu_vec.clone(),
        }];
        let builder = AddressSpaceMgrBuilder::new("shmem", "").unwrap();
        let as_mgr = builder.build(&res_mgr, &numa_region_infos).unwrap();
        let mut numa_node = NumaNode::new();
        numa_node.add_info(&NumaNodeInfo {
            base: GuestAddress(GUEST_MEM_START),
            size: mem_size,
        });
        numa_node.add_vcpu_ids(&cpu_vec);

        assert_eq!(*as_mgr.get_numa_nodes().get(&0).unwrap(), numa_node);
    }

    #[test]
    fn test_address_space_mgr_async_prealloc() {
        let res_mgr = ResourceManager::new(None);
        let mem_size = 2 << 20;
        let cpu_vec = vec![1, 2];
        let numa_region_infos = vec![NumaRegionInfo {
            size: mem_size >> 20,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids: cpu_vec,
        }];
        let mut builder = AddressSpaceMgrBuilder::new("hugeshmem", "").unwrap();
        builder.toggle_prealloc(true);
        let mut as_mgr = builder.build(&res_mgr, &numa_region_infos).unwrap();
        as_mgr.wait_prealloc(false).unwrap();
    }

    #[test]
    fn test_address_space_mgr_builder() {
        let mut builder = AddressSpaceMgrBuilder::new("shmem", "/tmp/shmem").unwrap();

        assert_eq!(builder.mem_type, "shmem");
        assert_eq!(builder.mem_file, "/tmp/shmem");
        assert_eq!(builder.mem_index, 0);
        assert!(builder.mem_suffix);
        assert!(!builder.mem_prealloc);
        assert!(!builder.dirty_page_logging);
        assert!(builder.vmfd.is_none());

        assert_eq!(&builder.get_next_mem_file(), "/tmp/shmem0");
        assert_eq!(&builder.get_next_mem_file(), "/tmp/shmem1");
        assert_eq!(&builder.get_next_mem_file(), "/tmp/shmem2");
        assert_eq!(builder.mem_index, 3);

        builder.toggle_file_suffix(false);
        assert_eq!(&builder.get_next_mem_file(), "/tmp/shmem");
        assert_eq!(&builder.get_next_mem_file(), "/tmp/shmem");
        assert_eq!(builder.mem_index, 3);

        builder.toggle_prealloc(true);
        builder.toggle_dirty_page_logging(true);
        assert!(builder.mem_prealloc);
        assert!(builder.dirty_page_logging);
    }

    #[test]
    fn test_configure_invalid_numa() {
        let res_mgr = ResourceManager::new(None);
        let mem_size = 128 << 20;
        let numa_region_infos = vec![NumaRegionInfo {
            size: mem_size >> 20,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids: vec![1, 2],
        }];
        let builder = AddressSpaceMgrBuilder::new("shmem", "").unwrap();
        let as_mgr = builder.build(&res_mgr, &numa_region_infos).unwrap();
        let mmap_reg = MmapRegion::new(8).unwrap();

        assert!(as_mgr.configure_numa(&mmap_reg, u32::MAX).is_err());
    }
}
