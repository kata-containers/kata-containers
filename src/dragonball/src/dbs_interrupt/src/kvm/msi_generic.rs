// Copyright (C) 2019 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Helper utilities for handling MSI interrupts.

use kvm_bindings::{kvm_irq_routing_entry, KVM_IRQ_ROUTING_MSI};
use vmm_sys_util::eventfd::EFD_NONBLOCK;

use super::*;

pub(crate) struct MsiConfig {
    pub(super) irqfd: EventFd,
    pub(crate) config: Mutex<MsiIrqSourceConfig>,
}

impl MsiConfig {
    pub(crate) fn new() -> Self {
        MsiConfig {
            irqfd: EventFd::new(EFD_NONBLOCK).unwrap(),
            config: Mutex::new(Default::default()),
        }
    }
}

pub(super) fn new_msi_routing_entry(
    gsi: InterruptIndex,
    msicfg: &MsiIrqSourceConfig,
) -> kvm_irq_routing_entry {
    let mut entry = kvm_irq_routing_entry {
        gsi,
        type_: KVM_IRQ_ROUTING_MSI,
        ..Default::default()
    };
    entry.u.msi.address_hi = msicfg.high_addr;
    entry.u.msi.address_lo = msicfg.low_addr;
    entry.u.msi.data = msicfg.data;
    if let Some(dev_id) = msicfg.device_id {
        entry.u.msi.__bindgen_anon_1.devid = dev_id;
        entry.flags = kvm_bindings::KVM_MSI_VALID_DEVID;
    }

    entry
}

#[allow(irrefutable_let_patterns)]
pub(super) fn create_msi_routing_entries(
    base: InterruptIndex,
    configs: &[InterruptSourceConfig],
) -> Result<Vec<kvm_irq_routing_entry>> {
    let _ = base
        .checked_add(configs.len() as u32)
        .ok_or_else(|| std::io::Error::from_raw_os_error(libc::EINVAL))?;
    let mut entries = Vec::with_capacity(configs.len());
    for (i, ref val) in configs.iter().enumerate() {
        if let InterruptSourceConfig::MsiIrq(msicfg) = val {
            let entry = new_msi_routing_entry(base + i as u32, msicfg);
            entries.push(entry);
        } else {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
    }

    Ok(entries)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_create_msiconfig() {
        let config = MsiConfig::new();
        config.irqfd.write(1).unwrap();
    }

    #[test]
    fn test_new_msi_routing_single() {
        let test_gsi = 4;
        let msi_source_config = MsiIrqSourceConfig {
            high_addr: 0x1234,
            low_addr: 0x5678,
            data: 0x9876,
            msg_ctl: 0,
            device_id: None,
        };
        let entry = new_msi_routing_entry(test_gsi, &msi_source_config);
        assert_eq!(entry.gsi, test_gsi);
        assert_eq!(entry.type_, KVM_IRQ_ROUTING_MSI);
        unsafe {
            assert_eq!(entry.u.msi.address_hi, msi_source_config.high_addr);
            assert_eq!(entry.u.msi.address_lo, msi_source_config.low_addr);
            assert_eq!(entry.u.msi.data, msi_source_config.data);
        }
    }

    #[cfg(all(feature = "legacy-irq", target_arch = "x86_64"))]
    #[test]
    fn test_new_msi_routing_multi() {
        let mut msi_fds = Vec::with_capacity(16);
        for _ in 0..16 {
            msi_fds.push(InterruptSourceConfig::MsiIrq(MsiIrqSourceConfig {
                high_addr: 0x1234,
                low_addr: 0x5678,
                data: 0x9876,
                msg_ctl: 0,
                device_id: None,
            }));
        }
        let mut legacy_fds = Vec::with_capacity(16);
        for _ in 0..16 {
            legacy_fds.push(InterruptSourceConfig::LegacyIrq(LegacyIrqSourceConfig {}));
        }

        let base = 0;
        let entrys = create_msi_routing_entries(0, &msi_fds).unwrap();

        for (i, entry) in entrys.iter().enumerate() {
            assert_eq!(entry.gsi, (base + i) as u32);
            assert_eq!(entry.type_, KVM_IRQ_ROUTING_MSI);
            if let InterruptSourceConfig::MsiIrq(config) = &msi_fds[i] {
                unsafe {
                    assert_eq!(entry.u.msi.address_hi, config.high_addr);
                    assert_eq!(entry.u.msi.address_lo, config.low_addr);
                    assert_eq!(entry.u.msi.data, config.data);
                }
            }
        }

        assert!(create_msi_routing_entries(0, &legacy_fds).is_err());
        assert!(create_msi_routing_entries(!0, &msi_fds).is_err());
    }
}
