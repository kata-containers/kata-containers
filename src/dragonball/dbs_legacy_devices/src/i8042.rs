// Copyright 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use dbs_device::{DeviceIoMut, PioAddress};
use dbs_utils::metric::{IncMetric, SharedIncMetric};
use log::error;
use serde::Serialize;
use vm_superio::{I8042Device as I8042Dev, Trigger};

use crate::EventFdTrigger;

/// Metrics specific to the i8042 device.
#[derive(Default, Serialize)]
pub struct I8042DeviceMetrics {
    /// Errors triggered while using the i8042 device.
    pub error_count: SharedIncMetric,
    /// Number of superfluous read intents on this i8042 device.
    pub missed_read_count: SharedIncMetric,
    /// Number of superfluous read intents on this i8042 device.
    pub missed_write_count: SharedIncMetric,
    /// Bytes read by this device.
    pub read_count: SharedIncMetric,
    /// Bytes written by this device.
    pub write_count: SharedIncMetric,
}

pub type I8042Device = I8042Wrapper<EventFdTrigger>;

pub struct I8042Wrapper<T: Trigger> {
    device: I8042Dev<T>,
    metrics: Arc<I8042DeviceMetrics>,
}

impl I8042Device {
    pub fn new(event: EventFdTrigger) -> Self {
        Self {
            device: I8042Dev::new(event),
            metrics: Arc::new(I8042DeviceMetrics::default()),
        }
    }

    pub fn metrics(&self) -> Arc<I8042DeviceMetrics> {
        self.metrics.clone()
    }
}

impl DeviceIoMut for I8042Wrapper<EventFdTrigger> {
    fn pio_read(&mut self, _base: PioAddress, offset: PioAddress, data: &mut [u8]) {
        if data.len() != 1 {
            self.metrics.missed_read_count.inc();
            return;
        }
        data[0] = self.device.read(offset.raw_value() as u8);
        self.metrics.read_count.inc();
    }

    fn pio_write(&mut self, _base: PioAddress, offset: PioAddress, data: &[u8]) {
        if data.len() != 1 {
            self.metrics.missed_write_count.inc();
            return;
        }
        if let Err(e) = self.device.write(offset.raw_value() as u8, data[0]) {
            self.metrics.error_count.inc();
            error!("Failed to trigger i8042 reset event: {:?}", e);
        } else {
            self.metrics.write_count.inc();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::prelude::FromRawFd;

    use vmm_sys_util::eventfd::EventFd;

    use super::*;

    const COMMAND_OFFSET: u8 = 4;
    const CMD_RESET_CPU: u8 = 0xFE;

    #[test]
    fn test_i8042_valid_ops() {
        let reset_evt = EventFdTrigger::new(EventFd::new(libc::EFD_NONBLOCK).unwrap());
        let mut i8042 = I8042Device::new(reset_evt.try_clone().unwrap());

        let mut v = [0x00u8; 1];
        i8042.pio_read(PioAddress(0), PioAddress(0), &mut v);
        assert_eq!(v[0], 0);
        assert_eq!(i8042.metrics.read_count.count(), 1);

        // Check if reset works.
        i8042.pio_write(
            PioAddress(0),
            PioAddress(COMMAND_OFFSET as u16),
            &[CMD_RESET_CPU],
        );
        assert_eq!(i8042.metrics.write_count.count(), 1);
        reset_evt.read().unwrap();
    }

    #[test]
    fn test_i8042_invalid_ops() {
        let reset_evt = EventFdTrigger::new(EventFd::new(libc::EFD_NONBLOCK).unwrap());
        let mut i8042 = I8042Device::new(reset_evt.try_clone().unwrap());

        let mut v = [0x00u8; 2];
        i8042.pio_read(PioAddress(0), PioAddress(0), &mut v);
        assert_eq!(v[0], 0);
        assert_eq!(i8042.metrics.read_count.count(), 0);
        assert_eq!(i8042.metrics.missed_read_count.count(), 1);

        i8042.pio_write(
            PioAddress(0),
            PioAddress(COMMAND_OFFSET as u16),
            &[CMD_RESET_CPU, 0],
        );
        assert_eq!(i8042.metrics.write_count.count(), 0);
        assert_eq!(i8042.metrics.missed_write_count.count(), 1);
    }

    #[test]
    #[ignore = "Issue #10821 - IO Safety violation: owned file descriptor already closed"]
    fn test_i8042_reset_err() {
        let reset_evt = EventFdTrigger::new(unsafe { EventFd::from_raw_fd(i32::MAX) });
        let mut i8042 = I8042Device::new(reset_evt);
        i8042.pio_write(
            PioAddress(0),
            PioAddress(COMMAND_OFFSET as u16),
            &[CMD_RESET_CPU],
        );
        assert_eq!(i8042.metrics.write_count.count(), 0);
        assert_eq!(i8042.metrics.error_count.count(), 1);
    }
}
