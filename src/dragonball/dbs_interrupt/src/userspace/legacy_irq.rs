// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use kvm_bindings::kvm_msi;
use kvm_ioctls::VmFd;
use vmm_sys_util::eventfd::EventFd;

use std::sync::{Arc, RwLock};

use super::{
    ioapic::*, InterruptIndex, InterruptSourceConfig, InterruptSourceGroup, InterruptSourceType,
    Result,
};

#[derive(Debug)]
pub(super) struct UserspaceLegacyIrq {
    irq: Arc<UserspaceLegacyIrqObj>,
}

#[derive(Debug)]
pub(super) struct UserspaceLegacyIrqObj {
    base: InterruptIndex,
    vmfd: Arc<VmFd>,
    enabled: RwLock<bool>,
    redir_entry: RwLock<IoapicRedirEntry>,
}

impl UserspaceLegacyIrqObj {
    pub(super) fn new(base: u32, vmfd: Arc<VmFd>) -> Self {
        Self {
            base,
            vmfd,
            enabled: RwLock::new(false),
            redir_entry: RwLock::new(IoapicRedirEntry::default()),
        }
    }

    pub(super) fn redir_entry_low(&self) -> IoapicRedirEntryLow {
        self.redir_entry.read().unwrap().low().clone()
    }

    pub(super) fn set_redir_entry_low(&self, entry: IoapicRedirEntryLow) {
        self.redir_entry.write().unwrap().set_low(entry);
    }

    pub(super) fn redir_entry_high(&self) -> IoapicRedirEntryHigh {
        self.redir_entry.read().unwrap().high().clone()
    }

    pub(super) fn set_redir_entry_high(&self, entry: IoapicRedirEntryHigh) {
        self.redir_entry.write().unwrap().set_high(entry);
    }

    fn enabled(&self) -> bool {
        *self.enabled.read().unwrap()
    }

    fn set_enabled(&self, enabled: bool) {
        *self.enabled.write().unwrap() = enabled;
    }

    fn masked(&self) -> bool {
        self.redir_entry.read().unwrap().low().masked()
    }

    fn set_masked(&self, masked: bool) {
        let mut entry = self.redir_entry.write().unwrap();
        let mut low = entry.low().clone();
        low.set_masked(masked);
        entry.set_low(low);
    }

    fn delivery_status(&self) -> bool {
        self.redir_entry.read().unwrap().low().delivery_status()
    }

    fn signal_msi(&self) -> Result<()> {
        let mut address_lo = MsiAddressLow::default();
        address_lo.set_dest_mode_logical(self.redir_entry_low().dest_mode_logical());
        address_lo.set_virt_destid_8_14(self.redir_entry_high().virt_destid_8_14());
        address_lo.set_destid_0_7(self.redir_entry_high().destid_0_7());
        address_lo.set_base_address(MSI_BASE_ADDR);

        let mut data = MsiData::default();
        data.set_vector(self.redir_entry_low().vector());
        data.set_delivery_mode(self.redir_entry_low().delivery_mode());
        data.set_dest_mode_logical(self.redir_entry_low().dest_mode_logical());
        data.set_active_low(self.redir_entry_low().active_low());
        data.set_is_level(self.redir_entry_low().is_level());

        let kvm_msi = kvm_msi {
            address_lo: address_lo.into(),
            data: data.into(),
            ..Default::default()
        };

        let ret = self.vmfd.signal_msi(kvm_msi)?;
        if ret < 0 {
            return Err(std::io::Error::from_raw_os_error(-ret));
        }

        Ok(())
    }
}

impl UserspaceLegacyIrq {
    pub fn new(irq: Arc<UserspaceLegacyIrqObj>) -> Self {
        Self { irq }
    }
}

impl InterruptSourceGroup for UserspaceLegacyIrq {
    fn interrupt_type(&self) -> InterruptSourceType {
        InterruptSourceType::LegacyIrq
    }

    fn len(&self) -> u32 {
        1
    }

    fn base(&self) -> u32 {
        self.irq.base
    }

    fn enable(&self, configs: &[InterruptSourceConfig]) -> Result<()> {
        if configs.len() != 1 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        self.irq.set_enabled(true);

        Ok(())
    }

    fn disable(&self) -> Result<()> {
        self.irq.set_enabled(false);

        Ok(())
    }

    fn update(&self, index: InterruptIndex, _config: &InterruptSourceConfig) -> Result<()> {
        // Update of redirection entries would be handled by IOAPIC manager via MMIO write
        // No-op here.
        if index != 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        Ok(())
    }

    fn notifier(&self, _index: InterruptIndex) -> Option<&EventFd> {
        // KVM would not manage irqfd for split irqchip, and interrupts cannot be injected
        // via writing to irqfd.
        // Therefore, we maintain no irqfd here.
        None
    }

    fn trigger(&self, index: InterruptIndex) -> Result<()> {
        if index != 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        if !self.irq.enabled() {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        if self.irq.masked() {
            return Ok(());
        }

        self.irq.signal_msi()
    }

    fn mask(&self, index: InterruptIndex) -> Result<()> {
        if index != 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        self.irq.set_masked(true);

        Ok(())
    }

    fn unmask(&self, index: InterruptIndex) -> Result<()> {
        if index != 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        self.irq.set_masked(false);

        Ok(())
    }

    fn get_pending_state(&self, index: InterruptIndex) -> bool {
        if index != 0 {
            return false;
        }

        self.irq.delivery_status()
    }
}

#[cfg(test)]
#[cfg(target_arch = "x86_64")]
mod test {
    use super::*;
    use crate::manager::tests::create_vm_fd;
    use crate::LegacyIrqSourceConfig;
    use kvm_bindings::{kvm_enable_cap, KVM_CAP_SPLIT_IRQCHIP};
    use test_utils::skip_if_kvm_unaccessable;

    fn enable_split_irqchip(vmfd: Arc<VmFd>) {
        let mut enable_split_irqchip = kvm_enable_cap {
            cap: KVM_CAP_SPLIT_IRQCHIP,
            ..Default::default()
        };
        enable_split_irqchip.args[0] = IOAPIC_MAX_NR_REDIR_ENTRIES as u64;
        vmfd.enable_cap(&enable_split_irqchip).unwrap();
    }

    #[test]
    fn test_userspace_legacy_irq() {
        skip_if_kvm_unaccessable!();
        let vmfd = Arc::new(create_vm_fd());
        enable_split_irqchip(vmfd.clone());
        let base = 0;

        let inner = Arc::new(UserspaceLegacyIrqObj::new(base, vmfd.clone()));
        assert_eq!(u32::from(inner.redir_entry_low()), 0);
        assert_eq!(u32::from(inner.redir_entry_high()), 0);

        let mut low = IoapicRedirEntryLow::default();
        low.set_vector(0x22);
        inner.set_redir_entry_low(low);
        assert_eq!(inner.redir_entry_low().vector(), 0x22);

        let irq = UserspaceLegacyIrq::new(inner);
        let configs = [InterruptSourceConfig::LegacyIrq(LegacyIrqSourceConfig {})];
        assert_eq!(irq.interrupt_type(), InterruptSourceType::LegacyIrq);
        assert_eq!(irq.len(), 1);
        assert_eq!(irq.base(), base);
        assert!(irq.notifier(0).is_none());

        assert_eq!(
            irq.trigger(0).unwrap_err().raw_os_error(),
            Some(libc::EINVAL)
        );

        irq.enable(&configs).unwrap();
        // Since the interrupt vector is not officially registered in KVM, KVM_SIGNAL_MSI
        // will report an error code anyway. But compared to other error codes (e.g., EINVAL),
        // error code 1 means that all prechecks are passed, and the ioctl only fails because
        // KVM is not able to find the proper APIC entry
        assert_eq!(irq.trigger(0).unwrap_err().raw_os_error(), Some(1));
        assert_eq!(
            irq.trigger(1).unwrap_err().raw_os_error(),
            Some(libc::EINVAL)
        );

        irq.update(0, &configs[0]).unwrap();

        assert!(!irq.irq.masked());
        irq.mask(0).unwrap();
        assert!(irq.irq.masked());
        irq.trigger(0).unwrap();
        irq.unmask(0).unwrap();
        assert!(!irq.irq.masked());

        assert!(!irq.get_pending_state(0));
    }
}
