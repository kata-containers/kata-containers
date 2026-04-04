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

fn address_to_id(address: &str) -> u64 {
    let cleaned_address = address.replace(":", "").replace(".", "");
    u64::from_str_radix(&cleaned_address, 16).unwrap_or(0)
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
            if let Ok(Some(dev)) =
                self.get_device_by_pci_bus_id(&device_address, vendor, &mut cache)
            {
                pci_devices.push(dev);
            }
        }

        pci_devices.sort_by_key(|dev| address_to_id(&dev.address));

        Ok(pci_devices)
    }

    pub fn get_device_by_pci_bus_id(
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
            device_name,
            class_name,
        };

        cache.insert(address.to_string(), pci_device.clone());

        Ok(Some(pci_device))
    }

}

/// Checks if the given BDF corresponds to a PCIe device.
/// The sysbus_pci_root is the path "/sys/bus/pci/devices"
pub fn is_pcie_device(bdf: &str, sysbus_pci_root: &str) -> bool {
    let bdf_with_domain = if bdf.split(':').count() == 2 {
        format!("{PCI_DEV_DOMAIN}:{bdf}")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;

    // domain number
    const TEST_PCI_DEV_DOMAIN: &str = "0000";

    // Mock data
    fn setup_mock_device_files() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir should not fail");
        // Create mock path and files for PCI devices
        let device_path = dir.path().join("0000:ff:1f.0");
        fs::create_dir_all(&device_path).unwrap();
        fs::write(device_path.join("vendor"), "0x8086").unwrap();
        fs::write(device_path.join("device"), "0x1234").unwrap();
        fs::write(device_path.join("class"), "0x060100").unwrap();
        fs::write(device_path.join("numa_node"), "0").unwrap();
        dir
    }

    #[test]
    fn test_get_all_devices() {
        // Setup mock data
        let tmpdir = setup_mock_device_files();

        // Initialize PCI device manager with the mock path
        let manager = PCIDeviceManager::new(&tmpdir.path().to_string_lossy());

        // Get all devices
        let devices_result = manager.get_all_devices(None);

        assert!(devices_result.is_ok());
        let devices = devices_result.unwrap();
        assert_eq!(devices.len(), 1);

        let device = &devices[0];
        assert_eq!(device.vendor, 0x8086);
        assert_eq!(device.device, 0x1234);
        assert_eq!(device.class, 0x060100);
    }

    #[test]
    fn test_is_pcie_device() {
        // Create a mock PCI device config file
        let bdf = format!("{TEST_PCI_DEV_DOMAIN}:ff:00.0");
        let tmpdir = tempfile::tempdir().expect("tempdir should not fail");
        let config_path = tmpdir.path().join(&bdf).join("config");
        let _ = fs::create_dir_all(config_path.parent().unwrap());

        // Write a file with a size larger than PCI_CONFIG_SPACE_SZ
        let mut file = fs::File::create(&config_path).unwrap();
        // Test size greater than PCI_CONFIG_SPACE_SZ
        file.write_all(&vec![0; 512]).unwrap();

        // It should be true
        assert!(is_pcie_device("ff:00.0", &tmpdir.path().to_string_lossy()));
    }
}
