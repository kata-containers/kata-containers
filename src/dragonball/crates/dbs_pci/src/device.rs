// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

#[cfg(target_arch = "aarch64")]
use dbs_device::resources::DeviceResources;
use dbs_device::DeviceIo;

/// Define PCI ECAM space length
#[cfg(target_arch = "aarch64")]
pub const ECAM_SPACE_LENGTH: u64 = 0x100000;

/// PCI bus resources are used to create pci bus fdt node
#[cfg(target_arch = "aarch64")]
pub struct PciBusResources {
    /// Save the ecam space, it only contain one mmio address space
    pub ecam_space: DeviceResources,
    /// Save the bar space, it contains 2 mmio address space
    pub bar_space: DeviceResources,
}

pub trait PciDevice: DeviceIo + Send + Sync {
    /// Get PCI device/function id on the PCI bus, which is in [0x0, 0xff].
    ///
    /// The higher 5 bits are device id and the lower 3 bits are function id.
    fn id(&self) -> u8;

    /// Write to the PCI device's configuration space.
    fn write_config(&self, offset: u32, data: &[u8]);

    /// Read from the PCI device's configuration space.
    fn read_config(&self, offset: u32, data: &mut [u8]);
}

impl PartialEq for dyn PciDevice {
    fn eq(&self, other: &dyn PciDevice) -> bool {
        self.id() == other.id()
    }
}
