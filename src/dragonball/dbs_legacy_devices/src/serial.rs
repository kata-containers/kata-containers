// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
use std::io::Write;
use std::sync::{Arc, Mutex};

use dbs_device::{DeviceIoMut, IoAddress, PioAddress};
use dbs_utils::metric::{IncMetric, SharedIncMetric};
use log::error;
use serde::Serialize;
use vm_superio::{serial::SerialEvents, Serial, Trigger};
use vmm_sys_util::eventfd::EventFd;

use crate::EventFdTrigger;

/// Trait for devices that handle raw non-blocking I/O requests.
pub trait ConsoleHandler {
    /// Send raw input to this emulated device.
    fn raw_input(&mut self, _data: &[u8]) -> std::io::Result<usize> {
        Ok(0)
    }

    /// Set the stream to receive raw output from this emulated device.
    fn set_output_stream(&mut self, out: Option<Box<dyn Write + Send>>);
}

/// Metrics specific to the UART device.
#[derive(Default, Serialize)]
pub struct SerialDeviceMetrics {
    /// Errors triggered while using the UART device.
    pub error_count: SharedIncMetric,
    /// Number of flush operations.
    pub flush_count: SharedIncMetric,
    /// Number of read calls that did not trigger a read.
    pub missed_read_count: SharedIncMetric,
    /// Number of write calls that did not trigger a write.
    pub missed_write_count: SharedIncMetric,
    /// Number of succeeded read calls.
    pub read_count: SharedIncMetric,
    /// Number of succeeded write calls.
    pub write_count: SharedIncMetric,
}

pub struct SerialEventsWrapper {
    pub metrics: Arc<SerialDeviceMetrics>,
    pub buffer_ready_event_fd: Option<EventFdTrigger>,
}

impl SerialEvents for SerialEventsWrapper {
    fn buffer_read(&self) {
        self.metrics.read_count.inc();
    }

    fn out_byte(&self) {
        self.metrics.write_count.inc();
    }

    fn tx_lost_byte(&self) {
        self.metrics.missed_write_count.inc();
    }

    fn in_buffer_empty(&self) {
        match self
            .buffer_ready_event_fd
            .as_ref()
            .map_or(Ok(()), |buf_ready| buf_ready.write(1))
        {
            Ok(_) => (),
            Err(err) => error!(
                "Could not signal that serial device buffer is ready: {:?}",
                err
            ),
        }
    }
}

pub type SerialDevice = SerialWrapper<EventFdTrigger, SerialEventsWrapper>;

impl SerialDevice {
    /// Creates a new SerialDevice instance.
    pub fn new(event: EventFd) -> Self {
        let out = Arc::new(Mutex::new(None));
        Self {
            serial: Serial::with_events(
                EventFdTrigger::new(event),
                SerialEventsWrapper {
                    metrics: Arc::new(SerialDeviceMetrics::default()),
                    buffer_ready_event_fd: None,
                },
                AdapterWriter(out.clone()),
            ),
            out,
        }
    }

    pub fn metrics(&mut self) -> Arc<SerialDeviceMetrics> {
        self.serial.events().metrics.clone()
    }
}

pub struct SerialWrapper<T: Trigger, EV: SerialEvents> {
    pub serial: Serial<T, EV, AdapterWriter>,
    pub out: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
}

impl ConsoleHandler for SerialWrapper<EventFdTrigger, SerialEventsWrapper> {
    fn raw_input(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.serial
            .enqueue_raw_bytes(data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{e:?}")))
    }

    fn set_output_stream(&mut self, out: Option<Box<dyn Write + Send>>) {
        *self.out.lock().unwrap() = out;
    }
}

impl DeviceIoMut for SerialWrapper<EventFdTrigger, SerialEventsWrapper> {
    fn pio_read(&mut self, _base: PioAddress, offset: PioAddress, data: &mut [u8]) {
        if data.len() != 1 {
            self.serial.events().metrics.missed_read_count.inc();
            return;
        }
        data[0] = self.serial.read(offset.raw_value() as u8);
    }
    fn pio_write(&mut self, _base: PioAddress, offset: PioAddress, data: &[u8]) {
        if data.len() != 1 {
            self.serial.events().metrics.missed_write_count.inc();
            return;
        }
        if let Err(e) = self.serial.write(offset.raw_value() as u8, data[0]) {
            error!("Failed the pio write to serial: {:?}", e);
            self.serial.events().metrics.error_count.inc();
        }
    }

    fn read(&mut self, _base: IoAddress, offset: IoAddress, data: &mut [u8]) {
        if data.len() != 1 {
            self.serial.events().metrics.missed_read_count.inc();
            return;
        }
        data[0] = self.serial.read(offset.raw_value() as u8);
    }
    fn write(&mut self, _base: IoAddress, offset: IoAddress, data: &[u8]) {
        if data.len() != 1 {
            self.serial.events().metrics.missed_write_count.inc();
            return;
        }
        if let Err(e) = self.serial.write(offset.raw_value() as u8, data[0]) {
            error!("Failed the write to serial: {:?}", e);
            self.serial.events().metrics.error_count.inc();
        }
    }
}

pub struct AdapterWriter(pub Arc<Mutex<Option<Box<dyn Write + Send>>>>);

impl Write for AdapterWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(w) = self.0.lock().unwrap().as_mut() {
            w.write(buf)
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(w) = self.0.lock().unwrap().as_mut() {
            w.flush()
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::sync::{Arc, Mutex};

    #[derive(Clone)]
    struct SharedBuffer {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl io::Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.buf.lock().unwrap().write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.buf.lock().unwrap().flush()
        }
    }

    #[test]
    fn test_serial_bus_write() {
        let serial_out_buf = Arc::new(Mutex::new(Vec::new()));
        let serial_out = Box::new(SharedBuffer {
            buf: serial_out_buf.clone(),
        });
        let intr_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();

        let mut serial = SerialDevice::new(intr_evt);
        let metrics = serial.serial.events().metrics.clone();

        serial.set_output_stream(Some(serial_out));

        let invalid_writes_before = serial.serial.events().metrics.missed_write_count.count();
        <dyn DeviceIoMut>::pio_write(&mut serial, PioAddress(0), PioAddress(0), &[b'x', b'y']);
        let writes_before = metrics.write_count.count();

        let invalid_writes_after = metrics.missed_write_count.count();
        assert_eq!(invalid_writes_before + 1, invalid_writes_after);

        assert_eq!(serial_out_buf.lock().unwrap().as_slice().len(), 0);
        <dyn DeviceIoMut>::write(&mut serial, IoAddress(0), IoAddress(0), &[b'x', b'y']);
        assert_eq!(serial_out_buf.lock().unwrap().as_slice().len(), 0);

        let invalid_writes_after = metrics.missed_write_count.count();
        assert_eq!(invalid_writes_before + 2, invalid_writes_after);

        <dyn DeviceIoMut>::pio_write(&mut serial, PioAddress(0), PioAddress(0), &[b'a']);
        <dyn DeviceIoMut>::pio_write(&mut serial, PioAddress(0), PioAddress(0), &[b'b']);
        <dyn DeviceIoMut>::write(&mut serial, IoAddress(0), IoAddress(0), &[b'c']);
        assert_eq!(
            serial_out_buf.lock().unwrap().as_slice(),
            &[b'a', b'b', b'c']
        );

        let invalid_writes_after_2 = metrics.missed_write_count.count();
        let writes_after = metrics.write_count.count();
        // The `invalid_write_count` metric should be the same as before the one-byte writes.
        assert_eq!(invalid_writes_after_2, invalid_writes_after);
        assert_eq!(writes_after, writes_before + 3);
    }

    #[test]
    fn test_serial_bus_read() {
        let intr_evt = EventFdTrigger::new(EventFd::new(libc::EFD_NONBLOCK).unwrap());

        let metrics = Arc::new(SerialDeviceMetrics::default());

        let out: Arc<Mutex<Option<Box<(dyn std::io::Write + Send + 'static)>>>> =
            Arc::new(Mutex::new(Some(Box::new(std::io::sink()))));
        let mut serial = SerialDevice {
            serial: Serial::with_events(
                intr_evt,
                SerialEventsWrapper {
                    metrics: metrics.clone(),
                    buffer_ready_event_fd: None,
                },
                AdapterWriter(out.clone()),
            ),
            out,
        };
        serial
            .serial
            .enqueue_raw_bytes(&[b'a', b'b', b'c'])
            .unwrap();

        let invalid_reads_before = metrics.missed_read_count.count();

        let mut v = [0x00; 2];
        <dyn DeviceIoMut>::pio_read(&mut serial, PioAddress(0), PioAddress(0), &mut v);
        assert_eq!(v[0], b'\0');

        let invalid_reads_after = metrics.missed_read_count.count();
        assert_eq!(invalid_reads_before + 1, invalid_reads_after);

        <dyn DeviceIoMut>::read(&mut serial, IoAddress(0), IoAddress(0), &mut v);
        assert_eq!(v[0], b'\0');

        let invalid_reads_after = metrics.missed_read_count.count();
        assert_eq!(invalid_reads_before + 2, invalid_reads_after);

        let mut v = [0x00; 1];
        <dyn DeviceIoMut>::pio_read(&mut serial, PioAddress(0), PioAddress(0), &mut v);
        assert_eq!(v[0], b'a');

        <dyn DeviceIoMut>::pio_read(&mut serial, PioAddress(0), PioAddress(0), &mut v);
        assert_eq!(v[0], b'b');

        <dyn DeviceIoMut>::read(&mut serial, IoAddress(0), IoAddress(0), &mut v);
        assert_eq!(v[0], b'c');

        let invalid_reads_after_2 = metrics.missed_read_count.count();
        // The `invalid_read_count` metric should be the same as before the one-byte reads.
        assert_eq!(invalid_reads_after_2, invalid_reads_after);
    }
}
