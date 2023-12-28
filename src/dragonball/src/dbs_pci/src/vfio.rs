// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::any::Any;
use std::io;
use std::os::unix::io::AsRawFd;
use std::ptr::null_mut;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use byteorder::{ByteOrder, LittleEndian};
use dbs_boot::PAGE_SIZE;
use dbs_device::device_manager::IoManagerContext;
use dbs_device::resources::{DeviceResources, MsiIrqType, Resource, ResourceConstraint};
#[cfg(target_arch = "x86_64")]
use dbs_device::PioAddress;
use dbs_device::{DeviceIo, IoAddress};
use dbs_interrupt::{
    DeviceInterruptManager, DeviceInterruptMode, InterruptSourceGroup, KvmIrqManager,
};
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::VmFd;
use log::{debug, error, warn};
use vfio_bindings::bindings::vfio::{
    VFIO_PCI_BAR0_REGION_INDEX, VFIO_PCI_CONFIG_REGION_INDEX, VFIO_PCI_INTX_IRQ_INDEX,
    VFIO_PCI_MSIX_IRQ_INDEX, VFIO_PCI_MSI_IRQ_INDEX, VFIO_PCI_ROM_REGION_INDEX,
    VFIO_REGION_INFO_FLAG_CAPS, VFIO_REGION_INFO_FLAG_MMAP, VFIO_REGION_INFO_FLAG_READ,
    VFIO_REGION_INFO_FLAG_WRITE,
};
use vfio_ioctls::{VfioContainer, VfioDevice, VfioRegionInfoCap};
use vm_memory::{Address, GuestAddress, GuestUsize};

use crate::{
    BarProgrammingParams, MsiCap, MsiState, MsixCap, MsixState, PciBarConfiguration,
    PciBarPrefetchable, PciBarRegionType, PciBus, PciCapability, PciCapabilityId, PciClassCode,
    PciConfiguration, PciDevice, PciHeaderType, PciInterruptPin, PciSubclass, PciSystemContext,
    MSIX_TABLE_ENTRY_SIZE,
};

// Vendor ID offset in the PCI config space
const PCI_CONFIG_VENDOR_OFFSET: u32 = 0x0;
// First BAR offset in the PCI config space.
const PCI_CONFIG_BAR_OFFSET: u32 = 0x10;
// Capability register offset in the PCI config space.
const PCI_CONFIG_CAPABILITY_OFFSET: u32 = 0x34;
// IO BAR when first BAR bit is 1.
const PCI_CONFIG_IO_BAR: u32 = 0x1;
// Memory BAR flags (lower 4 bits).
const PCI_CONFIG_MEMORY_BAR_FLAG_MASK: u32 = 0xf;
// 64-bit memory bar flag.
const PCI_CONFIG_MEMORY_BAR_64BIT: u32 = 0x4;
// Number of BARs for a PCI device
const BAR_NUMS: u32 = 6;
// PCI Header Type register index
const PCI_HEADER_TYPE_REG_INDEX: u32 = 3;
// First BAR register index
const PCI_CONFIG_BAR0_INDEX: u32 = 4;
// PCI ROM expansion BAR register index
const PCI_ROM_EXP_BAR_INDEX: u32 = 12;
// PCI interrupt pin and line register index
const PCI_INTX_REG_INDEX: u32 = 15;
// Vendor id for NVIDIA PCI device
pub const VENDOR_NVIDIA: u16 = 0x10de;
// Offset to shift bit to get the highest 32 bits.
const HIGH_32_BITS_OFFSET: u64 = 32;

#[derive(Debug, thiserror::Error)]
pub enum VfioPciError {
    #[error("failed to create BAR {0}: {1:?}")]
    CreateBar(u32, #[source] crate::Error),
    #[error("device manager error: {0:?}")]
    DeviceManager(#[source] dbs_device::device_manager::Error),
    #[error("invalid assigned resources for VFIO PCI device")]
    InvalidResources,
    #[error("pci_vfio internal error")]
    InternalError,
    #[error("failed to set interrupt mode: {0:?}")]
    InterruptManager(#[source] std::io::Error),
    #[error("failed to map VFIO PCI region into guest: {0:?}")]
    MapRegionGuest(#[source] kvm_ioctls::Error),
    #[error("failed to mmap PCI device region: {0:?}")]
    Mmap(#[source] std::io::Error),
    #[error("failed to issue VFIO ioctl: {0:?}")]
    Vfio(#[source] vfio_ioctls::VfioError),
    #[error("failed to manager vm-pci device: {0:?}")]
    VmPciError(#[source] crate::Error),
    #[error("failed to upgrade bus since it is already dropped")]
    BusIsDropped,
    #[error("failed to find a kvm mem slot")]
    KvmSlotNotFound,
}

type Result<T> = std::result::Result<T, VfioPciError>;

#[derive(Copy, Clone)]
enum PciVfioSubclass {
    VfioSubclass = 0xff,
}

impl PciSubclass for PciVfioSubclass {
    fn get_register_value(&self) -> u8 {
        *self as u8
    }
}

#[derive(PartialEq)]
pub(crate) struct VfioMsi {
    state: MsiState,
    cap: MsiCap,
    cap_offset: u32,
}

impl VfioMsi {
    fn with_in_range(&self, offset: u32) -> Option<u32> {
        if offset >= self.cap_offset && offset < self.cap_offset + self.cap.size() {
            Some(offset - self.cap_offset)
        } else {
            None
        }
    }
}

#[derive(PartialEq)]
pub(crate) struct VfioMsix {
    state: MsixState,
    cap: MsixCap,
    cap_offset: u32,
    table_bir: u32,
    table_offset: u64,
    table_size: u64,
    pba_bir: u32,
    pba_offset: u64,
    pba_size: u64,
}

impl VfioMsix {
    fn with_in_range(&self, offset: u32) -> Option<u32> {
        if offset >= self.cap_offset && offset < self.cap_offset + self.cap.len() as u32 {
            Some(offset - self.cap_offset)
        } else {
            None
        }
    }
}

struct Interrupt {
    vfio_dev: Arc<VfioDevice>,
    irq_manager: Option<DeviceInterruptManager<Arc<KvmIrqManager>>>,
    resources: DeviceResources,
    legacy_enabled: bool,
    // the ability of msi is enabled or not
    msi_enabled: bool,
    // the ability of msix is enabled or not
    msix_enabled: bool,
    // the actual resourece of legacy irq, none means ability not enabled or resource is not allocated.
    legacy_irq: Option<u32>,
    // the actual resourece of msi, none means ability not enabled or resource is not allocated.
    msi: Option<VfioMsi>,
    //the actual resourece of msix, none means ability not enabled or resource is not allocated.
    msix: Option<VfioMsix>,
}

// The PCI specification defines an priority order of:
//      PCI MSIx > PCI MSI > Legacy Interrupt
// That means all of PCI MSIx, PCI MSI and Legacy Interrupt could be enabled,
// and the device hardware choose the highest priority mechanism to trigger
// the interrupt. It's really to emulate the PCI MSI/MSIx capabilities:(
impl Interrupt {
    pub(crate) fn new(vfio_dev: Arc<VfioDevice>) -> Self {
        Interrupt {
            vfio_dev,
            irq_manager: None,
            resources: DeviceResources::new(),
            legacy_enabled: false,
            msi_enabled: false,
            msix_enabled: false,
            legacy_irq: None,
            msi: None,
            msix: None,
        }
    }

    pub(crate) fn initialize(&mut self, irq_mgr: Arc<KvmIrqManager>) -> Result<()> {
        let mut irq_manager = DeviceInterruptManager::new(irq_mgr, &self.resources)
            .map_err(VfioPciError::InterruptManager)?;

        // Enable legacy irq by default if it's present
        if self.legacy_irq.is_some() {
            irq_manager
                .set_working_mode(DeviceInterruptMode::LegacyIrq)
                .map_err(VfioPciError::InterruptManager)?;
            irq_manager
                .enable()
                .map_err(VfioPciError::InterruptManager)?;
            Self::enable_vfio_irqfds(&self.vfio_dev, VFIO_PCI_INTX_IRQ_INDEX, 1, &irq_manager)?;
            self.legacy_enabled = true;
        }

        self.irq_manager = Some(irq_manager);

        Ok(())
    }

    fn add_msi_irq_resource(&mut self, base: u32, size: u32) {
        if let Some(msix) = self.msix.as_ref() {
            assert!(msix.cap.table_size() as u32 <= size);
            self.resources.append(Resource::MsiIrq {
                ty: MsiIrqType::PciMsix,
                base,
                size: msix.cap.table_size() as u32,
            });
            return;
        }
        if let Some(msi) = self.msi.as_ref() {
            assert!(msi.cap.num_vectors() as u32 <= size);
            self.resources.append(Resource::MsiIrq {
                ty: MsiIrqType::PciMsi,
                base,
                size: msi.cap.num_vectors() as u32,
            });
        }
    }

    fn add_legacy_irq_resource(&mut self, base: u32) {
        self.legacy_irq = Some(base);
        self.resources.append(Resource::LegacyIrq(base));
    }

    fn get_irq_pin(&self) -> u32 {
        if let Some(legacy_irq) = self.legacy_irq {
            (PciInterruptPin::IntA as u32) << 8 | legacy_irq
        } else {
            0
        }
    }

    fn cap_read(&mut self, offset: u32, data: &mut [u8]) -> bool {
        if let Some(msix) = self.msix.as_mut() {
            if let Some(mut offset) = msix.with_in_range(offset) {
                for ptr in data.iter_mut() {
                    *ptr = msix.cap.read_u8(offset as usize);
                    offset += 1;
                }
                return true;
            }
        }

        if let Some(msi) = self.msi.as_mut() {
            if let Some(offset) = msi.with_in_range(offset) {
                match data.len() {
                    1 => data[0] = msi.cap.read_u8(offset as usize),
                    2 => LittleEndian::write_u16(data, msi.cap.read_u16(offset as usize)),
                    4 => LittleEndian::write_u32(data, msi.cap.read_u32(offset as usize)),
                    _ => debug!(
                        "invalid msi cap read data length {} and offset {}!",
                        data.len(),
                        offset
                    ),
                };
            }
        }

        false
    }

    fn cap_write(&mut self, offset: u32, data: &[u8]) -> bool {
        if let Some(msix) = &self.msix {
            if let Some(offset) = msix.with_in_range(offset) {
                if let Err(e) = self.update_msix_capability(offset, data) {
                    error!("Could not update MSI-X capability: {}", e);
                }
                return true;
            }
        }

        if let Some(msi) = self.msi.as_mut() {
            if let Some(offset) = msi.with_in_range(offset) {
                if let Err(e) = self.update_msi_capability(offset, data) {
                    error!("Could not update MSI capability: {}", e);
                }
            }
        }

        false
    }

    fn update_msix_capability(&mut self, offset: u32, data: &[u8]) -> Result<()> {
        if let Some(msix) = self.msix.as_mut() {
            // Update the MSIx capability data structure first.
            for (idx, value) in data.iter().enumerate() {
                msix.cap.write_u8(offset as usize + idx, *value);
            }

            // Then handle actual changes.
            let irq_manager = self.irq_manager.as_mut().unwrap();
            debug!(
                "MSIX state[{}, {}], msi:{}, legacy:{}",
                msix.state.enabled(),
                msix.cap.enabled(),
                self.msi_enabled,
                self.legacy_enabled
            );

            match (msix.state.enabled(), msix.cap.enabled()) {
                (false, true) => {
                    if self.msi_enabled {
                        if let Some(msi) = self.msi.as_mut() {
                            self.vfio_dev
                                .disable_irq(VFIO_PCI_MSI_IRQ_INDEX)
                                .map_err(VfioPciError::Vfio)?;
                            msi.state
                                .disable(irq_manager)
                                .map_err(VfioPciError::InterruptManager)?;
                        }
                    } else if self.legacy_enabled {
                        self.vfio_dev
                            .disable_irq(VFIO_PCI_INTX_IRQ_INDEX)
                            .map_err(VfioPciError::Vfio)?;
                        self.legacy_enabled = false;
                    }

                    msix.state
                        .set_msg_ctl(msix.cap.msg_ctl, irq_manager)
                        .map_err(VfioPciError::InterruptManager)?;
                    Self::enable_vfio_irqfds(
                        &self.vfio_dev,
                        VFIO_PCI_MSIX_IRQ_INDEX,
                        msix.cap.table_size() as u32,
                        irq_manager,
                    )?;
                    self.msix_enabled = true;
                }

                (true, false) => {
                    self.msix_enabled = false;
                    self.vfio_dev
                        .disable_irq(VFIO_PCI_MSIX_IRQ_INDEX)
                        .map_err(VfioPciError::Vfio)?;
                    msix.state
                        .set_msg_ctl(msix.cap.msg_ctl, irq_manager)
                        .map_err(VfioPciError::InterruptManager)?;

                    if self.msi_enabled {
                        self.synchronize_msi()?;
                    } else {
                        self.try_enable_legacy_irq()?;
                    }
                }

                (true, true) => {
                    if msix.cap.masked() != msix.state.masked() {
                        msix.state
                            .set_msg_ctl(msix.cap.msg_ctl, irq_manager)
                            .map_err(VfioPciError::InterruptManager)?;
                    }
                }

                (false, false) => {
                    debug!("msix state not enabled and msix cap not enabled.");
                }
            }
        }

        Ok(())
    }

    fn update_msi_capability(&mut self, offset: u32, data: &[u8]) -> Result<()> {
        if let Some(msi) = self.msi.as_mut() {
            // Update the MSIx capability data structure first.
            match data.len() {
                1 => msi.cap.write_u8(offset as usize, data[0]),
                2 => msi
                    .cap
                    .write_u16(offset as usize, LittleEndian::read_u16(data)),
                4 => msi
                    .cap
                    .write_u32(offset as usize, LittleEndian::read_u32(data)),
                _ => debug!("invalid msi cap write data length!"),
            }

            // PCI MSI-x has higher priority than PCI MSI.
            if !self.msix_enabled {
                self.synchronize_msi()?;
            }
        }

        Ok(())
    }

    fn synchronize_msi(&mut self) -> Result<()> {
        if let Some(msi) = self.msi.as_mut() {
            let irq_manager = self.irq_manager.as_mut().unwrap();

            debug!(
                "synchronize state[{}, {}], msi:{}, legacy:{}",
                msi.state.enabled(),
                msi.cap.enabled(),
                self.msi_enabled,
                self.legacy_enabled
            );

            match (msi.state.enabled(), msi.cap.enabled()) {
                (false, true) => {
                    if self.legacy_enabled {
                        self.vfio_dev
                            .disable_irq(VFIO_PCI_INTX_IRQ_INDEX)
                            .map_err(VfioPciError::Vfio)?;
                        self.legacy_enabled = false;
                    }

                    msi.state
                        .synchronize_state(&msi.cap, irq_manager)
                        .map_err(VfioPciError::InterruptManager)?;
                    Self::enable_vfio_irqfds(
                        &self.vfio_dev,
                        VFIO_PCI_MSI_IRQ_INDEX,
                        msi.cap.num_enabled_vectors() as u32,
                        irq_manager,
                    )?;
                    self.msi_enabled = true;
                }

                (true, false) => {
                    self.msi_enabled = false;
                    self.vfio_dev
                        .disable_irq(VFIO_PCI_MSI_IRQ_INDEX)
                        .map_err(VfioPciError::Vfio)?;
                    msi.state
                        .synchronize_state(&msi.cap, irq_manager)
                        .map_err(VfioPciError::InterruptManager)?;
                    self.try_enable_legacy_irq()?;
                }

                (true, true) => {
                    self.msi_enabled = true;
                    msi.state
                        .synchronize_state(&msi.cap, irq_manager)
                        .map_err(VfioPciError::InterruptManager)?;
                }

                (false, false) => {
                    self.msi_enabled = false;
                    msi.state
                        .synchronize_state(&msi.cap, irq_manager)
                        .map_err(VfioPciError::InterruptManager)?;
                }
            }
        }

        Ok(())
    }

    fn try_enable_legacy_irq(&mut self) -> Result<()> {
        if self.legacy_irq.is_some() {
            let irq_manager = self.irq_manager.as_mut().unwrap();
            irq_manager
                .reset()
                .map_err(VfioPciError::InterruptManager)?;
            irq_manager
                .set_working_mode(DeviceInterruptMode::LegacyIrq)
                .map_err(VfioPciError::InterruptManager)?;
            irq_manager
                .enable()
                .map_err(VfioPciError::InterruptManager)?;
            Self::enable_vfio_irqfds(&self.vfio_dev, VFIO_PCI_INTX_IRQ_INDEX, 1, irq_manager)?;
            self.legacy_enabled = true;
        }

        Ok(())
    }

    pub(crate) fn enable_vfio_irqfds(
        vfio_dev: &Arc<VfioDevice>,
        index: u32,
        count: u32,
        irq_mgr: &DeviceInterruptManager<Arc<KvmIrqManager>>,
    ) -> Result<Arc<Box<dyn InterruptSourceGroup>>> {
        if let Some(group) = irq_mgr.get_group() {
            if count > group.len() {
                debug!(
                    "Configure MSI vector number is too big({} > {})",
                    count,
                    group.len()
                );
                return Err(VfioPciError::InternalError);
            }

            let mut irqfds = Vec::with_capacity(count as usize);
            for idx in 0..count {
                if let Some(fd) = group.notifier(idx) {
                    irqfds.push(fd)
                } else {
                    warn!("pci_vfio: failed to get irqfd 0x{:x} for vfio device", idx);
                    return Err(VfioPciError::InternalError);
                }
            }
            vfio_dev
                .enable_irq(index, irqfds)
                .map_err(VfioPciError::Vfio)?;

            Ok(group)
        } else {
            warn!("pci_vfio: can not get interrupt group for vfio device");
            Err(VfioPciError::InternalError)
        }
    }

    fn msix_table_accessed(&self, bar_index: u32, offset: u64) -> bool {
        if let Some(msix) = self.msix.as_ref() {
            return bar_index == msix.table_bir
                && offset >= msix.table_offset
                && offset < msix.table_offset + msix.table_size;
        }

        false
    }

    fn msix_table_read(&mut self, offset: u64, data: &mut [u8]) {
        if let Some(msix) = self.msix.as_ref() {
            let offset = offset - u64::from(msix.cap.table_offset());
            msix.state.read_table(offset, data)
        }
    }

    fn msix_table_write(&mut self, offset: u64, data: &[u8]) {
        if let Some(msix) = self.msix.as_mut() {
            // Safe to unwrap() because initialize() has already been called.
            let intr_mgr = self.irq_manager.as_mut().unwrap();
            let offset = offset - u64::from(msix.cap.table_offset());
            if let Err(e) = msix.state.write_table(offset, data, intr_mgr) {
                debug!("failed to update PCI MSI-x table entry, {}", e);
            }
        }
    }

    fn msix_pba_accessed(&self, bar_index: u32, offset: u64) -> bool {
        if let Some(msix) = self.msix.as_ref() {
            return bar_index == msix.pba_bir
                && offset >= msix.pba_offset
                && offset < msix.pba_offset + msix.pba_size;
        }

        false
    }

    fn msix_pba_read(&mut self, offset: u64, data: &mut [u8]) {
        if let Some(msix) = self.msix.as_mut() {
            // Safe to unwrap() because initialize() has already been called.
            let intr_mgr = self.irq_manager.as_mut().unwrap();
            let offset = offset - u64::from(msix.cap.pba_offset());
            msix.state.read_pba(offset, data, intr_mgr)
        }
    }

    fn msix_pba_write(&mut self, _offset: u64, _data: &[u8]) {
        // The PBA area is readonly, just discarding writes.
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct MsixTable {
    pub(crate) offset: u64,
    pub(crate) length: u64,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PciRegionMmap {
    slot: u32,
    mmap_offset: u64,
    mmap_size: u64,
    mmap_host_addr: u64,
    prot_flags: i32,
}

pub(crate) struct Region {
    bar_index: u32,
    reg_index: u32,
    type_: PciBarRegionType,
    length: GuestUsize,
    start: GuestAddress,
    rom_bar_active: u32,
    is_mmio_bar: bool,
    mappable: bool,
    mapped: bool,
    mmaps: Vec<PciRegionMmap>,
    msix_table: Option<MsixTable>,
    is_sparse_mmap: bool,
    prefetchable: PciBarPrefetchable,
}

impl Region {
    #[inline]
    fn mappable(&self) -> bool {
        self.mappable
    }

    // use map to determine whether to create or delete physical memory slot
    fn set_user_memory_region(&mut self, index: usize, map: bool, vm: &Arc<VmFd>) -> Result<()> {
        let mmap = &self.mmaps[index];
        let mmap_size = match map {
            true => mmap.mmap_size,
            false => 0,
        };

        let mem_region = kvm_userspace_memory_region {
            slot: mmap.slot,
            guest_phys_addr: self.start.raw_value() + mmap.mmap_offset,
            memory_size: mmap_size,
            userspace_addr: mmap.mmap_host_addr,
            flags: 0,
        };
        log::info!("set_user_memory_region, index: {}, slot: 0x{:x}, guest phy: 0x{:x}, memory_size: 0x{:x}, user addr: 0x{:x}",
                index,
                mem_region.slot,
                mem_region.guest_phys_addr,
                mem_region.memory_size,
                mem_region.userspace_addr);
        // Safe because the guest regions are guaranteed not to overlap.
        unsafe {
            vm.set_user_memory_region(mem_region)
                .map_err(VfioPciError::MapRegionGuest)?;
        }
        Ok(())
    }

    // vfio map is to attach the device to the vfio container.
    fn map(
        &mut self,
        vfio_dev: &Arc<VfioDevice>,
        vm: &Arc<VmFd>,
        vfio_container: &Arc<VfioContainer>,
    ) -> Result<()> {
        if !self.mappable || self.mapped {
            return Ok(());
        }

        let region_flags = vfio_dev.get_region_flags(self.bar_index);
        let mut prot = 0;
        if region_flags & VFIO_REGION_INFO_FLAG_READ != 0 {
            prot |= libc::PROT_READ;
        }
        if region_flags & VFIO_REGION_INFO_FLAG_WRITE != 0 {
            prot |= libc::PROT_WRITE;
        }

        // NOTE:
        // Support for VFIO sparse mapping. Code here and the vfio-ioctls crate assumes that
        // there's one sparse mapping area in each region at most. The assumption doesn't
        // follow the VFIO specification, but may be acceptable for real PCI hardware.
        for i in 0..self.mmaps.len() {
            let mmap = &self.mmaps[i];
            let offset = vfio_dev.get_region_offset(self.bar_index) + mmap.mmap_offset;
            let fd = vfio_dev.as_raw_fd();

            let host_addr = unsafe {
                libc::mmap(
                    null_mut(),
                    mmap.mmap_size as usize,
                    prot,
                    libc::MAP_SHARED,
                    fd,
                    offset as libc::off_t,
                )
            };
            if host_addr == libc::MAP_FAILED {
                for j in 0..i {
                    let ret = unsafe {
                        libc::munmap(
                            self.mmaps[j].mmap_host_addr as *mut libc::c_void,
                            self.mmaps[j].mmap_size as usize,
                        )
                    };
                    if ret != 0 {
                        error!("unmap regions failed, error:{}", io::Error::last_os_error());
                    }
                }
                return Err(VfioPciError::Mmap(io::Error::last_os_error()));
            }

            self.mmaps[i].mmap_host_addr = host_addr as u64;
            self.mmaps[i].prot_flags = prot;
            self.set_user_memory_region(i, true, vm).map_err(|e| {
                for j in 0..i {
                    match self.set_user_memory_region(j, false, vm) {
                        Ok(_) => {}
                        Err(err) => {
                            error!("Could not delete kvm memory slot, error:{}", err);
                        }
                    }

                    let ret = unsafe {
                        libc::munmap(
                            self.mmaps[j].mmap_host_addr as *mut libc::c_void,
                            self.mmaps[j].mmap_size as usize,
                        )
                    };
                    if ret != 0 {
                        error!(
                            "unmap regions failed when set kvm mem, error:{}",
                            io::Error::last_os_error()
                        );
                    }
                }
                e
            })?;

            // FIXME: add readonly flag into vfio_dma_map in future PR when it is needed.
            // issue #8725
            if let Err(e) = vfio_container.vfio_dma_map(
                self.start.raw_value() + self.mmaps[i].mmap_offset,
                self.mmaps[i].mmap_size,
                host_addr as u64,
            ) {
                error!(
                    "vfio dma map failed, pci p2p dma may not work, due to {:?}",
                    e
                );
            }
        }

        self.mapped = true;

        Ok(())
    }

    fn unmap(&mut self, vm: &Arc<VmFd>, vfio_container: &Arc<VfioContainer>) -> Result<()> {
        if !self.mapped {
            return Ok(());
        }

        for i in 0..self.mmaps.len() {
            self.set_user_memory_region(i, false, vm)?;

            let ret = unsafe {
                libc::munmap(
                    self.mmaps[i].mmap_host_addr as *mut libc::c_void,
                    self.mmaps[i].mmap_size as usize,
                )
            };
            if ret != 0 {
                // Any way to recover here? Just ignoring the error?
                error!(
                    "Could not unmap regions, error:{}",
                    io::Error::last_os_error()
                );
            }

            if let Err(e) = vfio_container.vfio_dma_unmap(
                self.start.raw_value() + self.mmaps[i].mmap_offset,
                self.mmaps[i].mmap_size,
            ) {
                error!(
                    "vfio dma unmap failed, pci p2p dma may not work, due to {:?}",
                    e
                );
            }
        }

        self.mapped = false;

        Ok(())
    }

    fn remap(
        &mut self,
        vm: &Arc<VmFd>,
        params: &BarProgrammingParams,
        vfio_container: &Arc<VfioContainer>,
    ) -> Result<()> {
        if params.old_base != 0 && params.new_base == 0 {
            // Remove the KVM memory slot by setting `memory_size` to 0
            for i in 0..self.mmaps.len() {
                self.set_user_memory_region(i, false, vm)?;
                self.start = GuestAddress(0);
            }
        } else if params.new_base != 0 {
            for i in 0..self.mmaps.len() {
                if let Err(e) = vfio_container.vfio_dma_unmap(
                    self.start.raw_value() + self.mmaps[i].mmap_offset,
                    self.mmaps[i].mmap_size,
                ) {
                    error!(
                        "vfio dma unmap failed, pci p2p dma may not work, due to {:?}",
                        e
                    );
                }
                self.start = GuestAddress(params.new_base);
                self.set_user_memory_region(i, true, vm)?;
                // FIXME: add readonly flag into vfio_dma_map in future PR when it is needed.
                // issue #8725
                if let Err(e) = vfio_container.vfio_dma_map(
                    self.start.raw_value() + self.mmaps[i].mmap_offset,
                    self.mmaps[i].mmap_size,
                    self.mmaps[i].mmap_host_addr,
                ) {
                    error!(
                        "vfio dma map failed, pci p2p dma may not work, due to {:?}",
                        e
                    );
                }
            }
        }

        Ok(())
    }

    fn trap<D: IoManagerContext>(
        &mut self,
        ctx: &D,
        tx: &mut D::Context,
        device: Arc<dyn DeviceIo>,
    ) -> Result<()> {
        if !self.mapped || self.is_sparse_mmap || self.msix_table.is_some() {
            let resources = self.to_resources();
            ctx.register_device_io(tx, device, &resources)
                .map_err(VfioPciError::DeviceManager)?;
            self.mapped = true;
        }

        Ok(())
    }

    fn untrap<D: IoManagerContext>(&mut self, ctx: &D, tx: &mut D::Context) -> Result<()> {
        if self.mapped {
            let resources = self.to_resources();
            ctx.unregister_device_io(tx, &resources)
                .map_err(VfioPciError::DeviceManager)?;
            self.mapped = false;
        }

        Ok(())
    }

    fn retrap<D: IoManagerContext>(
        &mut self,
        ctx: &D,
        tx: &mut D::Context,
        device: Arc<dyn DeviceIo>,
        params: &BarProgrammingParams,
    ) -> Result<()> {
        let old_state = self.mapped;
        let old_addr = self.start;

        if old_state {
            self.untrap(ctx, tx)?
        }
        if params.new_base != 0 {
            self.start = GuestAddress(params.new_base);
            if let Err(e) = self.trap(ctx, tx, device) {
                self.mapped = old_state;
                self.start = old_addr;
                return Err(e);
            }
        }

        Ok(())
    }

    fn to_resources(&self) -> DeviceResources {
        let mut resources = DeviceResources::new();

        if self.is_mmio_bar {
            if self.mappable {
                if let Some(msix) = self.msix_table {
                    resources.append(Resource::MmioAddressRange {
                        base: self.start.raw_value() + msix.offset,
                        size: msix.length,
                    });
                }
            } else {
                resources.append(Resource::MmioAddressRange {
                    base: self.start.raw_value(),
                    size: self.length,
                });
            }
        } else {
            resources.append(Resource::PioAddressRange {
                base: self.start.raw_value() as u16,
                size: self.length as u16,
            });
        }

        resources
    }
}

pub struct VfioPciDeviceState<C: PciSystemContext> {
    vfio_path: String,
    interrupt: Interrupt,
    vfio_dev: Arc<VfioDevice>,
    context: Weak<C>,
    configuration: PciConfiguration,
    device: Option<Weak<dyn DeviceIo>>,
    regions: Vec<Region>,
    sys_constraints: Vec<ResourceConstraint>, //kvm slot, msix irq
    assigned_resources: DeviceResources,
    trapped_resources: DeviceResources,
    bus: Weak<PciBus>,
    vfio_container: Arc<VfioContainer>,
}

impl<C: PciSystemContext> VfioPciDeviceState<C> {
    fn new(
        vfio_path: String,
        vfio_dev: Arc<VfioDevice>,
        bus: Weak<PciBus>,
        context: Weak<C>,
        vendor_device_id: u32,
        clique_id: Option<u8>,
        vfio_container: Arc<VfioContainer>,
    ) -> Result<Self> {
        let (mut vendor_id, mut device_id) = (0, 0);
        if vendor_device_id != 0 {
            vendor_id = (vendor_device_id & 0xffff) as u16;
            device_id = ((vendor_device_id >> 16) & 0xffff) as u16;
        }

        let configuration = PciConfiguration::new(
            bus.clone(),
            vendor_id,
            device_id,
            PciClassCode::Other,
            &PciVfioSubclass::VfioSubclass,
            None,
            PciHeaderType::Device,
            0,
            0,
            clique_id,
        )
        .map_err(VfioPciError::VmPciError)?;

        let interrupt = Interrupt::new(vfio_dev.clone());

        Ok(VfioPciDeviceState {
            vfio_path,
            vfio_dev,
            context,
            configuration,
            interrupt,
            device: None,
            regions: Vec::new(),
            sys_constraints: Vec::new(),
            assigned_resources: DeviceResources::new(),
            trapped_resources: DeviceResources::new(),
            bus,
            vfio_container,
        })
    }

    pub fn vfio_dev(&self) -> &Arc<VfioDevice> {
        &self.vfio_dev
    }

    fn read_config_byte(&self, offset: u32) -> u8 {
        let mut data: [u8; 1] = [0];
        self.vfio_dev
            .region_read(VFIO_PCI_CONFIG_REGION_INDEX, data.as_mut(), offset.into());

        data[0]
    }

    fn read_config_word(&self, offset: u32) -> u16 {
        let mut data: [u8; 2] = [0, 0];
        self.vfio_dev
            .region_read(VFIO_PCI_CONFIG_REGION_INDEX, data.as_mut(), offset.into());

        u16::from_le_bytes(data)
    }

    fn read_config_dword(&self, offset: u32) -> u32 {
        let mut data: [u8; 4] = [0, 0, 0, 0];
        self.vfio_dev
            .region_read(VFIO_PCI_CONFIG_REGION_INDEX, data.as_mut(), offset.into());

        u32::from_le_bytes(data)
    }

    fn write_config_dword(&self, buf: u32, offset: u32) {
        let data: [u8; 4] = buf.to_le_bytes();
        self.vfio_dev
            .region_write(VFIO_PCI_CONFIG_REGION_INDEX, &data, offset.into())
    }

    fn is_mdev(&self) -> bool {
        self.vfio_path.contains("/sys/bus/mdev/devices")
    }
}

impl<C: PciSystemContext> VfioPciDeviceState<C> {
    fn probe_regions(&mut self, bus: Weak<PciBus>) -> Result<()> {
        let mut bar_id = VFIO_PCI_BAR0_REGION_INDEX;

        // Going through all regular regions to compute the BAR size.
        // We're not saving the BAR address to restore it, because we are going to allocate a guest
        // address for each BAR and write that new address back.
        while bar_id < VFIO_PCI_CONFIG_REGION_INDEX {
            let mut lsb_size: u32 = 0xffff_ffff;
            let mut msb_size = 0;
            let mut region_size: u64;
            let bar_addr: GuestAddress;

            // Read the BAR size (Starts by all 1s to the BAR)
            let bar_offset = if bar_id == VFIO_PCI_ROM_REGION_INDEX {
                PCI_ROM_EXP_BAR_INDEX * 4
            } else {
                PCI_CONFIG_BAR_OFFSET + bar_id * 4
            };
            self.write_config_dword(lsb_size, bar_offset);
            lsb_size = self.read_config_dword(bar_offset);
            // We've just read the BAR size back. Or at least its LSB.
            let lsb_flag = lsb_size & PCI_CONFIG_MEMORY_BAR_FLAG_MASK;
            if lsb_size == 0 {
                bar_id += 1;
                continue;
            }

            // Is this an IO BAR?
            let io_bar = if bar_id != VFIO_PCI_ROM_REGION_INDEX {
                lsb_flag & PCI_CONFIG_IO_BAR == PCI_CONFIG_IO_BAR
            } else {
                false
            };

            // Is this a 64-bit BAR?
            let is_64bit_bar = if bar_id != VFIO_PCI_ROM_REGION_INDEX {
                lsb_flag & PCI_CONFIG_MEMORY_BAR_64BIT == PCI_CONFIG_MEMORY_BAR_64BIT
            } else {
                false
            };

            // Is this BAR prefetchable?
            let mut is_prefetchable = PciBarPrefetchable::NotPrefetchable;
            if bar_id != VFIO_PCI_ROM_REGION_INDEX
                && lsb_flag & (PciBarPrefetchable::Prefetchable as u32)
                    == (PciBarPrefetchable::Prefetchable as u32)
            {
                is_prefetchable = PciBarPrefetchable::Prefetchable
            };

            // By default, the region type is 32 bits memory BAR.
            let mut region_type = PciBarRegionType::Memory32BitRegion;
            let mut mappable = false;
            let mut is_sparse_mmap = false;
            let mut constraints = Vec::new();
            let mut mmap_offset: u64 = 0;
            let mut mmap_size: u64 = 0;

            if io_bar {
                // IO BAR
                region_type = PciBarRegionType::IoRegion;
                // Clear first bit.
                lsb_size &= 0xffff_fffc;
                // Find the first bit that's set to 1.
                let first_bit = lsb_size.trailing_zeros();
                region_size = 2u64.pow(first_bit);

                constraints.push(ResourceConstraint::PioAddress {
                    range: None,
                    size: region_size as u16,
                    align: region_size as u16,
                });
                let resources = bus
                    .upgrade()
                    .ok_or(VfioPciError::BusIsDropped)?
                    .allocate_resources(&constraints)
                    .map_err(VfioPciError::VmPciError)?;
                // unwrap is safe here because we have just allocate the pio address above.
                let base = resources.get_pio_address_ranges().pop().unwrap();
                bar_addr = GuestAddress(base.0 as u64);
            } else {
                if is_64bit_bar {
                    // 64 bits Memory BAR
                    region_type = PciBarRegionType::Memory64BitRegion;
                    msb_size = 0xffff_ffff;
                    let msb_bar_offset: u32 = PCI_CONFIG_BAR_OFFSET + (bar_id + 1) * 4;

                    self.write_config_dword(msb_size, msb_bar_offset);
                    msb_size = self.read_config_dword(msb_bar_offset);
                }

                // Clear the first four bytes from our LSB.
                lsb_size &= 0xffff_fff0;

                region_size = u64::from(msb_size);
                region_size <<= HIGH_32_BITS_OFFSET;
                region_size |= u64::from(lsb_size);

                // Find the first that's set to 1.
                let first_bit = region_size.trailing_zeros();
                region_size = 2u64.pow(first_bit);

                // We need to allocate a guest MMIO address range for that BAR.
                // In case the BAR is mappable directly, this means it might be
                // set as KVM user memory region, which expects to deal with 4K
                // pages. Therefore, the aligment has to be set accordingly.
                let region_flags = self.vfio_dev.get_region_flags(bar_id);
                let region_size = self.vfio_dev.get_region_size(bar_id);
                let caps = self.vfio_dev.get_region_caps(bar_id);
                mmap_size = region_size;
                for cap in caps {
                    if let VfioRegionInfoCap::SparseMmap(m) = cap {
                        // assume there is only one mmap area
                        mmap_offset = m.areas[0].offset;
                        mmap_size = m.areas[0].size;
                    }
                }
                is_sparse_mmap =
                    region_flags & VFIO_REGION_INFO_FLAG_CAPS != 0 && mmap_size != region_size;
                mappable = region_flags & VFIO_REGION_INFO_FLAG_MMAP != 0 || is_sparse_mmap;
                if mappable {
                    self.sys_constraints.push(ResourceConstraint::KvmMemSlot {
                        slot: None,
                        size: 1,
                    })
                }
                let mut align = if (bar_id == VFIO_PCI_ROM_REGION_INDEX) || mappable {
                    // 4K alignment
                    0x1000u64
                } else {
                    // Default 16 bytes alignment
                    0x10u64
                };
                if region_size > align {
                    align = region_size;
                }

                if is_64bit_bar {
                    // Leave MMIO address below 4G for 32bit bars.
                    constraints.push(ResourceConstraint::MmioAddress {
                        range: Some((0x1_0000_0000, 0xffff_ffff_ffff_ffff)),
                        size: region_size,
                        align,
                    });
                } else {
                    constraints.push(ResourceConstraint::MmioAddress {
                        range: Some((0, 0xffff_ffff)),
                        size: region_size,
                        align,
                    });
                }
                let resources = bus
                    .upgrade()
                    .ok_or(VfioPciError::BusIsDropped)?
                    .allocate_resources(&constraints)
                    .map_err(VfioPciError::VmPciError)?;
                // unwrap is safe because we have just allocated mmio address resource.
                let base = resources.get_mmio_address_ranges().pop().unwrap();
                bar_addr = GuestAddress(base.0);
            }

            log::info!(
                "{} region info[{},{},0x{:x},0x{:x},{},{}]",
                self.vfio_path(),
                bar_id,
                bar_offset,
                region_size,
                bar_addr.raw_value(),
                io_bar,
                mappable
            );

            log::info!("mmap_size {}, mmap_offset {} ", mmap_size, mmap_offset);

            self.regions.push(Region {
                bar_index: bar_id,
                reg_index: bar_offset >> 2,
                type_: region_type,
                length: region_size,
                start: bar_addr,
                rom_bar_active: lsb_flag & 0x1,
                is_mmio_bar: !io_bar,
                mappable,
                mapped: false,
                mmaps: vec![PciRegionMmap {
                    slot: 0,
                    mmap_offset,
                    mmap_size,
                    mmap_host_addr: 0,
                    prot_flags: 0,
                }],
                msix_table: None,
                is_sparse_mmap,
                prefetchable: is_prefetchable,
            });

            bar_id += 1;
            if is_64bit_bar {
                bar_id += 1;
            }
        }

        Ok(())
    }

    fn fixup_msix_region(&mut self) {
        let msix = match &self.interrupt.msix {
            Some(msix) => msix,
            None => return,
        };

        for region in self.regions.iter_mut() {
            if !region.mappable {
                continue;
            }
            if region.bar_index == msix.cap.table_bir() {
                let align_to_pagesize = |address| address & !(PAGE_SIZE as u64 - 1);
                let start = align_to_pagesize(msix.table_offset);
                let end = align_to_pagesize(start + msix.table_size + PAGE_SIZE as u64 - 1);
                let region_size = region.mmaps[0].mmap_size;

                log::info!(
                    "{} fixup region, bar {}, msix table offset 0x{:x}, msix table end 0x{:x}",
                    self.vfio_path,
                    region.bar_index,
                    start,
                    end
                );

                if start == 0 {
                    if end >= region_size {
                        region.mappable = false;
                    } else {
                        region.mmaps[0].mmap_offset = end;
                        region.mmaps[0].mmap_size = region_size - end;
                    }
                } else if end >= region_size {
                    region.mmaps[0].mmap_size = start;
                } else {
                    region.mmaps[0].mmap_size = start;
                    region.mmaps.push(PciRegionMmap {
                        slot: 0,
                        mmap_offset: end,
                        mmap_size: region_size - end,
                        mmap_host_addr: 0,
                        prot_flags: 0,
                    });
                    self.sys_constraints.push(ResourceConstraint::KvmMemSlot {
                        slot: None,
                        size: 1,
                    });
                }
                region.msix_table = Some(MsixTable {
                    offset: start,
                    length: end - start,
                });
            }
        }
    }

    fn add_bar_for_region(&mut self, reg_idx: usize) -> Result<()> {
        let config = PciBarConfiguration::default()
            .set_bar_index(self.regions[reg_idx].bar_index as usize)
            .set_bar_type(self.regions[reg_idx].type_)
            .set_address(self.regions[reg_idx].start.raw_value())
            .set_size(self.regions[reg_idx].length)
            .set_prefetchable(self.regions[reg_idx].prefetchable);

        if self.regions[reg_idx].bar_index == VFIO_PCI_ROM_REGION_INDEX {
            self.configuration
                .add_device_rom_bar(&config, self.regions[reg_idx].rom_bar_active)
                .map_err(|e| VfioPciError::CreateBar(self.regions[reg_idx].bar_index, e))?;
        } else {
            self.configuration
                .add_device_bar(&config)
                .map_err(|e| VfioPciError::CreateBar(self.regions[reg_idx].bar_index, e))?;
        }

        Ok(())
    }

    fn find_region(&self, addr: u64, mmio_bar: bool) -> Option<&Region> {
        self.regions.iter().find(|&region| {
            region.is_mmio_bar == mmio_bar
                && addr >= region.start.raw_value()
                && addr < region.start.unchecked_add(region.length).raw_value()
        })
    }

    fn register_regions(&mut self, vm: &Arc<VmFd>) -> Result<()> {
        let ctx = self
            .context
            .upgrade()
            .ok_or(VfioPciError::BusIsDropped)?
            .get_device_manager_context();
        let mut tx = ctx.begin_tx();

        for region in self.regions.iter_mut() {
            let mappable = region.mappable();
            if mappable {
                region.map(&self.vfio_dev, vm, &self.vfio_container)?;
            }
            // If region contains sparse mmap or msix table, then also need to trap access.
            if !mappable || region.is_sparse_mmap || region.msix_table.is_some() {
                // Safe to unwrap because activate() has set self.device to a valid value.
                let device = self.device.as_ref().unwrap().clone();
                if let Err(e) = region.trap(
                    &ctx,
                    &mut tx,
                    device.upgrade().ok_or(VfioPciError::BusIsDropped)?,
                ) {
                    ctx.cancel_tx(tx);
                    // The transaction has been cancelled, restore state of trapped regions
                    for region in self.regions.iter_mut() {
                        if !region.mappable() {
                            region.mapped = false;
                        }
                    }
                    return Err(e);
                }
                for res in region.to_resources().iter() {
                    self.trapped_resources.append(res.clone())
                }
            }
        }

        ctx.commit_tx(tx);

        Ok(())
    }

    fn free_register_resources(&self) -> Result<()> {
        let mut register_resources = DeviceResources::new();
        for region in self.regions.iter() {
            let resources = region.to_resources();
            for res in resources.get_all_resources() {
                register_resources.append(res.clone());
            }
        }

        self.bus
            .upgrade()
            .ok_or(VfioPciError::BusIsDropped)?
            .free_resources(register_resources);

        Ok(())
    }

    fn unregister_regions(&mut self, vm: &Arc<VmFd>) -> Result<()> {
        // This routine handle VfioPciDevice dropped but not unmap memory
        if self.context.upgrade().is_none() {
            for region in self.regions.iter_mut() {
                if region.mappable() {
                    region.unmap(vm, &self.vfio_container)?;
                }
            }

            return Ok(());
        }

        let ctx = self
            .context
            .upgrade()
            .ok_or(VfioPciError::BusIsDropped)?
            .get_device_manager_context();
        let mut tx = ctx.begin_tx();

        for region in self.regions.iter_mut() {
            if region.mappable() {
                region.unmap(vm, &self.vfio_container)?;
            } else {
                region.untrap(&ctx, &mut tx)?;
            }
        }

        ctx.commit_tx(tx);

        Ok(())
    }

    fn program_bar(
        &mut self,
        reg_idx: u32,
        params: BarProgrammingParams,
        vm_fd: &Arc<VmFd>,
    ) -> Result<()> {
        for region in self.regions.iter_mut() {
            if region.reg_index == reg_idx {
                if region.mappable() {
                    region.remap(vm_fd, &params, &self.vfio_container)?;
                } else {
                    // Safe to unwrap because activate() has set self.device to a valid value.
                    let device = self.device.as_ref().unwrap().clone();
                    let ctx: <C as PciSystemContext>::D = self
                        .context
                        .upgrade()
                        .ok_or(VfioPciError::BusIsDropped)?
                        .get_device_manager_context();
                    let mut tx = ctx.begin_tx();

                    if let Err(e) = region.retrap(
                        &ctx,
                        &mut tx,
                        device.upgrade().ok_or(VfioPciError::BusIsDropped)?,
                        &params,
                    ) {
                        ctx.cancel_tx(tx);
                        return Err(e);
                    }

                    ctx.commit_tx(tx);
                }

                return Ok(());
            }
        }

        Err(VfioPciError::InternalError)
    }

    fn vfio_path(&self) -> &str {
        self.vfio_path.as_str()
    }
}

impl<C: PciSystemContext> VfioPciDeviceState<C> {
    fn parse_legacy_irq(&mut self) {
        // If the device supports legacy irq then we would allocate
        // one for it no matter its driver actually needs it or not.
        //
        // However, we do not support ACPI for now which means we
        // can't share irq line between different PCI devices using
        // legacy interrupt, and this causes `not enough irq resources`
        // when passthrough many devices as the above scenario.
        //
        // Up till now GPU is the only kind of devices that the driver
        // depends on legacy irq to perform device initialization.
        // So we limit legacy irq support to devices from such vendors.
        //
        // The NVIDIA device will share the first alloc NVIDIA device's irq.
        // Although still alloc irq constraint, except first will be ignore.
        // Only GPU use legancy irq, NVIDIA Switch use msi.
        // And GPU can not share irq with non-GPU device.
        let vendor_id = self.read_config_word(PCI_CONFIG_VENDOR_OFFSET);
        if vendor_id == VENDOR_NVIDIA {
            if let Some(count) = self
                .vfio_dev
                .get_irq_info(VFIO_PCI_INTX_IRQ_INDEX)
                .map(|info| info.count)
            {
                if count > 0 {
                    self.sys_constraints
                        .push(ResourceConstraint::LegacyIrq { irq: None });
                }
            }
        }
    }

    // Scan PCI MSI/MSIx capabilities.
    // Guest device drivers should not directly access PCI MSI/MSIx related hardware registers,
    // accesses to those hardware registers will be trapped and emulated,
    fn parse_capabilities(&mut self) {
        let mut cap_next = self.read_config_byte(PCI_CONFIG_CAPABILITY_OFFSET);

        while cap_next != 0 {
            let cap_id = self.read_config_byte(cap_next.into());

            match PciCapabilityId::from(cap_id) {
                PciCapabilityId::MessageSignalledInterrupts => {
                    self.parse_msi_capabilities(cap_next)
                }
                PciCapabilityId::MSIX => self.parse_msix_capabilities(cap_next),
                _ => {}
            }

            cap_next = self.read_config_byte((cap_next + 1).into());
        }

        // To reduce GSI consumption, we reuse the same allocated GSI vectors for both MSI and MSIx
        // because they won't be used at the same time.
        let mut msi_count = 0u32;
        if let Some(msix) = self.interrupt.msix.as_ref() {
            msi_count = msix.cap.table_size() as u32;
        }
        if let Some(msi) = self.interrupt.msi.as_ref() {
            if msi.cap.num_vectors() as u32 > msi_count {
                msi_count = msi.cap.num_vectors() as u32;
            }
        }
        if msi_count > 0 {
            self.sys_constraints
                .push(ResourceConstraint::PciMsixIrq { size: msi_count });
        }
    }

    fn parse_msix_capabilities(&mut self, cap: u8) {
        let cap_next = self.read_config_byte((cap + 1).into());
        let msg_ctl = self.read_config_word((cap + 2).into());
        let table = self.read_config_dword((cap + 4).into());
        let pba = self.read_config_dword((cap + 8).into());
        let msix_cap = MsixCap {
            cap_id: PciCapabilityId::MSIX as u8,
            cap_next,
            msg_ctl,
            table,
            pba,
        };

        let msix_config = MsixState::new(msix_cap.table_size());
        let table_bir: u32 = msix_cap.table_bir();
        let table_offset: u64 = u64::from(msix_cap.table_offset());
        let table_size: u64 = u64::from(msix_cap.table_size()) * (MSIX_TABLE_ENTRY_SIZE as u64);
        let pba_bir: u32 = msix_cap.pba_bir();
        let pba_offset: u64 = u64::from(msix_cap.pba_offset());
        let pba_size: u64 = (u64::from(msix_cap.table_size()) + 7) / 8;

        self.interrupt.msix = Some(VfioMsix {
            state: msix_config,
            cap: msix_cap,
            cap_offset: cap.into(),
            table_bir,
            table_offset,
            table_size,
            pba_bir,
            pba_offset,
            pba_size,
        });
    }

    fn parse_msi_capabilities(&mut self, cap: u8) {
        let cap_next = self.read_config_byte((cap + 1).into());
        let msg_ctl = self.read_config_word((cap + 2).into());
        let msi_cap = MsiCap::new(cap_next, msg_ctl);
        let state = MsiState::new(msg_ctl);

        self.interrupt.msi = Some(VfioMsi {
            state,
            cap: msi_cap,
            cap_offset: cap.into(),
        });
    }
}

/// VfioPciDevice represents a VFIO PCI device.
/// This structure implements the BusDevice and PciDevice traits.
///
/// A VfioPciDevice is bound to a VfioDevice and is also a PCI device.
/// The VMM creates a VfioDevice, then assigns it to a VfioPciDevice,
/// which then gets added to the PCI bus.
pub struct VfioPciDevice<C: PciSystemContext> {
    pub(crate) id: u8,
    pub(crate) vm_fd: Arc<VmFd>,
    state: Mutex<VfioPciDeviceState<C>>,
}

///  VfioPciDevice destructor
///
/// Unregister regions before drop VfioPciDevice, otherwise the device
/// fd's mmapped memory will not released.
impl<C: PciSystemContext> Drop for VfioPciDevice<C> {
    fn drop(&mut self) {
        let mut state = self.state();
        let _ = state.unregister_regions(&self.vm_fd);
    }
}

impl<C: PciSystemContext> VfioPciDevice<C> {
    /// Constructs a new Vfio Pci device for the given Vfio device
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        id: u8,
        path: String,
        bus: Weak<PciBus>,
        device: VfioDevice,
        context: Weak<C>,
        vm_fd: Arc<VmFd>,
        vendor_device_id: u32,
        clique_id: Option<u8>,
        vfio_container: Arc<VfioContainer>,
    ) -> Result<Self> {
        let device = Arc::new(device);
        let mut state = VfioPciDeviceState::new(
            path,
            device,
            Weak::clone(&bus),
            context,
            vendor_device_id,
            clique_id,
            vfio_container,
        )?;

        state.probe_regions(bus)?;
        state.parse_capabilities();
        state.parse_legacy_irq();
        state.fixup_msix_region();

        Ok(VfioPciDevice {
            id,
            vm_fd,
            state: Mutex::new(state),
        })
    }

    /// Get resource requirements of the VFIO PCI device.
    pub fn get_resource_requirements(&self, requests: &mut Vec<ResourceConstraint>) {
        let state = self.state();
        if !state.sys_constraints.is_empty() {
            let mut constraints = state.sys_constraints.clone();
            requests.append(&mut constraints);
        }
    }

    /// Because when routing MSI, KVM needs to know the device id of the device corresponding to the MSI.
    /// Therefore, when VfioPciDevice is activated, we will record its device id in its irq_manager
    /// structure for routing MSI.
    #[cfg(target_arch = "aarch64")]
    fn set_device_id(&self, state: &mut VfioPciDeviceState<C>) {
        let device_id = Some(self.id as u32);
        // unwrap is safe because we have initialized irq manager here.
        state
            .interrupt
            .irq_manager
            .as_mut()
            .unwrap()
            .set_device_id(device_id);
    }

    pub fn activate(&self, device: Weak<dyn DeviceIo>, resources: DeviceResources) -> Result<()> {
        let mut state = self.state();

        if resources.len() == 0 {
            return Err(VfioPciError::InvalidResources);
        }

        let mut all_kvm_slot = resources.get_kvm_mem_slots();

        let mut reg_idx = 0;
        while reg_idx < state.regions.len() {
            if state.regions[reg_idx].mappable() {
                state.regions[reg_idx].mmaps[0].slot =
                    all_kvm_slot.pop().ok_or(VfioPciError::KvmSlotNotFound)?;
                if state.regions[reg_idx].mmaps.len() == 2 {
                    state.regions[reg_idx].mmaps[1].slot =
                        all_kvm_slot.pop().ok_or(VfioPciError::KvmSlotNotFound)?;
                }
            }
            state.add_bar_for_region(reg_idx)?;
            reg_idx += 1;
        }

        if let Some(base) = resources.get_legacy_irq() {
            assert!(base < 0xffu32);
            state
                .configuration
                .set_irq(base as u8, PciInterruptPin::IntA);
            state.interrupt.add_legacy_irq_resource(base);
        }

        if let Some((base, size)) = resources.get_pci_msix_irqs() {
            state.interrupt.add_msi_irq_resource(base, size);
        }

        let irq_manager = state
            .context
            .upgrade()
            .ok_or(VfioPciError::BusIsDropped)?
            .get_interrupt_manager();
        state.interrupt.initialize(irq_manager)?;
        #[cfg(target_arch = "aarch64")]
        self.set_device_id(&mut state);
        state.device = Some(device);
        if let Err(e) = state.register_regions(&self.vm_fd) {
            error!(
                "{} register regions failed. error: {}",
                state.vfio_path(),
                e
            );
            let _ = state.unregister_regions(&self.vm_fd).map_err(|e| {
                // If unregistering regions goes wrong, the memory region in Dragonball will be in a mess,
                // so we panic here to avoid more serious problem. 
                panic!("failed to rollback changes of VfioPciDevice::register_regions() because error {:?}", e);
            });
        }

        debug!(
            "{} has been activated, legacy int is {:?}",
            state.vfio_path(),
            state.interrupt.legacy_irq
        );
        //which contains kvm slot and msi irq, but not mmio/pio
        state.assigned_resources = resources;

        Ok(())
    }

    pub fn state(&self) -> MutexGuard<VfioPciDeviceState<C>> {
        // Don't expect poisoned lock
        self.state
            .lock()
            .expect("poisoned lock for VFIO PCI device")
    }

    pub fn device_id(&self) -> u8 {
        self.id
    }

    pub fn bus_id(&self) -> Result<u8> {
        Ok(self
            .state()
            .bus
            .upgrade()
            .ok_or(VfioPciError::BusIsDropped)?
            .bus_id())
    }

    pub fn vendor_id(&self) -> u16 {
        self.state
            .lock()
            .expect("poisoned lock for VFIO PCI device")
            .read_config_word(PCI_CONFIG_VENDOR_OFFSET)
    }

    pub fn clear_device(&self) -> Result<()> {
        let mut state = self.state();
        state.free_register_resources()?;
        let _ = state.unregister_regions(&self.vm_fd);

        Ok(())
    }
}

impl<C: 'static + PciSystemContext> DeviceIo for VfioPciDevice<C> {
    fn read(&self, base: IoAddress, offset: IoAddress, data: &mut [u8]) {
        let base = base.raw_value();
        let offset = offset.raw_value();
        let addr = base + offset;
        let mut state = self.state();

        debug!(
            "{} read bar [0x{:x}, 0x{:x}, 0x{:x}]",
            state.vfio_path(),
            base,
            offset,
            addr
        );

        if let Some(region) = state.find_region(addr, true) {
            let offset = addr - region.start.raw_value();

            if state
                .interrupt
                .msix_table_accessed(region.bar_index, offset)
            {
                state.interrupt.msix_table_read(offset, data);
            } else if state.interrupt.msix_pba_accessed(region.bar_index, offset) {
                state.interrupt.msix_pba_read(offset, data);
            } else {
                state.vfio_dev.region_read(region.bar_index, data, offset);
            }
        }
    }

    fn write(&self, base: IoAddress, offset: IoAddress, data: &[u8]) {
        let base = base.raw_value();
        let offset = offset.raw_value();
        let addr = base + offset;
        let mut state = self.state();

        debug!(
            "{} write bar [0x{:x}, 0x{:x}, 0x{:x}]",
            state.vfio_path(),
            base,
            offset,
            addr
        );

        if let Some(region) = state.find_region(addr, true) {
            let offset = addr - region.start.raw_value();

            // If the MSI-X table is written to, we need to update our cache.
            if state
                .interrupt
                .msix_table_accessed(region.bar_index, offset)
            {
                state.interrupt.msix_table_write(offset, data);
            } else if state.interrupt.msix_pba_accessed(region.bar_index, offset) {
                state.interrupt.msix_pba_write(offset, data);
            } else {
                state.vfio_dev.region_write(region.bar_index, data, offset);
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn pio_read(&self, base: PioAddress, offset: PioAddress, data: &mut [u8]) {
        let base = base.raw_value() as u64;
        let offset = offset.raw_value() as u64;
        let addr = base + offset;
        let state = self.state();

        if let Some(region) = state.find_region(addr, false) {
            let offset = addr - region.start.raw_value();
            state.vfio_dev.region_read(region.bar_index, data, offset);
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn pio_write(&self, base: PioAddress, offset: PioAddress, data: &[u8]) {
        let base = base.raw_value() as u64;
        let offset = offset.raw_value() as u64;
        let addr = base + offset;
        let state = self.state();

        if let Some(region) = state.find_region(addr, false) {
            let offset = addr - region.start.raw_value();
            state.vfio_dev.region_write(region.bar_index, data, offset);
        }
    }

    fn get_assigned_resources(&self) -> DeviceResources {
        self.state().assigned_resources.clone()
    }

    fn get_trapped_io_resources(&self) -> DeviceResources {
        self.state().trapped_resources.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl<C: 'static + PciSystemContext> PciDevice for VfioPciDevice<C> {
    fn id(&self) -> u8 {
        self.id
    }

    fn write_config(&self, offset: u32, data: &[u8]) {
        let reg_idx = offset >> 2;
        let mut state = self.state();

        debug!(
            "{} write config [{}, {}]",
            state.vfio_path(),
            reg_idx,
            offset
        );
        // TODO: detect change in COMMAND register #8726
        if (PCI_CONFIG_BAR0_INDEX..PCI_CONFIG_BAR0_INDEX + BAR_NUMS).contains(&reg_idx)
            || reg_idx == PCI_ROM_EXP_BAR_INDEX
        {
            // When the guest wants to write to a BAR, we trap it into our local configuration space.
            // We're not reprogramming VFIO device. We keep our local cache updated with the BARs.
            // We'll read it back from there when the guest is asking for BARs.
            state.configuration.write_config(offset as usize, data);
            if let Some(params) = state.configuration.get_bar_programming_params() {
                if let Err(e) = state.program_bar(reg_idx, params, &self.vm_fd) {
                    debug!("failed to program VFIO PCI BAR, {}", e);
                }
            }
            // For device like nvidia vGPU the config space must also be updated.
            if state.is_mdev() {
                state
                    .vfio_dev
                    .region_write(VFIO_PCI_CONFIG_REGION_INDEX, data, offset as u64);
            }
        } else if !state.interrupt.cap_write(offset, data) {
            // TODO: check whether following comment is correct. #8727
            // Make sure to write to the device's PCI config space after MSI/MSI-X
            // interrupts have been enabled/disabled. In case of MSI, when the
            // interrupts are enabled through VFIO (using VFIO_DEVICE_SET_IRQS),
            // the MSI Enable bit in the MSI capability structure found in the PCI
            // config space is disabled by default. That's why when the guest is
            // enabling this bit, we first need to enable the MSI interrupts with
            // VFIO through VFIO_DEVICE_SET_IRQS ioctl, and only after we can write
            // to the device region to update the MSI Enable bit.
            state
                .vfio_dev
                .region_write(VFIO_PCI_CONFIG_REGION_INDEX, data, offset as u64);
        }
    }

    fn read_config(&self, offset: u32, data: &mut [u8]) {
        assert!((offset & 0x3) as usize + data.len() <= 4);
        let reg_idx = offset >> 2;
        let mut state = self.state();

        debug!(
            "{} read config [{}, {}]",
            state.vfio_path(),
            reg_idx,
            offset
        );
        // When reading the BARs, we trap it and return what comes from our local configuration
        // space. We want the guest to use that and not the VFIO device BARs as it does not map
        // with the guest address space.
        if (PCI_CONFIG_BAR0_INDEX..PCI_CONFIG_BAR0_INDEX + BAR_NUMS).contains(&reg_idx)
            || reg_idx == PCI_ROM_EXP_BAR_INDEX
        {
            state.configuration.read_config(offset as usize, data);
        } else if !state.interrupt.cap_read(offset, data) {
            // The config register read comes from the VFIO device itself.
            let mut value = state.read_config_dword(reg_idx * 4);

            // If the configuration has valid value, we use it instead of the device.
            if reg_idx == PCI_CONFIG_VENDOR_OFFSET {
                let mut d: [u8; 4] = [0, 0, 0, 0];
                state.configuration.read_config(reg_idx as usize, &mut d);
                let v = u32::from_le_bytes(d);
                if v != 0 {
                    value = v;
                }
            }

            // Since we don't support INTx (only MSI and MSI-X), we should not expose an invalid
            // Interrupt Pin to the guest. By using a specific mask in case the register being
            // read correspond to the interrupt register, this code makes sure to always expose
            // an Interrupt Pin value of 0, which stands for no interrupt pin support.
            //
            if reg_idx == PCI_INTX_REG_INDEX {
                value &= 0xffff_0000;
                value |= state.interrupt.get_irq_pin();
            // Since we don't support passing multi-functions devices, we should mask the
            // multi-function bit, bit 7 of the Header Type byte on the register 3.
            } else if reg_idx == PCI_HEADER_TYPE_REG_INDEX {
                value &= 0xff7f_ffff;
            };

            // PCI configuration space is little endian.
            value >>= (offset & 0x3) * 8;
            for item in data {
                *item = value as u8;
                value >>= 8;
            }
        }
    }
}

#[cfg(all(test, feature = "test-mock"))]
mod tests {
    use std::path::Path;

    use kvm_ioctls::Kvm;
    use vfio_ioctls::VfioContainer;

    use super::*;

    fn get_interrupt() -> Interrupt {
        let kvm = Kvm::new().unwrap();
        let vm_fd = Arc::new(kvm.create_vm().unwrap());
        let mut vfio_device = kvm_bindings::kvm_create_device {
            type_: kvm_bindings::kvm_device_type_KVM_DEV_TYPE_VFIO,
            fd: 0,
            flags: 0,
        };
        let kvm_device = Arc::new(vm_fd.create_device(&mut vfio_device).unwrap());
        let sysfspath = String::from("/sys/bus/pci/devices/0000:00:00.0/");
        let vfio_container = Arc::new(VfioContainer::new(kvm_device).unwrap());
        let vfio_dev =
            Arc::new(VfioDevice::new(Path::new(&sysfspath), vfio_container.clone()).unwrap());
        let irq_manager = Arc::new(KvmIrqManager::new(vm_fd.clone()));
        let mut irpt = Interrupt::new(vfio_dev);
        irpt.initialize(irq_manager).unwrap();

        irpt
    }

    #[test]
    fn test_interrupt() {
        let cap = MsiCap::new(0xa5, 0);
        let mut irpt = get_interrupt();

        assert_eq!(irpt.get_irq_pin(), 0);
        irpt.add_legacy_irq_resource(32);
        assert_eq!(irpt.get_irq_pin(), (PciInterruptPin::IntA as u32) << 8 | 32);

        // test msi.
        irpt.msi = Some(VfioMsi {
            state: MsiState::new(0),
            cap,
            cap_offset: 100,
        });

        let mut data_buf = [0; 1];
        let data = b"t";
        assert!(!irpt.cap_read(0, &mut data_buf));
        assert!(!irpt.cap_read(100, &mut data_buf));
        assert!(!irpt.cap_write(0, data));
        assert!(!irpt.cap_write(100, data));

        // test msix.
        let msix = VfioMsix {
            state: MsixState::new(1),
            cap: MsixCap::new(0, 100, 0, 0, 0),
            cap_offset: 100,
            table_bir: 0,
            table_offset: 0,
            table_size: 100,
            pba_bir: 0,
            pba_offset: 0,
            pba_size: 0,
        };
        irpt.msix = Some(msix);
        assert!(!irpt.cap_read(0, &mut data_buf));
        assert!(irpt.cap_read(100, &mut data_buf));
        assert!(!irpt.cap_write(0, data));
        assert!(irpt.cap_write(100, data));
    }
}
