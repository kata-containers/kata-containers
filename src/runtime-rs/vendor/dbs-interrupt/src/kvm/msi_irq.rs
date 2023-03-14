// Copyright (C) 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Manage virtual device's PCI MSI/PCI MSIx interrupts based on Linux KVM framework.
//!
//! To optimize for performance by avoiding unnecessary locking and state checking, we assume that
//! the caller will take the responsibility to maintain the interrupt states and only issue valid
//! requests to this driver. If the caller doesn't obey the contract, only the current virtual
//! machine will be affected, it shouldn't break the host or other virtual machines.

use super::msi_generic::{create_msi_routing_entries, new_msi_routing_entry, MsiConfig};
use super::*;

pub(super) struct MsiIrq {
    base: InterruptIndex,
    count: InterruptIndex,
    vmfd: Arc<VmFd>,
    irq_routing: Arc<KvmIrqRouting>,
    msi_configs: Vec<MsiConfig>,
}

impl MsiIrq {
    pub(super) fn new(
        base: InterruptIndex,
        count: InterruptIndex,
        max_msi_irqs: InterruptIndex,
        vmfd: Arc<VmFd>,
        irq_routing: Arc<KvmIrqRouting>,
    ) -> Result<Self> {
        if count > max_msi_irqs || base >= MAX_IRQS || base + count > MAX_IRQS {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        let mut msi_configs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            msi_configs.push(MsiConfig::new());
        }

        Ok(MsiIrq {
            base,
            count,
            vmfd,
            irq_routing,
            msi_configs,
        })
    }
}

impl InterruptSourceGroup for MsiIrq {
    fn interrupt_type(&self) -> InterruptSourceType {
        InterruptSourceType::MsiIrq
    }

    fn len(&self) -> u32 {
        self.count
    }

    fn base(&self) -> u32 {
        self.base
    }

    fn enable(&self, configs: &[InterruptSourceConfig]) -> Result<()> {
        if configs.len() != self.count as usize {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        // First add IRQ routings for all the MSI interrupts.
        let entries = create_msi_routing_entries(self.base, configs)?;

        self.irq_routing
            .add(&entries)
            .or_else(|err| match err.kind() {
                // The irq_routing was already restored when the snapshot was restored, so the AlreadyExists error is ignored here.
                std::io::ErrorKind::AlreadyExists => Ok(()),
                _ => Err(err),
            })?;

        // Then register irqfds to the KVM module.
        for i in 0..self.count {
            let irqfd = &self.msi_configs[i as usize].irqfd;
            self.vmfd
                .register_irqfd(irqfd, self.base + i)
                .map_err(from_sys_util_errno)?;
        }

        Ok(())
    }

    fn disable(&self) -> Result<()> {
        // First unregister all irqfds, so it won't trigger anymore.
        for i in 0..self.count {
            let irqfd = &self.msi_configs[i as usize].irqfd;
            self.vmfd
                .unregister_irqfd(irqfd, self.base + i)
                .map_err(from_sys_util_errno)?;
        }

        // Then tear down the IRQ routings for all the MSI interrupts.
        let mut entries = Vec::with_capacity(self.count as usize);
        for i in 0..self.count {
            // Safe to unwrap because there's no legal way to break the mutex.
            let msicfg = self.msi_configs[i as usize].config.lock().unwrap();
            let entry = new_msi_routing_entry(self.base + i, &msicfg);
            entries.push(entry);
        }
        self.irq_routing.remove(&entries)?;

        Ok(())
    }

    #[allow(irrefutable_let_patterns)]
    fn update(&self, index: InterruptIndex, config: &InterruptSourceConfig) -> Result<()> {
        if index >= self.count {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        if let InterruptSourceConfig::MsiIrq(ref cfg) = config {
            // Safe to unwrap because there's no legal way to break the mutex.
            let entry = {
                let mut msicfg = self.msi_configs[index as usize].config.lock().unwrap();
                msicfg.high_addr = cfg.high_addr;
                msicfg.low_addr = cfg.low_addr;
                msicfg.data = cfg.data;
                msicfg.device_id = cfg.device_id;
                new_msi_routing_entry(self.base + index, &msicfg)
            };
            self.irq_routing.modify(&entry)
        } else {
            Err(std::io::Error::from_raw_os_error(libc::EINVAL))
        }
    }

    fn notifier(&self, index: InterruptIndex) -> Option<&EventFd> {
        if index >= self.count {
            None
        } else {
            let msi_config = &self.msi_configs[index as usize];
            Some(&msi_config.irqfd)
        }
    }

    fn trigger(&self, index: InterruptIndex) -> Result<()> {
        // Assume that the caller will maintain the interrupt states and only call this function
        // when suitable.
        if index >= self.count {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        let msi_config = &self.msi_configs[index as usize];
        msi_config.irqfd.write(1)
    }

    fn mask(&self, index: InterruptIndex) -> Result<()> {
        if index >= self.count {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        let irqfd = &self.msi_configs[index as usize].irqfd;
        self.vmfd
            .unregister_irqfd(irqfd, self.base + index)
            .map_err(from_sys_util_errno)?;

        Ok(())
    }

    fn unmask(&self, index: InterruptIndex) -> Result<()> {
        if index >= self.count {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }

        let irqfd = &self.msi_configs[index as usize].irqfd;
        self.vmfd
            .register_irqfd(irqfd, self.base + index)
            .map_err(from_sys_util_errno)?;

        Ok(())
    }

    fn get_pending_state(&self, index: InterruptIndex) -> bool {
        if index >= self.count {
            return false;
        }

        // Peak the EventFd.count by reading and writing back.
        // The irqfd must be in NON-BLOCKING mode.
        let irqfd = &self.msi_configs[index as usize].irqfd;
        match irqfd.read() {
            Err(_) => false,
            Ok(count) => {
                if count != 0 && irqfd.write(count).is_err() {
                    // Hope the caller will handle the pending state corrrectly,
                    // then no interrupt will be lost.
                    // Really no way to recover here!
                }
                count != 0
            }
        }
    }
}

#[cfg(target_arch = "x86_64")]
#[cfg(test)]
mod test {
    use super::*;
    use crate::manager::tests::create_vm_fd;

    #[test]
    #[allow(unreachable_patterns)]
    fn test_msi_interrupt_group() {
        let vmfd = Arc::new(create_vm_fd());
        vmfd.create_irq_chip().unwrap();

        let rounting = Arc::new(KvmIrqRouting::new(vmfd.clone()));
        rounting.initialize().unwrap();

        let base = 168;
        let count = 32;
        let group = MsiIrq::new(
            base,
            count,
            DEFAULT_MAX_MSI_IRQS_PER_DEVICE,
            vmfd.clone(),
            rounting.clone(),
        )
        .unwrap();
        let mut msi_fds = Vec::with_capacity(count as usize);

        match group.interrupt_type() {
            InterruptSourceType::MsiIrq => {}
            _ => {
                panic!();
            }
        }

        for _ in 0..count {
            let msi_source_config = MsiIrqSourceConfig {
                high_addr: 0x1234,
                low_addr: 0x5678,
                data: 0x9876,
                msg_ctl: 0x6789,
                device_id: None,
            };
            msi_fds.push(InterruptSourceConfig::MsiIrq(msi_source_config));
        }

        group.enable(&msi_fds).unwrap();
        assert_eq!(group.len(), count);
        assert_eq!(group.base(), base);

        for i in 0..count {
            let msi_source_config = MsiIrqSourceConfig {
                high_addr: i + 0x1234,
                low_addr: i + 0x5678,
                data: i + 0x9876,
                msg_ctl: i + 0x6789,
                device_id: None,
            };
            group.notifier(i).unwrap().write(1).unwrap();
            group.trigger(i).unwrap();
            group
                .update(0, &InterruptSourceConfig::MsiIrq(msi_source_config))
                .unwrap();
        }
        assert!(group.trigger(33).is_err());
        group.disable().unwrap();

        assert!(MsiIrq::new(
            base,
            DEFAULT_MAX_MSI_IRQS_PER_DEVICE + 1,
            DEFAULT_MAX_MSI_IRQS_PER_DEVICE,
            vmfd.clone(),
            rounting.clone()
        )
        .is_err());
        assert!(MsiIrq::new(1100, 1, DEFAULT_MAX_MSI_IRQS_PER_DEVICE, vmfd, rounting).is_err());
    }
}
