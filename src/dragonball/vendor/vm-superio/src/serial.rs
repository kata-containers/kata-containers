// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Provides emulation for Linux serial console.
//!
//! This is done by emulating an UART serial port.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::result::Result;
use std::sync::Arc;

use crate::Trigger;

// Register offsets.
// Receiver and Transmitter registers offset, depending on the I/O
// access type: write -> THR, read -> RBR.
const DATA_OFFSET: u8 = 0;
const IER_OFFSET: u8 = 1;
const IIR_OFFSET: u8 = 2;
const LCR_OFFSET: u8 = 3;
const MCR_OFFSET: u8 = 4;
const LSR_OFFSET: u8 = 5;
const MSR_OFFSET: u8 = 6;
const SCR_OFFSET: u8 = 7;
const DLAB_LOW_OFFSET: u8 = 0;
const DLAB_HIGH_OFFSET: u8 = 1;

const FIFO_SIZE: usize = 0x40;

// Received Data Available interrupt - for letting the driver know that
// there is some pending data to be processed.
const IER_RDA_BIT: u8 = 0b0000_0001;
// Transmitter Holding Register Empty interrupt - for letting the driver
// know that the entire content of the output buffer was sent.
const IER_THR_EMPTY_BIT: u8 = 0b0000_0010;
// The interrupts that are available on 16550 and older models.
const IER_UART_VALID_BITS: u8 = 0b0000_1111;

//FIFO enabled.
const IIR_FIFO_BITS: u8 = 0b1100_0000;
const IIR_NONE_BIT: u8 = 0b0000_0001;
const IIR_THR_EMPTY_BIT: u8 = 0b0000_0010;
const IIR_RDA_BIT: u8 = 0b0000_0100;

const LCR_DLAB_BIT: u8 = 0b1000_0000;

const LSR_DATA_READY_BIT: u8 = 0b0000_0001;
// These two bits help the driver know if the device is ready to accept
// another character.
// THR is empty.
const LSR_EMPTY_THR_BIT: u8 = 0b0010_0000;
// The shift register, which takes a byte from THR and breaks it in bits
// for sending them on the line, is empty.
const LSR_IDLE_BIT: u8 = 0b0100_0000;

// The following five MCR bits allow direct manipulation of the device and
// are available on 16550 and older models.
// Data Terminal Ready.
const MCR_DTR_BIT: u8 = 0b0000_0001;
// Request To Send.
const MCR_RTS_BIT: u8 = 0b0000_0010;
// Auxiliary Output 1.
const MCR_OUT1_BIT: u8 = 0b0000_0100;
// Auxiliary Output 2.
const MCR_OUT2_BIT: u8 = 0b0000_1000;
// Loopback Mode.
const MCR_LOOP_BIT: u8 = 0b0001_0000;

// Clear To Send.
const MSR_CTS_BIT: u8 = 0b0001_0000;
// Data Set Ready.
const MSR_DSR_BIT: u8 = 0b0010_0000;
// Ring Indicator.
const MSR_RI_BIT: u8 = 0b0100_0000;
// Data Carrier Detect.
const MSR_DCD_BIT: u8 = 0b1000_0000;

// The following values can be used to set the baud rate to 9600 bps.
const DEFAULT_BAUD_DIVISOR_HIGH: u8 = 0x00;
const DEFAULT_BAUD_DIVISOR_LOW: u8 = 0x0C;

// No interrupts enabled.
const DEFAULT_INTERRUPT_ENABLE: u8 = 0x00;
// No pending interrupt.
const DEFAULT_INTERRUPT_IDENTIFICATION: u8 = IIR_NONE_BIT;
// We're setting the default to include LSR_EMPTY_THR_BIT and LSR_IDLE_BIT
// and never update those bits because we're working with a virtual device,
// hence we should always be ready to receive more data.
const DEFAULT_LINE_STATUS: u8 = LSR_EMPTY_THR_BIT | LSR_IDLE_BIT;
// 8 bits word length.
const DEFAULT_LINE_CONTROL: u8 = 0b0000_0011;
// Most UARTs need Auxiliary Output 2 set to '1' to enable interrupts.
const DEFAULT_MODEM_CONTROL: u8 = MCR_OUT2_BIT;
const DEFAULT_MODEM_STATUS: u8 = MSR_DSR_BIT | MSR_CTS_BIT | MSR_DCD_BIT;
const DEFAULT_SCRATCH: u8 = 0x00;

/// Defines a series of callbacks that are invoked in response to the occurrence of specific
/// events as part of the serial emulation logic (for example, when the driver reads data). The
/// methods below can be implemented by a backend that keeps track of such events by incrementing
/// metrics, logging messages, or any other action.
///
/// We're using a trait to avoid constraining the concrete characteristics of the backend in
/// any way, enabling zero-cost abstractions and use case-specific implementations.
// TODO: The events defined below are just some examples for now to validate the approach. If
// things look good, we can move on to establishing the initial list. It's also worth mentioning
// the methods can have extra parameters that provide additional information about the event.
pub trait SerialEvents {
    /// The driver reads data from the input buffer.
    fn buffer_read(&self);
    /// The driver successfully wrote one byte to serial output.
    fn out_byte(&self);
    /// An error occurred while writing a byte to serial output resulting in a lost byte.
    fn tx_lost_byte(&self);
    /// This event can be used by the consumer to re-enable events coming from
    /// the serial input.
    fn in_buffer_empty(&self);
}

/// Provides a no-op implementation of `SerialEvents` which can be used in situations that
/// do not require logging or otherwise doing anything in response to the events defined
/// as part of `SerialEvents`.
pub struct NoEvents;

impl SerialEvents for NoEvents {
    fn buffer_read(&self) {}
    fn out_byte(&self) {}
    fn tx_lost_byte(&self) {}
    fn in_buffer_empty(&self) {}
}

impl<EV: SerialEvents> SerialEvents for Arc<EV> {
    fn buffer_read(&self) {
        self.as_ref().buffer_read();
    }

    fn out_byte(&self) {
        self.as_ref().out_byte();
    }

    fn tx_lost_byte(&self) {
        self.as_ref().tx_lost_byte();
    }

    fn in_buffer_empty(&self) {
        self.as_ref().in_buffer_empty();
    }
}

/// The serial console emulation is done by emulating a serial COM port.
///
/// Each serial COM port (COM1-4) has an associated Port I/O address base and
/// 12 registers mapped into 8 consecutive Port I/O locations (with the first
/// one being the base).
/// This structure emulates the registers that make sense for UART 16550 (and below)
/// and helps in the interaction between the driver and device by using a
/// [`Trigger`](../trait.Trigger.html) object for notifications. It also writes the
/// guest's output to an `out` Write object.
///
/// # Example
///
/// ```rust
/// # use std::io::{sink, Error, Result};
/// # use std::ops::Deref;
/// # use vm_superio::Trigger;
/// # use vm_superio::Serial;
/// # use vmm_sys_util::eventfd::EventFd;
///
/// struct EventFdTrigger(EventFd);
/// impl Trigger for EventFdTrigger {
///     type E = Error;
///
///     fn trigger(&self) -> Result<()> {
///         self.write(1)
///     }
/// }
/// impl Deref for EventFdTrigger {
///     type Target = EventFd;
///     fn deref(&self) -> &Self::Target {
///         &self.0
///     }
/// }
/// impl EventFdTrigger {
///     pub fn new(flag: i32) -> Self {
///         EventFdTrigger(EventFd::new(flag).unwrap())
///     }
///     pub fn try_clone(&self) -> Self {
///         EventFdTrigger((**self).try_clone().unwrap())
///     }
/// }
///
/// let intr_evt = EventFdTrigger::new(libc::EFD_NONBLOCK);
/// let mut serial = Serial::new(intr_evt.try_clone(), Vec::new());
/// // std::io::Sink can be used if user is not interested in guest's output.
/// let serial_with_sink = Serial::new(intr_evt, sink());
///
/// // Write 0x01 to THR register.
/// serial.write(0, 0x01).unwrap();
/// // Read from RBR register.
/// let value = serial.read(0);
///
/// // Send more bytes to the guest in one shot.
/// let input = &[b'a', b'b', b'c'];
/// // Before enqueuing bytes we first check if there is enough free space
/// // in the FIFO.
/// if serial.fifo_capacity() >= input.len() {
///     serial.enqueue_raw_bytes(input).unwrap();
/// }
/// ```
pub struct Serial<T: Trigger, EV: SerialEvents, W: Write> {
    // Some UART registers.
    baud_divisor_low: u8,
    baud_divisor_high: u8,
    interrupt_enable: u8,
    interrupt_identification: u8,
    line_control: u8,
    line_status: u8,
    modem_control: u8,
    modem_status: u8,
    scratch: u8,
    // This is the buffer that is used for achieving the Receiver register
    // functionality in FIFO mode. Reading from RBR will return the oldest
    // unread byte from the RX FIFO.
    in_buffer: VecDeque<u8>,

    // Used for notifying the driver about some in/out events.
    interrupt_evt: T,
    events: EV,
    out: W,
}

/// Errors encountered while handling serial console operations.
#[derive(Debug)]
pub enum Error<E> {
    /// Failed to trigger interrupt.
    Trigger(E),
    /// Couldn't write/flush to the given destination.
    IOError(io::Error),
    /// No space left in FIFO.
    FullFifo,
}

impl<T: Trigger, W: Write> Serial<T, NoEvents, W> {
    /// Creates a new `Serial` instance which writes the guest's output to
    /// `out` and uses `trigger` object to notify the driver about new
    /// events.
    ///
    /// # Arguments
    /// * `trigger` - The Trigger object that will be used to notify the driver
    ///               about events.
    /// * `out` - An object for writing guest's output to. In case the output
    ///           is not of interest,
    ///           [std::io::Sink](https://doc.rust-lang.org/std/io/struct.Sink.html)
    ///           can be used here.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `Serial`](struct.Serial.html#example).
    pub fn new(trigger: T, out: W) -> Serial<T, NoEvents, W> {
        Self::with_events(trigger, NoEvents, out)
    }
}

impl<T: Trigger, EV: SerialEvents, W: Write> Serial<T, EV, W> {
    /// Creates a new `Serial` instance which writes the guest's output to
    /// `out`, uses `trigger` object to notify the driver about new
    /// events, and invokes the `serial_evts` implementation of `SerialEvents`
    /// during operation.
    ///
    /// # Arguments
    /// * `trigger` - The `Trigger` object that will be used to notify the driver
    ///               about events.
    /// * `serial_evts` - The `SerialEvents` implementation used to track the occurrence
    ///                   of significant events in the serial operation logic.
    /// * `out` - An object for writing guest's output to. In case the output
    ///           is not of interest,
    ///           [std::io::Sink](https://doc.rust-lang.org/std/io/struct.Sink.html)
    ///           can be used here.
    pub fn with_events(trigger: T, serial_evts: EV, out: W) -> Self {
        Serial {
            baud_divisor_low: DEFAULT_BAUD_DIVISOR_LOW,
            baud_divisor_high: DEFAULT_BAUD_DIVISOR_HIGH,
            interrupt_enable: DEFAULT_INTERRUPT_ENABLE,
            interrupt_identification: DEFAULT_INTERRUPT_IDENTIFICATION,
            line_control: DEFAULT_LINE_CONTROL,
            line_status: DEFAULT_LINE_STATUS,
            modem_control: DEFAULT_MODEM_CONTROL,
            modem_status: DEFAULT_MODEM_STATUS,
            scratch: DEFAULT_SCRATCH,
            in_buffer: VecDeque::new(),
            interrupt_evt: trigger,
            events: serial_evts,
            out,
        }
    }

    /// Provides a reference to the interrupt event object.
    pub fn interrupt_evt(&self) -> &T {
        &self.interrupt_evt
    }

    /// Provides a reference to the serial events object.
    pub fn events(&self) -> &EV {
        &self.events
    }

    fn is_dlab_set(&self) -> bool {
        (self.line_control & LCR_DLAB_BIT) != 0
    }

    fn is_rda_interrupt_enabled(&self) -> bool {
        (self.interrupt_enable & IER_RDA_BIT) != 0
    }

    fn is_thr_interrupt_enabled(&self) -> bool {
        (self.interrupt_enable & IER_THR_EMPTY_BIT) != 0
    }

    fn is_in_loop_mode(&self) -> bool {
        (self.modem_control & MCR_LOOP_BIT) != 0
    }

    fn trigger_interrupt(&mut self) -> Result<(), T::E> {
        self.interrupt_evt.trigger()
    }

    fn set_lsr_rda_bit(&mut self) {
        self.line_status |= LSR_DATA_READY_BIT
    }

    fn clear_lsr_rda_bit(&mut self) {
        self.line_status &= !LSR_DATA_READY_BIT
    }

    fn add_interrupt(&mut self, interrupt_bits: u8) {
        self.interrupt_identification &= !IIR_NONE_BIT;
        self.interrupt_identification |= interrupt_bits;
    }

    fn del_interrupt(&mut self, interrupt_bits: u8) {
        self.interrupt_identification &= !interrupt_bits;
        if self.interrupt_identification == 0x00 {
            self.interrupt_identification = IIR_NONE_BIT;
        }
    }

    fn thr_empty_interrupt(&mut self) -> Result<(), T::E> {
        if self.is_thr_interrupt_enabled() {
            // Trigger the interrupt only if the identification bit wasn't
            // set or acknowledged.
            if self.interrupt_identification & IIR_THR_EMPTY_BIT == 0 {
                self.add_interrupt(IIR_THR_EMPTY_BIT);
                self.trigger_interrupt()?
            }
        }
        Ok(())
    }

    fn received_data_interrupt(&mut self) -> Result<(), T::E> {
        if self.is_rda_interrupt_enabled() {
            // Trigger the interrupt only if the identification bit wasn't
            // set or acknowledged.
            if self.interrupt_identification & IIR_RDA_BIT == 0 {
                self.add_interrupt(IIR_RDA_BIT);
                self.trigger_interrupt()?
            }
        }
        Ok(())
    }

    fn reset_iir(&mut self) {
        self.interrupt_identification = DEFAULT_INTERRUPT_IDENTIFICATION
    }

    /// Handles a write request from the driver at `offset` offset from the
    /// base Port I/O address.
    ///
    /// # Arguments
    /// * `offset` - The offset that will be added to the base PIO address
    ///              for writing to a specific register.
    /// * `value` - The byte that should be written.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `Serial`](struct.Serial.html#example).
    pub fn write(&mut self, offset: u8, value: u8) -> Result<(), Error<T::E>> {
        match offset {
            DLAB_LOW_OFFSET if self.is_dlab_set() => self.baud_divisor_low = value,
            DLAB_HIGH_OFFSET if self.is_dlab_set() => self.baud_divisor_high = value,
            DATA_OFFSET => {
                if self.is_in_loop_mode() {
                    // In loopback mode, what is written in the transmit register
                    // will be immediately found in the receive register, so we
                    // simulate this behavior by adding in `in_buffer` the
                    // transmitted bytes and letting the driver know there is some
                    // pending data to be read, by setting RDA bit and its
                    // corresponding interrupt.
                    if self.in_buffer.len() < FIFO_SIZE {
                        self.in_buffer.push_back(value);
                        self.set_lsr_rda_bit();
                        self.received_data_interrupt().map_err(Error::Trigger)?;
                    }
                } else {
                    let res = self
                        .out
                        .write_all(&[value])
                        .map_err(Error::IOError)
                        .and_then(|_| self.out.flush().map_err(Error::IOError))
                        .map(|_| self.events.out_byte())
                        .map_err(|err| {
                            self.events.tx_lost_byte();
                            err
                        });
                    // Because we cannot block the driver, the THRE interrupt is sent
                    // irrespective of whether we are able to write the byte or not
                    self.thr_empty_interrupt().map_err(Error::Trigger)?;
                    return res;
                }
            }
            // We want to enable only the interrupts that are available for 16550A (and below).
            IER_OFFSET => self.interrupt_enable = value & IER_UART_VALID_BITS,
            LCR_OFFSET => self.line_control = value,
            MCR_OFFSET => self.modem_control = value,
            SCR_OFFSET => self.scratch = value,
            // We are not interested in writing to other offsets (such as FCR offset).
            _ => {}
        }
        Ok(())
    }

    /// Handles a read request from the driver at `offset` offset from the
    /// base Port I/O address.
    ///
    /// Returns the read value.
    ///
    /// # Arguments
    /// * `offset` - The offset that will be added to the base PIO address
    ///              for reading from a specific register.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `Serial`](struct.Serial.html#example).
    pub fn read(&mut self, offset: u8) -> u8 {
        match offset {
            DLAB_LOW_OFFSET if self.is_dlab_set() => self.baud_divisor_low,
            DLAB_HIGH_OFFSET if self.is_dlab_set() => self.baud_divisor_high,
            DATA_OFFSET => {
                // Here we emulate the reset method for when RDA interrupt
                // was raised (i.e. read the receive buffer and clear the
                // interrupt identification register and RDA bit when no
                // more data is available).
                self.del_interrupt(IIR_RDA_BIT);
                let byte = self.in_buffer.pop_front().unwrap_or_default();
                if self.in_buffer.is_empty() {
                    self.clear_lsr_rda_bit();
                    self.events.in_buffer_empty();
                }
                self.events.buffer_read();
                byte
            }
            IER_OFFSET => self.interrupt_enable,
            IIR_OFFSET => {
                // We're enabling FIFO capability by setting the serial port to 16550A:
                // https://elixir.bootlin.com/linux/latest/source/drivers/tty/serial/8250/8250_port.c#L1299.
                let iir = self.interrupt_identification | IIR_FIFO_BITS;
                self.reset_iir();
                iir
            }
            LCR_OFFSET => self.line_control,
            MCR_OFFSET => self.modem_control,
            LSR_OFFSET => self.line_status,
            MSR_OFFSET => {
                if self.is_in_loop_mode() {
                    // In loopback mode, the four modem control inputs (CTS, DSR, RI, DCD) are
                    // internally connected to the four modem control outputs (RTS, DTR, OUT1, OUT2).
                    // This way CTS is controlled by RTS, DSR by DTR, RI by OUT1 and DCD by OUT2.
                    // (so they will basically contain the same value).
                    let mut msr =
                        self.modem_status & !(MSR_DSR_BIT | MSR_CTS_BIT | MSR_RI_BIT | MSR_DCD_BIT);
                    if (self.modem_control & MCR_DTR_BIT) != 0 {
                        msr |= MSR_DSR_BIT;
                    }
                    if (self.modem_control & MCR_RTS_BIT) != 0 {
                        msr |= MSR_CTS_BIT;
                    }
                    if (self.modem_control & MCR_OUT1_BIT) != 0 {
                        msr |= MSR_RI_BIT;
                    }
                    if (self.modem_control & MCR_OUT2_BIT) != 0 {
                        msr |= MSR_DCD_BIT;
                    }
                    msr
                } else {
                    self.modem_status
                }
            }
            SCR_OFFSET => self.scratch,
            _ => 0,
        }
    }

    /// Returns how much space is still available in the FIFO.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `Serial`](struct.Serial.html#example).
    #[inline]
    pub fn fifo_capacity(&self) -> usize {
        FIFO_SIZE - self.in_buffer.len()
    }

    /// Helps in sending more bytes to the guest in one shot, by storing
    /// `input` bytes in UART buffer and letting the driver know there is
    /// some pending data to be read by setting RDA bit and its corresponding
    /// interrupt when not already triggered.
    ///
    /// # Arguments
    /// * `input` - The data to be sent to the guest.
    ///
    /// # Returns
    ///
    /// The function returns the number of bytes it was able to write to the fifo,
    /// or `FullFifo` error when the fifo is full. Users can use
    /// [`fifo_capacity`](#method.fifo_capacity) before calling this function
    /// to check the available space.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `Serial`](struct.Serial.html#example).
    pub fn enqueue_raw_bytes(&mut self, input: &[u8]) -> Result<usize, Error<T::E>> {
        let mut write_count = 0;
        if !self.is_in_loop_mode() {
            if self.fifo_capacity() == 0 {
                return Err(Error::FullFifo);
            }
            write_count = std::cmp::min(self.fifo_capacity(), input.len());
            if write_count > 0 {
                self.in_buffer.extend(&input[0..write_count]);
                self.set_lsr_rda_bit();
                self.received_data_interrupt().map_err(Error::Trigger)?;
            }
        }
        Ok(write_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::{sink, Result};
    use std::sync::atomic::AtomicU64;
    use std::sync::Arc;

    use vmm_sys_util::eventfd::EventFd;
    use vmm_sys_util::metric::Metric;

    const RAW_INPUT_BUF: [u8; 3] = [b'a', b'b', b'c'];

    impl Trigger for EventFd {
        type E = io::Error;

        fn trigger(&self) -> Result<()> {
            self.write(1)
        }
    }

    struct ExampleSerialEvents {
        read_count: AtomicU64,
        out_byte_count: AtomicU64,
        tx_lost_byte_count: AtomicU64,
        buffer_ready_event: EventFd,
    }

    impl ExampleSerialEvents {
        fn new() -> Self {
            ExampleSerialEvents {
                read_count: AtomicU64::new(0),
                out_byte_count: AtomicU64::new(0),
                tx_lost_byte_count: AtomicU64::new(0),
                buffer_ready_event: EventFd::new(libc::EFD_NONBLOCK).unwrap(),
            }
        }
    }

    impl SerialEvents for ExampleSerialEvents {
        fn buffer_read(&self) {
            self.read_count.inc();
            // We can also log a message here, or as part of any of the other methods.
        }

        fn out_byte(&self) {
            self.out_byte_count.inc();
        }

        fn tx_lost_byte(&self) {
            self.tx_lost_byte_count.inc();
        }

        fn in_buffer_empty(&self) {
            self.buffer_ready_event.write(1).unwrap();
        }
    }

    #[test]
    fn test_serial_output() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, Vec::new());

        // Valid one char at a time writes.
        RAW_INPUT_BUF
            .iter()
            .for_each(|&c| serial.write(DATA_OFFSET, c).unwrap());
        assert_eq!(serial.out.as_slice(), &RAW_INPUT_BUF);
    }

    #[test]
    fn test_serial_raw_input() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt.try_clone().unwrap(), sink());

        serial.write(IER_OFFSET, IER_RDA_BIT).unwrap();
        serial.enqueue_raw_bytes(&RAW_INPUT_BUF).unwrap();

        // Verify the serial raised an interrupt.
        assert_eq!(intr_evt.read().unwrap(), 1);

        // `DATA_READY` bit should've been set by `enqueue_raw_bytes()`.
        let mut lsr = serial.read(LSR_OFFSET);
        assert_ne!(lsr & LSR_DATA_READY_BIT, 0);

        // Verify reading the previously pushed buffer.
        RAW_INPUT_BUF.iter().for_each(|&c| {
            lsr = serial.read(LSR_OFFSET);
            // `DATA_READY` bit won't be cleared until there is
            // just one byte left in the receive buffer.
            assert_ne!(lsr & LSR_DATA_READY_BIT, 0);
            assert_eq!(serial.read(DATA_OFFSET), c);
            // The Received Data Available interrupt bit should be
            // cleared after reading the first pending byte.
            assert_eq!(
                serial.interrupt_identification,
                DEFAULT_INTERRUPT_IDENTIFICATION
            );
        });

        lsr = serial.read(LSR_OFFSET);
        assert_eq!(lsr & LSR_DATA_READY_BIT, 0);
    }

    #[test]
    fn test_serial_thr() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt.try_clone().unwrap(), sink());

        serial.write(IER_OFFSET, IER_THR_EMPTY_BIT).unwrap();
        assert_eq!(
            serial.interrupt_enable,
            IER_THR_EMPTY_BIT & IER_UART_VALID_BITS
        );
        serial.write(DATA_OFFSET, b'a').unwrap();

        // Verify the serial raised an interrupt.
        assert_eq!(intr_evt.read().unwrap(), 1);

        let ier = serial.read(IER_OFFSET);
        assert_eq!(ier & IER_UART_VALID_BITS, IER_THR_EMPTY_BIT);
        let iir = serial.read(IIR_OFFSET);
        // Verify the raised interrupt is indeed the empty THR one.
        assert_ne!(iir & IIR_THR_EMPTY_BIT, 0);

        // When reading from IIR offset, the returned value will tell us that
        // FIFO feature is enabled.
        assert_eq!(iir, IIR_THR_EMPTY_BIT | IIR_FIFO_BITS);
        assert_eq!(
            serial.interrupt_identification,
            DEFAULT_INTERRUPT_IDENTIFICATION
        );
    }

    #[test]
    fn test_serial_loop_mode() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt.try_clone().unwrap(), sink());

        serial.write(MCR_OFFSET, MCR_LOOP_BIT).unwrap();
        serial.write(IER_OFFSET, IER_RDA_BIT).unwrap();

        for value in 0..FIFO_SIZE as u8 {
            serial.write(DATA_OFFSET, value).unwrap();
            assert_eq!(intr_evt.read().unwrap(), 1);
            assert_eq!(serial.in_buffer.len(), 1);
            // Immediately read a pushed value.
            assert_eq!(serial.read(DATA_OFFSET), value);
        }

        assert_eq!(serial.line_status & LSR_DATA_READY_BIT, 0);

        for value in 0..FIFO_SIZE as u8 {
            serial.write(DATA_OFFSET, value).unwrap();
        }

        assert_eq!(intr_evt.read().unwrap(), 1);
        assert_eq!(serial.in_buffer.len(), FIFO_SIZE);

        // Read the pushed values at the end.
        for value in 0..FIFO_SIZE as u8 {
            assert_ne!(serial.line_status & LSR_DATA_READY_BIT, 0);
            assert_eq!(serial.read(DATA_OFFSET), value);
        }
        assert_eq!(serial.line_status & LSR_DATA_READY_BIT, 0);
    }

    #[test]
    fn test_serial_dlab() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, sink());

        // For writing to DLAB registers, `DLAB` bit from LCR should be set.
        serial.write(LCR_OFFSET, LCR_DLAB_BIT).unwrap();
        serial.write(DLAB_HIGH_OFFSET, 0x12).unwrap();
        assert_eq!(serial.read(DLAB_LOW_OFFSET), DEFAULT_BAUD_DIVISOR_LOW);
        assert_eq!(serial.read(DLAB_HIGH_OFFSET), 0x12);

        serial.write(DLAB_LOW_OFFSET, 0x34).unwrap();

        assert_eq!(serial.read(DLAB_LOW_OFFSET), 0x34);
        assert_eq!(serial.read(DLAB_HIGH_OFFSET), 0x12);

        // If LCR_DLAB_BIT is not set, the values from `DLAB_LOW_OFFSET` and
        // `DLAB_HIGH_OFFSET` won't be the expected ones.
        serial.write(LCR_OFFSET, 0x00).unwrap();
        assert_ne!(serial.read(DLAB_LOW_OFFSET), 0x12);
        assert_ne!(serial.read(DLAB_HIGH_OFFSET), 0x34);
    }

    #[test]
    fn test_basic_register_accesses() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, sink());

        // Writing to these registers does not alter the initial values to be written
        // and reading from these registers just returns those values, without
        // modifying them.
        let basic_register_accesses = [LCR_OFFSET, MCR_OFFSET, SCR_OFFSET];
        for offset in basic_register_accesses.iter() {
            serial.write(*offset, 0x12).unwrap();
            assert_eq!(serial.read(*offset), 0x12);
        }
    }

    #[test]
    fn test_invalid_access() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, sink());

        // Check if reading from an offset outside 0-7 returns for sure 0.
        serial.write(SCR_OFFSET + 1, 5).unwrap();
        assert_eq!(serial.read(SCR_OFFSET + 1), 0);
    }

    #[test]
    fn test_serial_msr() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, sink());

        assert_eq!(serial.read(MSR_OFFSET), DEFAULT_MODEM_STATUS);

        // Activate loopback mode.
        serial.write(MCR_OFFSET, MCR_LOOP_BIT).unwrap();

        // In loopback mode, MSR won't contain the default value anymore.
        assert_ne!(serial.read(MSR_OFFSET), DEFAULT_MODEM_STATUS);
        assert_eq!(serial.read(MSR_OFFSET), 0x00);

        // Depending on which bytes we enable for MCR, MSR will be modified accordingly.
        serial
            .write(MCR_OFFSET, DEFAULT_MODEM_CONTROL | MCR_LOOP_BIT)
            .unwrap();
        // DEFAULT_MODEM_CONTROL sets OUT2 from MCR to 1. In loopback mode, OUT2 is equivalent
        // to DCD bit from MSR.
        assert_eq!(serial.read(MSR_OFFSET), MSR_DCD_BIT);

        // The same should happen with OUT1 and RI.
        serial
            .write(MCR_OFFSET, MCR_OUT1_BIT | MCR_LOOP_BIT)
            .unwrap();
        assert_eq!(serial.read(MSR_OFFSET), MSR_RI_BIT);

        serial
            .write(MCR_OFFSET, MCR_LOOP_BIT | MCR_DTR_BIT | MCR_RTS_BIT)
            .unwrap();
        // DSR and CTS from MSR are "matching wires" to DTR and RTS from MCR (so they will
        // have the same value).
        assert_eq!(serial.read(MSR_OFFSET), MSR_DSR_BIT | MSR_CTS_BIT);
    }

    #[test]
    fn test_fifo_max_size() {
        let event_fd = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(event_fd, sink());

        // Test case: trying to write too many bytes in an empty fifo will just write
        // `FIFO_SIZE`. Any other subsequent writes, will return a `FullFifo` error.
        let too_many_bytes = vec![1u8; FIFO_SIZE + 1];
        let written_bytes = serial.enqueue_raw_bytes(&too_many_bytes).unwrap();
        assert_eq!(written_bytes, FIFO_SIZE);
        assert_eq!(serial.in_buffer.len(), FIFO_SIZE);

        // A subsequent call to `enqueue_raw_bytes` fails because the fifo is
        // now full.
        let one_byte_input = [1u8];
        match serial.enqueue_raw_bytes(&one_byte_input) {
            Err(Error::FullFifo) => (),
            _ => unreachable!(),
        }

        // Test case: consuming one byte from a full fifo does not allow writes
        // bigger than one byte.
        let _ = serial.read(DATA_OFFSET);
        let written_bytes = serial.enqueue_raw_bytes(&too_many_bytes[..2]).unwrap();
        assert_eq!(written_bytes, 1);
        assert_eq!(serial.in_buffer.len(), FIFO_SIZE);
    }

    #[test]
    fn test_serial_events() {
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();

        let events_ = Arc::new(ExampleSerialEvents::new());
        let mut oneslot_buf = [0u8; 1];
        let mut serial = Serial::with_events(intr_evt, events_, oneslot_buf.as_mut());

        // This should be an error because buffer_ready_event has not been
        // triggered yet so no one should have written to that fd yet.
        assert_eq!(
            serial.events.buffer_ready_event.read().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );

        // Check everything is equal to 0 at the beginning.
        assert_eq!(serial.events.read_count.count(), 0);
        assert_eq!(serial.events.out_byte_count.count(), 0);
        assert_eq!(serial.events.tx_lost_byte_count.count(), 0);

        // This DATA read should cause the `SerialEvents::buffer_read` method to be invoked.
        // And since the in_buffer is empty the buffer_ready_event should have
        // been triggered, hence we can read from that fd.
        serial.read(DATA_OFFSET);
        assert_eq!(serial.events.read_count.count(), 1);
        assert_eq!(serial.events.buffer_ready_event.read().unwrap(), 1);

        // This DATA write should cause `SerialEvents::out_byte` to be called.
        serial.write(DATA_OFFSET, 1).unwrap();
        assert_eq!(serial.events.out_byte_count.count(), 1);
        // `SerialEvents::tx_lost_byte` should not have been called.
        assert_eq!(serial.events.tx_lost_byte_count.count(), 0);

        // This DATA write should cause `SerialEvents::tx_lost_byte` to be called.
        serial.write(DATA_OFFSET, 1).unwrap_err();
        assert_eq!(serial.events.tx_lost_byte_count.count(), 1);

        // Check that every metric has the expected value at the end, to ensure we didn't
        // unexpectedly invoked any extra callbacks.
        assert_eq!(serial.events.read_count.count(), 1);
        assert_eq!(serial.events.out_byte_count.count(), 1);
        assert_eq!(serial.events.tx_lost_byte_count.count(), 1);

        // This DATA read should cause the `SerialEvents::buffer_read` method to be invoked.
        // And since it was the last byte from in buffer the `SerialEvents::in_buffer_empty`
        // was also invoked.
        serial.read(DATA_OFFSET);
        assert_eq!(serial.events.read_count.count(), 2);
        assert_eq!(serial.events.buffer_ready_event.read().unwrap(), 1);
        let _res = serial.enqueue_raw_bytes(&[1, 2]);
        serial.read(DATA_OFFSET);
        // Since there is still one byte in the in_buffer, buffer_ready_events
        // should have not been triggered so we shouldn't have anything to read
        // from that fd.
        assert_eq!(
            serial.events.buffer_ready_event.read().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn test_out_descrp_full_thre_sent() {
        let mut nospace_buf = [0u8; 0];
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut serial = Serial::new(intr_evt, nospace_buf.as_mut());

        // Enable THR interrupt.
        serial.write(IER_OFFSET, IER_THR_EMPTY_BIT).unwrap();

        // Write some data.
        let res = serial.write(DATA_OFFSET, 5);
        let iir = serial.read(IIR_OFFSET);

        // The write failed.
        assert!(
            matches!(res.unwrap_err(), Error::IOError(io_err) if io_err.kind() == io::ErrorKind::WriteZero
            )
        );
        // THR empty interrupt was raised nevertheless.
        assert_eq!(iir & IIR_THR_EMPTY_BIT, IIR_THR_EMPTY_BIT);
    }
}
