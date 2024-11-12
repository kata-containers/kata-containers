// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

/// Emulate a PCI root bus.
///
/// A PCI root bus is a special PCI bus, who has no parent PCI bus. The device 0 on PCI root bus
/// represents the root bus itself.
///
use std::sync::{Arc, Mutex, Weak};

use dbs_device::DeviceIo;

use crate::{
    Error, PciBridgeSubclass, PciBus, PciClassCode, PciConfiguration, PciDevice, PciHeaderType,
    Result,
};

const VENDOR_ID_INTEL: u16 = 0x8086;
const DEVICE_ID_INTEL_VIRT_PCIE_HOST: u16 = 0x0d57;
pub const PCI_ROOT_DEVICE_ID: u8 = 0;

/// Emulates the PCI host bridge device.
pub(crate) struct PciHostBridge {
    /// Device and Function Id.
    id: u8,
    /// Configuration space.
    config: Mutex<PciConfiguration>,
}

impl PciHostBridge {
    /// Create an empty PCI root bridge.
    pub fn new(id: u8, bus: Weak<PciBus>) -> Result<Self> {
        let host_bridge = PciHostBridge {
            id,
            config: Mutex::new(PciConfiguration::new(
                bus,
                VENDOR_ID_INTEL,
                DEVICE_ID_INTEL_VIRT_PCIE_HOST,
                PciClassCode::BridgeDevice,
                &PciBridgeSubclass::HostBridge,
                None,
                PciHeaderType::Device,
                0,
                0,
                None,
            )?),
        };

        Ok(host_bridge)
    }
}

impl PciDevice for PciHostBridge {
    fn id(&self) -> u8 {
        self.id
    }

    fn write_config(&self, offset: u32, data: &[u8]) {
        // Don't expect poisoned lock here.
        self.config
            .lock()
            .expect("poisoned lock for root bus configuration")
            .write_config(offset as usize, data);
    }

    fn read_config(&self, offset: u32, data: &mut [u8]) {
        // Don't expect poisoned lock here.
        self.config
            .lock()
            .expect("poisoned lock for root bus configuration")
            .read_config(offset as usize, data);
    }
}

impl DeviceIo for PciHostBridge {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Create the PCI root bus with the given bus ID.
pub fn create_pci_root_bus(bus_id: u8) -> Result<Arc<PciBus>> {
    let bus = Arc::new(PciBus::new(bus_id));
    let id = bus
        .allocate_device_id(Some(PCI_ROOT_DEVICE_ID))
        .ok_or(Error::NoResources)?;
    let dev = Arc::new(PciHostBridge::new(id, Arc::downgrade(&bus))?);

    bus.register_device(dev)?;

    Ok(bus)
}

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "x86_64")]
    use dbs_device::resources::{DeviceResources, Resource};
    #[cfg(target_arch = "x86_64")]
    use dbs_device::PioAddress;

    use super::*;
    #[cfg(target_arch = "x86_64")]
    use crate::PciRootDevice;

    #[test]
    fn test_create_pci_root_bus() {
        let root_bus = create_pci_root_bus(0).unwrap();
        let host_bridge = PciHostBridge::new(0, Arc::downgrade(&root_bus));

        assert_eq!(root_bus.bus_id(), 0);
        assert_eq!(host_bridge.unwrap().id(), 0);
        assert!(root_bus.get_device(0).is_some());
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn test_read_pci_root_root_bus_cfg() {
        let mut resources = DeviceResources::new();
        resources.append(Resource::PioAddressRange {
            base: 0xCF8,
            size: 8,
        });
        let root = PciRootDevice::create(255, resources).unwrap();

        let root_bus = create_pci_root_bus(0).unwrap();
        let host_bridge = PciHostBridge::new(0, Arc::downgrade(&root_bus));
        assert_eq!(host_bridge.unwrap().id(), 0);

        root.add_bus(root_bus, 0).unwrap();

        let buf = [0x00u8, 0x00u8, 0x00u8, 0x80u8];
        root.pio_write(PioAddress(0xcf8), PioAddress(0), &buf);

        let mut buf = [0u8; 4];
        root.pio_read(PioAddress(0xcf8), PioAddress(4), &mut buf);
        assert_eq!(buf, [0x86u8, 0x80u8, 0x57u8, 0x0du8]);

        let buf = [0x08u8, 0x00u8, 0x00u8, 0x80u8];
        root.pio_write(PioAddress(0xcf8), PioAddress(0), &buf);

        let mut buf = [0u8; 4];
        root.pio_read(PioAddress(0xcf8), PioAddress(4), &mut buf);
        assert_eq!(buf[3], PciClassCode::BridgeDevice.get_register_value());
        root.pio_write(PioAddress(0xcf8), PioAddress(7), &buf);
    }
}
