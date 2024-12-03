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
