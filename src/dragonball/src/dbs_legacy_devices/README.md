# dbs-legacy-devices

`dbs-legacy-devices` provides emulation for legacy devices.

## Serial Devices

Defined a wrapper over the Serial of [`vm-superio`](https://github.com/rust-vmm/vm-superio). 
This wrapper is needed because [Orphan rules](https://doc.rust-lang.org/reference/items/implementations.html#orphan-rules),
which is one crate can not implement a trait for a struct defined in
another crate. This wrapper also contains the input field that is
missing from upstream implementation.

## i8042 Devices

Defined a wrapper over the `i8042 PS/2 Controller` of [`vm-superio`](https://github.com/rust-vmm/vm-superio). 
The i8042 PS/2 controller emulates, at this point, only the CPU reset command which is needed for announcing the VMM about the guest's shutdown.

### Acknowledgement

Part of the code is derived from the [Firecracker](https://github.com/firecracker-microvm/firecracker) project.
And modified to use [`DeviceIoMut`](../dbs_device/src/lib.rs) to support serial port to Bus.


## License

This project is licensed under [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0).
