// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
#![allow(dead_code)]

use super::pci_manager::{MemoryResourceTrait, PCIDevice, PCIDeviceManager, PCIDevices};

const PCI_DEVICES_ROOT: &str = "/sys/bus/pci/devices";
const PCI_NVIDIA_VENDOR_ID: u16 = 0x10DE;
const PCI3D_CONTROLLER_CLASS: u32 = 0x030200;

struct NvidiaPCIDevice {
    vendor_id: u16,
    class_id: u32,
}

impl NvidiaPCIDevice {
    pub fn new(vendor_id: u16, class_id: u32) -> Self {
        Self {
            vendor_id,
            class_id,
        }
    }

    pub fn get_bars_max_addressable_memory(&self) -> (u64, u64) {
        let mut max_32bit = 2 * 1024 * 1024;
        let mut max_64bit = 2 * 1024 * 1024;

        let nvgpu_devices = self.get_pci_devices(Some(self.vendor_id));
        for dev in nvgpu_devices {
            let (mem_size_32bit, mem_size_64bit) = dev.resources.get_total_addressable_memory(true);
            if max_32bit < mem_size_32bit {
                max_32bit = mem_size_32bit;
            }
            if max_64bit < mem_size_64bit {
                max_64bit = mem_size_64bit;
            }
        }

        (max_32bit * 2, max_64bit)
    }

    fn is_vga_controller(&self, device: &PCIDevice) -> bool {
        self.class_id == device.class
    }

    fn is_3d_controller(&self, device: &PCIDevice) -> bool {
        self.class_id == device.class
    }

    fn is_gpu(&self, device: &PCIDevice) -> bool {
        self.is_vga_controller(device) || self.is_3d_controller(device)
    }
}

impl PCIDevices for NvidiaPCIDevice {
    fn get_pci_devices(&self, vendor: Option<u16>) -> Vec<PCIDevice> {
        let mut nvidia_devices: Vec<PCIDevice> = Vec::new();
        let devices = PCIDeviceManager::new(PCI_DEVICES_ROOT)
            .get_all_devices(vendor)
            .unwrap_or_else(|_| vec![]);
        for dev in devices.iter() {
            if self.is_gpu(dev) {
                nvidia_devices.push(dev.clone());
            }
        }

        return nvidia_devices;
    }
}

pub fn get_bars_max_addressable_memory() -> (u64, u64) {
    let nvdevice = NvidiaPCIDevice::new(PCI_NVIDIA_VENDOR_ID, PCI3D_CONTROLLER_CLASS);
    let (max_32bit, max_64bit) = nvdevice.get_bars_max_addressable_memory();

    (max_32bit, max_64bit)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use super::*;
    use crate::pcilibs::pci_manager::{
        MemoryResource, MemoryResources, MockPCIDevices, PCI_BASE_ADDRESS_MEM_TYPE32,
        PCI_BASE_ADDRESS_MEM_TYPE64,
    };
    use mockall::predicate::*;

    #[test]
    fn test_get_bars_max_addressable_memory() {
        let pci_device = PCIDevice {
            device_path: PathBuf::new(),
            address: "0000:00:00.0".to_string(),
            vendor: PCI_NVIDIA_VENDOR_ID,
            class: PCI3D_CONTROLLER_CLASS,
            class_name: "3D Controller".to_string(),
            device: 0x1c82,
            device_name: "NVIDIA Device".to_string(),
            driver: "nvidia".to_string(),
            iommu_group: 0,
            numa_node: 0,
            resources: MemoryResources::default(),
        };
        let devices = vec![pci_device.clone()];

        // Mock PCI device manager and devices
        let mut mock_pci_manager = MockPCIDevices::default();
        // Setting up Mock to return a device
        mock_pci_manager
            .expect_get_pci_devices()
            .with(eq(Some(PCI_NVIDIA_VENDOR_ID)))
            .returning(move |_| devices.clone());

        // Create NvidiaPCIDevice
        let nvidia_device = NvidiaPCIDevice::new(PCI_NVIDIA_VENDOR_ID, PCI3D_CONTROLLER_CLASS);

        // Prepare memory resources
        let mut resources: MemoryResources = HashMap::new();
        // resource0 memsz = end - start => 1024
        resources.insert(
            0,
            MemoryResource {
                start: 0,
                end: 1023,
                flags: PCI_BASE_ADDRESS_MEM_TYPE32,
                path: PathBuf::from("/fake/path/resource0"),
            },
        );
        // resource1 memsz = end - start => 1024
        resources.insert(
            1,
            MemoryResource {
                start: 1024,
                end: 2047,
                flags: PCI_BASE_ADDRESS_MEM_TYPE64,
                path: PathBuf::from("/fake/path/resource1"),
            },
        );

        let pci_device_with_resources = PCIDevice {
            resources: resources.clone(),
            ..pci_device
        };

        mock_pci_manager
            .expect_get_pci_devices()
            .with(eq(Some(PCI_NVIDIA_VENDOR_ID)))
            .returning(move |_| vec![pci_device_with_resources.clone()]);

        // Call the function under test
        let (max_32bit, max_64bit) = nvidia_device.get_bars_max_addressable_memory();

        // Assert the results
        assert_eq!(max_32bit, 2 * 2 * 1024 * 1024);
        assert_eq!(max_64bit, 2 * 1024 * 1024);
    }
}
