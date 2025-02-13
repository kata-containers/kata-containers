// Copyright 2022 Alibaba Cloud. All Rights Reserved.
// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

/// Export gicv2 interface
pub mod gicv2;
/// Export gicv3 interface
pub mod gicv3;
/// Export ITS interface
pub mod its;

use std::{boxed::Box, result};

use kvm_ioctls::{DeviceFd, VmFd};

use gicv2::GICv2;
use gicv3::GICv3;

// As per virt/kvm/arm/vgic/vgic-kvm-device.c we need
// the number of interrupts our GIC will support to be:
// * bigger than 32
// * less than 1023 and
// * a multiple of 32.
// We are setting up our interrupt controller to support a maximum of 128 interrupts.

/// First usable interrupt on aarch64.
pub const IRQ_BASE: u32 = 32;

/// Last usable interrupt on aarch64.
pub const IRQ_MAX: u32 = 159;

/// Define the gic register end address.
pub const GIC_REG_END_ADDRESS: u64 = 1 << 30; // 1GB

/// Errors thrown while setting up the GIC.
#[derive(Debug)]
pub enum Error {
    /// Error while calling KVM ioctl for setting up the global interrupt controller.
    CreateGIC(kvm_ioctls::Error),
    /// Error while setting device attributes for the GIC.
    SetDeviceAttribute(kvm_ioctls::Error),
    /// The number of vCPUs in the GicState doesn't match the number of vCPUs on the system
    InconsistentVcpuCount,
    /// The VgicSysRegsState is invalid
    InvalidVgicSysRegState,
    /// ERROR while create ITS fail
    CreateITS(kvm_ioctls::Error),
    /// ERROR while set ITS attr fail
    SetITSAttribute(kvm_ioctls::Error),
}
type Result<T> = result::Result<T, Error>;

/// Function that flushes `RDIST` pending tables into guest RAM.
///
/// The tables get flushed to guest RAM whenever the VM gets stopped.
pub fn save_pending_tables(fd: &DeviceFd) -> Result<()> {
    let init_gic_attr = kvm_bindings::kvm_device_attr {
        group: kvm_bindings::KVM_DEV_ARM_VGIC_GRP_CTRL,
        attr: u64::from(kvm_bindings::KVM_DEV_ARM_VGIC_SAVE_PENDING_TABLES),
        addr: 0,
        flags: 0,
    };
    fd.set_device_attr(&init_gic_attr)
        .map_err(Error::SetDeviceAttribute)
}

/// Trait for GIC devices.
pub trait GICDevice: Send {
    /// Returns the file descriptor of the GIC device
    fn device_fd(&self) -> &DeviceFd;

    /// Returns an array with GIC device properties
    fn device_properties(&self) -> &[u64];

    /// Returns the number of vCPUs this GIC handles
    fn vcpu_count(&self) -> u64;

    /// Returns the fdt compatibility property of the device
    fn fdt_compatibility(&self) -> &str;

    /// Returns the maint_irq fdt property of the device
    fn fdt_maint_irq(&self) -> u32;

    /// Get ITS reg range
    fn get_its_reg_range(&self, _its_type: &its::ItsType) -> Option<[u64; 2]> {
        None
    }

    /// Only gic-v3 has its
    fn attach_its(&mut self, _vm: &VmFd) -> Result<()> {
        Ok(())
    }

    /// Returns the GIC version of the device
    fn version() -> u32
    where
        Self: Sized;

    /// Create the GIC device object
    fn create_device(fd: DeviceFd, vcpu_count: u64) -> Box<dyn GICDevice>
    where
        Self: Sized;

    /// Setup the device-specific attributes
    fn init_device_attributes(gic_device: &dyn GICDevice) -> Result<()>
    where
        Self: Sized;

    /// Initialize a GIC device
    fn init_device(vm: &VmFd) -> Result<DeviceFd>
    where
        Self: Sized,
    {
        let mut gic_device = kvm_bindings::kvm_create_device {
            type_: Self::version(),
            fd: 0,
            flags: 0,
        };

        vm.create_device(&mut gic_device).map_err(Error::CreateGIC)
    }

    /// Set a GIC device attribute
    fn set_device_attribute(
        fd: &DeviceFd,
        group: u32,
        attr: u64,
        addr: u64,
        flags: u32,
    ) -> Result<()>
    where
        Self: Sized,
    {
        let attr = kvm_bindings::kvm_device_attr {
            group,
            attr,
            addr,
            flags,
        };
        fd.set_device_attr(&attr)
            .map_err(Error::SetDeviceAttribute)?;

        Ok(())
    }

    /// Finalize the setup of a GIC device
    fn finalize_device(gic_device: &dyn GICDevice) -> Result<()>
    where
        Self: Sized,
    {
        /* We need to tell the kernel how many irqs to support with this vgic.
         * See the `layout` module for details.
         */
        let nr_irqs: u32 = IRQ_MAX - IRQ_BASE + 1;
        let nr_irqs_ptr = &nr_irqs as *const u32;
        Self::set_device_attribute(
            gic_device.device_fd(),
            kvm_bindings::KVM_DEV_ARM_VGIC_GRP_NR_IRQS,
            0,
            nr_irqs_ptr as u64,
            0,
        )?;

        /* Finalize the GIC.
         * See https://code.woboq.org/linux/linux/virt/kvm/arm/vgic/vgic-kvm-device.c.html#211.
         */
        Self::set_device_attribute(
            gic_device.device_fd(),
            kvm_bindings::KVM_DEV_ARM_VGIC_GRP_CTRL,
            u64::from(kvm_bindings::KVM_DEV_ARM_VGIC_CTRL_INIT),
            0,
            0,
        )?;

        Ok(())
    }

    #[allow(clippy::new_ret_no_self)]
    /// Method to initialize the GIC device
    fn new(vm: &VmFd, vcpu_count: u64) -> Result<Box<dyn GICDevice>>
    where
        Self: Sized,
    {
        let vgic_fd = Self::init_device(vm)?;

        let mut device = Self::create_device(vgic_fd, vcpu_count);

        device.attach_its(vm)?;

        Self::init_device_attributes(device.as_ref())?;

        Self::finalize_device(device.as_ref())?;

        Ok(device)
    }
}

/// Create a GIC device.
///
/// It will try to create by default a GICv3 device. If that fails it will try
/// to fall-back to a GICv2 device.
pub fn create_gic(vm: &VmFd, vcpu_count: u64) -> Result<Box<dyn GICDevice>> {
    GICv3::new(vm, vcpu_count).or_else(|_| GICv2::new(vm, vcpu_count))
}

#[cfg(test)]
mod tests {

    use super::*;
    use kvm_ioctls::Kvm;

    #[test]
    fn test_create_gic() {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        assert!(create_gic(&vm, 1).is_ok());
    }
}
