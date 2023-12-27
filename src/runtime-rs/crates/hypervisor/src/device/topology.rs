//
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

/*
The design origins from https://github.com/qemu/qemu/blob/master/docs/pcie.txt

In order to better support the PCIe topologies of different VMMs, we adopt a layered approach.
The first layer is the base layer(the flatten PCIe topology), which mainly consists of the root bus,
which is mainly used by VMMs that only support devices being directly attached to the root bus.
However, not all VMMs have such simple PCIe topologies. For example, Qemu, which can fully simulate
the PCIe topology of the host, has a complex PCIe topology. In this case, we need to add PCIe RootPort,
PCIe Switch, and PCIe-PCI Bridge or pxb-pcie on top of the base layer, which is The Complex PCIe Topology.

The design graghs as below:

(1) The flatten PCIe Topology
pcie.0 bus (Root Complex)
----------------------------------------------------------------------------
|    |    |    |    |    |    |    |    |    |    |    |    |    |    | .. |
--|--------------------|------------------|-------------------------|-------
  |                    |                  |                         |
  V                    V                  V                         V
-----------       -----------         -----------                -----------
| PCI Dev |       | PCI Dev |         | PCI Dev |                | PCI Dev |
-----------       -----------         -----------                -----------

(2) The Complex PCIe Topology(It'll be implemented when Qemu is ready in runtime-rs)
pcie.0 bus (Root Complex)
----------------------------------------------------------------------------
|    |    |    |    |    |    |    |    |    |    |    |    |    |    | .. |
------|----------------|--------------------------------------|-------------
      |                |                                      |
      V                V                                      V
 -------------     -------------                        -------------
 | Root Port |     | Root Port |                        | Root Port |
 -------------     -------------                        -------------
     |                                                       |
     |                              -------------------------|-----------------------
------------                        |                -----------------              |
| PCIe Dev |                        |   PCI Express  | Upstream Port |              |
------------                        |     Switch     -----------------              |
                                    |                 |            |                |
                                    |   -------------------    -------------------  |
                                    |   | Downstream Port |    | Downstream Port |  |
                                    |   -------------------    -------------------  |
                                    -------------|-----------------------|-----------
                                            ------------
                                            | PCIe Dev |
                                            ------------
*/

use std::collections::{hash_map::Entry, HashMap};

use anyhow::{anyhow, Result};

use crate::device::pci_path::PciSlot;
use kata_types::config::hypervisor::TopologyConfigInfo;

use super::pci_path::PciPath;

const DEFAULT_PCIE_ROOT_BUS: &str = "pcie.0";
// Currently, CLH and Dragonball support device attachment solely on the root bus.
const DEFAULT_PCIE_ROOT_BUS_ADDRESS: &str = "0000:00";
pub const PCIE_ROOT_BUS_SLOTS_CAPACITY: u32 = 32;

// register_pcie_device: do pre register device into PCIe Topology which
// be called in device driver's attach before device real attached into
// VM. It'll allocate one available PCI path for the device.
// register_pcie_device can be expanded as below:
// register_pcie_device {
//     match pcie_topology {
//         Some(topology) => self.register(topology).await,
//         None => Ok(())
//     }
// }
#[macro_export]
macro_rules! register_pcie_device {
    ($self:ident, $opt:expr) => {
        match $opt {
            Some(topology) => $self.register(topology).await,
            None => Ok(()),
        }
    };
}

// update_pcie_device: do update device info, as some VMMs will be able to
// return the device info containing guest PCI path which differs the one allocated
// in runtime. So we need to compair the two PCI path, and finally update it or not
// based on the difference between them.
// update_pcie_device can be expanded as below:
// update_pcie_device {
//     match pcie_topology {
//         Some(topology) => self.register(topology).await,
//         None => Ok(())
//     }
// }
#[macro_export]
macro_rules! update_pcie_device {
    ($self:ident, $opt:expr) => {
        match $opt {
            Some(topology) => $self.register(topology).await,
            None => Ok(()),
        }
    };
}

// unregister_pcie_device: do unregister device from pcie topology.
// unregister_pcie_device can be expanded as below:
// unregister_pcie_device {
//     match pcie_topology {
//         Some(topology) => self.unregister(topology).await,
//         None => Ok(())
//     }
// }
#[macro_export]
macro_rules! unregister_pcie_device {
    ($self:ident, $opt:expr) => {
        match $opt {
            Some(topology) => $self.unregister(topology).await,
            None => Ok(()),
        }
    };
}

pub trait PCIeDevice: Send + Sync {
    fn device_id(&self) -> &str;
}

#[derive(Clone, Debug, Default)]
pub struct PCIeEndpoint {
    // device_id for device in device manager
    pub device_id: String,
    // device's PCI Path in Guest
    pub pci_path: PciPath,
    // root_port for PCIe Device
    pub root_port: Option<PCIeRootPort>,

    // device_type is for device virtio-pci/PCI or PCIe
    pub device_type: String,
}

impl PCIeDevice for PCIeEndpoint {
    fn device_id(&self) -> &str {
        self.device_id.as_str()
    }
}

// reserved resource
#[derive(Clone, Debug, Default)]
pub struct ResourceReserved {
    // This to work needs patches to QEMU
    // The PCIE-PCI bridge can be hot-plugged only into pcie-root-port that has 'bus-reserve'
    // property value to provide secondary bus for the hot-plugged bridge.
    pub bus_reserve: String,

    // reserve prefetched MMIO aperture, 64-bit
    pub pref64_reserve: String,
    // reserve prefetched MMIO aperture, 32-bit
    pub pref32_reserve: String,
    // reserve non-prefetched MMIO aperture, 32-bit *only*
    pub memory_reserve: String,

    // IO reservation
    pub io_reserve: String,
}

// PCIe Root Port
#[derive(Clone, Debug, Default)]
pub struct PCIeRootPort {
    // format: rp{n}, n>=0
    pub id: String,

    // default is pcie.0
    pub bus: String,
    // >=0, default is 0x00
    pub address: String,

    // (slot, chassis) pair is mandatory and must be unique for each pcie-root-port,
    // chassis >=0, default is 0x00
    pub chassis: u8,
    // slot >=0, default is 0x00
    pub slot: u8,

    // multi_function is for PCIe Device passthrough
    // true => "on", false => "off", default is off
    pub multi_function: bool,

    // reserved resource for some VMM, such as Qemu.
    pub resource_reserved: ResourceReserved,

    // romfile specifies the ROM file being used for this device.
    pub romfile: String,
}

// PCIe Root Complex
#[derive(Clone, Debug, Default)]
pub struct PCIeRootComplex {
    pub root_bus: String,
    pub root_bus_address: String,
    pub root_bus_devices: HashMap<String, PCIeEndpoint>,
}

#[derive(Debug, Default)]
pub struct PCIeTopology {
    pub hypervisor_name: String,
    pub root_complex: PCIeRootComplex,

    pub bridges: u32,
    pub pcie_root_ports: u32,
    pub hotplug_vfio_on_root_bus: bool,
}

impl PCIeTopology {
    // As some special case doesn't support PCIe devices, there's no need to build a PCIe Topology.
    pub fn new(config_info: Option<&TopologyConfigInfo>) -> Option<Self> {
        // if config_info is None, it will return None.
        let topo_config = config_info?;

        let root_complex = PCIeRootComplex {
            root_bus: DEFAULT_PCIE_ROOT_BUS.to_owned(),
            root_bus_address: DEFAULT_PCIE_ROOT_BUS_ADDRESS.to_owned(),
            root_bus_devices: HashMap::with_capacity(PCIE_ROOT_BUS_SLOTS_CAPACITY as usize),
        };

        Some(Self {
            hypervisor_name: topo_config.hypervisor_name.to_owned(),
            root_complex,
            bridges: topo_config.device_info.default_bridges,
            pcie_root_ports: topo_config.device_info.pcie_root_port,
            hotplug_vfio_on_root_bus: topo_config.device_info.hotplug_vfio_on_root_bus,
        })
    }

    pub fn insert_device(&mut self, ep: &mut PCIeEndpoint) -> Option<PciPath> {
        let to_pcipath = |v: u32| -> PciPath {
            PciPath {
                slots: vec![PciSlot(v as u8)],
            }
        };

        let to_string = |v: u32| -> String { to_pcipath(v).to_string() };

        // find the first available index as the allocated slot.
        let allocated_slot = (0..PCIE_ROOT_BUS_SLOTS_CAPACITY).find(|&i| {
            !self
                .root_complex
                .root_bus_devices
                .contains_key(&to_string(i))
        })?;

        let pcipath = to_string(allocated_slot);

        // update pci_path in Endpoint
        ep.pci_path = to_pcipath(allocated_slot);
        // convert the allocated slot to pci path and then insert it with ep
        self.root_complex
            .root_bus_devices
            .insert(pcipath, ep.clone());

        Some(to_pcipath(allocated_slot))
    }

    pub fn remove_device(&mut self, device_id: &str) -> Option<String> {
        let mut target_device: Option<String> = None;
        self.root_complex.root_bus_devices.retain(|k, v| {
            if v.device_id() != device_id {
                true
            } else {
                target_device = Some((*k).to_string());
                false
            }
        });

        target_device
    }

    pub fn update_device(&mut self, ep: &PCIeEndpoint) -> Option<PciPath> {
        let pci_addr = ep.pci_path.clone();

        // First, find the PCIe Endpoint corresponding to the endpoint in the Hash Map based on the PCI path.
        // If found, it means that we do not need to update the device's position in the Hash Map.
        // If not found, it means that the PCI Path corresponding to the device has changed, and the device's
        // position in the Hash Map needs to be updated.
        match self
            .root_complex
            .root_bus_devices
            .entry(pci_addr.to_string())
        {
            Entry::Occupied(_) => None,
            Entry::Vacant(_entry) => {
                self.remove_device(&ep.device_id);
                self.root_complex
                    .root_bus_devices
                    .insert(pci_addr.to_string(), ep.clone());

                Some(pci_addr)
            }
        }
    }

    pub fn find_device(&mut self, device_id: &str) -> bool {
        for v in self.root_complex.root_bus_devices.values() {
            info!(
                sl!(),
                "find_device with: {:?}, {:?}.",
                &device_id,
                v.device_id()
            );
            if v.device_id() == device_id {
                return true;
            }
        }

        false
    }

    pub fn do_insert_or_update(&mut self, pciep: &mut PCIeEndpoint) -> Result<PciPath> {
        // Try to check whether the device is present in the PCIe Topology.
        // If the device dosen't exist, it proceeds to register it within the topology
        let pci_path = if !self.find_device(&pciep.device_id) {
            // Register a device within the PCIe topology, allocating and assigning it an available PCI Path.
            // Upon successful insertion, it updates the pci_path in PCIeEndpoint and returns it.
            // Finally, update both the guest_pci_path and devices_options with the allocated PciPath.
            if let Some(pci_addr) = self.insert_device(pciep) {
                pci_addr
            } else {
                return Err(anyhow!("pci path allocated failed."));
            }
        } else {
            // If the device exists, it proceeds to update its pcipath within
            // the topology and the device's guest_pci_path and device_options.
            if let Some(pci_addr) = self.update_device(pciep) {
                pci_addr
            } else {
                return Ok(pciep.pci_path.clone());
            }
        };

        Ok(pci_path)
    }
}

// do_add_pcie_endpoint do add a device into PCIe topology with pcie endpoint
// device_id: device's Unique ID in Device Manager.
// allocated_pcipath: allocated pcipath before add_device
// topology: PCIe Topology for devices to build a PCIe Topology in Guest.
pub fn do_add_pcie_endpoint(
    device_id: String,
    allocated_pcipath: Option<PciPath>,
    topology: &mut PCIeTopology,
) -> Result<PciPath> {
    let pcie_endpoint = &mut PCIeEndpoint {
        device_type: "PCIe".to_string(),
        device_id,
        ..Default::default()
    };

    if let Some(pci_path) = allocated_pcipath {
        pcie_endpoint.pci_path = pci_path;
    }

    topology.do_insert_or_update(pcie_endpoint)
}
