// Copyright (C) 2019-2020 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Interrupt manager to manage and switch device interrupt modes.
//!
//! A device may support multiple interrupt modes. For example, a PCI device may support legacy, PCI
//! MSI and PCI MSIx interrupts. This interrupt manager helps a device backend driver to manage its
//! interrupts and provides interfaces to switch interrupt working modes.
use std::io::{Error, Result};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use dbs_device::resources::DeviceResources;

#[cfg(feature = "legacy-irq")]
use super::LegacyIrqSourceConfig;
#[cfg(feature = "msi-irq")]
use super::MsiIrqSourceConfig;
use super::{InterruptManager, InterruptSourceConfig, InterruptSourceGroup, InterruptSourceType};

/// Defines the offset when device_id is recorded to msi.
///
/// For the origin of this value, please refer to the comment of set_msi_device_id function.
pub const MSI_DEVICE_ID_SHIFT: u8 = 3;

#[cfg(feature = "legacy-irq")]
const LEGACY_CONFIGS: [InterruptSourceConfig; 1] =
    [InterruptSourceConfig::LegacyIrq(LegacyIrqSourceConfig {})];

#[cfg(feature = "msi-irq")]
const MSI_INT_MASK_BIT: u8 = 0;
#[cfg(feature = "msi-irq")]
const MSI_INT_MASK: u32 = (1 << MSI_INT_MASK_BIT) as u32;

/// Device interrupt working modes.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DeviceInterruptMode {
    /// The device interrupt manager has been disabled.
    Disabled = 0,
    /// The device interrupt manager works in legacy irq mode.
    LegacyIrq = 1,
    /// The device interrupt manager works in generic MSI mode.
    GenericMsiIrq = 2,
    /// The device interrupt manager works in PCI MSI mode.
    PciMsiIrq = 3,
    /// The device interrupt manager works in PCI MSI-x mode.
    PciMsixIrq = 4,
}

/// A struct to manage interrupts and interrupt modes for a device.
///
/// The interrupt manager may support multiple working mode. For example, an interrupt manager for a
/// PCI device may work in legacy mode, PCI MSI mode or PCI MSIx mode. Under certain conditions, the
/// interrupt manager may switch between interrupt working modes. To simplify implementation,
/// switching working mode is only supported at configuration stage and will be disabled at runtime
/// stage. The DeviceInterruptManager::enable() switches the interrupt manager from configuration
/// stage into runtime stage. And DeviceInterruptManager::reset() switches from runtime stage back
/// to initial configuration stage.
pub struct DeviceInterruptManager<T: InterruptManager> {
    mode: DeviceInterruptMode,
    activated: bool,
    current_idx: usize,
    mode2idx: [usize; 5],
    intr_mgr: T,
    intr_groups: Vec<Arc<Box<dyn InterruptSourceGroup>>>,
    #[cfg(feature = "msi-irq")]
    msi_config: Vec<InterruptSourceConfig>,
    /// Device id indicate the device who triggers msi irq.
    device_id: Option<u32>,
}

impl<T: InterruptManager> DeviceInterruptManager<T> {
    /// Create an interrupt manager for a device.
    ///
    /// # Arguments
    /// * `intr_mgr`: underline interrupt manager to allocate/free interrupt groups.
    /// * `resources`: resources assigned to the device, including assigned interrupt resources.
    pub fn new(intr_mgr: T, resources: &DeviceResources) -> Result<Self> {
        let mut mgr = DeviceInterruptManager {
            mode: DeviceInterruptMode::Disabled,
            activated: false,
            current_idx: usize::MAX,
            mode2idx: [usize::MAX; 5],
            intr_mgr,
            intr_groups: Vec::new(),
            #[cfg(feature = "msi-irq")]
            msi_config: Vec::new(),
            device_id: None,
        };

        #[cfg(feature = "legacy-irq")]
        {
            if let Some(irq) = resources.get_legacy_irq() {
                let group = mgr
                    .intr_mgr
                    .create_group(InterruptSourceType::LegacyIrq, irq, 1)?;
                mgr.mode2idx[DeviceInterruptMode::LegacyIrq as usize] = mgr.intr_groups.len();
                mgr.intr_groups.push(group);
            }
        }

        #[cfg(feature = "msi-irq")]
        {
            if let Some(msi) = resources.get_generic_msi_irqs() {
                let group = mgr
                    .intr_mgr
                    .create_group(InterruptSourceType::MsiIrq, msi.0, msi.1)?;
                mgr.resize_msi_config_space(group.len());
                mgr.mode2idx[DeviceInterruptMode::GenericMsiIrq as usize] = mgr.intr_groups.len();
                mgr.intr_groups.push(group);
            }

            if let Some(msi) = resources.get_pci_msi_irqs() {
                let group = mgr
                    .intr_mgr
                    .create_group(InterruptSourceType::MsiIrq, msi.0, msi.1)?;
                mgr.resize_msi_config_space(group.len());
                mgr.mode2idx[DeviceInterruptMode::PciMsiIrq as usize] = mgr.intr_groups.len();
                mgr.intr_groups.push(group);
            }

            if let Some(msi) = resources.get_pci_msix_irqs() {
                let group = mgr
                    .intr_mgr
                    .create_group(InterruptSourceType::MsiIrq, msi.0, msi.1)?;
                mgr.resize_msi_config_space(group.len());
                mgr.mode2idx[DeviceInterruptMode::PciMsixIrq as usize] = mgr.intr_groups.len();
                mgr.intr_groups.push(group);
            }
        }

        Ok(mgr)
    }

    /// Set device_id for MSI routing
    pub fn set_device_id(&mut self, device_id: Option<u32>) {
        self.device_id = device_id;
    }

    /// Check whether the interrupt manager has been activated.
    pub fn is_enabled(&self) -> bool {
        self.activated
    }

    /// Switch the interrupt manager from configuration stage into runtime stage.
    ///
    /// The working mode could only be changed at configuration stage, and all requests to change
    /// working mode at runtime stage will be rejected.
    ///
    /// If the interrupt manager is still in DISABLED mode when DeviceInterruptManager::enable() is
    /// called, it will be put into LEGACY mode if LEGACY mode is supported.
    pub fn enable(&mut self) -> Result<()> {
        if self.activated {
            return Ok(());
        }

        // Enter Legacy mode by default if Legacy mode is supported.
        if self.mode == DeviceInterruptMode::Disabled
            && self.mode2idx[DeviceInterruptMode::LegacyIrq as usize] != usize::MAX
        {
            self.set_working_mode(DeviceInterruptMode::LegacyIrq)?;
        }
        if self.mode == DeviceInterruptMode::Disabled {
            return Err(Error::from_raw_os_error(libc::EINVAL));
        }

        self.intr_groups[self.current_idx].enable(self.get_configs(self.mode))?;
        self.activated = true;

        Ok(())
    }

    /// Switch the interrupt manager from runtime stage back into initial configuration stage.
    ///
    /// Currently we doesn't track the usage of interrupt group object given out by `get_group()`,
    /// so the the caller needs to take the responsibility to release all interrupt group object
    /// reference before calling DeviceInterruptManager::reset().
    pub fn reset(&mut self) -> Result<()> {
        if self.activated {
            self.activated = false;
            self.intr_groups[self.current_idx].disable()?;
        }
        self.set_working_mode(DeviceInterruptMode::Disabled)?;

        Ok(())
    }

    /// Get the current interrupt working mode.
    pub fn get_working_mode(&mut self) -> DeviceInterruptMode {
        self.mode
    }

    /// Switch interrupt working mode.
    ///
    /// Currently switching working mode is only supported during device configuration stage and
    /// will always return failure if called during device runtime stage. The device switches from
    /// configuration stage to runtime stage by invoking `DeviceInterruptManager::enable()`. With
    /// this constraint, the device drivers may call `DeviceInterruptManager::get_group()` to get
    /// the underline active interrupt group object, and directly calls the interrupt group object's
    /// methods to trigger/acknowledge interrupts.
    ///
    /// This is a key design decision for optimizing performance. Though the DeviceInterruptManager
    /// object itself is not multi-thread safe and must be protected from concurrent access by the
    /// caller, the interrupt source group object is multi-thread safe and could be called
    /// concurrently to trigger/acknowledge interrupts. This design may help to improve performance
    /// for MSI interrupts.
    ///
    /// # Arguments
    /// * `mode`: target working mode.
    pub fn set_working_mode(&mut self, mode: DeviceInterruptMode) -> Result<()> {
        // Can't switch mode agian once enabled.
        if self.activated {
            return Err(Error::from_raw_os_error(libc::EINVAL));
        }

        if mode != self.mode {
            // Supported state transitions:
            // - other state -> DISABLED
            // - DISABLED -> other
            // - non-legacy -> legacy
            // - legacy -> non-legacy
            if self.mode != DeviceInterruptMode::Disabled
                && self.mode != DeviceInterruptMode::LegacyIrq
                && mode != DeviceInterruptMode::LegacyIrq
                && mode != DeviceInterruptMode::Disabled
            {
                return Err(Error::from_raw_os_error(libc::EINVAL));
            }

            // Then enter new state
            if mode != DeviceInterruptMode::Disabled {
                self.current_idx = self.mode2idx[mode as usize];
            } else {
                // We should reset irq configs when disable interrupt
                self.reset_configs(mode);
            }
            self.mode = mode;
        }

        Ok(())
    }

    /// Get the underline interrupt source group object, so the device driver could concurrently
    /// trigger/acknowledge interrupts by using the returned group object.
    pub fn get_group(&self) -> Option<Arc<Box<dyn InterruptSourceGroup>>> {
        if !self.activated || self.mode == DeviceInterruptMode::Disabled {
            None
        } else {
            Some(self.intr_groups[self.current_idx].clone())
        }
    }

    /// Get the underline interrupt source group object, ignore the mode
    pub fn get_group_unchecked(&self) -> Arc<Box<dyn InterruptSourceGroup>> {
        self.intr_groups[self.current_idx].clone()
    }

    /// Reconfigure a specific interrupt in current working mode at configuration or runtime stage.
    ///
    /// It's mainly used to reconfigure Generic MSI/PCI MSI/PCI MSIx interrupts. Actually legacy
    /// interrupts don't support reconfiguration yet.
    #[allow(unused_variables)]
    pub fn update(&mut self, index: u32) -> Result<()> {
        if !self.activated {
            return Err(Error::from_raw_os_error(libc::EINVAL));
        }

        match self.mode {
            #[cfg(feature = "msi-irq")]
            DeviceInterruptMode::GenericMsiIrq
            | DeviceInterruptMode::PciMsiIrq
            | DeviceInterruptMode::PciMsixIrq => {
                let group = &self.intr_groups[self.current_idx];
                if index >= group.len() || index >= self.msi_config.len() as u32 {
                    return Err(Error::from_raw_os_error(libc::EINVAL));
                }
                group.update(index, &self.msi_config[index as usize])?;
                Ok(())
            }
            _ => Err(Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    fn get_configs(&self, mode: DeviceInterruptMode) -> &[InterruptSourceConfig] {
        match mode {
            #[cfg(feature = "legacy-irq")]
            DeviceInterruptMode::LegacyIrq => &LEGACY_CONFIGS[..],
            #[cfg(feature = "msi-irq")]
            DeviceInterruptMode::GenericMsiIrq
            | DeviceInterruptMode::PciMsiIrq
            | DeviceInterruptMode::PciMsixIrq => {
                let idx = self.mode2idx[mode as usize];
                let group_len = self.intr_groups[idx].len() as usize;
                &self.msi_config[0..group_len]
            }
            _ => panic!("unhandled interrupt type in get_configs()"),
        }
    }

    fn reset_configs(&mut self, mode: DeviceInterruptMode) {
        match mode {
            #[cfg(feature = "msi-irq")]
            DeviceInterruptMode::GenericMsiIrq
            | DeviceInterruptMode::PciMsiIrq
            | DeviceInterruptMode::PciMsixIrq => {
                self.msi_config = vec![
                    InterruptSourceConfig::MsiIrq(MsiIrqSourceConfig::default());
                    self.msi_config.len()
                ];
            }
            _ => {}
        }
    }
}

#[cfg(feature = "msi-irq")]
impl<T: InterruptManager> DeviceInterruptManager<T> {
    /// Set the high address for a MSI message.
    #[allow(irrefutable_let_patterns)]
    pub fn set_msi_high_address(&mut self, index: u32, data: u32) -> Result<()> {
        if (index as usize) < self.msi_config.len() {
            if let InterruptSourceConfig::MsiIrq(ref mut msi) = self.msi_config[index as usize] {
                msi.high_addr = data;
                return Ok(());
            }
        }
        Err(Error::from_raw_os_error(libc::EINVAL))
    }

    /// Set the low address for a MSI message.
    #[allow(irrefutable_let_patterns)]
    pub fn set_msi_low_address(&mut self, index: u32, data: u32) -> Result<()> {
        if (index as usize) < self.msi_config.len() {
            if let InterruptSourceConfig::MsiIrq(ref mut msi) = self.msi_config[index as usize] {
                msi.low_addr = data;
                return Ok(());
            }
        }
        Err(Error::from_raw_os_error(libc::EINVAL))
    }

    /// Set the data for a MSI message.
    #[allow(irrefutable_let_patterns)]
    pub fn set_msi_data(&mut self, index: u32, data: u32) -> Result<()> {
        if (index as usize) < self.msi_config.len() {
            if let InterruptSourceConfig::MsiIrq(ref mut msi) = self.msi_config[index as usize] {
                msi.data = data;
                return Ok(());
            }
        }
        Err(Error::from_raw_os_error(libc::EINVAL))
    }

    /// Set msi irq MASK bit
    #[allow(irrefutable_let_patterns)]
    pub fn set_msi_mask(&mut self, index: u32, mask: bool) -> Result<()> {
        if (index as usize) < self.msi_config.len() {
            if let InterruptSourceConfig::MsiIrq(ref mut msi) = self.msi_config[index as usize] {
                let mut msg_ctl = msi.msg_ctl;
                msg_ctl &= !MSI_INT_MASK;
                if mask {
                    msg_ctl |= MSI_INT_MASK;
                }
                msi.msg_ctl = msg_ctl;
                return Ok(());
            }
        }
        Err(Error::from_raw_os_error(libc::EINVAL))
    }

    /// Get msi irq MASK state
    #[allow(irrefutable_let_patterns)]
    pub fn get_msi_mask(&self, index: u32) -> Result<bool> {
        if (index as usize) < self.msi_config.len() {
            if let InterruptSourceConfig::MsiIrq(ref msi) = self.msi_config[index as usize] {
                return Ok((msi.msg_ctl & MSI_INT_MASK) == MSI_INT_MASK);
            }
        }
        Err(Error::from_raw_os_error(libc::EINVAL))
    }

    #[cfg(target_arch = "aarch64")]
    /// Set the device id for a MSI irq
    pub fn set_msi_device_id(&mut self, index: u32) -> Result<()> {
        if (index as usize) < self.msi_config.len() {
            if let InterruptSourceConfig::MsiIrq(ref mut msi) = self.msi_config[index as usize] {
                msi.device_id = self.device_id.map(|dev_id| {
                    // An pci device attach to ITS will have a new device id which is use for msi
                    // irq routing.  It is calculated according to kernel function PCI_DEVID(),
                    // new_dev_id = (bus << 8) | devfn. In addition, devfn = device_id << 3,
                    // according to pci-host-ecam-generic's spec, and we implement bus = 0.
                    dev_id << MSI_DEVICE_ID_SHIFT
                });
                return Ok(());
            }
        }
        Err(Error::from_raw_os_error(libc::EINVAL))
    }

    fn resize_msi_config_space(&mut self, size: u32) {
        if self.msi_config.len() < size as usize {
            self.msi_config =
                vec![InterruptSourceConfig::MsiIrq(MsiIrqSourceConfig::default()); size as usize];
        }
    }
}

/// Struct to implement a 32-bit interrupt status register.
#[derive(Default, Debug)]
pub struct InterruptStatusRegister32 {
    status: AtomicU32,
}

impl InterruptStatusRegister32 {
    /// Create a status register instance.
    pub fn new() -> Self {
        InterruptStatusRegister32 {
            status: AtomicU32::new(0),
        }
    }

    /// Read current value of the status register.
    pub fn read(&self) -> u32 {
        self.status.load(Ordering::SeqCst)
    }

    /// Write value to the status register.
    pub fn write(&self, value: u32) {
        self.status.store(value, Ordering::SeqCst);
    }

    /// Read current value and reset the status register to 0.
    pub fn read_and_clear(&self) -> u32 {
        self.status.swap(0, Ordering::SeqCst)
    }

    /// Set bits into `value`.
    pub fn set_bits(&self, value: u32) {
        self.status.fetch_or(value, Ordering::SeqCst);
    }

    /// Clear bits present in `value`.
    pub fn clear_bits(&self, value: u32) {
        self.status.fetch_and(!value, Ordering::SeqCst);
    }
}

#[cfg(all(test, feature = "kvm-legacy-irq", feature = "kvm-msi-irq"))]
pub(crate) mod tests {
    use std::sync::Arc;

    use dbs_device::resources::{DeviceResources, MsiIrqType, Resource};
    use kvm_ioctls::{Kvm, VmFd};

    use super::*;
    use crate::KvmIrqManager;

    pub(crate) fn create_vm_fd() -> VmFd {
        let kvm = Kvm::new().unwrap();
        kvm.create_vm().unwrap()
    }

    fn create_init_resources() -> DeviceResources {
        let mut resources = DeviceResources::new();

        resources.append(Resource::MmioAddressRange {
            base: 0xd000_0000,
            size: 0x10_0000,
        });
        resources.append(Resource::LegacyIrq(0));
        resources.append(Resource::MsiIrq {
            ty: MsiIrqType::GenericMsi,
            base: 0x200,
            size: 0x10,
        });
        resources.append(Resource::MsiIrq {
            ty: MsiIrqType::PciMsi,
            base: 0x100,
            size: 0x20,
        });
        resources.append(Resource::MsiIrq {
            ty: MsiIrqType::PciMsix,
            base: 0x300,
            size: 0x30,
        });

        resources
    }

    fn create_interrupt_manager() -> DeviceInterruptManager<Arc<KvmIrqManager>> {
        let vmfd = Arc::new(create_vm_fd());
        #[cfg(target_arch = "x86_64")]
        vmfd.create_irq_chip().unwrap();
        #[cfg(target_arch = "aarch64")]
        let _ = dbs_arch::gic::create_gic(&vmfd, 1);
        let intr_mgr = Arc::new(KvmIrqManager::new(vmfd));

        let resource = create_init_resources();
        intr_mgr.initialize().unwrap();
        DeviceInterruptManager::new(intr_mgr, &resource).unwrap()
    }

    #[test]
    fn test_create_device_interrupt_manager() {
        let mut mgr = create_interrupt_manager();

        assert_eq!(mgr.mode, DeviceInterruptMode::Disabled);
        assert!(!mgr.activated);
        assert_eq!(mgr.current_idx, usize::MAX);
        assert_eq!(mgr.intr_groups.len(), 4);
        assert!(!mgr.is_enabled());
        assert!(mgr.get_group().is_none());

        // Enter legacy mode by default
        mgr.enable().unwrap();
        assert!(mgr.is_enabled());
        assert_eq!(
            mgr.mode2idx[DeviceInterruptMode::LegacyIrq as usize],
            mgr.current_idx
        );
        assert!(mgr.get_group().is_some());
        assert_eq!(
            mgr.get_group_unchecked().interrupt_type(),
            InterruptSourceType::LegacyIrq
        );

        // Disable interrupt manager
        mgr.reset().unwrap();
        assert!(!mgr.is_enabled());
        assert_eq!(
            mgr.mode2idx[DeviceInterruptMode::LegacyIrq as usize],
            mgr.current_idx
        );
        assert_eq!(mgr.get_working_mode(), DeviceInterruptMode::Disabled);
        assert!(mgr.get_group().is_none());
    }

    #[test]
    fn test_device_interrupt_manager_switch_mode() {
        let mut mgr = create_interrupt_manager();

        // Can't switch working mode in enabled state.
        mgr.enable().unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsiIrq)
            .unwrap_err();
        mgr.set_working_mode(DeviceInterruptMode::PciMsixIrq)
            .unwrap_err();
        mgr.set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap_err();
        mgr.reset().unwrap();

        // Switch from LEGACY to PciMsi mode
        mgr.set_working_mode(DeviceInterruptMode::LegacyIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::LegacyIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsiIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsiIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsixIrq)
            .unwrap_err();
        mgr.set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap_err();

        // Switch from LEGACY to PciMsix mode
        mgr.set_working_mode(DeviceInterruptMode::LegacyIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsixIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsixIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsiIrq)
            .unwrap_err();
        mgr.set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap_err();

        // Switch from LEGACY to GenericMsi mode
        mgr.set_working_mode(DeviceInterruptMode::LegacyIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsiIrq)
            .unwrap_err();
        mgr.set_working_mode(DeviceInterruptMode::PciMsixIrq)
            .unwrap_err();

        // Switch from DISABLED to PciMsi mode
        mgr.set_working_mode(DeviceInterruptMode::Disabled).unwrap();
        mgr.set_working_mode(DeviceInterruptMode::Disabled).unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsiIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsixIrq)
            .unwrap_err();
        mgr.set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap_err();

        // Switch from DISABLED to PciMsix mode
        mgr.set_working_mode(DeviceInterruptMode::Disabled).unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsixIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsiIrq)
            .unwrap_err();
        mgr.set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap_err();

        // Switch from DISABLED to GenericMsi mode
        mgr.set_working_mode(DeviceInterruptMode::Disabled).unwrap();
        mgr.set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap();
        mgr.set_working_mode(DeviceInterruptMode::PciMsiIrq)
            .unwrap_err();
        mgr.set_working_mode(DeviceInterruptMode::PciMsixIrq)
            .unwrap_err();

        mgr.set_working_mode(DeviceInterruptMode::Disabled).unwrap();
        mgr.set_working_mode(DeviceInterruptMode::Disabled).unwrap();
    }

    #[test]
    fn test_msi_config() {
        let mut interrupt_manager = create_interrupt_manager();

        assert!(interrupt_manager.set_msi_data(512, 0).is_err());
        interrupt_manager.set_msi_data(0, 0).unwrap();
        assert!(interrupt_manager.set_msi_high_address(512, 0).is_err());
        interrupt_manager.set_msi_high_address(0, 0).unwrap();
        assert!(interrupt_manager.set_msi_low_address(512, 0).is_err());
        interrupt_manager.set_msi_low_address(0, 0).unwrap();
        assert!(interrupt_manager.get_msi_mask(512).is_err());
        assert!(!interrupt_manager.get_msi_mask(0).unwrap());
        assert!(interrupt_manager.set_msi_mask(512, true).is_err());
        interrupt_manager.set_msi_mask(0, true).unwrap();
        assert!(interrupt_manager.get_msi_mask(0).unwrap());
    }

    #[test]
    fn test_set_working_mode_after_activated() {
        let mut interrupt_manager = create_interrupt_manager();
        interrupt_manager.activated = true;
        assert!(interrupt_manager
            .set_working_mode(DeviceInterruptMode::Disabled)
            .is_err());
        assert!(interrupt_manager
            .set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .is_err());
        assert!(interrupt_manager
            .set_working_mode(DeviceInterruptMode::LegacyIrq)
            .is_err());
        assert!(interrupt_manager
            .set_working_mode(DeviceInterruptMode::PciMsiIrq)
            .is_err());
        assert!(interrupt_manager
            .set_working_mode(DeviceInterruptMode::PciMsixIrq)
            .is_err());
    }

    #[test]
    fn test_disable2legacy() {
        let mut interrupt_manager = create_interrupt_manager();
        interrupt_manager.activated = false;
        interrupt_manager.mode = DeviceInterruptMode::Disabled;
        interrupt_manager
            .set_working_mode(DeviceInterruptMode::LegacyIrq)
            .unwrap();
    }

    #[test]
    fn test_disable2nonlegacy() {
        let mut interrupt_manager = create_interrupt_manager();
        interrupt_manager.activated = false;
        interrupt_manager.mode = DeviceInterruptMode::Disabled;
        interrupt_manager
            .set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap();
    }

    #[test]
    fn test_legacy2nonlegacy() {
        let mut interrupt_manager = create_interrupt_manager();
        interrupt_manager.activated = false;
        interrupt_manager.mode = DeviceInterruptMode::Disabled;
        interrupt_manager
            .set_working_mode(DeviceInterruptMode::LegacyIrq)
            .unwrap();
        interrupt_manager
            .set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap();
    }

    #[test]
    fn test_nonlegacy2legacy() {
        let mut interrupt_manager = create_interrupt_manager();
        interrupt_manager.activated = false;
        interrupt_manager.mode = DeviceInterruptMode::Disabled;
        interrupt_manager
            .set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap();
        interrupt_manager
            .set_working_mode(DeviceInterruptMode::LegacyIrq)
            .unwrap();
    }

    #[test]
    fn test_update() {
        let mut interrupt_manager = create_interrupt_manager();
        interrupt_manager
            .set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .unwrap();
        interrupt_manager.enable().unwrap();
        assert!(interrupt_manager.update(0x10).is_err());
        interrupt_manager.update(0x01).unwrap();
        interrupt_manager.reset().unwrap();
        interrupt_manager
            .set_working_mode(DeviceInterruptMode::LegacyIrq)
            .unwrap();
        assert!(interrupt_manager.update(0x10).is_err());
    }

    #[test]
    fn test_get_configs() {
        // legacy irq config
        {
            let interrupt_manager = create_interrupt_manager();

            let legacy_config = interrupt_manager.get_configs(DeviceInterruptMode::LegacyIrq);
            assert_eq!(legacy_config, LEGACY_CONFIGS);
        }

        // generic irq config
        {
            let mut interrupt_manager = create_interrupt_manager();
            interrupt_manager
                .set_working_mode(DeviceInterruptMode::GenericMsiIrq)
                .unwrap();
            let msi_config = interrupt_manager.get_configs(DeviceInterruptMode::GenericMsiIrq);
            assert_eq!(msi_config.len(), 0x10);
        }

        // msi irq config
        {
            let mut interrupt_manager = create_interrupt_manager();
            interrupt_manager
                .set_working_mode(DeviceInterruptMode::PciMsiIrq)
                .unwrap();
            let msi_config = interrupt_manager.get_configs(DeviceInterruptMode::PciMsiIrq);
            assert_eq!(msi_config.len(), 0x20);
        }

        // msix irq config
        {
            let mut interrupt_manager = create_interrupt_manager();
            interrupt_manager
                .set_working_mode(DeviceInterruptMode::PciMsixIrq)
                .unwrap();
            let msi_config = interrupt_manager.get_configs(DeviceInterruptMode::PciMsixIrq);
            assert_eq!(msi_config.len(), 0x30);
        }
    }

    #[test]
    fn test_reset_configs() {
        let mut interrupt_manager = create_interrupt_manager();

        interrupt_manager.reset_configs(DeviceInterruptMode::LegacyIrq);
        interrupt_manager.reset_configs(DeviceInterruptMode::LegacyIrq);

        interrupt_manager.set_msi_data(0, 100).unwrap();
        interrupt_manager.set_msi_high_address(0, 200).unwrap();
        interrupt_manager.set_msi_low_address(0, 300).unwrap();

        interrupt_manager.reset_configs(DeviceInterruptMode::GenericMsiIrq);
        assert_eq!(
            interrupt_manager.msi_config[0],
            InterruptSourceConfig::MsiIrq(MsiIrqSourceConfig::default())
        );
    }

    #[test]
    fn test_interrupt_status_register() {
        let status = InterruptStatusRegister32::new();

        assert_eq!(status.read(), 0);
        status.write(0x13);
        assert_eq!(status.read(), 0x13);
        status.clear_bits(0x11);
        assert_eq!(status.read(), 0x2);
        status.set_bits(0x100);
        assert_eq!(status.read_and_clear(), 0x102);
        assert_eq!(status.read(), 0);
    }
}
