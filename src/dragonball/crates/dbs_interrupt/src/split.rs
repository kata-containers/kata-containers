// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use kvm_ioctls::VmFd;

use std::io::{Error, ErrorKind};
use std::sync::Arc;

#[cfg(feature = "msi-irq")]
use super::KvmIrqManager;
#[cfg(feature = "legacy-irq")]
use super::UserspaceIoapicManager;
use super::{InterruptIndex, InterruptManager, InterruptSourceGroup, InterruptSourceType, Result};

/// Structure to manage interrupt sources for a virtual machine in with both KVM and
/// userspace IOAPIC.
///
/// In case of split irqchip, legacy IRQs are managed with userspace IOAPIC, while
/// MSI IRQs are managed by KVM.
pub struct SplitIrqManager {
    #[cfg(feature = "legacy-irq")]
    userspace_mgr: UserspaceIoapicManager,
    #[cfg(feature = "msi-irq")]
    kvm_mgr: KvmIrqManager,
}

impl SplitIrqManager {
    /// Create a new interrupt manager in split mode
    pub fn new(vmfd: Arc<VmFd>) -> Result<Self> {
        Ok(Self {
            #[cfg(feature = "legacy-irq")]
            userspace_mgr: UserspaceIoapicManager::create_default_ioapic_manager(vmfd.clone())?,
            #[cfg(feature = "msi-irq")]
            kvm_mgr: KvmIrqManager::new_with_kvm_legacy_disabled(vmfd.clone()),
        })
    }

    #[cfg(feature = "msi-irq")]
    /// Set maximum supported MSI interrupts per device.
    pub fn set_max_msi_irqs(&self, max_msi_irqs: InterruptIndex) {
        self.kvm_mgr.set_max_msi_irqs(max_msi_irqs);
    }
}

impl InterruptManager for SplitIrqManager {
    fn initialize(&self) -> Result<()> {
        #[cfg(feature = "legacy-irq")]
        self.userspace_mgr.initialize()?;
        #[cfg(feature = "msi-irq")]
        self.kvm_mgr.initialize()?;
        Ok(())
    }

    fn create_group(
        &self,
        ty: InterruptSourceType,
        base: InterruptIndex,
        count: u32,
    ) -> Result<Arc<Box<dyn InterruptSourceGroup>>> {
        #[allow(unreachable_patterns)]
        let group = match ty {
            #[cfg(feature = "legacy-irq")]
            InterruptSourceType::LegacyIrq => self.userspace_mgr.create_group(ty, base, count)?,
            #[cfg(feature = "msi-irq")]
            InterruptSourceType::MsiIrq => self.kvm_mgr.create_group(ty, base, count)?,
            _ => return Err(Error::from(ErrorKind::InvalidInput)),
        };
        Ok(group)
    }

    fn destroy_group(&self, group: Arc<Box<dyn InterruptSourceGroup>>) -> Result<()> {
        #[allow(unreachable_patterns)]
        match group.interrupt_type() {
            #[cfg(feature = "legacy-irq")]
            InterruptSourceType::LegacyIrq => self.userspace_mgr.destroy_group(group)?,
            #[cfg(feature = "msi-irq")]
            InterruptSourceType::MsiIrq => self.kvm_mgr.destroy_group(group)?,
            _ => return Err(Error::from(ErrorKind::InvalidInput)),
        };
        Ok(())
    }

    fn ioapic_read(&self, addr: u64, data: &mut [u8]) -> Result<()> {
        #[cfg(feature = "legacy-irq")]
        return self.userspace_mgr.ioapic_read(addr, data);
        #[cfg(not(feature = "legacy-irq"))]
        {
            let _ = (addr, data);
            Ok(())
        }
    }

    fn ioapic_write(&self, addr: u64, data: &[u8]) -> Result<()> {
        #[cfg(feature = "legacy-irq")]
        return self.userspace_mgr.ioapic_write(addr, data);
        #[cfg(not(feature = "legacy-irq"))]
        {
            let _ = (addr, data);
            Ok(())
        }
    }
}
