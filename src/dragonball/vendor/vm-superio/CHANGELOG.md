# Changelog

# v0.5.0

## Added

- Added `RtcState`, and support for saving and restoring the state of the `Rtc`
  device. This support is useful for snapshot use cases, such as live
  migration ([#65](https://github.com/rust-vmm/vm-superio/pull/65)).

## Fixed

- Fixed potential overflow in the `Rtc` implementation caused by an invalid
  offset ([#65](https://github.com/rust-vmm/vm-superio/pull/65)).

# v0.4.0

## Added

- Added `in_buffer_empty` to SerialEvents trait. This helps with handling
  the registration of I/O events related to the serial input
  ([#63](https://github.com/rust-vmm/vm-superio/pull/63)).

## Changed

- Changed `RTC` to `Rtc` and `RTCEvents` to `RtcEvents` as part of the Rust
  version update to 1.52.1
  ([#57](https://github.com/rust-vmm/vm-superio/pull/57)).

# v0.3.0

## Fixed

- Fixed implementation of Data Register (DR) which caused the guest time to be
  in the year 1970 ([#47](https://github.com/rust-vmm/vm-superio/issues/47)).

# v0.2.0

## Added

- Added emulation support for an i8042 controller that only handles the CPU
  reset ([#11](https://github.com/rust-vmm/vm-superio/pull/11)).
- Added `SerialEvents` trait, which can be implemented by a backend that wants
  to keep track of serial events using metrics, logs etc
  ([#5](https://github.com/rust-vmm/vm-superio/issues/5)).
- Added a threat model to the serial console documentation
  ([#16](https://github.com/rust-vmm/vm-superio/issues/16)).
- Added emulation support for an ARM PL031 Real Time Clock
  ([#22](https://github.com/rust-vmm/vm-superio/issues/22)), and the `RTCEvents`
  trait, used for keeping track of RTC events
  ([#34](https://github.com/rust-vmm/vm-superio/issues/34)).
- Added an implementation for `Arc<EV>` for both serial console and RTC device
  ([#40](https://github.com/rust-vmm/vm-superio/pull/40)).
- Added methods for retrieving a reference to the events object for both serial
  console and RTC device
  ([#40](https://github.com/rust-vmm/vm-superio/pull/40)).

## Changed

- Changed the notification mechanism from EventFd to the Trigger abstraction
  for both serial console and i8042
  ([#7](https://github.com/rust-vmm/vm-superio/issues/7)).

## Fixed

- Limited the maximum number of bytes allowed at a time, when enqueuing input
  for serial, to 64 (FIFO_SIZE) to avoid memory pressure
  ([#17](https://github.com/rust-vmm/vm-superio/issues/17)).
- Fixed possible indefinite blocking of the serial driver by always sending the
  THR Empty interrupt to it when trying to write to the device
  ([#23](https://github.com/rust-vmm/vm-superio/issues/23)).

# v0.1.0

This is the first `vm-superio` release.
The `vm-superio` crate provides emulation for legacy devices. For now, it offers
this support only for the Linux serial console.
