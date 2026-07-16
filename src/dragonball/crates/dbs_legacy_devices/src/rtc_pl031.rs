// Copyright 2022 Alibaba Cloud. All Rights Reserved.
// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! ARM PL031 Real Time Clock
//!
//! This module implements a PL031 Real Time Clock (RTC) that provides to provides long time base counter.
use std::convert::TryInto;
use std::sync::Arc;

use dbs_device::{DeviceIoMut, IoAddress};
use dbs_utils::metric::{IncMetric, SharedIncMetric};
use log::warn;
use serde::Serialize;
use vm_superio::rtc_pl031::{Rtc, RtcEvents};

/// Metrics specific to the RTC device
#[derive(Default, Serialize)]
pub struct RTCDeviceMetrics {
    /// Errors triggered while using the RTC device.
    pub error_count: SharedIncMetric,
    /// Number of superfluous read intents on this RTC device.
    pub missed_read_count: SharedIncMetric,
    /// Number of superfluous write intents on this RTC device.
    pub missed_write_count: SharedIncMetric,
}

impl RtcEvents for RTCDeviceMetrics {
    fn invalid_read(&self) {
        self.missed_read_count.inc();
        self.error_count.inc();
    }

    fn invalid_write(&self) {
        self.missed_write_count.inc();
        self.error_count.inc();
    }
}

/// The wrapper of Rtc struct to implement DeviceIoMut trait.
pub struct RTCDevice {
    rtc: Rtc<Arc<RTCDeviceMetrics>>,
}

impl Default for RTCDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl RTCDevice {
    pub fn new() -> Self {
        let metrics = Arc::new(RTCDeviceMetrics::default());
        Self {
            rtc: Rtc::with_events(metrics),
        }
    }

    pub fn metrics(&self) -> Arc<RTCDeviceMetrics> {
        Arc::clone(self.rtc.events())
    }
}

impl DeviceIoMut for RTCDevice {
    fn read(&mut self, _base: IoAddress, offset: IoAddress, data: &mut [u8]) {
        if data.len() == 4 {
            self.rtc
                .read(offset.raw_value() as u16, data.try_into().unwrap())
        } else {
            warn!(
                "Invalid RTC PL031 read: offset {}, data length {}",
                offset.raw_value(),
                data.len()
            );
            self.rtc.events().invalid_read();
        }
    }

    fn write(&mut self, _base: IoAddress, offset: IoAddress, data: &[u8]) {
        if data.len() == 4 {
            self.rtc
                .write(offset.raw_value() as u16, data.try_into().unwrap())
        } else {
            warn!(
                "Invalid RTC PL031 write: offset {}, data length {}",
                offset.raw_value(),
                data.len()
            );
            self.rtc.events().invalid_write();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl RTCDevice {
        fn read(&mut self, offset: u64, data: &mut [u8]) {
            DeviceIoMut::read(self, IoAddress::from(0), IoAddress::from(offset), data)
        }

        fn write(&mut self, offset: u64, data: &[u8]) {
            DeviceIoMut::write(self, IoAddress::from(0), IoAddress::from(offset), data)
        }
    }

    #[test]
    fn test_rtc_read_write_and_event() {
        let mut rtc_device = RTCDevice::new();
        let data = [0; 4];

        // Write to the DR register. Since this is a RO register, the write
        // function should fail.
        let invalid_writes_before = rtc_device.rtc.events().missed_write_count.count();
        let error_count_before = rtc_device.rtc.events().error_count.count();
        rtc_device.rtc.write(0x000, &data);
        let invalid_writes_after = rtc_device.rtc.events().missed_write_count.count();
        let error_count_after = rtc_device.rtc.events().error_count.count();
        assert_eq!(invalid_writes_after - invalid_writes_before, 1);
        assert_eq!(error_count_after - error_count_before, 1);

        let write_data_good = 123u32.to_le_bytes();
        let mut data_bad = [0; 2];
        let mut read_data_good = [0; 4];

        rtc_device.write(0x008, &write_data_good);
        rtc_device.write(0x008, &data_bad);
        rtc_device.read(0x008, &mut read_data_good);
        rtc_device.read(0x008, &mut data_bad);
        assert_eq!(u32::from_le_bytes(read_data_good), 123);
        assert_eq!(u16::from_le_bytes(data_bad), 0);
    }
}
