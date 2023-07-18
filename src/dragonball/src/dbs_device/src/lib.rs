// Copyright 2020 Alibaba Cloud. All Rights Reserved.
// Copyright © 2019 Intel Corporation. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![deny(missing_docs)]

//! Device model for Dragonball Secure Sandbox.
//!
//! The `dbs-device` crate, as a counterpart of [vm-device], defines device model for the
//! Dragonball Secure Sandbox. The `dbs-device` crate shares some common concepts and data structures
//! with [vm-device], but it also diverges from [vm-device] due to different VMM designs.
//!
//! [vm-device]: https://github.com/rust-vmm/vm-device

use std::any::Any;
use std::cmp::{Ord, PartialOrd};
use std::convert::TryFrom;
use std::sync::Mutex;

use self::resources::DeviceResources;

pub mod device_manager;
pub mod resources;

/// Size of MMIO range/access request.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct IoSize(pub u64);

impl IoSize {
    /// Get the raw value as u64 to make operation simple.
    #[inline]
    pub fn raw_value(self) -> u64 {
        self.0
    }
}

impl From<u64> for IoSize {
    #[inline]
    fn from(size: u64) -> Self {
        IoSize(size)
    }
}

impl From<IoSize> for u64 {
    #[inline]
    fn from(size: IoSize) -> Self {
        size.0
    }
}

/// Memory Mapped IO (MMIO) address.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct IoAddress(pub u64);

impl IoAddress {
    /// Get the raw value of IO Address to make operation simple.
    #[inline]
    pub fn raw_value(self) -> u64 {
        self.0
    }
}

impl From<u64> for IoAddress {
    #[inline]
    fn from(addr: u64) -> Self {
        IoAddress(addr)
    }
}

impl From<IoAddress> for u64 {
    #[inline]
    fn from(addr: IoAddress) -> Self {
        addr.0
    }
}

type PioAddressType = u16;

/// Size of Port I/O range/request.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PioSize(pub PioAddressType);

impl PioSize {
    /// Get the raw value as u64 to make operation simple.
    #[inline]
    pub fn raw_value(self) -> PioAddressType {
        self.0
    }
}

impl From<PioAddressType> for PioSize {
    #[inline]
    fn from(size: PioAddressType) -> Self {
        PioSize(size)
    }
}

impl From<PioSize> for PioAddressType {
    #[inline]
    fn from(size: PioSize) -> Self {
        size.0
    }
}

impl TryFrom<IoSize> for PioSize {
    type Error = IoSize;

    #[inline]
    fn try_from(size: IoSize) -> Result<Self, Self::Error> {
        if size.raw_value() <= std::u16::MAX as u64 {
            Ok(PioSize(size.raw_value() as PioAddressType))
        } else {
            Err(size)
        }
    }
}

impl From<PioSize> for IoSize {
    #[inline]
    fn from(size: PioSize) -> Self {
        IoSize(size.raw_value() as u64)
    }
}

/// Port IO (PIO) address.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PioAddress(pub PioAddressType);

impl PioAddress {
    /// Get the raw value of IO Address to make operation simple.
    #[inline]
    pub fn raw_value(self) -> PioAddressType {
        self.0
    }
}

impl From<PioAddressType> for PioAddress {
    #[inline]
    fn from(addr: PioAddressType) -> Self {
        PioAddress(addr)
    }
}

impl From<PioAddress> for PioAddressType {
    #[inline]
    fn from(addr: PioAddress) -> Self {
        addr.0
    }
}

impl TryFrom<IoAddress> for PioAddress {
    type Error = IoAddress;

    #[inline]
    fn try_from(addr: IoAddress) -> Result<Self, Self::Error> {
        if addr.0 <= std::u16::MAX as u64 {
            Ok(PioAddress(addr.raw_value() as PioAddressType))
        } else {
            Err(addr)
        }
    }
}

impl From<PioAddress> for IoAddress {
    #[inline]
    fn from(addr: PioAddress) -> Self {
        IoAddress(addr.raw_value() as u64)
    }
}

/// Trait for device to handle trapped MMIO/PIO access requests with interior mutability
/// for high performance.
///
/// Any device which needs to trap MMIO/PIO access requests should implement the [DeviceIo] or
/// [DeviceIoMut] trait and register itself to the [IoManager](crate::device_manager::IoManager)
/// with those trapped IO address ranges. When the guest access those trapped address ranges,
/// the access request will be routed to the registered callbacks.
///
/// The [DeviceIo] trait adopts the interior mutability pattern so we can get a real concurrent
/// multiple threads handling. For device backend drivers not focusing on high performance,
/// the Mutex<T: DeviceIoMut> adapter may be used to simplify the implementation.
#[allow(unused_variables)]
pub trait DeviceIo: Send + Sync {
    /// Read from the MMIO address `base + offset` into `data`.
    fn read(&self, base: IoAddress, offset: IoAddress, data: &mut [u8]) {}

    /// Write from `data` to the MMIO address `base + offset`.
    fn write(&self, base: IoAddress, offset: IoAddress, data: &[u8]) {}

    /// Read from port `base + offset` into `data`.
    fn pio_read(&self, base: PioAddress, offset: PioAddress, data: &mut [u8]) {}

    /// Write from `data` to the port `base + offset`.
    fn pio_write(&self, base: PioAddress, offset: PioAddress, data: &[u8]) {}

    /// Get resources assigned to the device.
    fn get_assigned_resources(&self) -> DeviceResources {
        DeviceResources::new()
    }

    /// Get the trapped IO address ranges for the device.
    ///
    /// Only MMIO/PIO address ranges in the resource list will be handled, other resources will be
    /// ignored. So the device does not need to filter out non-MMIO/PIO resources.
    fn get_trapped_io_resources(&self) -> DeviceResources {
        self.get_assigned_resources()
    }

    /// Used to downcast to the specific type.
    fn as_any(&self) -> &dyn Any;
}

/// Trait for device to handle trapped MMIO/PIO access requests.
///
/// Many device backend drivers will mutate itself when handling IO requests. The [DeviceIo] trait
/// assumes interior mutability, but it's a little complex to support interior mutability.
/// So the Mutex<T: DeviceIoMut> adapter may be used to ease device backend driver implementations.
///
/// The Mutex<T: DeviceIoMut> adapter is an zero overhead abstraction without performance penalty.
#[allow(unused_variables)]
pub trait DeviceIoMut {
    /// Read from the MMIO address `base + offset` into `data`.
    fn read(&mut self, base: IoAddress, offset: IoAddress, data: &mut [u8]) {}

    /// Write from `data` to the MMIO address `base + offset`.
    fn write(&mut self, base: IoAddress, offset: IoAddress, data: &[u8]) {}

    /// Read from port `base + offset` into `data`.
    fn pio_read(&mut self, base: PioAddress, offset: PioAddress, data: &mut [u8]) {}

    /// Write from `data` to the port `base + offset`.
    fn pio_write(&mut self, base: PioAddress, offset: PioAddress, data: &[u8]) {}

    /// Get resources assigned to the device.
    fn get_assigned_resources(&self) -> DeviceResources {
        DeviceResources::new()
    }

    /// Get the trapped IO address ranges for the device.
    ///
    /// Only MMIO/PIO address ranges in the resource list will be handled, other resources will be
    /// ignored. So the device does not need to filter out non-MMIO/PIO resources.
    fn get_trapped_io_resources(&self) -> DeviceResources {
        self.get_assigned_resources()
    }
}

impl<T: DeviceIoMut + Send + 'static> DeviceIo for Mutex<T> {
    fn read(&self, base: IoAddress, offset: IoAddress, data: &mut [u8]) {
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.lock().unwrap().read(base, offset, data)
    }

    fn write(&self, base: IoAddress, offset: IoAddress, data: &[u8]) {
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.lock().unwrap().write(base, offset, data)
    }

    fn pio_read(&self, base: PioAddress, offset: PioAddress, data: &mut [u8]) {
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.lock().unwrap().pio_read(base, offset, data)
    }

    fn pio_write(&self, base: PioAddress, offset: PioAddress, data: &[u8]) {
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.lock().unwrap().pio_write(base, offset, data)
    }

    fn get_assigned_resources(&self) -> DeviceResources {
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.lock().unwrap().get_assigned_resources()
    }

    fn get_trapped_io_resources(&self) -> DeviceResources {
        // Safe to unwrap() because we don't expect poisoned lock here.
        self.lock().unwrap().get_trapped_io_resources()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;
    use std::sync::Arc;

    use super::*;

    #[derive(Default)]
    struct MockDevice {
        data: Mutex<u8>,
    }

    impl DeviceIo for MockDevice {
        fn read(&self, _base: IoAddress, _offset: IoAddress, data: &mut [u8]) {
            data[0] = *self.data.lock().unwrap();
        }

        fn write(&self, _base: IoAddress, _offset: IoAddress, data: &[u8]) {
            *self.data.lock().unwrap() = data[0];
        }

        fn pio_read(&self, _base: PioAddress, _offset: PioAddress, data: &mut [u8]) {
            data[0] = *self.data.lock().unwrap();
        }

        fn pio_write(&self, _base: PioAddress, _offset: PioAddress, data: &[u8]) {
            *self.data.lock().unwrap() = data[0];
        }
        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[derive(Default)]
    struct MockDeviceMut {
        data: u8,
    }

    impl DeviceIoMut for MockDeviceMut {
        fn read(&mut self, _base: IoAddress, _offset: IoAddress, data: &mut [u8]) {
            data[0] = self.data;
        }

        fn write(&mut self, _base: IoAddress, _offset: IoAddress, data: &[u8]) {
            self.data = data[0];
        }

        fn pio_read(&mut self, _base: PioAddress, _offset: PioAddress, data: &mut [u8]) {
            data[0] = self.data;
        }

        fn pio_write(&mut self, _base: PioAddress, _offset: PioAddress, data: &[u8]) {
            self.data = data[0];
        }
    }

    fn register_device(device: Arc<dyn DeviceIo>) {
        device.write(IoAddress(0), IoAddress(0), &[0x10u8]);
        let mut buf = [0x0u8];
        device.read(IoAddress(0), IoAddress(0), &mut buf);
        assert_eq!(buf[0], 0x10);

        {
            device.pio_write(PioAddress(0), PioAddress(0), &[0x10u8]);
            let mut buf = [0x0u8];
            device.pio_read(PioAddress(0), PioAddress(0), &mut buf);
            assert_eq!(buf[0], 0x10);
        }

        // test trait's default implementation
        let resource = DeviceResources::new();
        assert_eq!(resource, device.get_assigned_resources());
        assert_eq!(resource, device.get_trapped_io_resources());
    }

    #[test]
    fn test_device_io_adapter() {
        let device = Arc::new(MockDevice::default());

        register_device(device.clone());
        assert_eq!(*device.data.lock().unwrap(), 0x010);
    }

    #[test]
    fn test_device_io_mut_adapter() {
        let device_mut = Arc::new(Mutex::new(MockDeviceMut::default()));

        register_device(device_mut.clone());
        assert_eq!(device_mut.lock().unwrap().data, 0x010);
    }

    #[test]
    fn test_io_data_struct() {
        let io_size = IoSize::from(0x1111u64);
        assert_eq!(io_size.raw_value(), 0x1111u64);
        assert_eq!(u64::from(io_size), 0x1111u64);
        assert_eq!(io_size, io_size.clone());
        let io_size1 = IoSize::from(0x1112u64);
        assert!(io_size < io_size1);

        let io_addr = IoAddress::from(0x1234u64);
        assert_eq!(io_addr.raw_value(), 0x1234u64);
        assert_eq!(u64::from(io_addr), 0x1234u64);
        assert_eq!(io_addr, io_addr.clone());
        let io_addr1 = IoAddress::from(0x1235u64);
        assert!(io_addr < io_addr1);
    }

    #[test]
    fn test_pio_data_struct() {
        let pio_size = PioSize::from(0x1111u16);
        assert_eq!(pio_size.raw_value(), 0x1111u16);
        assert_eq!(u16::from(pio_size), 0x1111u16);
        assert_eq!(pio_size, pio_size.clone());
        let pio_size1 = PioSize::from(0x1112u16);
        assert!(pio_size < pio_size1);

        let pio_size = PioSize::try_from(IoSize(0x1111u64)).unwrap();
        assert_eq!(pio_size.raw_value(), 0x1111u16);

        assert!(PioSize::try_from(IoSize(std::u16::MAX as u64 + 1)).is_err());

        let io_size = IoSize::from(PioSize::from(0x1111u16));
        assert_eq!(io_size.raw_value(), 0x1111u64);

        let pio_addr = PioAddress::from(0x1234u16);
        assert_eq!(pio_addr.raw_value(), 0x1234u16);
        assert_eq!(u16::from(pio_addr), 0x1234u16);
        assert_eq!(pio_addr, pio_addr.clone());
        let pio_addr1 = PioAddress::from(0x1235u16);
        assert!(pio_addr < pio_addr1);

        assert!(PioAddress::try_from(IoAddress::from(0x12_3456u64)).is_err());
        assert!(PioAddress::try_from(IoAddress::from(0x1234u64)).is_ok());
        assert_eq!(IoAddress::from(pio_addr).raw_value(), 0x1234u64);
    }
}
