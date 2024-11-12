// Copyright (C) 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Manage virtual device's legacy interrupts based on Linux KVM framework.
//!
//! On x86 platforms, legacy interrupts are those managed by the Master PIC, the slave PIC and
//! IOAPICs.

use kvm_bindings::KVM_IRQ_ROUTING_IRQCHIP;
#[cfg(target_arch = "x86_64")]
use kvm_bindings::{KVM_IRQCHIP_IOAPIC, KVM_IRQCHIP_PIC_MASTER, KVM_IRQCHIP_PIC_SLAVE};
use vmm_sys_util::eventfd::EFD_NONBLOCK;

use super::*;

#[cfg(target_arch = "x86_64")]
/// Maximum number of legacy interrupts supported.
pub const MAX_LEGACY_IRQS: u32 = 24;

#[cfg(target_arch = "aarch64")]
/// Maximum number of legacy interrupts supported.
pub const MAX_LEGACY_IRQS: u32 = 128;

pub(super) struct LegacyIrq {
    base: u32,
    vmfd: Arc<VmFd>,
    irqfd: EventFd,
}

impl LegacyIrq {
    pub(super) fn new(
        base: InterruptIndex,
        count: InterruptIndex,
        vmfd: Arc<VmFd>,
        _routes: Arc<KvmIrqRouting>,
    ) -> Result<Self> {
        if count != 1 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        if base >= MAX_LEGACY_IRQS {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        Ok(LegacyIrq {
            base,
            vmfd,
            irqfd: EventFd::new(EFD_NONBLOCK)?,
        })
    }

    #[cfg(target_arch = "x86_64")]
    fn add_legacy_entry(
        gsi: u32,
        chip: u32,
        pin: u32,
        routes: &mut HashMap<u64, kvm_irq_routing_entry>,
    ) -> Result<()> {
        let mut entry = kvm_irq_routing_entry {
            gsi,
            type_: KVM_IRQ_ROUTING_IRQCHIP,
            ..Default::default()
        };
        // Safe because we are initializing all fields of the `irqchip` struct.
        entry.u.irqchip.irqchip = chip;
        entry.u.irqchip.pin = pin;
        routes.insert(hash_key(&entry), entry);

        Ok(())
    }

    /// Build routings for IRQs connected to the master PIC, the slave PIC or the first IOAPIC.
    #[cfg(target_arch = "x86_64")]
    pub(super) fn initialize_legacy(
        routes: &mut HashMap<u64, kvm_irq_routing_entry>,
    ) -> Result<()> {
        // Build routings for the master PIC
        for i in 0..8 {
            if i != 2 {
                Self::add_legacy_entry(i, KVM_IRQCHIP_PIC_MASTER, i, routes)?;
            }
        }

        // Build routings for the slave PIC
        for i in 8..16 {
            Self::add_legacy_entry(i, KVM_IRQCHIP_PIC_SLAVE, i - 8, routes)?;
        }

        // Build routings for the first IOAPIC
        for i in 0..MAX_LEGACY_IRQS {
            if i == 0 {
                Self::add_legacy_entry(i, KVM_IRQCHIP_IOAPIC, 2, routes)?;
            } else if i != 2 {
                Self::add_legacy_entry(i, KVM_IRQCHIP_IOAPIC, i, routes)?;
            };
        }

        Ok(())
    }

    #[cfg(target_arch = "aarch64")]
    pub(super) fn initialize_legacy(
        routes: &mut HashMap<u64, kvm_irq_routing_entry>,
    ) -> Result<()> {
        for i in 0..MAX_LEGACY_IRQS {
            let mut entry = kvm_irq_routing_entry {
                gsi: i,
                type_: KVM_IRQ_ROUTING_IRQCHIP,
                ..Default::default()
            };
            entry.u.irqchip.irqchip = 0;
            entry.u.irqchip.pin = i;
            routes.insert(hash_key(&entry), entry);
        }
        Ok(())
    }
}

impl InterruptSourceGroup for LegacyIrq {
    fn interrupt_type(&self) -> InterruptSourceType {
        InterruptSourceType::LegacyIrq
    }

    fn len(&self) -> u32 {
        1
    }

    fn base(&self) -> u32 {
        self.base
    }

    fn enable(&self, configs: &[InterruptSourceConfig]) -> Result<()> {
        if configs.len() != 1 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        // The IRQ routings for legacy IRQs have been configured during KvmIrqManager::initialize(),
        // so only need to register irqfd to the KVM driver.
        self.vmfd
            .register_irqfd(&self.irqfd, self.base)
            .map_err(from_sys_util_errno)
    }

    fn disable(&self) -> Result<()> {
        self.vmfd
            .unregister_irqfd(&self.irqfd, self.base)
            .map_err(from_sys_util_errno)
    }

    fn update(&self, index: InterruptIndex, _config: &InterruptSourceConfig) -> Result<()> {
        // For legacy interrupts, the routing configuration is managed by the PIC/IOAPIC interrupt
        // controller drivers, so nothing to do here.
        if index != 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        Ok(())
    }

    fn notifier(&self, index: InterruptIndex) -> Option<&EventFd> {
        if index != 0 {
            None
        } else {
            Some(&self.irqfd)
        }
    }

    fn trigger(&self, index: InterruptIndex) -> Result<()> {
        if index != 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        self.irqfd.write(1)
    }

    fn mask(&self, index: InterruptIndex) -> Result<()> {
        if index > 1 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        self.vmfd
            .unregister_irqfd(&self.irqfd, self.base + index)
            .map_err(from_sys_util_errno)?;

        Ok(())
    }

    fn unmask(&self, index: InterruptIndex) -> Result<()> {
        if index > 1 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        self.vmfd
            .register_irqfd(&self.irqfd, self.base + index)
            .map_err(from_sys_util_errno)?;

        Ok(())
    }

    fn get_pending_state(&self, index: InterruptIndex) -> bool {
        if index > 1 {
            return false;
        }

        // Peak the EventFd.count by reading and writing back.
        // The irqfd must be in NON-BLOCKING mode.
        match self.irqfd.read() {
            Err(_) => false,
            Ok(count) => {
                if count != 0 && self.irqfd.write(count).is_err() {
                    // Hope the caller will handle the pending state corrrectly,
                    // then no interrupt will be lost.
                }
                count != 0
            }
        }
    }
}

#[cfg(test)]
#[cfg(target_arch = "x86_64")]
mod test {
    use super::*;
    use crate::manager::tests::create_vm_fd;

    const MASTER_PIC: usize = 7;
    const SLAVE_PIC: usize = 8;
    const IOAPIC: usize = 23;

    #[test]
    #[allow(unreachable_patterns)]
    fn test_legacy_interrupt_group() {
        let vmfd = Arc::new(create_vm_fd());
        let rounting = Arc::new(KvmIrqRouting::new(vmfd.clone()));
        let base = 0;
        let count = 1;
        let group = LegacyIrq::new(base, count, vmfd.clone(), rounting.clone()).unwrap();

        let legacy_fds = vec![InterruptSourceConfig::LegacyIrq(LegacyIrqSourceConfig {})];

        match group.interrupt_type() {
            InterruptSourceType::LegacyIrq => {}
            _ => {
                panic!();
            }
        }
        vmfd.create_irq_chip().unwrap();
        assert_eq!(group.len(), 1);
        assert_eq!(group.base(), base);
        group.enable(&legacy_fds).unwrap();
        group.notifier(0).unwrap().write(1).unwrap();
        group.trigger(0).unwrap();
        assert!(group.trigger(1).is_err());
        group
            .update(
                0,
                &InterruptSourceConfig::LegacyIrq(LegacyIrqSourceConfig {}),
            )
            .unwrap();
        group.disable().unwrap();

        assert!(LegacyIrq::new(base, 2, vmfd.clone(), rounting.clone()).is_err());
        assert!(LegacyIrq::new(110, 1, vmfd, rounting).is_err());
    }

    #[test]
    fn test_irq_routing_initialize_legacy() {
        let vmfd = Arc::new(create_vm_fd());
        let routing = KvmIrqRouting::new(vmfd.clone());

        // this would ok on 4.9 kernel
        assert!(routing.initialize().is_err());

        vmfd.create_irq_chip().unwrap();
        routing.initialize().unwrap();

        let routes = &routing.routes.lock().unwrap();
        assert_eq!(routes.len(), MASTER_PIC + SLAVE_PIC + IOAPIC);
    }

    #[test]
    fn test_routing_opt() {
        let vmfd = Arc::new(create_vm_fd());
        let routing = KvmIrqRouting::new(vmfd.clone());

        // this would ok on 4.9 kernel
        assert!(routing.initialize().is_err());

        vmfd.create_irq_chip().unwrap();
        routing.initialize().unwrap();

        let mut entry = kvm_irq_routing_entry {
            gsi: 8,
            type_: kvm_bindings::KVM_IRQ_ROUTING_IRQCHIP,
            ..Default::default()
        };

        // Safe because we are initializing all fields of the `irqchip` struct.
        entry.u.irqchip.irqchip = 0;
        entry.u.irqchip.pin = 3;

        let entrys = vec![entry];

        assert!(routing.modify(&entry).is_err());
        routing.add(&entrys).unwrap();
        entry.u.irqchip.pin = 4;
        routing.modify(&entry).unwrap();
        routing.remove(&entrys).unwrap();
        assert!(routing.modify(&entry).is_err());
    }

    #[test]
    fn test_routing_set_routing() {
        let vmfd = Arc::new(create_vm_fd());
        let routing = KvmIrqRouting::new(vmfd.clone());

        // this would ok on 4.9 kernel
        assert!(routing.initialize().is_err());

        vmfd.create_irq_chip().unwrap();
        routing.initialize().unwrap();

        let mut entry = kvm_irq_routing_entry {
            gsi: 8,
            type_: kvm_bindings::KVM_IRQ_ROUTING_IRQCHIP,
            ..Default::default()
        };
        entry.u.irqchip.irqchip = 0;
        entry.u.irqchip.pin = 3;

        routing
            .routes
            .lock()
            .unwrap()
            .insert(hash_key(&entry), entry);
        let routes = routing.routes.lock().unwrap();
        routing.set_routing(&routes).unwrap();
    }

    #[test]
    fn test_has_key() {
        let gsi = 4;
        let mut entry = kvm_irq_routing_entry {
            gsi,
            type_: kvm_bindings::KVM_IRQ_ROUTING_IRQCHIP,
            ..Default::default()
        };
        // Safe because we are initializing all fields of the `irqchip` struct.
        entry.u.irqchip.irqchip = kvm_bindings::KVM_IRQCHIP_PIC_MASTER;
        entry.u.irqchip.pin = gsi;
        assert_eq!(hash_key(&entry), 0x0001_0000_0004);
    }
}
