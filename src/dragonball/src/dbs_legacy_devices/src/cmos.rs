// Copyright 2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::cmp::min;
use std::mem;

use libc::{clock_gettime, gmtime_r, timespec, tm, CLOCK_REALTIME};
use vmm_sys_util::eventfd::EventFd;

use dbs_device::{DeviceIoMut, PioAddress};

/// The value of index offset register is always guaranteed to be in range via INDEX_MASK.
const INDEX_MASK: u8 = 0x7f;
/// Offset of index offset register.
const INDEX_OFFSET: u16 = 0x0;
/// Offset of data offset register.
const DATA_OFFSET: u16 = 0x1;
/// Length of Cmos memory.
const DATA_LEN: usize = 128;

/// A CMOS/RTC device commonly seen on x86 I/O port 0x70/0x71.
pub struct CmosDevice {
    index: u8,
    data: [u8; DATA_LEN],
    reset_evt: EventFd,
}

impl CmosDevice {
    /// Constructs a CMOS/RTC device with initial data.
    /// `mem_below_4g` is the size of memory in bytes below the 32-bit gap.
    /// `mem_above_4g` is the size of memory in bytes above the 32-bit gap.
    pub fn new(mem_below_4g: u64, mem_above_4g: u64, reset_evt: EventFd) -> CmosDevice {
        let mut data = [0u8; DATA_LEN];
        // Extended memory from 16 MB to 4 GB in units of 64 KB
        let ext_mem = min(
            0xFFFF,
            mem_below_4g.saturating_sub(16 * 1024 * 1024) / (64 * 1024),
        );
        data[0x34] = ext_mem as u8;
        data[0x35] = (ext_mem >> 8) as u8;
        // High memory (> 4GB) in units of 64 KB
        let high_mem = min(0x00FF_FFFF, mem_above_4g / (64 * 1024));
        data[0x5b] = high_mem as u8;
        data[0x5c] = (high_mem >> 8) as u8;
        data[0x5d] = (high_mem >> 16) as u8;
        CmosDevice {
            index: 0,
            data,
            reset_evt,
        }
    }
}
impl DeviceIoMut for CmosDevice {
    fn pio_write(&mut self, _base: PioAddress, offset: PioAddress, data: &[u8]) {
        if data.len() != 1 {
            return;
        }
        match offset.raw_value() {
            INDEX_OFFSET => self.index = data[0],
            DATA_OFFSET => {
                if self.index == 0x8f && data[0] == 0 {
                    self.reset_evt.write(1).unwrap();
                } else {
                    self.data[(self.index & INDEX_MASK) as usize] = data[0]
                }
            }
            _ => {}
        };
    }
    fn pio_read(&mut self, _base: PioAddress, offset: PioAddress, data: &mut [u8]) {
        fn to_bcd(v: u8) -> u8 {
            assert!(v < 100);
            ((v / 10) << 4) | (v % 10)
        }
        if data.len() != 1 {
            return;
        }
        data[0] = match offset.raw_value() {
            INDEX_OFFSET => self.index,
            DATA_OFFSET => {
                let seconds;
                let minutes;
                let hours;
                let week_day;
                let day;
                let month;
                let year;
                // The clock_gettime and gmtime_r calls are safe as long as the structs they are
                // given are large enough, and neither of them fail. It is safe to zero initialize
                // the tm and timespec struct because it contains only plain data.
                let update_in_progress = unsafe {
                    let mut timespec: timespec = mem::zeroed();
                    clock_gettime(CLOCK_REALTIME, &mut timespec as *mut _);
                    let now = timespec.tv_sec;
                    let mut tm: tm = mem::zeroed();
                    gmtime_r(&now, &mut tm as *mut _);
                    // The following lines of code are safe but depend on tm being in scope.
                    seconds = tm.tm_sec;
                    minutes = tm.tm_min;
                    hours = tm.tm_hour;
                    week_day = tm.tm_wday + 1;
                    day = tm.tm_mday;
                    month = tm.tm_mon + 1;
                    year = tm.tm_year;
                    // Update in Progress bit held for last 224us of each second
                    const NANOSECONDS_PER_SECOND: i64 = 1_000_000_000;
                    const UIP_HOLD_LENGTH: i64 = 8 * NANOSECONDS_PER_SECOND / 32768;
                    timespec.tv_nsec >= (NANOSECONDS_PER_SECOND - UIP_HOLD_LENGTH)
                };
                match self.index {
                    0x00 => to_bcd(seconds as u8),
                    0x02 => to_bcd(minutes as u8),
                    0x04 => to_bcd(hours as u8),
                    0x06 => to_bcd(week_day as u8),
                    0x07 => to_bcd(day as u8),
                    0x08 => to_bcd(month as u8),
                    0x09 => to_bcd((year % 100) as u8),
                    // Bit 5 for 32kHz clock. Bit 7 for Update in Progress
                    0x0a => 1 << 5 | (update_in_progress as u8) << 7,
                    // Bit 0-6 are reserved and must be 0.
                    // Bit 7 must be 1 (CMOS has power)
                    0x0d => 1 << 7,
                    0x32 => to_bcd(((year + 1900) / 100) as u8),
                    _ => {
                        // self.index is always guaranteed to be in range via INDEX_MASK.
                        self.data[(self.index & INDEX_MASK) as usize]
                    }
                }
            }
            _ => 0,
        }
    }
}
