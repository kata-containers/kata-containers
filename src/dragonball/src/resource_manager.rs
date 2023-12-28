// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::sync::Mutex;

use dbs_allocator::{Constraint, IntervalTree, Range};
use dbs_boot::layout::{
    GUEST_MEM_END, GUEST_MEM_START, GUEST_PHYS_END, IRQ_BASE as LEGACY_IRQ_BASE,
    IRQ_MAX as LEGACY_IRQ_MAX, MMIO_LOW_END, MMIO_LOW_START,
};
use dbs_device::resources::{DeviceResources, MsiIrqType, Resource, ResourceConstraint};

// We reserve the LEGACY_IRQ_BASE(5) for shared IRQ.
const SHARED_IRQ: u32 = LEGACY_IRQ_BASE;
// Since ioapic2 have 24 pins for legacy devices, so irq number 0-23 are used. We will set MSI_IRQ_BASE at 24.
#[cfg(target_arch = "x86_64")]
const MSI_IRQ_BASE: u32 = 24;
#[cfg(target_arch = "aarch64")]
/// We define MSI_IRQ_BASE as LEGACY_IRQ_MAX for aarch64 in order not to conflict with legacy irq numbers.
const MSI_IRQ_BASE: u32 = LEGACY_IRQ_MAX + 1;

// kvm max irq is defined in arch/x86/include/asm/kvm_host.h
const MSI_IRQ_MAX: u32 = 1023;
// x86's kvm user mem slots is defined in arch/x86/include/asm/kvm_host.h
#[cfg(target_arch = "x86_64")]
const KVM_USER_MEM_SLOTS: u32 = 509;
// aarch64's kvm user mem slots is defined in arch/arm64/include/asm/kvm_host.h
#[cfg(target_arch = "aarch64")]
const KVM_USER_MEM_SLOTS: u32 = 512;
const PIO_MIN: u16 = 0x0;
const PIO_MAX: u16 = 0xFFFF;
// Reserve the 64MB MMIO address range just below 4G, x86 systems have special
// devices, such as LAPIC, IOAPIC, HPET etc, in this range. And we don't explicitly
// allocate MMIO address for those devices.
const MMIO_SPACE_RESERVED: u64 = 0x400_0000;

/// Errors associated with resource management operations
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ResourceError {
    /// Unknown/unsupported resource type.
    #[error("unsupported resource type")]
    UnknownResourceType,

    /// Invalid resource range.
    #[error("invalid resource range for resource type : {0}")]
    InvalidResourceRange(String),

    /// No resource available.
    #[error("no resource available")]
    NoAvailResource,
}

#[derive(Default)]
struct ResourceManagerBuilder {
    // IntervalTree for allocating legacy irq number.
    legacy_irq_pool: IntervalTree<()>,
    // IntervalTree for allocating message signal interrupt (MSI) irq number.
    msi_irq_pool: IntervalTree<()>,
    // IntervalTree for allocating port-mapped io (PIO) address.
    pio_pool: IntervalTree<()>,
    // IntervalTree for allocating memory-mapped io (MMIO) address.
    mmio_pool: IntervalTree<()>,
    // IntervalTree for allocating guest memory.
    mem_pool: IntervalTree<()>,
    // IntervalTree for allocating kvm memory slot.
    kvm_mem_slot_pool: IntervalTree<()>,
}

impl ResourceManagerBuilder {
    /// init legacy_irq_pool with arch specific constants.
    fn init_legacy_irq_pool(mut self) -> Self {
        // The LEGACY_IRQ_BASE irq is reserved for shared IRQ and won't be allocated / reallocated,
        // so we don't insert it into the legacy_irq interval tree.
        self.legacy_irq_pool
            .insert(Range::new(LEGACY_IRQ_BASE + 1, LEGACY_IRQ_MAX), None);
        self
    }

    /// init msi_irq_pool with arch specific constants.
    fn init_msi_irq_pool(mut self) -> Self {
        self.msi_irq_pool
            .insert(Range::new(MSI_IRQ_BASE, MSI_IRQ_MAX), None);
        self
    }

    /// init pio_pool with arch specific constants.
    fn init_pio_pool(mut self) -> Self {
        self.pio_pool.insert(Range::new(PIO_MIN, PIO_MAX), None);
        self
    }

    /// Create mmio_pool with arch specific constants.
    /// allow(clippy) is because `GUEST_MEM_START > MMIO_LOW_END`, we may modify GUEST_MEM_START or
    /// MMIO_LOW_END in the future.
    #[allow(clippy::absurd_extreme_comparisons)]
    fn init_mmio_pool_helper(mmio: &mut IntervalTree<()>) {
        mmio.insert(Range::new(MMIO_LOW_START, MMIO_LOW_END), None);
        if !(*GUEST_MEM_END < MMIO_LOW_START
            || GUEST_MEM_START > MMIO_LOW_END
            || MMIO_LOW_START == MMIO_LOW_END)
        {
            #[cfg(target_arch = "x86_64")]
            {
                let constraint = Constraint::new(MMIO_SPACE_RESERVED)
                    .min(MMIO_LOW_END - MMIO_SPACE_RESERVED)
                    .max(0xffff_ffffu64);
                let key = mmio.allocate(&constraint);
                if let Some(k) = key.as_ref() {
                    mmio.update(k, ());
                } else {
                    panic!("failed to reserve MMIO address range for x86 system devices");
                }
            }
        }

        if *GUEST_MEM_END < *GUEST_PHYS_END {
            mmio.insert(Range::new(*GUEST_MEM_END + 1, *GUEST_PHYS_END), None);
        }
    }

    /// init mmio_pool with helper function
    fn init_mmio_pool(mut self) -> Self {
        Self::init_mmio_pool_helper(&mut self.mmio_pool);
        self
    }

    /// Create mem_pool with arch specific constants.
    /// deny(clippy) is because `GUEST_MEM_START > MMIO_LOW_END`, we may modify GUEST_MEM_START or
    /// MMIO_LOW_END in the future.
    #[allow(clippy::absurd_extreme_comparisons)]
    pub(crate) fn init_mem_pool_helper(mem: &mut IntervalTree<()>) {
        if *GUEST_MEM_END < MMIO_LOW_START
            || GUEST_MEM_START > MMIO_LOW_END
            || MMIO_LOW_START == MMIO_LOW_END
        {
            mem.insert(Range::new(GUEST_MEM_START, *GUEST_MEM_END), None);
        } else {
            if MMIO_LOW_START > GUEST_MEM_START {
                mem.insert(Range::new(GUEST_MEM_START, MMIO_LOW_START - 1), None);
            }
            if MMIO_LOW_END < *GUEST_MEM_END {
                mem.insert(Range::new(MMIO_LOW_END + 1, *GUEST_MEM_END), None);
            }
        }
    }

    /// init mem_pool with helper function
    fn init_mem_pool(mut self) -> Self {
        Self::init_mem_pool_helper(&mut self.mem_pool);
        self
    }

    /// init kvm_mem_slot_pool with arch specific constants.
    fn init_kvm_mem_slot_pool(mut self, max_kvm_mem_slot: Option<usize>) -> Self {
        let max_slots = max_kvm_mem_slot.unwrap_or(KVM_USER_MEM_SLOTS as usize);
        self.kvm_mem_slot_pool
            .insert(Range::new(0, max_slots as u64), None);
        self
    }

    fn build(self) -> ResourceManager {
        ResourceManager {
            legacy_irq_pool: Mutex::new(self.legacy_irq_pool),
            msi_irq_pool: Mutex::new(self.msi_irq_pool),
            pio_pool: Mutex::new(self.pio_pool),
            mmio_pool: Mutex::new(self.mmio_pool),
            mem_pool: Mutex::new(self.mem_pool),
            kvm_mem_slot_pool: Mutex::new(self.kvm_mem_slot_pool),
        }
    }
}

/// Resource manager manages all resources for a virtual machine instance.
pub struct ResourceManager {
    legacy_irq_pool: Mutex<IntervalTree<()>>,
    msi_irq_pool: Mutex<IntervalTree<()>>,
    pio_pool: Mutex<IntervalTree<()>>,
    mmio_pool: Mutex<IntervalTree<()>>,
    mem_pool: Mutex<IntervalTree<()>>,
    kvm_mem_slot_pool: Mutex<IntervalTree<()>>,
}

impl Default for ResourceManager {
    fn default() -> Self {
        ResourceManagerBuilder::default().build()
    }
}

impl ResourceManager {
    /// Create a resource manager instance.
    pub fn new(max_kvm_mem_slot: Option<usize>) -> Self {
        let res_manager_builder = ResourceManagerBuilder::default();
        res_manager_builder
            .init_legacy_irq_pool()
            .init_msi_irq_pool()
            .init_pio_pool()
            .init_mmio_pool()
            .init_mem_pool()
            .init_kvm_mem_slot_pool(max_kvm_mem_slot)
            .build()
    }

    /// Init mem_pool with arch specific constants.
    pub fn init_mem_pool(&self) {
        let mut mem = self.mem_pool.lock().unwrap();
        ResourceManagerBuilder::init_mem_pool_helper(&mut mem);
    }

    /// Check if mem_pool is empty.
    pub fn is_mem_pool_empty(&self) -> bool {
        self.mem_pool.lock().unwrap().is_empty()
    }

    /// Allocate one legacy irq number.
    ///
    /// Allocate the specified irq number if `fixed` contains an irq number.
    pub fn allocate_legacy_irq(&self, shared: bool, fixed: Option<u32>) -> Option<u32> {
        // if shared_irq is used, just return the shared irq num.
        if shared {
            return Some(SHARED_IRQ);
        }

        let mut constraint = Constraint::new(1u32);
        if let Some(v) = fixed {
            if v == SHARED_IRQ {
                return None;
            }

            constraint.min = v as u64;
            constraint.max = v as u64;
        }
        // Safe to unwrap() because we don't expect poisoned lock here.
        let mut legacy_irq_pool = self.legacy_irq_pool.lock().unwrap();
        let key = legacy_irq_pool.allocate(&constraint);
        if let Some(k) = key.as_ref() {
            legacy_irq_pool.update(k, ());
        }
        key.map(|v| v.min as u32)
    }

    /// Free a legacy irq number.
    ///
    /// Panic if the irq number is invalid.
    pub fn free_legacy_irq(&self, irq: u32) -> Result<(), ResourceError> {
        // if the irq number is shared_irq, we don't need to do anything.
        if irq == SHARED_IRQ {
            return Ok(());
        }

        if !(LEGACY_IRQ_BASE..=LEGACY_IRQ_MAX).contains(&irq) {
            return Err(ResourceError::InvalidResourceRange(
                "Legacy IRQ".to_string(),
            ));
        }
        let key = Range::new(irq, irq);
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.legacy_irq_pool.lock().unwrap().free(&key);
        Ok(())
    }

    /// Allocate a group of MSI irq numbers.
    ///
    /// The allocated MSI irq numbers may or may not be naturally aligned.
    pub fn allocate_msi_irq(&self, count: u32) -> Option<u32> {
        let constraint = Constraint::new(count);
        // Safe to unwrap() because we don't expect poisoned lock here.
        let mut msi_irq_pool = self.msi_irq_pool.lock().unwrap();
        let key = msi_irq_pool.allocate(&constraint);
        if let Some(k) = key.as_ref() {
            msi_irq_pool.update(k, ());
        }
        key.map(|v| v.min as u32)
    }

    /// Allocate a group of MSI irq numbers, naturally aligned to `count`.
    ///
    /// This may be used to support PCI MSI, which requires the allocated irq number is naturally
    /// aligned.
    pub fn allocate_msi_irq_aligned(&self, count: u32) -> Option<u32> {
        let constraint = Constraint::new(count).align(count);
        // Safe to unwrap() because we don't expect poisoned lock here.
        let mut msi_irq_pool = self.msi_irq_pool.lock().unwrap();
        let key = msi_irq_pool.allocate(&constraint);
        if let Some(k) = key.as_ref() {
            msi_irq_pool.update(k, ());
        }
        key.map(|v| v.min as u32)
    }

    /// Free a group of MSI irq numbers.
    ///
    /// Panic if `irq` or `count` is invalid.
    pub fn free_msi_irq(&self, irq: u32, count: u32) -> Result<(), ResourceError> {
        if irq < MSI_IRQ_BASE
            || count == 0
            || irq.checked_add(count).is_none()
            || irq + count - 1 > MSI_IRQ_MAX
        {
            return Err(ResourceError::InvalidResourceRange("MSI IRQ".to_string()));
        }
        let key = Range::new(irq, irq + count - 1);
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.msi_irq_pool.lock().unwrap().free(&key);
        Ok(())
    }

    /// Allocate a group of PIO address and returns the allocated PIO base address.
    pub fn allocate_pio_address_simple(&self, size: u16) -> Option<u16> {
        let constraint = Constraint::new(size);
        self.allocate_pio_address(&constraint)
    }

    /// Allocate a group of PIO address and returns the allocated PIO base address.
    pub fn allocate_pio_address(&self, constraint: &Constraint) -> Option<u16> {
        // Safe to unwrap() because we don't expect poisoned lock here.
        let mut pio_pool = self.pio_pool.lock().unwrap();
        let key = pio_pool.allocate(constraint);
        if let Some(k) = key.as_ref() {
            pio_pool.update(k, ());
        }
        key.map(|v| v.min as u16)
    }

    /// Free PIO address range `[base, base + size - 1]`.
    ///
    /// Panic if `base` or `size` is invalid.
    pub fn free_pio_address(&self, base: u16, size: u16) -> Result<(), ResourceError> {
        if base.checked_add(size).is_none() {
            return Err(ResourceError::InvalidResourceRange(
                "PIO Address".to_string(),
            ));
        }
        let key = Range::new(base, base + size - 1);
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.pio_pool.lock().unwrap().free(&key);
        Ok(())
    }

    /// Allocate a MMIO address range alinged to `align` and returns the allocated base address.
    pub fn allocate_mmio_address_aligned(&self, size: u64, align: u64) -> Option<u64> {
        let constraint = Constraint::new(size).align(align);
        self.allocate_mmio_address(&constraint)
    }

    /// Allocate a MMIO address range and returns the allocated base address.
    pub fn allocate_mmio_address(&self, constraint: &Constraint) -> Option<u64> {
        // Safe to unwrap() because we don't expect poisoned lock here.
        let mut mmio_pool = self.mmio_pool.lock().unwrap();
        let key = mmio_pool.allocate(constraint);
        key.map(|v| v.min)
    }

    /// Free MMIO address range `[base, base + size - 1]`
    pub fn free_mmio_address(&self, base: u64, size: u64) -> Result<(), ResourceError> {
        if base.checked_add(size).is_none() {
            return Err(ResourceError::InvalidResourceRange(
                "MMIO Address".to_string(),
            ));
        }
        let key = Range::new(base, base + size - 1);
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.mmio_pool.lock().unwrap().free(&key);
        Ok(())
    }

    /// Allocate guest memory address range and returns the allocated base memory address.
    pub fn allocate_mem_address(&self, constraint: &Constraint) -> Option<u64> {
        // Safe to unwrap() because we don't expect poisoned lock here.
        let mut mem_pool = self.mem_pool.lock().unwrap();
        let key = mem_pool.allocate(constraint);

        key.map(|v| v.min)
    }

    /// Free the guest memory address range `[base, base + size - 1]`.
    ///
    /// Panic if the guest memory address range is invalid.
    /// allow(clippy) is because `base < GUEST_MEM_START`, we may modify GUEST_MEM_START in the future.
    #[allow(clippy::absurd_extreme_comparisons)]
    pub fn free_mem_address(&self, base: u64, size: u64) -> Result<(), ResourceError> {
        if base.checked_add(size).is_none()
            || base < GUEST_MEM_START
            || base + size > *GUEST_MEM_END
        {
            return Err(ResourceError::InvalidResourceRange(
                "MEM Address".to_string(),
            ));
        }
        let key = Range::new(base, base + size - 1);
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.mem_pool.lock().unwrap().free(&key);
        Ok(())
    }

    /// Allocate a kvm memory slot number.
    ///
    /// Allocate the specified slot if `fixed` contains a slot number.
    pub fn allocate_kvm_mem_slot(&self, size: u32, fixed: Option<u32>) -> Option<u32> {
        let mut constraint = Constraint::new(size);
        if let Some(v) = fixed {
            constraint.min = v as u64;
            constraint.max = v as u64;
        }
        // Safe to unwrap() because we don't expect poisoned lock here.
        let mut kvm_mem_slot_pool = self.kvm_mem_slot_pool.lock().unwrap();
        let key = kvm_mem_slot_pool.allocate(&constraint);
        if let Some(k) = key.as_ref() {
            kvm_mem_slot_pool.update(k, ());
        }
        key.map(|v| v.min as u32)
    }

    /// Free a kvm memory slot number.
    pub fn free_kvm_mem_slot(&self, slot: u32) -> Result<(), ResourceError> {
        let key = Range::new(slot, slot);
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.kvm_mem_slot_pool.lock().unwrap().free(&key);
        Ok(())
    }

    /// Allocate requested resources for a device.
    #[allow(clippy::question_mark)]
    pub fn allocate_device_resources(
        &self,
        requests: &[ResourceConstraint],
        shared_irq: bool,
    ) -> std::result::Result<DeviceResources, ResourceError> {
        let mut resources = DeviceResources::new();
        for resource in requests.iter() {
            let res = match resource {
                ResourceConstraint::PioAddress { range, align, size } => {
                    let mut constraint = Constraint::new(*size).align(*align);
                    if let Some(r) = range {
                        constraint.min = r.0 as u64;
                        constraint.max = r.1 as u64;
                    }
                    match self.allocate_pio_address(&constraint) {
                        Some(base) => Resource::PioAddressRange { base, size: *size },
                        None => {
                            if let Err(e) = self.free_device_resources(&resources) {
                                return Err(e);
                            } else {
                                return Err(ResourceError::NoAvailResource);
                            }
                        }
                    }
                }
                ResourceConstraint::MmioAddress { range, align, size } => {
                    let mut constraint = Constraint::new(*size).align(*align);
                    if let Some(r) = range {
                        constraint.min = r.0;
                        constraint.max = r.1;
                    }
                    match self.allocate_mmio_address(&constraint) {
                        Some(base) => Resource::MmioAddressRange { base, size: *size },
                        None => {
                            if let Err(e) = self.free_device_resources(&resources) {
                                return Err(e);
                            } else {
                                return Err(ResourceError::NoAvailResource);
                            }
                        }
                    }
                }
                ResourceConstraint::MemAddress { range, align, size } => {
                    let mut constraint = Constraint::new(*size).align(*align);
                    if let Some(r) = range {
                        constraint.min = r.0;
                        constraint.max = r.1;
                    }
                    match self.allocate_mem_address(&constraint) {
                        Some(base) => Resource::MemAddressRange { base, size: *size },
                        None => {
                            if let Err(e) = self.free_device_resources(&resources) {
                                return Err(e);
                            } else {
                                return Err(ResourceError::NoAvailResource);
                            }
                        }
                    }
                }
                ResourceConstraint::LegacyIrq { irq } => {
                    match self.allocate_legacy_irq(shared_irq, *irq) {
                        Some(v) => Resource::LegacyIrq(v),
                        None => {
                            if let Err(e) = self.free_device_resources(&resources) {
                                return Err(e);
                            } else {
                                return Err(ResourceError::NoAvailResource);
                            }
                        }
                    }
                }
                ResourceConstraint::PciMsiIrq { size } => {
                    match self.allocate_msi_irq_aligned(*size) {
                        Some(base) => Resource::MsiIrq {
                            ty: MsiIrqType::PciMsi,
                            base,
                            size: *size,
                        },
                        None => {
                            if let Err(e) = self.free_device_resources(&resources) {
                                return Err(e);
                            } else {
                                return Err(ResourceError::NoAvailResource);
                            }
                        }
                    }
                }
                ResourceConstraint::PciMsixIrq { size } => match self.allocate_msi_irq(*size) {
                    Some(base) => Resource::MsiIrq {
                        ty: MsiIrqType::PciMsix,
                        base,
                        size: *size,
                    },
                    None => {
                        if let Err(e) = self.free_device_resources(&resources) {
                            return Err(e);
                        } else {
                            return Err(ResourceError::NoAvailResource);
                        }
                    }
                },
                ResourceConstraint::GenericIrq { size } => match self.allocate_msi_irq(*size) {
                    Some(base) => Resource::MsiIrq {
                        ty: MsiIrqType::GenericMsi,
                        base,
                        size: *size,
                    },
                    None => {
                        if let Err(e) = self.free_device_resources(&resources) {
                            return Err(e);
                        } else {
                            return Err(ResourceError::NoAvailResource);
                        }
                    }
                },
                ResourceConstraint::KvmMemSlot { slot, size } => {
                    match self.allocate_kvm_mem_slot(*size, *slot) {
                        Some(v) => Resource::KvmMemSlot(v),
                        None => {
                            if let Err(e) = self.free_device_resources(&resources) {
                                return Err(e);
                            } else {
                                return Err(ResourceError::NoAvailResource);
                            }
                        }
                    }
                }
            };
            resources.append(res);
        }

        Ok(resources)
    }

    /// Free resources allocated for a device.
    pub fn free_device_resources(&self, resources: &DeviceResources) -> Result<(), ResourceError> {
        for res in resources.iter() {
            let result = match res {
                Resource::PioAddressRange { base, size } => self.free_pio_address(*base, *size),
                Resource::MmioAddressRange { base, size } => self.free_mmio_address(*base, *size),
                Resource::MemAddressRange { base, size } => self.free_mem_address(*base, *size),
                Resource::LegacyIrq(base) => self.free_legacy_irq(*base),
                Resource::MsiIrq { ty: _, base, size } => self.free_msi_irq(*base, *size),
                Resource::KvmMemSlot(slot) => self.free_kvm_mem_slot(*slot),
                Resource::MacAddress(_) => Ok(()),
            };
            result?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_legacy_irq() {
        let mgr = ResourceManager::new(None);

        // Allocate/free shared IRQs multiple times.
        assert_eq!(mgr.allocate_legacy_irq(true, None).unwrap(), SHARED_IRQ);
        assert_eq!(mgr.allocate_legacy_irq(true, None).unwrap(), SHARED_IRQ);
        mgr.free_legacy_irq(SHARED_IRQ).unwrap();
        mgr.free_legacy_irq(SHARED_IRQ).unwrap();
        mgr.free_legacy_irq(SHARED_IRQ).unwrap();

        // Allocate specified IRQs.
        assert_eq!(
            mgr.allocate_legacy_irq(false, Some(LEGACY_IRQ_BASE + 10))
                .unwrap(),
            LEGACY_IRQ_BASE + 10
        );
        mgr.free_legacy_irq(LEGACY_IRQ_BASE + 10).unwrap();
        assert_eq!(
            mgr.allocate_legacy_irq(false, Some(LEGACY_IRQ_BASE + 10))
                .unwrap(),
            LEGACY_IRQ_BASE + 10
        );
        assert!(mgr
            .allocate_legacy_irq(false, Some(LEGACY_IRQ_BASE + 10))
            .is_none());

        assert!(mgr.allocate_legacy_irq(false, None).is_some());

        assert!(mgr
            .allocate_legacy_irq(false, Some(LEGACY_IRQ_BASE - 1))
            .is_none());
        assert!(mgr
            .allocate_legacy_irq(false, Some(LEGACY_IRQ_MAX + 1))
            .is_none());
        assert!(mgr.allocate_legacy_irq(false, Some(SHARED_IRQ)).is_none());
    }

    #[test]
    fn test_invalid_free_legacy_irq() {
        let mgr = ResourceManager::new(None);
        assert_eq!(
            mgr.free_legacy_irq(LEGACY_IRQ_MAX + 1),
            Err(ResourceError::InvalidResourceRange(
                "Legacy IRQ".to_string(),
            ))
        );
    }

    #[test]
    fn test_allocate_msi_irq() {
        let mgr = ResourceManager::new(None);

        let msi = mgr.allocate_msi_irq(3).unwrap();
        mgr.free_msi_irq(msi, 3).unwrap();
        let msi = mgr.allocate_msi_irq(3).unwrap();
        mgr.free_msi_irq(msi, 3).unwrap();

        let irq = mgr.allocate_msi_irq_aligned(8).unwrap();
        assert_eq!(irq & 0x7, 0);
        mgr.free_msi_irq(msi, 8).unwrap();
        let irq = mgr.allocate_msi_irq_aligned(8).unwrap();
        assert_eq!(irq & 0x7, 0);

        let irq = mgr.allocate_msi_irq_aligned(512).unwrap();
        assert_eq!(irq, 512);
        mgr.free_msi_irq(irq, 512).unwrap();
        let irq = mgr.allocate_msi_irq_aligned(512).unwrap();
        assert_eq!(irq, 512);

        assert!(mgr.allocate_msi_irq(4099).is_none());
    }

    #[test]
    fn test_invalid_free_msi_irq() {
        let mgr = ResourceManager::new(None);
        assert_eq!(
            mgr.free_msi_irq(MSI_IRQ_MAX, 3),
            Err(ResourceError::InvalidResourceRange("MSI IRQ".to_string()))
        );
    }

    #[test]
    fn test_allocate_pio_addr() {
        let mgr = ResourceManager::new(None);
        assert!(mgr.allocate_pio_address_simple(10).is_some());
        let mut requests = vec![
            ResourceConstraint::PioAddress {
                range: None,
                align: 0x1000,
                size: 0x2000,
            },
            ResourceConstraint::PioAddress {
                range: Some((0x8000, 0x9000)),
                align: 0x1000,
                size: 0x1000,
            },
            ResourceConstraint::PioAddress {
                range: Some((0x9000, 0xa000)),
                align: 0x1000,
                size: 0x1000,
            },
            ResourceConstraint::PioAddress {
                range: Some((0xb000, 0xc000)),
                align: 0x1000,
                size: 0x1000,
            },
        ];
        let resources = mgr.allocate_device_resources(&requests, false).unwrap();
        mgr.free_device_resources(&resources).unwrap();
        let resources = mgr.allocate_device_resources(&requests, false).unwrap();
        mgr.free_device_resources(&resources).unwrap();
        requests.push(ResourceConstraint::PioAddress {
            range: Some((0xc000, 0xc000)),
            align: 0x1000,
            size: 0x1000,
        });
        assert!(mgr.allocate_device_resources(&requests, false).is_err());
        let resources = mgr
            .allocate_device_resources(&requests[0..requests.len() - 1], false)
            .unwrap();
        mgr.free_device_resources(&resources).unwrap();
    }

    #[test]
    fn test_invalid_free_pio_addr() {
        let mgr = ResourceManager::new(None);
        assert_eq!(
            mgr.free_pio_address(u16::MAX, 3),
            Err(ResourceError::InvalidResourceRange(
                "PIO Address".to_string(),
            ))
        );
    }

    #[test]
    fn test_allocate_kvm_mem_slot() {
        let mgr = ResourceManager::new(None);
        assert_eq!(mgr.allocate_kvm_mem_slot(1, None).unwrap(), 0);
        assert_eq!(mgr.allocate_kvm_mem_slot(1, Some(200)).unwrap(), 200);
        mgr.free_kvm_mem_slot(200).unwrap();
        assert_eq!(mgr.allocate_kvm_mem_slot(1, Some(200)).unwrap(), 200);
        assert_eq!(
            mgr.allocate_kvm_mem_slot(1, Some(KVM_USER_MEM_SLOTS))
                .unwrap(),
            KVM_USER_MEM_SLOTS
        );
        assert!(mgr
            .allocate_kvm_mem_slot(1, Some(KVM_USER_MEM_SLOTS + 1))
            .is_none());
    }

    #[test]
    fn test_allocate_mmio_address() {
        let mgr = ResourceManager::new(None);

        #[cfg(target_arch = "x86_64")]
        {
            // Can't allocate from reserved region
            let constraint = Constraint::new(0x100_0000u64)
                .min(0x1_0000_0000u64 - 0x200_0000u64)
                .max(0xffff_ffffu64);
            assert!(mgr.allocate_mmio_address(&constraint).is_none());
        }
        let constraint = Constraint::new(0x100_0000u64).min(0x1_0000_0000u64 - 0x200_0000u64);
        assert!(mgr.allocate_mmio_address(&constraint).is_some());

        #[cfg(target_arch = "x86_64")]
        {
            // Can't allocate from reserved region
            let constraint = Constraint::new(0x100_0000u64)
                .min(0x1_0000_0000u64 - 0x200_0000u64)
                .max(0xffff_ffffu64);
            assert!(mgr.allocate_mem_address(&constraint).is_none());
        }
        #[cfg(target_arch = "aarch64")]
        {
            let constraint = Constraint::new(0x200_0000u64)
                .min(0x1_0000_0000u64 - 0x200_0000u64)
                .max(0xffff_fffeu64);
            assert!(mgr.allocate_mem_address(&constraint).is_none());
        }
        let constraint = Constraint::new(0x100_0000u64).min(0x1_0000_0000u64 - 0x200_0000u64);
        assert!(mgr.allocate_mem_address(&constraint).is_some());
    }

    #[test]
    #[should_panic]
    fn test_allocate_duplicate_memory() {
        let mgr = ResourceManager::new(None);

        let constraint_1 = Constraint::new(0x100_0000u64)
            .min(0x1_0000_0000u64)
            .max(0x1_0000_0000u64 + 0x100_0000u64);
        let constraint_2 = Constraint::new(0x100_0000u64)
            .min(0x1_0000_0000u64)
            .max(0x1_0000_0000u64 + 0x100_0000u64);

        assert!(mgr.allocate_mem_address(&constraint_1).is_some());
        assert!(mgr.allocate_mem_address(&constraint_2).is_some());
    }
}
