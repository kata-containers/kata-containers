// Copyright 2020-2022 Alibaba Cloud. All Rights Reserved.
// Copyright © 2019 Intel Corporation. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! IO Device Manager to handle trapped MMIO/PIO access requests.
//!
//! The [IoManager](self::IoManager) is responsible for managing all trapped MMIO/PIO accesses for
//! virtual devices. It cooperates with the Secure Sandbox/VMM and device drivers to handle trapped
//! accesses. The flow is as below:
//! - device drivers allocate resources from the VMM/resource manager, including trapped MMIO/PIO
//!   address ranges.
//! - the device manager registers devices to the [IoManager](self::IoManager) with trapped MMIO/PIO
//!   address ranges.
//! - VM IO Exit events get triggered when the guest accesses those trapped address ranges.
//! - the vmm handle those VM IO Exit events, and dispatch them to the [IoManager].
//! - the [IoManager] invokes registered callbacks/device drivers to handle those accesses, if there
//!   is a device registered for the address.
//!
//! # Examples
//!
//! Creating a dummy deivce which implement DeviceIo trait, and register it to [IoManager] with
//! trapped MMIO/PIO address ranges:
//!
//! ```
//! use std::sync::Arc;
//! use std::any::Any;
//!
//! use dbs_device::device_manager::IoManager;
//! use dbs_device::resources::{DeviceResources, Resource};
//! use dbs_device::{DeviceIo, IoAddress, PioAddress};
//!
//! struct DummyDevice {}
//!
//! impl DeviceIo for DummyDevice {
//!     fn read(&self, base: IoAddress, offset: IoAddress, data: &mut [u8]) {
//!         println!(
//!             "mmio read, base: 0x{:x}, offset: 0x{:x}",
//!             base.raw_value(),
//!             offset.raw_value()
//!         );
//!     }
//!
//!     fn write(&self, base: IoAddress, offset: IoAddress, data: &[u8]) {
//!         println!(
//!             "mmio write, base: 0x{:x}, offset: 0x{:x}",
//!             base.raw_value(),
//!             offset.raw_value()
//!         );
//!     }
//!
//!     fn pio_read(&self, base: PioAddress, offset: PioAddress, data: &mut [u8]) {
//!         println!(
//!             "pio read, base: 0x{:x}, offset: 0x{:x}",
//!             base.raw_value(),
//!             offset.raw_value()
//!         );
//!     }
//!
//!     fn pio_write(&self, base: PioAddress, offset: PioAddress, data: &[u8]) {
//!         println!(
//!             "pio write, base: 0x{:x}, offset: 0x{:x}",
//!             base.raw_value(),
//!             offset.raw_value()
//!         );
//!     }
//!
//!     fn as_any(&self) -> &dyn Any {
//!         self
//!     }
//! }
//!
//! // Allocate resources for device
//! let mut resources = DeviceResources::new();
//! resources.append(Resource::MmioAddressRange {
//!     base: 0,
//!     size: 4096,
//! });
//! resources.append(Resource::PioAddressRange { base: 0, size: 32 });
//!
//! // Register device to `IoManager` with resources
//! let device = Arc::new(DummyDevice {});
//! let mut manager = IoManager::new();
//! manager.register_device_io(device, &resources).unwrap();
//!
//! // Dispatch I/O event from `IoManager` to device
//! manager.mmio_write(0, &vec![0, 1]).unwrap();
//! {
//!     let mut buffer = vec![0; 4];
//!     manager.pio_read(0, &mut buffer);
//! }
//! ```

use std::cmp::{Ord, Ordering, PartialEq, PartialOrd};
use std::collections::btree_map::BTreeMap;
use std::ops::Deref;
use std::result;
use std::sync::Arc;

use thiserror::Error;

use crate::resources::Resource;
use crate::{DeviceIo, IoAddress, IoSize, PioAddress};

/// Error types for `IoManager` related operations.
#[derive(Error, Debug)]
pub enum Error {
    /// The inserting device overlaps with a current device.
    #[error("device address conflicts with existing devices")]
    DeviceOverlap,
    /// The device doesn't exist.
    #[error("no such device")]
    NoDevice,
}

/// A specialized version of [std::result::Result] for [IoManager] realted operations.
pub type Result<T> = result::Result<T, Error>;

/// Structure representing an IO address range.
#[derive(Debug, Copy, Clone, Eq)]
pub struct IoRange {
    base: IoAddress,
    size: IoSize,
}

impl IoRange {
    fn new_pio_range(base: u16, size: u16) -> Self {
        IoRange {
            base: IoAddress(base as u64),
            size: IoSize(size as u64),
        }
    }

    fn new_mmio_range(base: u64, size: u64) -> Self {
        IoRange {
            base: IoAddress(base),
            size: IoSize(size),
        }
    }
}

impl PartialEq for IoRange {
    fn eq(&self, other: &IoRange) -> bool {
        self.base == other.base
    }
}

impl Ord for IoRange {
    fn cmp(&self, other: &IoRange) -> Ordering {
        self.base.cmp(&other.base)
    }
}

impl PartialOrd for IoRange {
    fn partial_cmp(&self, other: &IoRange) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// IO manager to handle all trapped MMIO/PIO access requests.
///
/// All devices handling trapped MMIO/PIO accesses should register themself to the IO manager
/// with trapped address ranges. When guest vm accesses those trapped MMIO/PIO address ranges,
/// VM IO Exit events will be triggered and the VMM dispatches those events to IO manager.
/// And then the registered callbacks will invoked by IO manager.
#[derive(Clone, Default)]
pub struct IoManager {
    /// Range mapping for VM exit pio operations.
    pio_bus: BTreeMap<IoRange, Arc<dyn DeviceIo>>,
    /// Range mapping for VM exit mmio operations.
    mmio_bus: BTreeMap<IoRange, Arc<dyn DeviceIo>>,
}

impl IoManager {
    /// Create a new instance of [IoManager].
    pub fn new() -> Self {
        IoManager::default()
    }

    /// Register a new device to the [IoManager], with trapped MMIO/PIO address ranges.
    ///
    /// # Arguments
    ///
    /// * `device`: device object to handle trapped IO access requests
    /// * `resources`: resources representing trapped MMIO/PIO address ranges. Only MMIO/PIO address
    ///    ranges will be handled, and other types of resource will be ignored. So the caller does
    ///    not need to filter out non-MMIO/PIO resources.
    pub fn register_device_io(
        &mut self,
        device: Arc<dyn DeviceIo>,
        resources: &[Resource],
    ) -> Result<()> {
        for (idx, res) in resources.iter().enumerate() {
            match *res {
                Resource::PioAddressRange { base, size } => {
                    if self
                        .pio_bus
                        .insert(IoRange::new_pio_range(base, size), device.clone())
                        .is_some()
                    {
                        // Rollback registered resources.
                        self.unregister_device_io(&resources[0..idx])
                            .expect("failed to unregister devices");

                        return Err(Error::DeviceOverlap);
                    }
                }
                Resource::MmioAddressRange { base, size } => {
                    if self
                        .mmio_bus
                        .insert(IoRange::new_mmio_range(base, size), device.clone())
                        .is_some()
                    {
                        // Rollback registered resources.
                        self.unregister_device_io(&resources[0..idx])
                            .expect("failed to unregister devices");

                        return Err(Error::DeviceOverlap);
                    }
                }
                _ => continue,
            }
        }
        Ok(())
    }

    /// Unregister a device from `IoManager`.
    ///
    /// # Arguments
    ///
    /// * `resources`: resource list containing all trapped address ranges for the device.
    pub fn unregister_device_io(&mut self, resources: &[Resource]) -> Result<()> {
        for res in resources.iter() {
            match *res {
                Resource::PioAddressRange { base, size } => {
                    self.pio_bus.remove(&IoRange::new_pio_range(base, size));
                }
                Resource::MmioAddressRange { base, size } => {
                    self.mmio_bus.remove(&IoRange::new_mmio_range(base, size));
                }
                _ => continue,
            }
        }
        Ok(())
    }

    /// Handle VM IO Exit events triggered by trapped MMIO read accesses.
    ///
    /// Return error if failed to get the device.
    pub fn mmio_read(&self, addr: u64, data: &mut [u8]) -> Result<()> {
        self.get_mmio_device(IoAddress(addr))
            .map(|(device, base)| device.read(base, IoAddress(addr - base.raw_value()), data))
            .ok_or(Error::NoDevice)
    }

    /// Handle VM IO Exit events triggered by trapped MMIO write accesses.
    ///
    /// Return error if failed to get the device.
    pub fn mmio_write(&self, addr: u64, data: &[u8]) -> Result<()> {
        self.get_mmio_device(IoAddress(addr))
            .map(|(device, base)| device.write(base, IoAddress(addr - base.raw_value()), data))
            .ok_or(Error::NoDevice)
    }

    /// Get the registered device handling the trapped MMIO address `addr`.
    fn get_mmio_device(&self, addr: IoAddress) -> Option<(&Arc<dyn DeviceIo>, IoAddress)> {
        let range = IoRange::new_mmio_range(addr.raw_value(), 0);
        if let Some((range, dev)) = self.mmio_bus.range(..=&range).nth_back(0) {
            if (addr.raw_value() - range.base.raw_value()) < range.size.raw_value() {
                return Some((dev, range.base));
            }
        }
        None
    }
}

impl IoManager {
    /// Handle VM IO Exit events triggered by trapped PIO read accesses.
    ///
    /// Return error if failed to get the device.
    pub fn pio_read(&self, addr: u16, data: &mut [u8]) -> Result<()> {
        self.get_pio_device(PioAddress(addr))
            .map(|(device, base)| device.pio_read(base, PioAddress(addr - base.raw_value()), data))
            .ok_or(Error::NoDevice)
    }

    /// Handle VM IO Exit events triggered by trapped PIO write accesses.
    ///
    /// Return error if failed to get the device.
    pub fn pio_write(&self, addr: u16, data: &[u8]) -> Result<()> {
        self.get_pio_device(PioAddress(addr))
            .map(|(device, base)| device.pio_write(base, PioAddress(addr - base.raw_value()), data))
            .ok_or(Error::NoDevice)
    }

    /// Get the registered device handling the trapped PIO address `addr`.
    fn get_pio_device(&self, addr: PioAddress) -> Option<(&Arc<dyn DeviceIo>, PioAddress)> {
        let range = IoRange::new_pio_range(addr.raw_value(), 0);
        if let Some((range, dev)) = self.pio_bus.range(..=&range).nth_back(0) {
            if (addr.raw_value() as u64 - range.base.raw_value()) < range.size.raw_value() {
                return Some((dev, PioAddress(range.base.0 as u16)));
            }
        }
        None
    }
}

impl PartialEq for IoManager {
    fn eq(&self, other: &IoManager) -> bool {
        if self.pio_bus.len() != other.pio_bus.len() {
            return false;
        }
        if self.mmio_bus.len() != other.mmio_bus.len() {
            return false;
        }

        for (io_range, device_io) in self.pio_bus.iter() {
            if !other.pio_bus.contains_key(io_range) {
                return false;
            }
            let other_device_io = &other.pio_bus[io_range];
            if device_io.get_trapped_io_resources() != other_device_io.get_trapped_io_resources() {
                return false;
            }
        }

        for (io_range, device_io) in self.mmio_bus.iter() {
            if !other.mmio_bus.contains_key(io_range) {
                return false;
            }
            let other_device_io = &other.mmio_bus[io_range];
            if device_io.get_trapped_io_resources() != other_device_io.get_trapped_io_resources() {
                return false;
            }
        }

        true
    }
}

/// Trait for IO manager context object to support device hotplug at runtime.
///
/// The `IoManagerContext` objects are passed to devices by the IO manager, so the devices could
/// use it to hot-add/hot-remove other devices at runtime. It provides a transaction mechanism
/// to hot-add/hot-remove devices.
pub trait IoManagerContext {
    /// Type of context object passed to the callbacks.
    type Context;

    /// Begin a transaction and return a context object.
    ///
    /// The returned context object must be passed to commit_tx() or cancel_tx() later.
    fn begin_tx(&self) -> Self::Context;

    /// Commit the transaction.
    fn commit_tx(&self, ctx: Self::Context);

    /// Cancel the transaction.
    fn cancel_tx(&self, ctx: Self::Context);

    /// Register a new device with its associated resources to the IO manager.
    ///
    /// # Arguments
    ///
    /// * `ctx`: context object returned by begin_tx().
    /// * `device`: device instance object to be registered
    /// * `resources`: resources representing trapped MMIO/PIO address ranges. Only MMIO/PIO address
    ///    ranges will be handled, and other types of resource will be ignored. So the caller does
    ///    not need to filter out non-MMIO/PIO resources.
    fn register_device_io(
        &self,
        ctx: &mut Self::Context,
        device: Arc<dyn DeviceIo>,
        resources: &[Resource],
    ) -> Result<()>;

    /// Unregister a device from the IO manager.
    ///
    /// # Arguments
    ///
    /// * `ctx`: context object returned by begin_tx().
    /// * `resources`: resource list containing all trapped address ranges for the device.
    fn unregister_device_io(&self, ctx: &mut Self::Context, resources: &[Resource]) -> Result<()>;
}

impl<T: IoManagerContext> IoManagerContext for Arc<T> {
    type Context = T::Context;

    fn begin_tx(&self) -> Self::Context {
        self.deref().begin_tx()
    }

    fn commit_tx(&self, ctx: Self::Context) {
        self.deref().commit_tx(ctx)
    }

    fn cancel_tx(&self, ctx: Self::Context) {
        self.deref().cancel_tx(ctx)
    }

    fn register_device_io(
        &self,
        ctx: &mut Self::Context,
        device: Arc<dyn DeviceIo>,
        resources: &[Resource],
    ) -> std::result::Result<(), Error> {
        self.deref().register_device_io(ctx, device, resources)
    }

    fn unregister_device_io(
        &self,
        ctx: &mut Self::Context,
        resources: &[Resource],
    ) -> std::result::Result<(), Error> {
        self.deref().unregister_device_io(ctx, resources)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::Mutex;

    use super::*;
    use crate::resources::DeviceResources;

    const PIO_ADDRESS_SIZE: u16 = 4;
    const PIO_ADDRESS_BASE: u16 = 0x40;
    const MMIO_ADDRESS_SIZE: u64 = 0x8765_4321;
    const MMIO_ADDRESS_BASE: u64 = 0x1234_5678;
    const LEGACY_IRQ: u32 = 4;
    const CONFIG_DATA: u32 = 0x1234;

    struct DummyDevice {
        config: Mutex<u32>,
    }

    impl DummyDevice {
        fn new(config: u32) -> Self {
            DummyDevice {
                config: Mutex::new(config),
            }
        }
    }

    impl DeviceIo for DummyDevice {
        fn read(&self, _base: IoAddress, _offset: IoAddress, data: &mut [u8]) {
            if data.len() > 4 {
                return;
            }
            for (idx, iter) in data.iter_mut().enumerate() {
                let config = self.config.lock().expect("failed to acquire lock");
                *iter = (*config >> (idx * 8) & 0xff) as u8;
            }
        }

        fn write(&self, _base: IoAddress, _offset: IoAddress, data: &[u8]) {
            let mut config = self.config.lock().expect("failed to acquire lock");
            *config = u32::from(data[0]) & 0xff;
        }

        fn pio_read(&self, _base: PioAddress, _offset: PioAddress, data: &mut [u8]) {
            if data.len() > 4 {
                return;
            }
            for (idx, iter) in data.iter_mut().enumerate() {
                let config = self.config.lock().expect("failed to acquire lock");
                *iter = (*config >> (idx * 8) & 0xff) as u8;
            }
        }

        fn pio_write(&self, _base: PioAddress, _offset: PioAddress, data: &[u8]) {
            let mut config = self.config.lock().expect("failed to acquire lock");
            *config = u32::from(data[0]) & 0xff;
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_clone_io_manager() {
        let mut io_mgr = IoManager::new();
        let dummy = DummyDevice::new(0);
        let dum = Arc::new(dummy);

        let mut resource: Vec<Resource> = Vec::new();
        let mmio = Resource::MmioAddressRange {
            base: MMIO_ADDRESS_BASE,
            size: MMIO_ADDRESS_SIZE,
        };
        let irq = Resource::LegacyIrq(LEGACY_IRQ);

        resource.push(mmio);
        resource.push(irq);

        let pio = Resource::PioAddressRange {
            base: PIO_ADDRESS_BASE,
            size: PIO_ADDRESS_SIZE,
        };
        resource.push(pio);

        assert!(io_mgr.register_device_io(dum.clone(), &resource).is_ok());

        let io_mgr2 = io_mgr.clone();
        assert_eq!(io_mgr2.mmio_bus.len(), 1);

        assert_eq!(io_mgr2.pio_bus.len(), 1);

        let (dev, addr) = io_mgr2
            .get_mmio_device(IoAddress(MMIO_ADDRESS_BASE + 1))
            .unwrap();
        assert_eq!(Arc::strong_count(dev), 5);

        assert_eq!(addr, IoAddress(MMIO_ADDRESS_BASE));

        drop(io_mgr);
        assert_eq!(Arc::strong_count(dev), 3);

        drop(io_mgr2);
        assert_eq!(Arc::strong_count(&dum), 1);
    }

    #[test]
    fn test_register_unregister_device_io() {
        let mut io_mgr = IoManager::new();
        let dummy = DummyDevice::new(0);
        let dum = Arc::new(dummy);

        let mut resources = DeviceResources::new();
        let mmio = Resource::MmioAddressRange {
            base: MMIO_ADDRESS_BASE,
            size: MMIO_ADDRESS_SIZE,
        };
        let pio = Resource::PioAddressRange {
            base: PIO_ADDRESS_BASE,
            size: PIO_ADDRESS_SIZE,
        };
        let irq = Resource::LegacyIrq(LEGACY_IRQ);

        resources.append(mmio);
        resources.append(pio);
        resources.append(irq);

        assert!(io_mgr.register_device_io(dum.clone(), &resources).is_ok());
        assert!(io_mgr.register_device_io(dum, &resources).is_err());
        assert!(io_mgr.unregister_device_io(&resources).is_ok())
    }

    #[test]
    fn test_mmio_read_write() {
        let mut io_mgr: IoManager = Default::default();
        let dum = Arc::new(DummyDevice::new(CONFIG_DATA));
        let mut resource: Vec<Resource> = Vec::new();

        let mmio = Resource::MmioAddressRange {
            base: MMIO_ADDRESS_BASE,
            size: MMIO_ADDRESS_SIZE,
        };
        resource.push(mmio);
        assert!(io_mgr.register_device_io(dum.clone(), &resource).is_ok());

        let mut data = [0; 4];
        assert!(io_mgr.mmio_read(MMIO_ADDRESS_BASE, &mut data).is_ok());
        assert_eq!(data, [0x34, 0x12, 0, 0]);

        assert!(io_mgr
            .mmio_read(MMIO_ADDRESS_BASE + MMIO_ADDRESS_SIZE, &mut data)
            .is_err());

        data = [0; 4];
        assert!(io_mgr.mmio_write(MMIO_ADDRESS_BASE, &data).is_ok());
        assert_eq!(*dum.config.lock().unwrap(), 0);

        assert!(io_mgr
            .mmio_write(MMIO_ADDRESS_BASE + MMIO_ADDRESS_SIZE, &data)
            .is_err());
    }

    #[test]
    fn test_pio_read_write() {
        let mut io_mgr: IoManager = Default::default();
        let dum = Arc::new(DummyDevice::new(CONFIG_DATA));
        let mut resource: Vec<Resource> = Vec::new();

        let pio = Resource::PioAddressRange {
            base: PIO_ADDRESS_BASE,
            size: PIO_ADDRESS_SIZE,
        };
        resource.push(pio);
        assert!(io_mgr.register_device_io(dum.clone(), &resource).is_ok());

        let mut data = [0; 4];
        assert!(io_mgr.pio_read(PIO_ADDRESS_BASE, &mut data).is_ok());
        assert_eq!(data, [0x34, 0x12, 0, 0]);

        assert!(io_mgr
            .pio_read(PIO_ADDRESS_BASE + PIO_ADDRESS_SIZE, &mut data)
            .is_err());

        data = [0; 4];
        assert!(io_mgr.pio_write(PIO_ADDRESS_BASE, &data).is_ok());
        assert_eq!(*dum.config.lock().unwrap(), 0);

        assert!(io_mgr
            .pio_write(PIO_ADDRESS_BASE + PIO_ADDRESS_SIZE, &data)
            .is_err());
    }

    #[test]
    fn test_device_manager_data_structs() {
        let range1 = IoRange::new_mmio_range(0x1000, 0x1000);
        let range2 = IoRange::new_mmio_range(0x1000, 0x2000);
        let range3 = IoRange::new_mmio_range(0x2000, 0x1000);

        assert_eq!(range1, range1.clone());
        assert_eq!(range1, range2);
        assert!(range1 < range3);
    }

    #[test]
    fn test_error_code() {
        let err = super::Error::DeviceOverlap;

        assert!(err.source().is_none());
        assert_eq!(
            format!("{err}"),
            "device address conflicts with existing devices"
        );

        let err = super::Error::NoDevice;
        assert!(err.source().is_none());
        assert_eq!(format!("{err:#?}"), "NoDevice");
    }

    #[test]
    fn test_io_manager_partial_eq() {
        let mut io_mgr1 = IoManager::new();
        let mut io_mgr2 = IoManager::new();
        let dummy1 = Arc::new(DummyDevice::new(0));
        let dummy2 = Arc::new(DummyDevice::new(0));

        let mut resources1 = DeviceResources::new();
        let mut resources2 = DeviceResources::new();

        let mmio = Resource::MmioAddressRange {
            base: MMIO_ADDRESS_BASE,
            size: MMIO_ADDRESS_SIZE,
        };
        let pio = Resource::PioAddressRange {
            base: PIO_ADDRESS_BASE,
            size: PIO_ADDRESS_SIZE,
        };

        resources1.append(mmio.clone());
        resources1.append(pio.clone());

        resources2.append(mmio);
        resources2.append(pio);

        io_mgr1.register_device_io(dummy1, &resources1).unwrap();
        io_mgr2.register_device_io(dummy2, &resources2).unwrap();

        assert!(io_mgr1 == io_mgr2);
    }

    #[test]
    fn test_io_manager_partial_neq() {
        let mut io_mgr1 = IoManager::new();
        let mut io_mgr2 = IoManager::new();
        let dummy1 = Arc::new(DummyDevice::new(0));
        let dummy2 = Arc::new(DummyDevice::new(0));

        let mut resources1 = DeviceResources::new();
        let mut resources2 = DeviceResources::new();

        let mmio = Resource::MmioAddressRange {
            base: MMIO_ADDRESS_BASE,
            size: MMIO_ADDRESS_SIZE,
        };
        let pio = Resource::PioAddressRange {
            base: PIO_ADDRESS_BASE,
            size: PIO_ADDRESS_SIZE,
        };

        resources1.append(mmio.clone());
        resources1.append(pio);

        resources2.append(mmio);

        io_mgr1.register_device_io(dummy1, &resources1).unwrap();
        io_mgr2.register_device_io(dummy2, &resources2).unwrap();

        assert!(io_mgr1 != io_mgr2);
    }
}
