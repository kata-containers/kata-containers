// Copyright (c) NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
use crate::qemu::cmdline_generator::QemuCmdLine;
use crate::qemu::cmdline_generator::ToQemuParams;
use anyhow::Result;
use async_trait::async_trait;

/// NUMA Support with PCIe Expander Bus, each PXB represents a NUMA node.
///
///     pcie.0 bus
///     --------------------------------------------------------------------
///          |                                     |
///          | numa_node=0                         | numa_node=1
///          |                                     |
///     -------------                        -------------
///     |    PXB    |                        |    PXB    |
///     ------------                         -------------
///          |                                     |
///          | pcie.1                              | pcie.2
///          |                                     |
///     -------------                        -------------
///     | Root Port |                        | Root Port |
///     ------------                         -------------
///           |           -------------------------|------------------------
///      ------------     |                 -----------------              |
///      | PCIe Dev |     |    PCI Express  | Upstream Port |              |
///      ------------     |      Switch     -----------------              |
///                       |                  |            |                |
///                       |    -------------------    -------------------  |
///                       |    | Downstream Port |    | Downstream Port |  |
///                       |    -------------------    -------------------  |
///                       -------------|-----------------------|------------
///                              ------------              ------------
///                              | PCIe Dev |              | PCIe Dev |
///                              ------------              ------------
///

/// PCIeExpanderBusDevice is the only entity that is numa node affine. /////////
/// -device pxb-pcie,id=pxb0,bus=pcie.1,bus_nr=20,numa_node=0
#[derive(Debug, Default)]
pub struct PCIeExpanderBusDevice {
    id: String,
    bus: String,
    bus_nr: u32,
    numa_node: u32,
}

impl PCIeExpanderBusDevice {
    fn new(id: &str, bus: &str, bus_nr: u32, numa_node: u32) -> Self {
        PCIeExpanderBusDevice {
            id: id.to_owned(),
            bus: bus.to_owned(),
            bus_nr,
            numa_node,
        }
    }
}

#[async_trait]
impl ToQemuParams for PCIeExpanderBusDevice {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut device_params = Vec::new();
        device_params.push(format!("{},id={}", "pxb-pcie", self.id));
        device_params.push(format!("bus={}", self.bus));
        device_params.push(format!("bus_nr={}", self.bus_nr));
        device_params.push(format!("numa_node={}", self.numa_node));
        Ok(vec!["-device".to_owned(), device_params.join(",")])
    }
}
/// ACPIGenericInitiator lets you associate a PCI device with one or more NUMA
/// nodes, so QEMU can emit ACPI SRAT Generic Initiator Affinity structures for
/// that device
#[derive(Debug, Default)]
pub struct ACPIGenericInitiator {
	id: String,
	pci_dev: String,
	node: u32,
}

impl ACPIGenericInitiator {
	fn new(id: &str, pci_dev: &str, node: u32) -> Self {
		ACPIGenericInitiator {
			id: id.to_owned(),
			pci_dev: pci_dev.to_owned(),
			node,
		}
	}
}

#[async_trait]
impl ToQemuParams for ACPIGenericInitiator {
	async fn qemu_params(&self) -> Result<Vec<String>> {
		let mut device_params = Vec::new();
		device_params.push(format!("{},id={}", "acpi-generic-initiator", self.id));
		device_params.push(format!("pci-dev={}", self.pci_dev));
		device_params.push(format!("node={}", self.node));
		Ok(vec!["-object".to_owned(), device_params.join(",")])
	}
}

impl<'a> QemuCmdLine<'a> {
    #[allow(dead_code)]
    pub fn add_pcie_expander_bus(&mut self, id: &str, bus_nr: u32, numa_node: u32) -> Result<()> {
        let machine_type: &str = &self.config.machine_info.machine_type;
        let bus = match machine_type {
            "q35" | "virt" => "pcie.0",
            _ => {
                info!(
                    sl!(),
                    "PCIe Expander Bus not supported for machine type: {}", machine_type
                );
                return Ok(());
            }
        };

        let pxb_device = PCIeExpanderBusDevice::new(id, bus, bus_nr, numa_node);
        self.devices.push(Box::new(pxb_device));

        Ok(())
    }
    #[allow(dead_code)]
    pub fn add_acpi_generic_initiator(&mut self, id: &str, pci_dev: &str, node: u32) -> Result<()> {
        let acpi_generic_initiator = ACPIGenericInitiator::new(id, pci_dev, node);
        self.devices.push(Box::new(acpi_generic_initiator));

        Ok(())
    }
}
