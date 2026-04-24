// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use kvm_ioctls::VmFd;

use std::convert::TryInto;
use std::sync::{Arc, RwLock};

#[cfg(feature = "split-legacy-irq")]
use super::legacy_irq::*;
use super::{
    ioapic::*, InterruptIndex, InterruptManager, InterruptSourceGroup, InterruptSourceType, Result,
};

/// Structure to manage interrupt sources for a virtual machine in userspace based on IOAPIC
/// protocol.
///
/// The structure emulates IOAPIC registers, and allows for editing specific IOAPIC entries via
/// MMIO calls.
pub struct UserspaceIoapicManager {
    ioregsel: RwLock<IoRegSel>,
    ioapicid: RwLock<IoapicId>,
    // IoapicVer register is read-only
    ioapicver: IoapicVer,
    ioapicarb: RwLock<IoapicArb>,
    #[cfg(feature = "split-legacy-irq")]
    irqs: Vec<Arc<UserspaceLegacyIrqObj>>,
}

impl UserspaceIoapicManager {
    /// Create a new IOAPIC manager instance
    pub fn create_ioapic_manager(
        vmfd: Arc<VmFd>,
        version: u8,
        nr_redir_entries: InterruptIndex,
    ) -> Result<Self> {
        if nr_redir_entries == 0 || nr_redir_entries > IOAPIC_MAX_NR_REDIR_ENTRIES {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        let mut ioapicver = IoapicVer::default();
        ioapicver.set_version(version);
        ioapicver.set_entries(nr_redir_entries as u8 - 1);

        #[cfg(feature = "split-legacy-irq")]
        let irqs = {
            let mut irqs = Vec::with_capacity(nr_redir_entries as usize);
            for i in 0..nr_redir_entries {
                irqs.push(Arc::new(UserspaceLegacyIrqObj::new(i, vmfd.clone())));
            }
            irqs
        };

        Ok(Self {
            ioregsel: RwLock::new(IoRegSel::default()),
            ioapicid: RwLock::new(IoapicId::default()),
            ioapicver,
            ioapicarb: RwLock::new(IoapicArb::default()),
            #[cfg(feature = "split-legacy-irq")]
            irqs,
        })
    }

    /// Create a new IOAPIC manager instance with default version and redirection table size
    pub fn create_default_ioapic_manager(vmfd: Arc<VmFd>) -> Result<Self> {
        Self::create_ioapic_manager(
            vmfd,
            IOAPIC_DEFAULT_VERSION,
            IOAPIC_DEFAULT_NR_REDIR_ENTRIES,
        )
    }

    fn ioregsel(&self) -> u32 {
        self.ioregsel.read().unwrap().clone().into()
    }

    fn set_ioregsel(&self, val: u32) -> Result<()> {
        let val = IoRegSel::from(val);
        let select = val.register_index();
        if !(select == IOAPIC_IOAPICID_INDEX
            || select == IOAPIC_IOAPICVER_INDEX
            || select == IOAPIC_IOAPICARB_INDEX
            || (select >= IOAPIC_REDIR_TABLE_START_INDEX
                && select < IOAPIC_REDIR_TABLE_START_INDEX + 2 * self.nr_redir_entries() as u8))
        {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        *self.ioregsel.write().unwrap() = val;

        Ok(())
    }

    fn iowin(&self) -> u32 {
        match self.ioregsel.read().unwrap().register_index() {
            IOAPIC_IOAPICID_INDEX => self.ioapicid.read().unwrap().clone().into(),
            IOAPIC_IOAPICVER_INDEX => self.ioapicver.clone().into(),
            IOAPIC_IOAPICARB_INDEX => self.ioapicarb.read().unwrap().clone().into(),
            // We have checked the validity of ioregsel while setting, therefore all values beyond the
            // special IOAPIC registers above would become a valid redirection entry
            index => {
                #[cfg(feature = "split-legacy-irq")]
                {
                    let offset = (index - IOAPIC_REDIR_TABLE_START_INDEX) as usize;
                    let is_low = (offset & 0x1) == 0;
                    let irq_base = offset >> 1;

                    if is_low {
                        self.irqs[irq_base].redir_entry_low().into()
                    } else {
                        self.irqs[irq_base].redir_entry_high().into()
                    }
                }
                #[cfg(not(feature = "split-legacy-irq"))]
                0
            }
        }
    }

    fn set_iowin(&self, val: u32) -> Result<()> {
        match self.ioregsel.read().unwrap().register_index() {
            IOAPIC_IOAPICID_INDEX => {
                *self.ioapicid.write().unwrap() = IoapicId::from(val);
                Ok(())
            }
            IOAPIC_IOAPICVER_INDEX => {
                // IOAPICVER register is read-only
                Err(std::io::Error::from_raw_os_error(libc::EINVAL))
            }
            IOAPIC_IOAPICARB_INDEX => {
                *self.ioapicarb.write().unwrap() = IoapicArb::from(val);
                Ok(())
            }
            // We have checked the validity of ioregsel while setting, therefore all values beyond the four
            // special IOAPIC registers above would become a valid redirection entry
            index => {
                #[cfg(feature = "split-legacy-irq")]
                {
                    let offset = (index - IOAPIC_REDIR_TABLE_START_INDEX) as usize;
                    let is_low = (offset & 0x1) == 0;
                    let irq_base = offset >> 1;

                    if is_low {
                        self.irqs[irq_base].set_redir_entry_low(IoapicRedirEntryLow::from(val));
                    } else {
                        self.irqs[irq_base].set_redir_entry_high(IoapicRedirEntryHigh::from(val));
                    }
                }

                Ok(())
            }
        }
    }

    fn nr_redir_entries(&self) -> InterruptIndex {
        self.ioapicver.entries() as InterruptIndex + 1
    }
}

impl InterruptManager for UserspaceIoapicManager {
    fn create_group(
        &self,
        ty: InterruptSourceType,
        base: InterruptIndex,
        count: u32,
    ) -> Result<Arc<Box<dyn InterruptSourceGroup>>> {
        let group = match ty {
            #[cfg(feature = "split-legacy-irq")]
            InterruptSourceType::LegacyIrq => {
                if count != 1 {
                    return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
                }
                if base >= self.nr_redir_entries() {
                    return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
                }
                // Irq has already been created while initializing the manager, so we
                // only return the corresponding entry here.
                let irq = self.irqs[base as usize].clone();
                let group: Arc<Box<dyn InterruptSourceGroup>> =
                    Arc::new(Box::new(UserspaceLegacyIrq::new(irq)));
                group
            }
            _ => {
                return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
            }
        };

        Ok(group)
    }

    fn destroy_group(&self, _group: Arc<Box<dyn InterruptSourceGroup>>) -> Result<()> {
        Ok(())
    }

    fn ioapic_read(&self, addr: u64, data: &mut [u8]) -> Result<()> {
        if data.len() != 4 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        let mut val = 0;

        if addr == IOAPIC_IOREGSEL_BASE as u64 {
            val = self.ioregsel();
        } else if addr == IOAPIC_IOWIN_BASE as u64 {
            val = self.iowin();
        }

        data.copy_from_slice(&val.to_le_bytes());

        Ok(())
    }

    fn ioapic_write(&self, addr: u64, data: &[u8]) -> Result<()> {
        if data.len() != 4 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        // Safe because we have checked that the length of data is 32 bits
        let val = u32::from_le_bytes(data.try_into().expect("length checked to be 4"));

        if addr == IOAPIC_IOREGSEL_BASE as u64 {
            self.set_ioregsel(val)?;
        } else if addr == IOAPIC_IOWIN_BASE as u64 {
            self.set_iowin(val)?;
        }

        Ok(())
    }
}

#[cfg(test)]
#[cfg(target_arch = "x86_64")]
pub(crate) mod test {
    use super::*;
    use crate::manager::tests::create_vm_fd;
    use crate::{InterruptSourceConfig, LegacyIrqSourceConfig};
    use bilge::prelude::*;
    use test_utils::skip_if_kvm_unaccessable;

    #[test]
    fn test_create_userspace_ioapic_manager() {
        skip_if_kvm_unaccessable!();
        let vmfd = Arc::new(create_vm_fd());

        assert!(UserspaceIoapicManager::create_ioapic_manager(vmfd.clone(), 0, 0).is_err());
        assert!(UserspaceIoapicManager::create_ioapic_manager(
            vmfd.clone(),
            0,
            IOAPIC_MAX_NR_REDIR_ENTRIES + 1
        )
        .is_err());

        let manager =
            UserspaceIoapicManager::create_ioapic_manager(vmfd.clone(), IOAPIC_DEFAULT_VERSION, 12)
                .unwrap();
        assert_eq!(manager.nr_redir_entries(), 12);
        assert_eq!(manager.ioregsel.read().unwrap().register_index(), 0);
        assert_eq!(manager.ioapicid.read().unwrap().id(), 0);
        assert_eq!(manager.ioapicver.version(), IOAPIC_DEFAULT_VERSION);
        assert_eq!(manager.ioapicver.entries(), 11);
        assert_eq!(
            manager.ioapicarb.read().unwrap().arbitration(),
            u4::from_u8(0)
        );

        #[cfg(feature = "split-legacy-irq")]
        {
            assert_eq!(manager.irqs.len(), 12);
            for irq in manager.irqs.iter() {
                assert_eq!(u32::from(irq.redir_entry_low()), 0);
                assert_eq!(u32::from(irq.redir_entry_high()), 0);
            }
        }
    }

    #[test]
    fn test_userspace_ioapic_rw() {
        skip_if_kvm_unaccessable!();
        let vmfd = Arc::new(create_vm_fd());
        let manager = UserspaceIoapicManager::create_default_ioapic_manager(vmfd.clone()).unwrap();
        let ioapicver = ((IOAPIC_MAX_NR_REDIR_ENTRIES - 1) << 16) + IOAPIC_DEFAULT_VERSION as u32;

        assert_eq!(manager.ioregsel(), 0);
        assert_eq!(manager.iowin(), 0);

        manager.set_ioregsel(0x01).unwrap();
        assert_eq!(manager.ioregsel(), 1);
        assert_eq!(manager.iowin(), ioapicver,);

        manager.set_ioregsel(0x02).unwrap();
        assert_eq!(manager.iowin(), 0);

        assert!(manager.set_ioregsel(0x04).is_err());
        assert!(manager.set_ioregsel(0x40).is_err());

        manager.set_ioregsel(0x10).unwrap();
        assert_eq!(manager.iowin(), 0);

        manager.set_ioregsel(0x00).unwrap();
        manager.set_iowin(0xffffffff).unwrap();
        assert_eq!(manager.iowin(), 0xffffffff);

        manager.set_ioregsel(0x01).unwrap();
        assert!(manager.set_iowin(0xeeeeeeee).is_err());
        assert_eq!(manager.iowin(), ioapicver);

        manager.set_ioregsel(0x02).unwrap();
        manager.set_iowin(0xdddddddd).unwrap();
        assert_eq!(manager.iowin(), 0xdddddddd);

        manager.set_ioregsel(0x12).unwrap();
        manager.set_iowin(0xcccccccc).unwrap();
        assert_eq!(manager.iowin(), 0xcccccccc);
        #[cfg(feature = "split-legacy-irq")]
        {
            assert_eq!(u32::from(manager.irqs[1].redir_entry_low()), 0xcccccccc);
        }

        manager.set_ioregsel(0x15).unwrap();
        manager.set_iowin(0xbbbbbbbb).unwrap();
        assert_eq!(manager.iowin(), 0xbbbbbbbb);
        #[cfg(feature = "split-legacy-irq")]
        {
            assert_eq!(u32::from(manager.irqs[2].redir_entry_high()), 0xbbbbbbbb);
        }
    }

    #[test]
    fn test_userspace_interrupt_manager() {
        skip_if_kvm_unaccessable!();
        let vmfd = Arc::new(create_vm_fd());
        let manager = UserspaceIoapicManager::create_default_ioapic_manager(vmfd.clone()).unwrap();
        manager.initialize().unwrap();

        assert!(manager
            .create_group(InterruptSourceType::LegacyIrq, 0, 2)
            .is_err());
        assert!(manager
            .create_group(InterruptSourceType::LegacyIrq, 24, 1)
            .is_err());

        let group = manager
            .create_group(InterruptSourceType::LegacyIrq, 5, 1)
            .unwrap();
        let configs = [InterruptSourceConfig::LegacyIrq(LegacyIrqSourceConfig {})];
        group.enable(&configs).unwrap();
        group.mask(0).unwrap();
        group.trigger(0).unwrap();

        let mut data = [0u8; 3];
        assert!(manager
            .ioapic_read(IOAPIC_IOREGSEL_BASE as u64, &mut data)
            .is_err());
        assert!(manager
            .ioapic_write(IOAPIC_IOREGSEL_BASE as u64, &data)
            .is_err());

        let mut data = 2u32.to_le_bytes();
        manager
            .ioapic_write(IOAPIC_IOREGSEL_BASE as u64, &data)
            .unwrap();
        data.fill(0);
        manager
            .ioapic_read(IOAPIC_IOREGSEL_BASE as u64, &mut data)
            .unwrap();
        assert_eq!(u32::from_le_bytes(data.try_into().unwrap()), 2);

        data = 0xffffffffu32.to_le_bytes();
        manager
            .ioapic_write(IOAPIC_IOWIN_BASE as u64, &data)
            .unwrap();
        data.fill(0);
        manager
            .ioapic_read(IOAPIC_IOWIN_BASE as u64, &mut data)
            .unwrap();
        assert_eq!(u32::from_le_bytes(data.try_into().unwrap()), 0xffffffff);

        manager.destroy_group(group).unwrap();
    }
}
