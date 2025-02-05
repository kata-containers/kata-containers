// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! PciBus to manage child PCI devices and bus resources.
//!
//! According to PCI Local Bus and PCIe specifications, the hardware may build up an hierarchy
//! topology with PCI root bridges, P2P bridges, PCIe switches and PCI endpoint devices.
//! To simplify the implementation, P2P bridges and PCIe switches aren't supported by most VMMs.
//! The PCI/PCIe topology for virtual machines are abstracted as:
//! - one PCI root device: handle accesses to the PCI configuration spaces and owns PCI root buses.
//! - one or more PCI root buses: manages resources (IO port, MMIO address range) and all children
//!   connecting to it. All PCI buses are PCI root buses, no P2P bus on virtual machines.
//! - PCI devices: implement device functionality by allocating resources from the parent bus and
//!   registering to the global address space manager.
//!
//! The VMM allocates resources from the global resource manager and assigns those allocated
//! resources to PCI root buses. All PCI devices allocates resources from PCI root buses instead
//! of from the global resource allocator. By this way, it will be easier to handle PCI Bar
//! reprogramming events and better follows the hardware logic.

use std::sync::{Arc, RwLock, RwLockWriteGuard};

use dbs_allocator::{Constraint, IntervalTree, NodeState, Range};
use dbs_device::resources::{DeviceResources, Resource, ResourceConstraint};
use log::debug;

use crate::{fill_config_data, Error, PciDevice, Result};

#[derive(PartialEq)]
struct PciBusContent {
    resources: Option<DeviceResources>,
    ioport_resources: IntervalTree<()>,
    iomem_resources: IntervalTree<()>,
}

impl PciBusContent {
    fn new() -> Self {
        PciBusContent {
            resources: None,
            ioport_resources: IntervalTree::new(),
            iomem_resources: IntervalTree::new(),
        }
    }

    fn assign_resources(&mut self, resources: DeviceResources, id: u8) -> Result<()> {
        for res in resources.iter() {
            match res {
                Resource::PioAddressRange { base, size } => {
                    debug!(
                        "assign pio address base:0x{:x}, size:0x{:x} to PCI bus {}",
                        *base, *size, id
                    );
                    if *size == 0 {
                        return Err(Error::InvalidResource(res.clone()));
                    }
                    let end = base
                        .checked_add(*size - 1)
                        .ok_or_else(|| Error::InvalidResource(res.clone()))?;
                    self.ioport_resources.insert(Range::new(*base, end), None);
                }
                Resource::MmioAddressRange { base, size } => {
                    debug!(
                        "assign mmio address base:0x{:x}, size:0x{:x} to PCI bus {}",
                        *base, *size, id
                    );
                    if *size == 0 {
                        return Err(Error::InvalidResource(res.clone()));
                    }
                    let end = base
                        .checked_add(*size - 1)
                        .ok_or_else(|| Error::InvalidResource(res.clone()))?;
                    self.iomem_resources.insert(Range::new(*base, end), None);
                }
                _ => debug!("unknown resource assigned to PCI bus {}", id),
            }
        }

        self.resources = Some(resources);

        Ok(())
    }
}

/// Struct to emulate PCI buses.
///
/// To simplify the implementation, PCI hierarchy topology is not supported. That means all PCI
/// devices are directly connected to the PCI root bus.
pub struct PciBus {
    bus_id: u8,
    state: RwLock<PciBusContent>,
    devices: RwLock<IntervalTree<Arc<dyn PciDevice>>>,
}

impl PciBus {
    /// Create a new PCI bus object with the assigned bus id.
    pub fn new(bus_id: u8) -> Self {
        let mut devices = IntervalTree::new();

        Self::assign_default_device_id(&mut devices);

        PciBus {
            bus_id,
            devices: RwLock::new(devices),
            state: RwLock::new(PciBusContent::new()),
        }
    }

    fn assign_default_device_id(devices: &mut IntervalTree<Arc<dyn PciDevice>>) {
        // At present, device id means slot id of device.
        // TODO: Code logic needs to be optimized to support multifunction devices.
        devices.insert(Range::new(0x0u8, 0x1fu8), None);
    }

    /// Get bus ID for this PCI bus instance.
    pub fn bus_id(&self) -> u8 {
        self.bus_id
    }

    /// Allocate an unused PCI device ID.
    ///
    /// # Arguments:
    /// * - `device_id`: allocate the specified device ID if it's valid.
    pub fn allocate_device_id(&self, device_id: Option<u8>) -> Option<u8> {
        // A PCI device supports 8 functions at most, aligning on 8 means only using function 0.
        let mut constraint = Constraint::new(1u64).align(1u64);
        if let Some(id) = device_id {
            constraint = constraint.min(id as u64).max(id as u64);
        }

        debug!(
            "allocate device id constraint size: {}, min: {}, max: {}",
            constraint.size, constraint.min, constraint.max
        );
        // Do not expect poisoned lock here.
        self.devices
            .write()
            .expect("poisoned RwLock() for PCI bus")
            .allocate(&constraint)
            .map(|e| e.min as u8)
    }

    /// Free the previously allocated device id and return data associated with the id.
    pub fn free_device_id(&self, device_id: u32) -> Option<Arc<dyn PciDevice>> {
        if device_id > 0x1f {
            return None;
        }
        // Safe to unwrap because no legal way to generate a poisoned RwLock.
        self.devices
            .write()
            .unwrap()
            .free(&Range::new(device_id as u64, device_id as u64))
    }

    /// Add a child PCI device to the bus.
    pub fn register_device(&self, device: Arc<dyn PciDevice>) -> Result<()> {
        // Do not expect poisoned lock here.
        let device_id = device.id();
        let mut devices = self.devices.write().expect("poisoned lock for PCI bus");

        debug!("add device id {} to bus", device_id);
        let old = devices.update(&Range::new(device_id, device_id), device.clone());
        assert!(old.is_none());

        Ok(())
    }

    /// Get the device instance associated with the `device_id`.
    pub fn get_device(&self, device_id: u32) -> Option<Arc<dyn PciDevice>> {
        if device_id > 0x1f {
            return None;
        }
        let devices = self.devices.read().unwrap();
        match devices.get(&Range::new(device_id as u64, device_id as u64)) {
            Some(NodeState::Valued(d)) => Some(d.clone()),
            _ => None,
        }
    }

    /// Read from PCI device configuration space.
    pub fn read_config(&self, dev: u32, func: u32, offset: u32, data: &mut [u8]) {
        if check_pci_cfg_valid(dev, func, offset, data.len()) {
            return fill_config_data(data);
        }

        // Safe to unwrap because no legal way to generate a poisoned RwLock.
        let devices = self.devices.read().unwrap();
        match devices.get(&Range::new(dev as u64, dev as u64)) {
            Some(NodeState::Valued(d)) => d.read_config(offset, data),
            _ => fill_config_data(data),
        };
    }

    /// Write to PCI device configuration space.
    pub fn write_config(&self, dev: u32, func: u32, offset: u32, data: &[u8]) {
        if check_pci_cfg_valid(dev, func, offset, data.len()) {
            return;
        }

        // Safe to unwrap because no legal way to generate a poisoned RwLock.
        let devices = self.devices.read().unwrap();
        if let Some(NodeState::Valued(d)) = devices.get(&Range::new(dev as u64, dev as u64)) {
            d.write_config(offset, data);
        }
    }

    /// Get PCI bus device resource. This function is use for create PCI_BUS fdt node for arm,
    /// so there is only care about mmio resource. We need to copy mmio resource, because there is
    /// a read write lock, we can't return DeviceResources's address for caller using.
    #[cfg(target_arch = "aarch64")]
    pub fn get_device_resources(&self) -> DeviceResources {
        let mut device_resources = DeviceResources::new();
        if let Some(resources) = &self
            .state
            .read()
            .expect("poisoned RwLock for PCI bus")
            .resources
        {
            let mmio_resources = resources.get_mmio_address_ranges();
            for (base, size) in mmio_resources {
                let entry = Resource::MmioAddressRange { base, size };
                device_resources.append(entry);
            }
        }
        device_resources
    }

    /// Assign resources to be allocated by all child devices.
    pub fn assign_resources(&self, resources: DeviceResources) -> Result<()> {
        // Do not expect poisoned lock here.
        self.state
            .write()
            .expect("poisoned RwLock for PCI bus")
            .assign_resources(resources, self.bus_id)
    }

    /// Allocate PCI IO resources from the bus resource pool.
    pub fn allocate_resources(
        &self,
        constraints: &[ResourceConstraint],
    ) -> Result<DeviceResources> {
        let mut resources = DeviceResources::new();
        // Safe to unwrap because no legal way to generate a poisoned RwLock.
        let mut state = self.state.write().unwrap();
        for req in constraints {
            match req {
                ResourceConstraint::PioAddress { range, align, size } => {
                    let mut constraint = Constraint::new(*size as u64).align(*align as u64);
                    if let Some((min, max)) = range {
                        constraint = constraint.min(*min as u64).max(*max as u64);
                    }
                    match state.ioport_resources.allocate(&constraint) {
                        Some(range) => {
                            resources.append(Resource::PioAddressRange {
                                base: range.min as u16,
                                size: range.len() as u16,
                            });
                        }
                        None => {
                            Self::free_all_resource(state, resources);
                            return Err(Error::NoResources);
                        }
                    }
                }
                ResourceConstraint::MmioAddress { range, align, size } => {
                    let mut constraint = Constraint::new(*size).align(*align);
                    if let Some((min, max)) = range {
                        constraint = constraint.min(*min).max(*max);
                    }
                    match state.iomem_resources.allocate(&constraint) {
                        Some(range) => {
                            resources.append(Resource::MmioAddressRange {
                                base: range.min,
                                size: range.len(),
                            });
                        }
                        None => {
                            Self::free_all_resource(state, resources);
                            return Err(Error::NoResources);
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(resources)
    }

    /// Free allocated PCI IO resources.
    pub fn free_resources(&self, resources: DeviceResources) {
        // Don't expect poisoned lock here.
        let state = self.state.write().unwrap();
        Self::free_all_resource(state, resources);
    }

    // free all device resource under pci bus
    fn free_all_resource(mut state: RwLockWriteGuard<PciBusContent>, resources: DeviceResources) {
        for res in resources.get_all_resources().iter() {
            match res {
                Resource::PioAddressRange { base, size } => {
                    let range = Range::new(*base as u64, (*base + *size - 1) as u64);
                    state.ioport_resources.free(&range);
                }
                Resource::MmioAddressRange { base, size } => {
                    let range = Range::new(*base, *base + *size - 1);
                    state.iomem_resources.free(&range);
                }
                _ => {}
            }
        }
    }
}

impl PartialEq for PciBus {
    fn eq(&self, other: &PciBus) -> bool {
        self.bus_id == other.bus_id
            && *self.state.read().unwrap() == *other.state.read().unwrap()
            && *self.devices.read().unwrap() == *other.devices.read().unwrap()
    }
}

#[inline]
fn check_pci_cfg_valid(dev: u32, func: u32, offset: u32, data_len: usize) -> bool {
    dev > 0x1f || func != 0 || offset >= 0x1000 || offset & (data_len - 1) as u32 & 0x3 != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bus_allocate_device_id() {
        let bus = PciBus::new(0);
        let id = bus.allocate_device_id(None);
        assert_eq!(id, Some(0));
        let id = bus.allocate_device_id(None);
        assert_eq!(id, Some(1));

        let id = bus.allocate_device_id(Some(15));
        assert_eq!(id, Some(15));

        assert!(bus.get_device(0x1f).is_none());

        let old = bus.free_device_id(15);
        if old.is_some() {
            panic!("invalid return value for free_device_id");
        }
        let id = bus.allocate_device_id(Some(15));
        assert_eq!(id, Some(15));
    }

    #[test]
    fn test_bus_allocate_resource() {
        let bus = PciBus::new(0);

        let mut resources = DeviceResources::new();
        resources.append(Resource::PioAddressRange {
            base: 0,
            size: 0x1000,
        });
        resources.append(Resource::MmioAddressRange {
            base: 0x10_0000,
            size: 0x10_0000,
        });
        assert_eq!(resources.get_all_resources().len(), 2);
        assert!(bus.assign_resources(resources).is_ok());
        assert!(bus.state.read().unwrap().resources.is_some());

        let constraints = [
            ResourceConstraint::PioAddress {
                range: Some((0x100, 0x10f)),
                size: 0xf,
                align: 1,
            },
            ResourceConstraint::MmioAddress {
                range: Some((0x10_0001, 0x10_2000)),
                size: 0x100,
                align: 0x1000,
            },
        ];
        let resources = bus.allocate_resources(&constraints).unwrap();
        assert_eq!(resources.len(), 2);

        let pio = resources.get_pio_address_ranges();
        assert_eq!(pio.len(), 1);
        assert_eq!(pio[0].0, 0x100);
        assert_eq!(pio[0].1, 0xf);

        let mmio = resources.get_mmio_address_ranges();
        assert_eq!(mmio.len(), 1);
        assert_eq!(mmio[0].0, 0x10_1000);
        assert_eq!(mmio[0].0 & 0xfff, 0x0);
        assert_eq!(mmio[0].1, 0x100);

        bus.free_resources(resources);

        let resources = bus.allocate_resources(&constraints).unwrap();
        assert_eq!(resources.len(), 2);

        let pio = resources.get_pio_address_ranges();
        assert_eq!(pio.len(), 1);
        assert_eq!(pio[0].0, 0x100);
        assert_eq!(pio[0].1, 0xf);

        let mmio = resources.get_mmio_address_ranges();
        assert_eq!(mmio.len(), 1);
        assert_eq!(mmio[0].0, 0x10_1000);
        assert_eq!(mmio[0].0 & 0xfff, 0x0);
        assert_eq!(mmio[0].1, 0x100);
    }
}
