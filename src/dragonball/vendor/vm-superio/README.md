# vm-superio


`vm-superio` provides emulation for legacy devices. For now, it offers this
support only for the
[Linux serial console](https://en.wikipedia.org/wiki/Linux_console), a minimal
[i8042 PS/2 Controller](https://wiki.osdev.org/%228042%22_PS/2_Controller) and
an
[ARM PL031 Real Time Clock](https://developer.arm.com/documentation/ddi0224/c/Programmers-model).
To enable snapshot use cases, such as live migration, it also provides support
for saving and restoring the state, and for persisting it (for now only for the
Rtc device).
In order to achieve this, and to keep a clear separation of concerns,
`vm-superio` is a
[workspace](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html),
containing the following crates:
- `vm-superio` - which keeps the state of the component;
- `vm-superio-ser` - which mirrors the state structure from `vm-superio` and
   adds the required version constraints on it, and derives/implements the
  required (de)serialization traits (i.e. `serde`'s `Serialize` and
  `Deserialize`; `Versionize`).

## Serial Console

### Design

The console emulation is done by emulating a simple
[UART 16550A serial port](https://en.wikipedia.org/wiki/16550_UART) with a
64-byte FIFO.
This UART is an improvement of the original
[UART 8250 serial port](https://en.wikibooks.org/w/index.php?title=Serial_Programming/8250_UART_Programming&section=15#Serial_COM_Port_Memory_and_I/O_Allocation),
mostly because of the FIFO buffers that allow storing more than one byte at a
time, which, in virtualized environments, is essential.

For a VMM to be able to use this device, besides the emulation part which is
covered in this crate, the VMM needs to do the following operations:
- add the serial port to the Bus (either PIO or MMIO)
- define the serial backend
- event handling (optional)

The following UART registers are emulated via the
[`Serial` structure](crates/vm-superio/src/serial.rs): DLL, IER, DLH, IIR, LCR,
LSR, MCR, MSR and SR (a brief, but nice presentation about these,
[here](https://www.lammertbies.nl/comm/info/serial-uart#regs)).
The Fifo Control Register (FCR) is not emulated; there is no support yet for
directly controlling the FIFOs (which, in this implementation, are always
enabled). The serial console implements only the RX FIFO (and its
corresponding RBR register). The RX buffer helps in testing the UART when
running in loopback mode and for sending more bytes to the guest in one shot.
The TX FIFO is trivially implemented by immediately writing a byte coming from
the driver to an `io::Write` object (`out`), which can be, for example,
`io::Stdout` or `io::Sink`. This object has to be provided when
[initializing the serial console](https://docs.rs/vm-superio/0.1.1/vm_superio/serial/struct.Serial.html#method.new).
A `Trigger` object is the currently used mechanism for notifying the driver
about in/out events that need to be handled.

## Threat model

Trusted actors:
* host kernel

Untrusted actors:
* guest kernel
* guest drivers

The untrusted actors can change the state of the device through reads and
writes on the Bus at the address where the device resides.

|#NR	|Threat	|Mitigation	|
|---	|---	|---	|
|1 | A malicious guest generates large memory allocations by flooding the serial console input. [CVE-2020-27173](https://nvd.nist.gov/vuln/detail/CVE-2020-27173)	|The serial console limits the number of elements in the FIFO corresponding to the serial console input to `FIFO_SIZE` (=64), returning a FIFO Full error when the limit is reached. This error MUST be handled by the crate customer. When the serial console input is connected to an event loop, the customer MUST ensure that the loop is not flooded with events coming from untrusted sources when no space is available in the FIFO.	|
|2	|A malicious guest can fill up the host disk by generating a high amount of data to be written to the serial output.	|Mitigation is not possible at the emulation layer because we are not controlling the output (`Writer`). This needs to be mitigated at the VMM level by adding a rate limiting mechanism. We recommend using as output a resource that has a fixed size (e.g. ring buffer or a named pipe).	|

### Usage

The interaction between the serial console and its driver, at the emulation
level, is done by the two `read` and `write` specific methods, which handle
one byte accesses. For sending more input, `enqueue_raw_bytes` can be used.

## i8042 PS/2 Controller

The i8042 PS/2 controller emulates, at this point, only the
[CPU reset command](https://wiki.osdev.org/%228042%22_PS/2_Controller#CPU_Reset)
which is needed for announcing the VMM about the guest's shutdown.

## ARM PL031 Real Time Clock

This module emulates the ARM PrimeCell Real Time Clock (RTC)
[PL031](https://developer.arm.com/documentation/ddi0224/c/Functional-overview/RTC-operation/RTC-operation).
The PL031 provides a long time base counter with a 1HZ counter signal and
a configurable offset.

This implementation emulates all control, peripheral ID, and PrimeCell ID
registers; however, the interrupt based on the value of the Match Register
(RTCMR) is not currently implemented (i.e., setting the Match Register has
no effect).

For a VMM to be able to use this device, the VMM needs to do the following:
- add the RTC to the Bus (either PIO or MMIO)
- provide a structure that implements RTCEvents to track the occurrence of significant events (optional)

Note that because the Match Register is the only possible source of an event,
and the Match Register is not currently implemented, no event handling
is required.

### Threat model

Trusted actors:
* host kernel

Untrusted actors:
* guest kernel
* guest drivers

The untrusted actors can change the state of the device through reads and
writes on the Bus at the address where the device resides.

|#NR	|Threat	|Mitigation	|
|---	|---	|---	|
|1	|A malicious guest writes invalid values in the Load Register to cause overflows on subsequent reads of the Data Register.	|The arithmetic operations in the RTC are checked for overflows. When such a situation occurs, the state of the device is reset.	|
|2	|A malicious guest performs reads and writes from invalid offsets (that do not correspond to the RTC registers) to cause crashes or to get access to data.	|Reads and writes of invalid offsets are denied by the emulation, and an `invalid_read/write` event is called. These events can be implemented by VMMs, and extend them to generate alarms (and for example stop the execution of the malicious guest).	|

## Save/restore state support

This support is offered for now only for the `Rtc` device, by the following
abstractions:
- `RtcState` -> which keeps the hardware state of the `Rtc`;
- `RtcStateSer` -> which can be used by customers who need an `RtcState` that
  is also `(De)Serialize` and/or `Versionize`. If the customers want a
  different state than the upstream one, then they can implement `From` (or
  similar mechanisms) in their products to convert the upstream state to the
  desired product state.

A detailed design document for the save/restore state support in rust-vmm can
be found [here](https://github.com/rust-vmm/community/pull/118/files).

### Compatibility between `vm-superio` and `vm-superio-ser` versions

Each time there's a change in a state from `vm-superio`, that change needs to
be propagated in the corresponding state from `vm-superio-ser`. To keep the
compatibility between the release versions of these two crates, once we have a
new release of `vm-superio` that implied a change in `vm-superio-ser`, we need
to have a new release of `vm-superio-ser` as well.
Therefore, the `vm-superio-ser` crate has an exact version of `vm-superio` as
dependency.

## License

This project is licensed under either of

- [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0
- [BSD-3-Clause License](https://opensource.org/licenses/BSD-3-Clause)
