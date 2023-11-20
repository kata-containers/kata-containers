// Copyright 2020 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::io;
use std::sync::{Arc, Mutex};

use dbs_address_space::{
    AddressSpace, AddressSpaceError, AddressSpaceRegion, MPOL_MF_MOVE, MPOL_PREFERRED, USABLE_END,
};
use dbs_utils::epoll_manager::EpollManager;
use dbs_virtio_devices as virtio;
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::VmFd;
use nix::sys::mman;
use serde_derive::{Deserialize, Serialize};
use slog::{debug, error, info, warn};
use virtio::mem::{Mem, MemRegionFactory};
use virtio::Error as VirtioError;
use vm_memory::{
    Address, GuestAddress, GuestAddressSpace, GuestMemory, GuestRegionMmap, GuestUsize, MmapRegion,
};

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::config_manager::{ConfigItem, DeviceConfigInfo, DeviceConfigInfos};
use crate::device_manager::DbsMmioV2Device;
use crate::device_manager::{DeviceManager, DeviceMgrError, DeviceOpContext};
use crate::vm::VmConfigInfo;

// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq.
const USE_GENERIC_IRQ: bool = false;

const HUGE_PAGE_2M: usize = 0x200000;

// max numa node ids on host
const MAX_NODE: u32 = 64;

/// Errors associated with `MemDeviceConfig`.
#[derive(Debug, thiserror::Error)]
pub enum MemDeviceError {
    /// The mem device was already used.
    #[error("the virtio-mem ID was already added to a different device")]
    MemDeviceAlreadyExists,

    /// Cannot perform the requested operation after booting the microVM.
    #[error("the update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// insert mem device error
    #[error("cannot add virtio-mem device, {0}")]
    InsertDeviceFailed(#[source] DeviceMgrError),

    /// create mem device error
    #[error("cannot create virito-mem device, {0}")]
    CreateMemDevice(#[source] DeviceMgrError),

    /// create mmio device error
    #[error("cannot create virito-mem mmio device, {0}")]
    CreateMmioDevice(#[source] DeviceMgrError),

    /// resize mem device error
    #[error("failure while resizing virtio-mem device, {0}")]
    ResizeFailed(#[source] VirtioError),

    /// mem device does not exist
    #[error("mem device does not exist")]
    DeviceNotExist,

    /// address space region error
    #[error("address space region error, {0}")]
    AddressSpaceRegion(#[source] AddressSpaceError),

    /// Cannot initialize a mem device or add a device to the MMIO Bus.
    #[error("failure while registering mem device: {0}")]
    RegisterMemDevice(#[source] DeviceMgrError),

    /// The mem device id doesn't exist.
    #[error("invalid mem device id '{0}'")]
    InvalidDeviceId(String),

    /// The device manager errors.
    #[error("DeviceManager error: {0}")]
    DeviceManager(#[source] DeviceMgrError),
}

/// Configuration information for a virtio-mem device.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct MemDeviceConfigInfo {
    /// Unique identifier of the pmem device
    pub mem_id: String,
    /// Memory size mib
    pub size_mib: u64,
    /// Memory capacity mib
    pub capacity_mib: u64,
    /// Use multi_region or not
    pub multi_region: bool,
    /// host numa node id
    pub host_numa_node_id: Option<u32>,
    /// guest numa node id
    pub guest_numa_node_id: Option<u16>,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

impl ConfigItem for MemDeviceConfigInfo {
    type Err = MemDeviceError;

    fn id(&self) -> &str {
        &self.mem_id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), MemDeviceError> {
        if self.mem_id.as_str() == other.mem_id.as_str() {
            Err(MemDeviceError::MemDeviceAlreadyExists)
        } else {
            Ok(())
        }
    }
}

/// Mem Device Info
pub type MemDeviceInfo = DeviceConfigInfo<MemDeviceConfigInfo>;

impl ConfigItem for MemDeviceInfo {
    type Err = MemDeviceError;

    fn id(&self) -> &str {
        &self.config.mem_id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), MemDeviceError> {
        if self.config.mem_id.as_str() == other.config.mem_id.as_str() {
            Err(MemDeviceError::MemDeviceAlreadyExists)
        } else {
            Ok(())
        }
    }
}

/// Wrapper for the collection that holds all the Mem Devices Configs
#[derive(Clone)]
pub struct MemDeviceMgr {
    /// A list of `MemDeviceConfig` objects.
    info_list: DeviceConfigInfos<MemDeviceConfigInfo>,
    pub(crate) use_shared_irq: bool,
}

impl MemDeviceMgr {
    /// Inserts `mem_cfg` in the virtio-mem device configuration list.
    /// If an entry with the same id already exists, it will attempt to update
    /// the existing entry.
    pub fn insert_or_update_device(
        &mut self,
        mut ctx: DeviceOpContext,
        mem_cfg: MemDeviceConfigInfo,
    ) -> std::result::Result<(), MemDeviceError> {
        if !cfg!(feature = "hotplug") && ctx.is_hotplug {
            error!(ctx.logger(), "hotplug feature has been disabled.";
            "subsystem" => "virito-mem");
            return Err(MemDeviceError::UpdateNotAllowedPostBoot);
        }

        let epoll_mgr = ctx.get_epoll_mgr().map_err(MemDeviceError::DeviceManager)?;

        // If the id of the drive already exists in the list, the operation is update.
        if let Some(index) = self.get_index_of_mem_dev(&mem_cfg.mem_id) {
            // Update an existing memory device
            if ctx.is_hotplug {
                info!(
                    ctx.logger(),
                    "update memory device: {}, size: 0x{:x}MB.",
                    mem_cfg.mem_id,
                    mem_cfg.size_mib;
                    "subsystem" => "virito-mem"
                );
                self.update_memory_size(index, mem_cfg.size_mib)?;
            }
            self.info_list.insert_or_update(&mem_cfg)?;
        } else {
            // Create a new memory device
            if !ctx.is_hotplug {
                self.info_list.insert_or_update(&mem_cfg)?;
                return Ok(());
            }

            info!(
                ctx.logger(),
                "hot-add memory device: {}, size: 0x{:x}MB.", mem_cfg.mem_id, mem_cfg.size_mib;
                "subsystem" => "virito-mem"
            );

            let device = Self::create_memory_device(&mem_cfg, &ctx, &epoll_mgr)
                .map_err(MemDeviceError::CreateMemDevice)?;
            let mmio_device =
                DeviceManager::create_mmio_virtio_device_with_device_change_notification(
                    Box::new(device),
                    &mut ctx,
                    mem_cfg.use_shared_irq.unwrap_or(self.use_shared_irq),
                    mem_cfg.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                )
                .map_err(MemDeviceError::CreateMmioDevice)?;

            #[cfg(not(test))]
            ctx.insert_hotplug_mmio_device(&mmio_device, None)
                .map_err(|e| {
                    error!(
                        ctx.logger(),
                        "failed to hot-add virtio-mem device {}, {}", &mem_cfg.mem_id, e;
                        "subsystem" => "virito-mem"
                    );
                    MemDeviceError::InsertDeviceFailed(e)
                })?;

            let index = self.info_list.insert_or_update(&mem_cfg)?;
            self.info_list[index].set_device(mmio_device);
        }

        Ok(())
    }

    /// Attaches all virtio-mem devices from the MemDevicesConfig.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), MemDeviceError> {
        let epoll_mgr = ctx.get_epoll_mgr().map_err(MemDeviceError::DeviceManager)?;

        for info in self.info_list.iter_mut() {
            let config = &info.config;
            info!(
                ctx.logger(),
                "attach virtio-mem device {}, size 0x{:x}.", config.mem_id, config.size_mib;
                "subsystem" => "virito-mem"
            );
            // Ignore virtio-mem device with zero memory capacity.
            if config.size_mib == 0 {
                debug!(
                    ctx.logger(),
                    "ignore zero-sizing memory device {}.", config.mem_id;
                    "subsystem" => "virito-mem"
                );
                continue;
            }

            let device = Self::create_memory_device(config, ctx, &epoll_mgr)
                .map_err(MemDeviceError::CreateMemDevice)?;
            let mmio_device =
                DeviceManager::create_mmio_virtio_device_with_device_change_notification(
                    Box::new(device),
                    ctx,
                    config.use_shared_irq.unwrap_or(self.use_shared_irq),
                    config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                )
                .map_err(MemDeviceError::RegisterMemDevice)?;

            info.set_device(mmio_device);
        }

        Ok(())
    }

    fn get_index_of_mem_dev(&self, mem_id: &str) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.mem_id.eq(mem_id))
    }

    fn create_memory_device(
        config: &MemDeviceConfigInfo,
        ctx: &DeviceOpContext,
        epoll_mgr: &EpollManager,
    ) -> std::result::Result<virtio::mem::Mem<GuestAddressSpaceImpl>, DeviceMgrError> {
        let factory = Arc::new(Mutex::new(MemoryRegionFactory::new(
            ctx,
            config.mem_id.clone(),
            config.host_numa_node_id,
        )?));

        let mut capacity_mib = config.capacity_mib;
        if capacity_mib == 0 {
            capacity_mib = *USABLE_END >> 20;
        }
        // get boot memory size for calculate alignment
        let boot_mem_size = {
            let boot_size = (ctx.get_vm_config()?.mem_size_mib << 20) as u64;
            // increase 1G memory because of avoiding mmio hole
            match boot_size {
                x if x > dbs_boot::layout::MMIO_LOW_START => x + (1 << 30),
                _ => boot_size,
            }
        };

        virtio::mem::Mem::new(
            config.mem_id.clone(),
            capacity_mib,
            config.size_mib,
            config.multi_region,
            config.guest_numa_node_id,
            epoll_mgr.clone(),
            factory,
            boot_mem_size,
        )
        .map_err(DeviceMgrError::Virtio)
    }

    /// Removes all virtio-mem devices
    pub fn remove_devices(&self, ctx: &mut DeviceOpContext) -> Result<(), DeviceMgrError> {
        for info in self.info_list.iter() {
            if let Some(device) = &info.device {
                DeviceManager::destroy_mmio_virtio_device(device.clone(), ctx)?;
            }
        }

        Ok(())
    }

    fn update_memory_size(
        &self,
        index: usize,
        size_mib: u64,
    ) -> std::result::Result<(), MemDeviceError> {
        let device = self.info_list[index]
            .device
            .as_ref()
            .ok_or_else(|| MemDeviceError::DeviceNotExist)?;
        if let Some(mmio_dev) = device.as_any().downcast_ref::<DbsMmioV2Device>() {
            let guard = mmio_dev.state();
            let inner_dev = guard.get_inner_device();
            if let Some(mem_dev) = inner_dev
                .as_any()
                .downcast_ref::<Mem<GuestAddressSpaceImpl>>()
            {
                return mem_dev
                    .set_requested_size(size_mib)
                    .map_err(MemDeviceError::ResizeFailed);
            }
        }
        Ok(())
    }
}

impl Default for MemDeviceMgr {
    /// Create a new `MemDeviceMgr` object..
    fn default() -> Self {
        MemDeviceMgr {
            info_list: DeviceConfigInfos::new(),
            use_shared_irq: USE_SHARED_IRQ,
        }
    }
}

struct MemoryRegionFactory {
    mem_id: String,
    vm_as: GuestAddressSpaceImpl,
    address_space: AddressSpace,
    vm_config: VmConfigInfo,
    vm_fd: Arc<VmFd>,
    logger: Arc<slog::Logger>,
    host_numa_node_id: Option<u32>,
    instance_id: String,
}

impl MemoryRegionFactory {
    fn new(
        ctx: &DeviceOpContext,
        mem_id: String,
        host_numa_node_id: Option<u32>,
    ) -> Result<Self, DeviceMgrError> {
        let vm_as = ctx.get_vm_as()?;
        let address_space = ctx.get_address_space()?;
        let vm_config = ctx.get_vm_config()?;
        let logger = Arc::new(ctx.logger().new(slog::o!()));

        let shared_info = ctx.shared_info.read().unwrap();
        let instance_id = shared_info.id.clone();

        Ok(MemoryRegionFactory {
            mem_id,
            vm_as,
            address_space,
            vm_config,
            vm_fd: ctx.vm_fd.clone(),
            logger,
            host_numa_node_id,
            instance_id,
        })
    }

    fn configure_anon_mem(&self, mmap_reg: &MmapRegion) -> Result<(), VirtioError> {
        unsafe {
            mman::madvise(
                mmap_reg.as_ptr() as *mut libc::c_void,
                mmap_reg.size(),
                mman::MmapAdvise::MADV_DONTFORK,
            )
        }
        .map_err(VirtioError::Madvise)?;

        Ok(())
    }

    fn configure_numa(&self, mmap_reg: &MmapRegion, node_id: u32) -> Result<(), VirtioError> {
        let nodemask = 1_u64
            .checked_shl(node_id)
            .ok_or(VirtioError::InvalidInput)?;
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
                self.logger,
                "failed to mbind memory to host_numa_node_id {}: this may affect performance",
                node_id;
                "subsystem" => "virito-mem"
            );
        }
        Ok(())
    }

    fn configure_thp(&mut self, mmap_reg: &MmapRegion) -> Result<(), VirtioError> {
        debug!(
            self.logger,
            "Setting MADV_HUGEPAGE on AddressSpaceRegion addr {:x?} len {:x?}",
            mmap_reg.as_ptr(),
            mmap_reg.size();
            "subsystem" => "virito-mem"
        );

        // Safe because we just create the MmapRegion
        unsafe {
            mman::madvise(
                mmap_reg.as_ptr() as *mut libc::c_void,
                mmap_reg.size(),
                mman::MmapAdvise::MADV_HUGEPAGE,
            )
        }
        .map_err(VirtioError::Madvise)?;

        Ok(())
    }

    fn map_to_kvm(
        &mut self,
        slot: u32,
        reg: &Arc<AddressSpaceRegion>,
        mmap_reg: &MmapRegion,
    ) -> Result<(), VirtioError> {
        let host_addr = mmap_reg.as_ptr() as u64;

        let flags = 0u32;

        let mem_region = kvm_userspace_memory_region {
            slot,
            guest_phys_addr: reg.start_addr().raw_value(),
            memory_size: reg.len(),
            userspace_addr: host_addr,
            flags,
        };

        // Safe because the user mem region is just created, and kvm slot is allocated
        // by resource allocator.
        unsafe { self.vm_fd.set_user_memory_region(mem_region) }
            .map_err(VirtioError::SetUserMemoryRegion)?;

        Ok(())
    }
}

impl MemRegionFactory for MemoryRegionFactory {
    fn create_region(
        &mut self,
        guest_addr: GuestAddress,
        region_len: GuestUsize,
        kvm_slot: u32,
    ) -> std::result::Result<Arc<GuestRegionMmap>, VirtioError> {
        // create address space region
        let mem_type = self.vm_config.mem_type.as_str();
        let mut mem_file_path = self.vm_config.mem_file_path.clone();
        let mem_file_name = format!(
            "/virtiomem_{}_{}",
            self.instance_id.as_str(),
            self.mem_id.as_str()
        );
        mem_file_path.push_str(mem_file_name.as_str());
        let region = Arc::new(
            AddressSpaceRegion::create_default_memory_region(
                guest_addr,
                region_len,
                self.host_numa_node_id,
                mem_type,
                mem_file_path.as_str(),
                false,
                true,
            )
            .map_err(|e| {
                error!(self.logger, "failed to insert address space region: {}", e);
                // dbs-virtio-devices should not depend on dbs-address-space.
                // So here io::Error is used instead of AddressSpaceError directly.
                VirtioError::IOError(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "invalid address space region ({0:#x}, {1:#x})",
                        guest_addr.0, region_len
                    ),
                ))
            })?,
        );
        info!(
            self.logger,
            "VM: mem_type: {} mem_file_path: {}, numa_node_id: {:?} file_offset: {:?}",
            mem_type,
            mem_file_path,
            self.host_numa_node_id,
            region.file_offset();
            "subsystem" => "virito-mem"
        );

        let mmap_region = MmapRegion::build(
            region.file_offset().cloned(),
            region_len as usize,
            region.prot_flags(),
            region.perm_flags(),
        )
        .map_err(VirtioError::NewMmapRegion)?;
        let host_addr: u64 = mmap_region.as_ptr() as u64;

        // thp
        if mem_type == "hugeanon" || mem_type == "hugeshmem" {
            self.configure_thp(&mmap_region)?;
        }

        // Handle numa
        if let Some(numa_node_id) = self.host_numa_node_id {
            self.configure_numa(&mmap_region, numa_node_id)?;
        }

        // add to guest memory mapping
        self.map_to_kvm(kvm_slot, &region, &mmap_region)?;

        info!(
            self.logger,
            "kvm set user memory region: slot: {}, flags: {}, guest_phys_addr: {:X}, memory_size: {}, userspace_addr: {:X}",
            kvm_slot,
            0,
            guest_addr.raw_value(),
            region_len,
            host_addr;
            "subsystem" => "virito-mem"
        );

        // All value should be valid.
        let memory_region = Arc::new(
            GuestRegionMmap::new(mmap_region, guest_addr).map_err(VirtioError::InsertMmap)?,
        );

        let vm_as_new = self
            .vm_as
            .memory()
            .insert_region(memory_region.clone())
            .map_err(VirtioError::InsertMmap)?;
        self.vm_as.lock().unwrap().replace(vm_as_new);
        self.address_space.insert_region(region).map_err(|e| {
            error!(self.logger, "failed to insert address space region: {}", e);
            // dbs-virtio-devices should not depend on dbs-address-space.
            // So here io::Error is used instead of AddressSpaceError directly.
            VirtioError::IOError(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "invalid address space region ({0:#x}, {1:#x})",
                    guest_addr.0, region_len
                ),
            ))
        })?;

        Ok(memory_region)
    }

    fn restore_region_addr(
        &self,
        guest_addr: GuestAddress,
    ) -> std::result::Result<*mut u8, VirtioError> {
        let memory = self.vm_as.memory();
        // NOTE: We can't clone `GuestRegionMmap` reference directly!!!
        //
        // Since an important role of the member `mapping` (type is
        // `MmapRegion`) in `GuestRegionMmap` is to mmap the memory during
        // construction and munmap the memory during drop. However, when the
        // life time of cloned data is over, the drop operation will be
        // performed, which will munmap the origional mmap memory, which will
        // cause some memory in dragonall to be inaccessable. And remember the
        // data structure that was cloned is still alive now, when its life time
        // is over, it will perform the munmap operation again, which will cause
        // a memory exception!
        memory
            .get_host_address(guest_addr)
            .map_err(VirtioError::GuestMemory)
    }

    fn get_host_numa_node_id(&self) -> Option<u32> {
        self.host_numa_node_id
    }

    fn set_host_numa_node_id(&mut self, host_numa_node_id: Option<u32>) {
        self.host_numa_node_id = host_numa_node_id;
    }
}

#[cfg(test)]
mod tests {
    use vm_memory::GuestMemoryRegion;

    use super::*;
    use crate::test_utils::tests::create_vm_for_test;

    impl Default for MemDeviceConfigInfo {
        fn default() -> Self {
            MemDeviceConfigInfo {
                mem_id: "".to_string(),
                size_mib: 0,
                capacity_mib: 1024,
                multi_region: true,
                host_numa_node_id: None,
                guest_numa_node_id: None,
                use_generic_irq: None,
                use_shared_irq: None,
            }
        }
    }

    #[test]
    fn test_mem_config_check_conflicts() {
        let config = MemDeviceConfigInfo::default();
        let mut config2 = MemDeviceConfigInfo::default();
        assert!(config.check_conflicts(&config2).is_err());
        config2.mem_id = "dummy_mem".to_string();
        assert!(config.check_conflicts(&config2).is_ok());
    }

    #[test]
    fn test_create_mem_devices_configs() {
        let mgr = MemDeviceMgr::default();
        assert_eq!(mgr.info_list.len(), 0);
        assert_eq!(mgr.get_index_of_mem_dev(""), None);
    }

    #[test]
    fn test_mem_insert_or_update_device() {
        // Init vm for test.
        let mut vm = create_vm_for_test();

        // We don't need to use virtio-mem before start vm
        // Test for standard config with hotplug
        let device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            vm.vm_address_space().cloned(),
            true,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );

        let dummy_mem_device = MemDeviceConfigInfo::default();
        vm.device_manager_mut()
            .mem_manager
            .insert_or_update_device(device_op_ctx, dummy_mem_device)
            .unwrap();
        assert_eq!(vm.device_manager().mem_manager.info_list.len(), 1);
    }

    #[test]
    fn test_mem_attach_device() {
        // Init vm and insert mem config for test.
        let mut vm = create_vm_for_test();
        let dummy_mem_device = MemDeviceConfigInfo::default();
        vm.device_manager_mut()
            .mem_manager
            .info_list
            .insert_or_update(&dummy_mem_device)
            .unwrap();
        assert_eq!(vm.device_manager().mem_manager.info_list.len(), 1);

        // Test for standard config
        let mut device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            vm.vm_address_space().cloned(),
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        vm.device_manager_mut()
            .mem_manager
            .attach_devices(&mut device_op_ctx)
            .unwrap();
        assert_eq!(vm.device_manager().mem_manager.info_list.len(), 1);
    }

    #[test]
    fn test_mem_create_region() {
        let vm = create_vm_for_test();
        let ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            vm.vm_address_space().cloned(),
            true,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        let mem_id = String::from("mem0");
        let guest_addr = GuestAddress(0x1_0000_0000);
        let region_len = 0x1000_0000;
        let kvm_slot = 2;

        // no vfio manager, no numa node
        let mut factory = MemoryRegionFactory::new(&ctx, mem_id, None).unwrap();
        let region_opt = factory.create_region(guest_addr, region_len, kvm_slot);
        assert_eq!(region_opt.unwrap().len(), region_len);
    }
}
