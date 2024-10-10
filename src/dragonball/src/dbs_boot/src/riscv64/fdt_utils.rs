// Copyright 2024 Alibaba Cloud. All Rights Reserved.
// Copyright © 2024, Institute of Software, CAS. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! This module abstract some structs for constructing fdt. Instead of using
//! multiple parameters.

use std::collections::HashMap;

use dbs_arch::{aia::AIADevice, DeviceInfoForFDT, DeviceType};
use vm_memory::mmap::GuestMemoryMmap;

use crate::InitrdConfig;

/// Struct to save vcpu information
pub struct FdtVcpuInfo {
    /// number of vcpu
    vcpu_num: u32,
}

impl FdtVcpuInfo {
    /// Generate FdtVcpuInfo
    pub fn new(
        vcpu_num: u32,
    ) -> Self {
        FdtVcpuInfo {
            vcpu_num,
        }
    }
}

/// Struct to save vm information.
pub struct FdtVmInfo<'a> {
    /// guest meory
    guest_memory: &'a GuestMemoryMmap,
    /// command line
    cmdline: &'a str,
    /// initrd config
    initrd_config: Option<&'a InitrdConfig>,
    /// vcpu information
    vcpu_info: FdtVcpuInfo,
}

impl FdtVmInfo<'_> {
    /// Generate FdtVmInfo.
    pub fn new<'a>(
        guest_memory: &'a GuestMemoryMmap,
        cmdline: &'a str,
        initrd_config: Option<&'a InitrdConfig>,
        vcpu_info: FdtVcpuInfo,
    ) -> FdtVmInfo<'a> {
        FdtVmInfo {
            guest_memory,
            cmdline,
            initrd_config,
            vcpu_info,
        }
    }

    /// Get guest_memory.
    pub fn get_guest_memory(&self) -> &GuestMemoryMmap {
        self.guest_memory
    }

    /// Get cmdline.
    pub fn get_cmdline(&self) -> &str {
        self.cmdline
    }

    /// Get initrd_config.
    pub fn get_initrd_config(&self) -> Option<&InitrdConfig> {
        self.initrd_config
    }

    /// Get number of vcpu.
    pub fn get_vcpu_num(&self) -> u32 {
        self.vcpu_info.vcpu_num
    }
}

/// Struct to save device information.
pub struct FdtDeviceInfo<'a, T: DeviceInfoForFDT> {
    /// mmio device information
    mmio_device_info: Option<&'a HashMap<(DeviceType, String), T>>,
    /// interrupt controller
    irq_chip: &'a dyn AIADevice,
}

impl<T: DeviceInfoForFDT> FdtDeviceInfo<'_, T> {
    /// Generate FdtDeviceInfo.
    pub fn new<'a>(
        mmio_device_info: Option<&'a HashMap<(DeviceType, String), T>>,
        irq_chip: &'a dyn AIADevice,
    ) -> FdtDeviceInfo<'a, T> {
        FdtDeviceInfo {
            mmio_device_info,
            irq_chip,
        }
    }

    /// Get mmio device information.
    pub fn get_mmio_device_info(&self) -> Option<&HashMap<(DeviceType, String), T>> {
        self.mmio_device_info
    }

    /// Get interrupt controller.
    pub fn get_irqchip(&self) -> &dyn AIADevice {
        self.irq_chip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use dbs_arch::aia::create_aia;
    use vm_memory::{GuestAddress, GuestMemory};

    const CMDLINE: &str = "console=tty0";
    const INITRD_CONFIG: InitrdConfig = InitrdConfig {
        address: GuestAddress(0x10000000),
        size: 0x1000,
    };
    const VCPU_NUM: u32 = 1;

    #[inline]
    fn helper_generate_fdt_vm_info(guest_memory: &GuestMemoryMmap) -> FdtVmInfo<'_> {
        FdtVmInfo::new(
            guest_memory,
            CMDLINE,
            Some(&INITRD_CONFIG),
            FdtVcpuInfo::new(
                VCPU_NUM
            ),
        )
    }

    #[test]
    fn test_fdtutils_fdt_vm_info() {
        let ranges = vec![(GuestAddress(0x80000000), 0x40000)];
        let guest_memory: GuestMemoryMmap<()> =
            GuestMemoryMmap::<()>::from_ranges(ranges.as_slice())
                .expect("Cannot initialize memory");
        let vm_info = helper_generate_fdt_vm_info(&guest_memory);

        assert_eq!(
            guest_memory.check_address(GuestAddress(0x80001000)),
            Some(GuestAddress(0x80001000))
        );
        assert_eq!(guest_memory.check_address(GuestAddress(0x80050000)), None);
        assert!(guest_memory.check_range(GuestAddress(0x80000000), 0x40000));
        assert_eq!(vm_info.get_cmdline(), CMDLINE);
        assert_eq!(
            vm_info.get_initrd_config().unwrap().address,
            INITRD_CONFIG.address
        );
        assert_eq!(
            vm_info.get_initrd_config().unwrap().size,
            INITRD_CONFIG.size
        );
        assert_eq!(vm_info.get_vcpu_num(), VCPU_NUM);
    }
}
