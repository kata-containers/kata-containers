// Copyright 2022 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use kvm_ioctls::{DeviceFd, VmFd};

use super::gicv3::GICv3;
use super::{Error, GICDevice, Result};

// ITS register range
const REG_RANGE_LEN: u64 = 0x20000;

/// ITS type
#[derive(Hash, PartialEq, Eq)]
pub enum ItsType {
    /// platform msi its
    PlatformMsiIts,
    /// pci msi its
    PciMsiIts,
}

/// Only GIC-V3 can use ITS
pub struct ITS {
    /// The file descriptor for the KVM device
    fd: DeviceFd,
    reg_range: [u64; 2],
}

impl ITS {
    /// Create an ITS device
    pub fn new(vm: &VmFd, gic_ctl: &GICv3, its_type: ItsType) -> Result<ITS> {
        let fd = ITS::create_device_fd(vm)?;
        // Define the mmio space of platform msi its after the mmio space of pci msi its
        let offset = match its_type {
            ItsType::PlatformMsiIts => REG_RANGE_LEN,
            ItsType::PciMsiIts => REG_RANGE_LEN * 2,
        };
        let vcpu_count = gic_ctl.vcpu_count();
        // No document has been found to accurately describe the storage location and
        // length of the ITS register. Currently, we store the ITS register in front of
        // the redistributor register. And temporarily refer to the "arm, gic-v3-its"
        // kernel document to set the ITS register length to 0x20000.In addition,
        // reg_range is a two-tuple, representing the register base address and the
        // length of the register address space.
        let reg_range: [u64; 2] = [GICv3::get_redists_addr(vcpu_count) - offset, REG_RANGE_LEN];
        let its = ITS { fd, reg_range };
        let reg_base_addr = its.get_reg_range_base_addr();
        its.set_attribute(reg_base_addr)?;
        Ok(its)
    }

    fn create_device_fd(vm: &VmFd) -> Result<DeviceFd> {
        let mut its_device = kvm_bindings::kvm_create_device {
            type_: kvm_bindings::kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_ITS,
            fd: 0,
            flags: 0,
        };
        vm.create_device(&mut its_device).map_err(Error::CreateITS)
    }

    fn set_attribute(&self, reg_base_addr: u64) -> Result<()> {
        let attribute = kvm_bindings::kvm_device_attr {
            group: kvm_bindings::KVM_DEV_ARM_VGIC_GRP_ADDR,
            attr: u64::from(kvm_bindings::KVM_VGIC_ITS_ADDR_TYPE),
            addr: &reg_base_addr as *const u64 as u64,
            flags: 0,
        };
        self.fd
            .set_device_attr(&attribute)
            .map_err(Error::SetITSAttribute)?;
        Ok(())
    }

    fn get_reg_range_base_addr(&self) -> u64 {
        self.reg_range[0]
    }

    /// Get its reg range
    pub fn get_reg_range(&self) -> [u64; 2] {
        self.reg_range
    }
}
