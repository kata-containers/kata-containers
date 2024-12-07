# dbs-interrupt

Interrupts are used by hardware devices to indicate asynchronous events to the processor.
The `dbs-interrupt` crate provides traits and data structures for the `Dragonball Sandbox` to manage
interrupts for virtual and physical devices.

An interrupt alerts the processor to a high-priority condition requiring the interruption of
the current code the processor is executing. The processor responds by suspending its current activities,
saving its state, and executing a function called an interrupt handler (or an interrupt service routine, ISR)
to deal with the event. This interruption is temporary, and, after the interrupt handler finishes,
unless handling the interrupt has emitted a fatal error, the processor resumes normal activities.

Hardware interrupts are used by devices to communicate that they require attention from the
operating system, or a bare-metal program running on the CPU if there are no OSes. The act of
initiating a hardware interrupt is referred to as an interrupt request (IRQ). Different devices are
usually associated with different interrupts using a unique value associated with each interrupt.
This makes it possible to know which hardware device caused which interrupts. These interrupt values
are often called IRQ lines, or just interrupt lines.

Nowadays, IRQ lines is not the only mechanism to deliver device interrupts to processors. MSI
(Message Signaled Interrupt) is another commonly used alternative in-band method of signaling an
interrupt, using special in-band messages to replace traditional out-of-band assertion of dedicated
interrupt lines. While more complex to implement in a device, message signaled interrupts have some
significant advantages over pin-based out-of-band interrupt signaling. Message signaled interrupts
are supported in PCI bus since its version 2.2, and in later available PCI Express bus. Some non-PCI
architectures also use message signaled interrupts.

While IRQ is a term commonly used by Operating Systems when dealing with hardware interrupts, the
IRQ numbers managed by OSes are independent of the ones managed by VMM. For simplicity sake, the
term Interrupt Source is used instead of IRQ to represent both pin-based interrupts and MSI
interrupts.

A device may support multiple types of interrupts, and each type of interrupt may support one or
multiple interrupt sources. For example, a PCI device may support:

- Legacy Irq: exactly one interrupt source.
- PCI MSI Irq: 1,2,4,8,16,32 interrupt sources.
- PCI MSIx Irq: 2^n(n=0-11) interrupt sources.

A distinct Interrupt Source Identifier (ISID) will be assigned to each interrupt source. An ID
allocator will be used to allocate and free Interrupt Source Identifiers for devices. To decouple
this crate from the ID allocator, here we doesn't take the responsibility to allocate/free Interrupt
Source IDs but only makes use of assigned IDs.

The overall flow to deal with interrupts is:

- the VMM creates an interrupt manager
- the VMM creates a device manager, passing on an reference to the interrupt manager
- the device manager passes on an reference to the interrupt manager to all registered devices
- guest kernel loads drivers for virtual devices
- guest device driver determines the type and number of interrupts needed, and update the device
  configuration
- the virtual device backend requests the interrupt manager to create an interrupt group according to guest configuration information

The dbs-device crate provides:

- [trait `InterruptManager`]: manage interrupt sources for virtual device backend
- [struct `DeviceInterruptManager`]: an implementation of [`InterruptManager`],  manage interrupts and interrupt modes for a device
- [trait `InterruptSourceGroup`]: manage a group of interrupt sources for a device, provide methods to control the interrupts
- [enum `InterruptSourceType`]: type of interrupt source
- [enum `InterruptSourceConfig`], [struct `LegacyIrqSourceConfig`] and [struct `MsiIrqSourceConfig`]: configuration data for interrupt sources

## License

This project is licensed under [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0).

[trait InterruptManager]: src/lib.rs
[struct DeviceInterruptManager]: src/manager.rs
[trait InterruptSourceGroup]: src/lib.rs
[enum InterruptSourceType]: src/lib.rs
[enum InterruptSourceConfig]: src/lib.rs
[struct LegacyIrqSourceConfig]: src/lib.rs
[struct MsiIrqSourceConfig]: src/lib.rs
