// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use dbs_boot::layout::{GUEST_MEM_END, GUEST_PHYS_END};
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use dbs_device::resources::Resource;
use dbs_device::resources::{DeviceResources, ResourceConstraint};
use dbs_interrupt::KvmIrqManager;
#[cfg(target_arch = "aarch64")]
use dbs_pci::ECAM_SPACE_LENGTH;
use dbs_pci::{create_pci_root_bus, PciBus, PciDevice, PciRootDevice, PciSystemContext};

use super::{Result, VfioDeviceError};
#[cfg(target_arch = "aarch64")]
use crate::device_manager::vfio_dev_mgr::USE_SHARED_IRQ;
use crate::device_manager::DeviceManagerContext;
use crate::resource_manager::ResourceManager;

/// we only support one pci bus
pub const PCI_BUS_DEFAULT: u8 = 0;
/// The default mmio size for pci root bus.
const PCI_MMIO_DEFAULT_SIZE: u64 = 2048u64 << 30;

/// PCI pass-through device manager.
#[derive(Clone)]
pub struct PciSystemManager {
    pub irq_manager: Arc<KvmIrqManager>,
    pub io_context: DeviceManagerContext,
    pub pci_root: Arc<PciRootDevice>,
    pub pci_root_bus: Arc<PciBus>,
}

impl PciSystemManager {
    /// Create a new PCI pass-through device manager.
    pub fn new(
        irq_manager: Arc<KvmIrqManager>,
        io_context: DeviceManagerContext,
        res_manager: Arc<ResourceManager>,
    ) -> std::result::Result<Self, VfioDeviceError> {
        let resources = PciSystemManager::allocate_root_device_resources(res_manager)?;
        let pci_root = Arc::new(
            PciRootDevice::create(PCI_BUS_DEFAULT, resources).map_err(VfioDeviceError::PciError)?,
        );
        let pci_root_bus =
            create_pci_root_bus(PCI_BUS_DEFAULT).map_err(VfioDeviceError::PciError)?;

        Ok(PciSystemManager {
            irq_manager,
            io_context,
            pci_root,
            pci_root_bus,
        })
    }

    // The x86 pci root device is a pio device with a fixed pio base address and length.
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn allocate_root_device_resources(
        _res_manager: Arc<ResourceManager>,
    ) -> Result<DeviceResources> {
        let mut resources = DeviceResources::new();
        resources.append(Resource::PioAddressRange {
            // PCI CONFIG_ADDRESS port address 0xcf8 and uses 32 bits
            // PCI COFIG_DATA port address 0xcfc and uses 32 bits
            // so the resource registered begins at 0xcf8 and takes 8 bytes as size
            base: 0xcf8,
            size: 0x8,
        });
        Ok(resources)
    }

    // The pci root device of arm is a mmio device, and its reg range is ECAM space,
    // which needs to be dynamically applied from the resource pool. In addition,
    // the ECAM space is used to enumerate and identify PCI devices.
    #[cfg(target_arch = "aarch64")]
    fn allocate_root_device_resources(
        res_manager: Arc<ResourceManager>,
    ) -> Result<DeviceResources> {
        let requests = vec![ResourceConstraint::MmioAddress {
            range: Some((0x0, 0xffff_ffff)),
            align: 4096,
            size: ECAM_SPACE_LENGTH,
        }];
        let resources = res_manager
            .allocate_device_resources(&requests, USE_SHARED_IRQ)
            .map_err(VfioDeviceError::AllocateDeviceResource)?;
        Ok(resources)
    }

    /// Activate the PCI subsystem.
    pub fn activate(&mut self, resources: DeviceResources) -> Result<()> {
        let bus_id = self.pci_root_bus.bus_id();

        self.pci_root
            .add_bus(self.pci_root_bus.clone(), bus_id)
            .map_err(VfioDeviceError::PciError)?;
        PciRootDevice::activate(self.pci_root.clone(), &mut self.io_context)
            .map_err(VfioDeviceError::PciError)?;

        self.pci_root_bus
            .assign_resources(resources)
            .map_err(VfioDeviceError::PciError)?;

        Ok(())
    }

    /// Get resource requirements of the PCI subsystem.
    #[allow(clippy::vec_init_then_push)]
    pub fn resource_requirements(&self) -> Vec<ResourceConstraint> {
        let mut requests = Vec::new();

        // allocate 512MB MMIO address below 4G.
        requests.push(ResourceConstraint::MmioAddress {
            range: Some((0x0, 0xffff_ffff)),
            align: 4096,
            size: 512u64 << 20,
        });
        // allocate 2048GB MMIO address above 4G.
        requests.push(ResourceConstraint::MmioAddress {
            range: Some((0x1_0000_0000, 0xffff_ffff_ffff_ffff)),
            align: 4096,
            size: Self::get_mmio_size(),
        });
        // allocate 8KB IO port
        requests.push(ResourceConstraint::PioAddress {
            range: None,
            align: 1,
            size: 8u16 << 10,
        });

        requests
    }

    fn get_mmio_size() -> u64 {
        if (*GUEST_PHYS_END - *GUEST_MEM_END) > PCI_MMIO_DEFAULT_SIZE {
            PCI_MMIO_DEFAULT_SIZE
        } else {
            (*GUEST_PHYS_END - *GUEST_MEM_END) / 2
        }
    }

    /// Get the PCI root bus.
    pub fn pci_root_bus(&self) -> Arc<PciBus> {
        self.pci_root_bus.clone()
    }

    /// Allocate a PCI device id.
    pub fn new_device_id(&self, device_id: Option<u8>) -> Option<u8> {
        self.pci_root_bus.allocate_device_id(device_id)
    }

    pub fn free_device_id(&self, device_id: u32) -> Option<Arc<dyn PciDevice>> {
        self.pci_root_bus.free_device_id(device_id)
    }

    /// Obtain ECAM space resources, that is, pci root device resources.
    #[cfg(target_arch = "aarch64")]
    pub fn get_ecam_space(&self) -> DeviceResources {
        self.pci_root.get_device_resources()
    }

    /// Obtain BAR space resources, that is, pci root bus resources.
    #[cfg(target_arch = "aarch64")]
    pub fn get_bar_space(&self) -> DeviceResources {
        self.pci_root_bus.get_device_resources()
    }
}

impl PciSystemContext for PciSystemManager {
    type D = DeviceManagerContext;

    fn get_device_manager_context(&self) -> Self::D {
        self.io_context.clone()
    }

    fn get_interrupt_manager(&self) -> Arc<KvmIrqManager> {
        self.irq_manager.clone()
    }
}
