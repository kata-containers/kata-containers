// Copyright (c) 2024 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
#![allow(dead_code)]
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::PathBuf;

use pci_ids::{Classes, Vendors};

const PCI_DEV_DOMAIN: &str = "0000";
const PCI_CONFIG_SPACE_SZ: u64 = 256;

const UNKNOWN_DEVICE: &str = "UNKNOWN_DEVICE";
const UNKNOWN_CLASS: &str = "UNKNOWN_CLASS";

const PCI_IOV_NUM_BAR: usize = 6;
const PCI_BASE_ADDRESS_MEM_TYPE_MASK: u64 = 0x06;

pub(crate) const PCI_BASE_ADDRESS_MEM_TYPE32: u64 = 0x00; // 32 bit address
pub(crate) const PCI_BASE_ADDRESS_MEM_TYPE64: u64 = 0x04; // 64 bit address

fn address_to_id(address: &str) -> u64 {
    let cleaned_address = address.replace(":", "").replace(".", "");
    u64::from_str_radix(&cleaned_address, 16).unwrap_or(0)
}

// Calculate the next power of 2.
fn calc_next_power_of_2(mut n: u64) -> u64 {
    if n < 1 {
        return 1_u64;
    }

    n -= 1;
    n |= n >> 1;
    n |= n >> 2;
    n |= n >> 4;
    n |= n >> 8;
    n |= n >> 16;
    n |= n >> 32;
    n + 1
}

#[derive(Clone, Debug, Default)]
pub(crate) struct MemoryResource {
    pub(crate) start: u64,
    pub(crate) end: u64,
    pub(crate) flags: u64,
    pub(crate) path: PathBuf,
}

pub(crate) type MemoryResources = HashMap<usize, MemoryResource>;

pub(crate) trait MemoryResourceTrait {
    fn get_total_addressable_memory(&self, round_up: bool) -> (u64, u64);
}

impl MemoryResourceTrait for MemoryResources {
    fn get_total_addressable_memory(&self, round_up: bool) -> (u64, u64) {
        let mut num_bar = 0;
        let mut mem_size_32bit = 0u64;
        let mut mem_size_64bit = 0u64;

        let mut keys: Vec<_> = self.keys().cloned().collect();
        keys.sort();

        for key in keys {
            if key as usize >= PCI_IOV_NUM_BAR || num_bar == PCI_IOV_NUM_BAR {
                break;
            }
            num_bar += 1;

            if let Some(region) = self.get(&key) {
                let flags = region.flags & PCI_BASE_ADDRESS_MEM_TYPE_MASK;
                let mem_type_32bit = flags == PCI_BASE_ADDRESS_MEM_TYPE32;
                let mem_type_64bit = flags == PCI_BASE_ADDRESS_MEM_TYPE64;
                let mem_size = (region.end - region.start + 1) as u64;

                if mem_type_32bit {
                    mem_size_32bit += mem_size;
                }
                if mem_type_64bit {
                    mem_size_64bit += mem_size;
                }
            }
        }

        if round_up {
            mem_size_32bit = calc_next_power_of_2(mem_size_32bit);
            mem_size_64bit = calc_next_power_of_2(mem_size_64bit);
        }

        (mem_size_32bit, mem_size_64bit)
    }
}

pub trait PCIDevices {
    fn get_pci_devices(&self, vendor: Option<u16>) -> Vec<PCIDevice>;
}

#[derive(Clone, Debug, Default)]
pub struct PCIDevice {
    pub(crate) device_path: PathBuf,
    pub(crate) address: String,
    pub(crate) vendor: u16,
    pub(crate) class: u32,
    pub(crate) class_name: String,
    pub(crate) device: u16,
    pub(crate) device_name: String,
    pub(crate) driver: String,
    pub(crate) iommu_group: i64,
    pub(crate) numa_node: i64,
    pub(crate) resources: MemoryResources,
}

pub struct PCIDeviceManager {
    pci_devices_root: PathBuf,
}

impl PCIDeviceManager {
    pub fn new(pci_devices_root: &str) -> Self {
        PCIDeviceManager {
            pci_devices_root: PathBuf::from(pci_devices_root),
        }
    }

    pub fn get_all_devices(&self, vendor: Option<u16>) -> io::Result<Vec<PCIDevice>> {
        let mut pci_devices = Vec::new();
        let device_dirs = fs::read_dir(&self.pci_devices_root)?;

        let mut cache: HashMap<String, PCIDevice> = HashMap::new();

        for entry in device_dirs {
            let device_dir = entry?;
            let device_address = device_dir.file_name().to_string_lossy().to_string();
            if let Ok(device) = self.get_device_by_pci_bus_id(&device_address, vendor, &mut cache) {
                if let Some(dev) = device {
                    pci_devices.push(dev);
                }
            }
        }

        pci_devices.sort_by_key(|dev| address_to_id(&dev.address));

        Ok(pci_devices)
    }

    fn get_device_by_pci_bus_id(
        &self,
        address: &str,
        vendor: Option<u16>,
        cache: &mut HashMap<String, PCIDevice>,
    ) -> io::Result<Option<PCIDevice>> {
        if let Some(device) = cache.get(address) {
            return Ok(Some(device.clone()));
        }

        let device_path = self.pci_devices_root.join(address);

        // read vendor ID
        let vendor_str = fs::read_to_string(device_path.join("vendor"))?;
        let vendor_id = u16::from_str_radix(vendor_str.trim().trim_start_matches("0x"), 16)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if let Some(vend_id) = vendor {
            if vendor_id != vend_id {
                return Ok(None);
            }
        }

        let class_str = fs::read_to_string(device_path.join("class"))?;
        let class_id = u32::from_str_radix(class_str.trim().trim_start_matches("0x"), 16).unwrap();

        let device_str = fs::read_to_string(device_path.join("device"))?;
        let device_id =
            u16::from_str_radix(device_str.trim().trim_start_matches("0x"), 16).unwrap();

        let driver = match fs::read_link(device_path.join("driver")) {
            Ok(path) => path.file_name().unwrap().to_string_lossy().to_string(),
            Err(_) => String::new(),
        };

        let iommu_group = match fs::read_link(device_path.join("iommu_group")) {
            Ok(path) => path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
                .parse::<i64>()
                .unwrap_or(-1),
            Err(_) => -1,
        };

        let numa_node = fs::read_to_string(device_path.join("numa_node"))
            .map(|numa| numa.trim().parse::<i64>().unwrap_or(-1))
            .unwrap_or(-1);

        let resources = self.parse_resources(&device_path)?;

        let mut device_name = UNKNOWN_DEVICE.to_string();
        for vendor in Vendors::iter() {
            for device in vendor.devices() {
                if vendor.id() == vendor_id && device.id() == device_id {
                    device_name = device.name().to_owned();
                    break;
                }
            }
        }

        let mut class_name = UNKNOWN_CLASS.to_string();
        for class in Classes::iter() {
            if u32::from(class.id()) == class_id {
                class_name = class.name().to_owned();
                break;
            }
        }

        let pci_device = PCIDevice {
            device_path,
            address: address.to_string(),
            vendor: vendor_id,
            class: class_id,
            device: device_id,
            driver,
            iommu_group,
            numa_node,
            resources,
            device_name,
            class_name,
        };

        cache.insert(address.to_string(), pci_device.clone());

        Ok(Some(pci_device))
    }

    fn parse_resources(&self, device_path: &PathBuf) -> io::Result<MemoryResources> {
        let content = fs::read_to_string(device_path.join("resource"))?;
        let mut resources: MemoryResources = MemoryResources::new();
        for (i, line) in content.lines().enumerate() {
            let values: Vec<&str> = line.split_whitespace().collect();
            if values.len() != 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("there's more than 3 entries in line '{}'", i),
                ));
            }

            let mem_start = u64::from_str_radix(values[0].trim_start_matches("0x"), 16)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let mem_end = u64::from_str_radix(values[1].trim_start_matches("0x"), 16)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let mem_flags = u64::from_str_radix(values[2].trim_start_matches("0x"), 16)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

            if mem_end > mem_start {
                resources.insert(
                    i,
                    MemoryResource {
                        start: mem_start,
                        end: mem_end,
                        flags: mem_flags,
                        path: device_path.join(format!("resource{}", i)),
                    },
                );
            }
        }

        Ok(resources)
    }
}

/// Checks if the given BDF corresponds to a PCIe device.
/// The sysbus_pci_root is the path "/sys/bus/pci/devices"
pub fn is_pcie_device(bdf: &str, sysbus_pci_root: &str) -> bool {
    let bdf_with_domain = if bdf.split(':').count() == 2 {
        format!("{}:{}", PCI_DEV_DOMAIN, bdf)
    } else {
        bdf.to_string()
    };

    let config_path = PathBuf::from(sysbus_pci_root)
        .join(bdf_with_domain)
        .join("config");

    match fs::metadata(config_path) {
        Ok(metadata) => metadata.len() > PCI_CONFIG_SPACE_SZ,
        // Error reading the file, assume it's not a PCIe device
        Err(_) => false,
    }
}
