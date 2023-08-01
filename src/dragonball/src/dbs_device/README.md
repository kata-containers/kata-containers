# dbs-device

The `dbs-device` crate, as a counterpart of [`vm-device`], defines device model for the Dragonball Secure Sandbox.
The `dbs-device` crate shares some common concepts and data structures with [`vm-device`], but it also diverges from
[`vm-device`] due to different VMM designs.

The dbs-device crate provides:

- [`DeviceIo`] and [`DeviceIoMut`]: trait for device to handle trapped MMIO/PIO access requests.
- [`IoManager`]: IO manager to handle trapped MMIO/PIO access requests.
- [`IoManagerContext`]: trait for IO manager context object to support device hotplug at runtime.
- [`ResourceConstraint`], [Resource] and [`DeviceResources`]: resource allocation requirements and constraints.

## Design

The dbs-device crate is designed to support the virtual machine's device model.

The core concepts of device model are [Port I/O](https://wiki.osdev.org/I/O_Ports) and 
[Memory-mapped I/O](https://en.wikipedia.org/wiki/Memory-mapped_I/O),
which are two main methods of performing I/O between CPU and devices.

The device model provided by the dbs-device crate works as below:
- The VMM creates a global resource manager, device manager and IO manager.
- The device manager creates virtual devices configured by the VMM
    - create device object
    - query device allocation requirements and constraints, the device returns an array of [`ResourceConstraint`].
    - allocate resources for device from the resource manager, resource manager returns a [`DeviceResources`] object.
    - assign the allocated resources to the device.
 - The device manager register devices to the IO manager.
    - query trapped address ranges by calling [`DeviceIo::get_trapped_io_resources()`]
    - register the device to the IO manager with trapped address range
 - The guest access those trapped MMIO/PIO address ranges, and triggers VM IO Exit events to trap into the VMM.
 - The VMM parses the VM exit events and dispatch those events to the IO manager.
 - The IO manager looks up device by searching trapped address ranges, and call the device's [`DeviceIO`]
   handler to process those trapped MMIO/PIO access requests.

## Usage

First, a VM needs to create an [`IoManager`] to help it dispatch I/O events to devices.
And an [`IoManager`] has two types of bus, the PIO bus and the MMIO bus, to handle different types of IO.

Then, when creating a device, it needs to implement the [`DeviceIo`] or [`DeviceIoMut`] trait to receive read or write
events send by driver in guest OS:
- `read()` and `write()` methods is used to deal with MMIO events
- `pio_read()` and `pio_write()` methods is used to deal with PIO events
- `get_assigned_resources()` method is used to get all resources assigned to the device
- `get_trapped_io_resources()` method is used to get only MMIO/PIO resources assigned to the device

The difference of [`DeviceIo`] and [`DeviceIoMut`] is the reference type of `self` passed to method:
- [`DeviceIo`] trait would pass a immutable reference `&self` to method, so the implementation of device would provide
  interior mutability and thread-safe protection itself
- [`DeviceIoMut`] trait would pass a mutable reference `&mut self` to method, and it can give mutability to device
  which is wrapped by `Mutex` directly to simplify the difficulty of achieving interior mutability.
  
Additionally, the [`DeviceIo`] trait has an auto implement for `Mutex<T: DeviceIoMut>`

Last, the device needs to be added to [`IoManager`] by using `register_device_io()`, and the function would add device
to PIO bus and/or MMIO bus by the resources it have. If a device has not only MMIO resource but PIO resource,
it would be added to both pio bus and mmio bus. So the device would wrapped by `Arc<T>`.

From now on, the [`IoManager`] will dispatch I/O requests for the registered address ranges to the device.

## Examples


```rust
use std::sync::Arc;

use dbs_device::device_manager::IoManager;
use dbs_device::resources::{DeviceResources, Resource};
use dbs_device::{DeviceIo, IoAddress, PioAddress};

struct DummyDevice {}

impl DeviceIo for DummyDevice {
    fn read(&self, base: IoAddress, offset: IoAddress, data: &mut [u8]) {
        println!(
            "mmio read, base: 0x{:x}, offset: 0x{:x}",
            base.raw_value(),
            offset.raw_value()
        );
    }

    fn write(&self, base: IoAddress, offset: IoAddress, data: &[u8]) {
        println!(
            "mmio write, base: 0x{:x}, offset: 0x{:x}",
            base.raw_value(),
            offset.raw_value()
        );
    }

    fn pio_read(&self, base: PioAddress, offset: PioAddress, data: &mut [u8]) {
        println!(
            "pio read, base: 0x{:x}, offset: 0x{:x}",
            base.raw_value(),
            offset.raw_value()
        );
    }

    fn pio_write(&self, base: PioAddress, offset: PioAddress, data: &[u8]) {
        println!(
            "pio write, base: 0x{:x}, offset: 0x{:x}",
            base.raw_value(),
            offset.raw_value()
        );
    }
}

// Allocate resources for device
let mut resources = DeviceResources::new();
resources.append(Resource::MmioAddressRange {
    base: 0,
    size: 4096,
});
resources.append(Resource::PioAddressRange { base: 0, size: 32 });

// Register device to `IoManager` with resources
let device = Arc::new(DummyDevice {});
let mut manager = IoManager::new();
manager.register_device_io(device, &resources).unwrap();

// Dispatch I/O event from `IoManager` to device
manager.mmio_write(0, &vec![0, 1]).unwrap();

let mut buffer = vec![0; 4];
manager.pio_read(0, &mut buffer);
```

## License

This project is licensed under [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0).

[DeviceIo::get_trapped_io_resources()]: https://docs.rs/dbs-device/0.1.0/dbs_device/trait.DeviceIo.html#method.get_trapped_io_resources
[DeviceIo]: src/lib.rs
[DeviceIoMut]: src/lib.rs
[IoManager]: src/device_manager.rs
[IoManagerContext]: src/device_manager.rs
[ResourceConstraint]: src/resources.rs
[Resource]: src/resources.rs
[DeviceResources]: src/resources.rs
[vm-device]: https://github.com/rust-vmm/vm-device
