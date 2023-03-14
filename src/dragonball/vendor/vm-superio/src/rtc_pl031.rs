// Copyright 2021 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Provides emulation for a minimal ARM PL031 Real Time Clock.
//!
//! This module implements a PL031 Real Time Clock (RTC) that provides a long
//! time base counter. This is achieved by generating an interrupt signal after
//! counting for a programmed number of cycles of a real-time clock input.

use std::convert::TryFrom;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// The following defines are mapping to the specification:
// https://developer.arm.com/documentation/ddi0224/c/Programmers-model/Summary-of-RTC-registers
//
// From 0x0 to 0x1C we have following registers:
const RTCDR: u16 = 0x000; // Data Register (RO).
const RTCMR: u16 = 0x004; // Match Register.
const RTCLR: u16 = 0x008; // Load Register.
const RTCCR: u16 = 0x00C; // Control Register.
const RTCIMSC: u16 = 0x010; // Interrupt Mask Set or Clear Register.
const RTCRIS: u16 = 0x014; // Raw Interrupt Status (RO).
const RTCMIS: u16 = 0x018; // Masked Interrupt Status (RO).
const RTCICR: u16 = 0x01C; // Interrupt Clear Register (WO).

// From 0x020 to 0xFDC => reserved space.

// From 0xFE0 to 0xFFF => Peripheral and PrimeCell Identification Registers
//  These are read-only registers, so we store their values in a constant array.
//  The values are found in the 'Reset value' column of Table 3.1 (Summary of
//  RTC registers) in the the reference manual linked above.
const AMBA_IDS: [u8; 8] = [0x31, 0x10, 0x04, 0x00, 0x0d, 0xf0, 0x05, 0xb1];

// Since we are specifying the AMBA IDs in an array, instead of in individual
// registers, these constants bound the register addresses where these IDs
// would normally be located.
const AMBA_ID_LOW: u16 = 0xFE0;
const AMBA_ID_HIGH: u16 = 0xFFF;

/// Defines a series of callbacks that are invoked in response to the occurrence of specific
/// failure or missed events as part of the RTC operation (e.g., write to an invalid offset). The
/// methods below can be implemented by a backend that keeps track of such events by incrementing
/// metrics, logging messages, or any other action.
///
/// We're using a trait to avoid constraining the concrete characteristics of the backend in
/// any way, enabling zero-cost abstractions and use case-specific implementations.
pub trait RtcEvents {
    /// The driver attempts to read from an invalid offset.
    fn invalid_read(&self);

    /// The driver attempts to write to an invalid offset.
    fn invalid_write(&self);
}

/// Provides a no-op implementation of `RtcEvents` which can be used in situations that
/// do not require logging or otherwise doing anything in response to the events defined
/// as part of `RtcEvents`.
pub struct NoEvents;

impl RtcEvents for NoEvents {
    fn invalid_read(&self) {}
    fn invalid_write(&self) {}
}

impl<EV: RtcEvents> RtcEvents for Arc<EV> {
    fn invalid_read(&self) {
        self.as_ref().invalid_read();
    }

    fn invalid_write(&self) {
        self.as_ref().invalid_write();
    }
}

/// A PL031 Real Time Clock (RTC) that emulates a long time base counter.
///
/// This structure emulates the registers for the RTC.
///
/// # Example
///
/// ```rust
/// # use std::thread;
/// # use std::io::Error;
/// # use std::ops::Deref;
/// # use std::time::{Instant, Duration, SystemTime, UNIX_EPOCH};
/// # use vm_superio::Rtc;
///
/// let mut data = [0; 4];
/// let mut rtc = Rtc::new();
/// const RTCDR: u16 = 0x0; // Data Register.
/// const RTCLR: u16 = 0x8; // Load Register.
///
/// // Write system time since UNIX_EPOCH in seconds to the load register.
/// let v = SystemTime::now()
///     .duration_since(UNIX_EPOCH)
///     .unwrap()
///     .as_secs() as u32;
/// data = v.to_le_bytes();
/// rtc.write(RTCLR, &data);
///
/// // Read the value back out of the load register.
/// rtc.read(RTCLR, &mut data);
/// assert_eq!(v, u32::from_le_bytes(data));
///
/// // Sleep for 1.5 seconds to let the counter tick.
/// let delay = Duration::from_millis(1500);
/// thread::sleep(delay);
///
/// // Read the current RTC value from the Data Register.
/// rtc.read(RTCDR, &mut data);
/// assert!(u32::from_le_bytes(data) > v);
/// ```
pub struct Rtc<EV: RtcEvents> {
    // The load register.
    lr: u32,

    // The offset applied to the counter to get the RTC value.
    offset: i64,

    // The MR register is used for implementing the RTC alarm. A
    // real time clock alarm is a feature that can be used to allow
    // a computer to 'wake up' after shut down to execute tasks
    // every day or on a certain day. It can sometimes be found in
    // the 'Power Management' section of a motherboard's BIOS setup.
    // This is not currently implemented, so we raise an error.
    // TODO: Implement the match register functionality.
    mr: u32,

    // The interrupt mask.
    imsc: u32,

    // The raw interrupt value.
    ris: u32,

    // Used for tracking the occurrence of significant events.
    events: EV,
}

/// The state of the Rtc device.
#[derive(Clone, Debug, PartialEq)]
pub struct RtcState {
    /// The load register.
    pub lr: u32,
    /// The offset applied to the counter to get the RTC value.
    pub offset: i64,
    /// The MR register.
    pub mr: u32,
    /// The interrupt mask.
    pub imsc: u32,
    /// The raw interrupt value.
    pub ris: u32,
}

fn get_current_time() -> u32 {
    let epoch_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        // This expect should never fail because UNIX_EPOCH is in 1970,
        // and the only possible failure is if `now` time is before UNIX EPOCH.
        .expect("SystemTime::duration_since failed");
    // The following conversion is safe because u32::MAX would correspond to
    // year 2106. By then we would not be able to use the RTC in its
    // current form because RTC only works with 32-bits registers, and a bigger
    // time value would not fit.
    epoch_time.as_secs() as u32
}

impl Default for Rtc<NoEvents> {
    fn default() -> Self {
        Self::new()
    }
}

// This is the state from which a fresh Rtc can be created.
impl Default for RtcState {
    fn default() -> Self {
        RtcState {
            // The load register is initialized to 0.
            lr: 0,
            offset: 0,
            // The match register is initialised to zero (not currently used).
            // TODO: Implement the match register functionality.
            mr: 0,
            // The interrupt mask is initialised as not set.
            imsc: 0,
            // The raw interrupt is initialised as not asserted.
            ris: 0,
        }
    }
}

impl Rtc<NoEvents> {
    /// Creates a new `AMBA PL031 RTC` instance without any metric capabilities. The instance is
    /// created from the default state.
    pub fn new() -> Self {
        Self::from_state(&RtcState::default(), NoEvents)
    }
}

impl<EV: RtcEvents> Rtc<EV> {
    /// Creates a new `AMBA PL031 RTC` instance from a given `state` and that is able to track
    /// events during operation using the passed `rtc_events` object.
    /// For creating the instance from a fresh state, [`with_events`](#method.with_events) or
    /// [`new`](#method.new) methods can be used.
    ///
    /// # Arguments
    /// * `state` - A reference to the state from which the `Rtc` is constructed.
    /// * `rtc_events` - The `RtcEvents` implementation used to track the occurrence
    ///                  of failure or missed events in the RTC operation.
    pub fn from_state(state: &RtcState, rtc_events: EV) -> Self {
        Rtc {
            lr: state.lr,
            offset: state.offset,
            mr: state.mr,
            imsc: state.imsc,
            ris: state.ris,
            // A struct implementing `RtcEvents` for tracking the occurrence of
            // significant events.
            events: rtc_events,
        }
    }

    /// Creates a new `AMBA PL031 RTC` instance that is able to track events during operation using
    /// the passed `rtc_events` object. The instance is created from the default state.
    ///
    /// # Arguments
    /// * `rtc_events` - The `RtcEvents` implementation used to track the occurrence
    ///                  of failure or missed events in the RTC operation.
    pub fn with_events(rtc_events: EV) -> Self {
        Self::from_state(&RtcState::default(), rtc_events)
    }

    /// Returns the state of the RTC.
    pub fn state(&self) -> RtcState {
        RtcState {
            lr: self.lr,
            offset: self.offset,
            mr: self.mr,
            imsc: self.imsc,
            ris: self.ris,
        }
    }

    /// Provides a reference to the RTC events object.
    pub fn events(&self) -> &EV {
        &self.events
    }

    fn get_rtc_value(&self) -> u32 {
        // The RTC value is the time + offset as per:
        // https://developer.arm.com/documentation/ddi0224/c/Functional-overview/RTC-functional-description/Update-block
        //
        // In the unlikely case of the value not fitting in an u32, we just set the time to
        // the current time on the host.
        let current_host_time = get_current_time();
        u32::try_from(
            (current_host_time as i64)
                .checked_add(self.offset)
                .unwrap_or(current_host_time as i64),
        )
        .unwrap_or(current_host_time)
    }

    /// Handles a write request from the driver at `offset` offset from the
    /// base register address.
    ///
    /// # Arguments
    /// * `offset` - The offset from the base register specifying
    ///              the register to be written.
    /// * `data` - The little endian, 4 byte array to write to the register
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `Rtc`](struct.Rtc.html#example).
    pub fn write(&mut self, offset: u16, data: &[u8; 4]) {
        let val = u32::from_le_bytes(*data);

        match offset {
            RTCMR => {
                // Set the match register.
                // TODO: Implement the match register functionality.
                self.mr = val;
            }
            RTCLR => {
                // The guest can make adjustments to its time by writing to
                // this register. When these adjustments happen, we calculate the
                // offset as the difference between the LR value and the host time.
                // This offset is later used to calculate the RTC value (see
                // `get_rtc_value`).
                self.lr = val;
                // Both lr & offset are u32, hence the following
                // conversions are safe, and the result fits in an i64.
                self.offset = self.lr as i64 - get_current_time() as i64;
            }
            RTCCR => {
                // Writing 1 to the control register resets the RTC value,
                // which means both the load register and the offset are reset.
                if val == 1 {
                    self.lr = 0;
                    self.offset = 0;
                }
            }
            RTCIMSC => {
                // Set or clear the interrupt mask.
                self.imsc = val & 1;
            }
            RTCICR => {
                // Writing 1 clears the interrupt; however, since the match
                // register is unimplemented, this should never be necessary.
                self.ris &= !val;
            }
            _ => {
                // RTCDR, RTCRIS, and RTCMIS are read-only, so writes to these
                // registers or to an invalid offset are ignored; however,
                // We increment the invalid_write() method of the events struct.
                self.events.invalid_write();
            }
        };
    }

    /// Handles a read request from the driver at `offset` offset from the
    /// base register address.
    ///
    /// # Arguments
    /// * `offset` - The offset from the base register specifying
    ///              the register to be read.
    /// * `data` - The little-endian, 4 byte array storing the read value.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `Rtc`](struct.Rtc.html#example).
    pub fn read(&mut self, offset: u16, data: &mut [u8; 4]) {
        let v = if (AMBA_ID_LOW..=AMBA_ID_HIGH).contains(&offset) {
            let index = ((offset - AMBA_ID_LOW) >> 2) as usize;
            u32::from(AMBA_IDS[index])
        } else {
            match offset {
                RTCDR => self.get_rtc_value(),
                RTCMR => {
                    // Read the match register.
                    // TODO: Implement the match register functionality.
                    self.mr
                }
                RTCLR => self.lr,
                RTCCR => 1, // RTC is always enabled.
                RTCIMSC => self.imsc,
                RTCRIS => self.ris,
                RTCMIS => self.ris & self.imsc,
                _ => {
                    // RTCICR is write only.  For reads of this register or
                    // an invalid offset, call the invalid_read method of the
                    // events struct and return.
                    self.events.invalid_read();
                    return;
                }
            }
        };

        *data = v.to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::AtomicU64;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use vmm_sys_util::metric::Metric;

    #[derive(Default)]
    struct ExampleRtcMetrics {
        invalid_read_count: AtomicU64,
        invalid_write_count: AtomicU64,
    }

    impl RtcEvents for ExampleRtcMetrics {
        fn invalid_read(&self) {
            self.invalid_read_count.inc();
            // We can also log a message here, or as part of any of the other methods.
        }

        fn invalid_write(&self) {
            self.invalid_write_count.inc();
        }
    }

    #[test]
    fn test_regression_year_1970() {
        // This is a regression test for: https://github.com/rust-vmm/vm-superio/issues/47.
        // The problem is that the time in the guest would show up as in the 1970s.
        let mut rtc = Rtc::new();
        let expected_time = get_current_time();

        let mut actual_time = [0u8; 4];
        rtc.read(RTCDR, &mut actual_time);
        // Check that the difference between the current time, and the time read from the
        // RTC device is never bigger than one second. This should hold true irrespective of
        // scheduling.
        assert!(u32::from_le_bytes(actual_time) - expected_time <= 1);
    }

    #[test]
    fn test_data_register() {
        // Verify we can read the Data Register, but not write to it,
        // and that the Data Register RTC count increments over time.
        // This also tests that the invalid write metric is incremented for
        // writes to RTCDR.
        let metrics = Arc::new(ExampleRtcMetrics::default());
        let mut rtc = Rtc::with_events(metrics);
        let mut data = [0; 4];

        // Check metrics are equal to 0 at the beginning.
        assert_eq!(rtc.events.invalid_read_count.count(), 0);
        assert_eq!(rtc.events.invalid_write_count.count(), 0);

        // Read the data register.
        rtc.read(RTCDR, &mut data);
        let first_read = u32::from_le_bytes(data);

        // Sleep for 1.5 seconds to let the counter tick.
        let delay = Duration::from_millis(1500);
        thread::sleep(delay);

        // Read the data register again.
        rtc.read(RTCDR, &mut data);
        let second_read = u32::from_le_bytes(data);

        // The second time should be greater than the first
        assert!(second_read > first_read);

        // Sleep for 1.5 seconds to let the counter tick.
        let delay = Duration::from_millis(1500);
        thread::sleep(delay);

        // Writing the data register should have no effect.
        data = 0u32.to_le_bytes();
        rtc.write(RTCDR, &data);

        // Invalid write should increment. All others should not change.
        assert_eq!(rtc.events.invalid_read_count.count(), 0);
        assert_eq!(rtc.events.invalid_write_count.count(), 1);

        // Read the data register again.
        rtc.read(RTCDR, &mut data);
        let third_read = u32::from_le_bytes(data);

        // The third time should be greater than the second.
        assert!(third_read > second_read);

        // Confirm metrics are unchanged.
        assert_eq!(rtc.events.invalid_read_count.count(), 0);
        assert_eq!(rtc.events.invalid_write_count.count(), 1);
    }

    #[test]
    fn test_match_register() {
        // Test reading and writing to the match register.
        // TODO: Implement the alarm functionality and confirm an interrupt
        // is raised when the match register is set.
        let mut rtc = Rtc::new();
        let mut data: [u8; 4];

        // Write to the match register.
        data = 123u32.to_le_bytes();
        rtc.write(RTCMR, &data);

        // Read the value back out of the match register and confirm it was
        // correctly written.
        rtc.read(RTCMR, &mut data);
        assert_eq!(123, u32::from_le_bytes(data));
    }

    #[test]
    fn test_load_register() {
        // Read and write to the load register to confirm we can both
        // set the RTC value forward and backward.
        // This also tests the default Rtc constructor.
        let mut rtc: Rtc<NoEvents> = Default::default();
        let mut data = [0; 4];

        // Get the RTC value with a load register of 0 (the initial value).
        rtc.read(RTCDR, &mut data);
        let old_val = u32::from_le_bytes(data);

        // Increment LR and verify that the value was updated.
        let lr = get_current_time() + 100;
        data = lr.to_le_bytes();
        rtc.write(RTCLR, &data);

        // Read the load register and verify it matches the value just loaded.
        rtc.read(RTCLR, &mut data);
        assert_eq!(lr, u32::from_le_bytes(data));

        // Read the data register and verify it matches the value just loaded.
        // Note that this assumes less than 1 second has elapsed between
        // setting RTCLR and this read (based on the RTC counter
        // tick rate being 1Hz).
        rtc.read(RTCDR, &mut data);
        assert_eq!(lr, u32::from_le_bytes(data));

        // Confirm that the new RTC value is greater than the old
        let new_val = u32::from_le_bytes(data);
        assert!(new_val > old_val);

        // Set the LR in the past, and check that the RTC value is updated.
        let lr = get_current_time() - 100;
        data = lr.to_le_bytes();
        rtc.write(RTCLR, &data);

        rtc.read(RTCDR, &mut data);
        let rtc_value = u32::from_le_bytes(data);
        assert!(rtc_value < get_current_time());

        // Checking that setting the maximum possible value for the LR does
        // not cause overflows.
        let lr = u32::MAX;
        data = lr.to_le_bytes();
        rtc.write(RTCLR, &data);
        rtc.read(RTCDR, &mut data);
        assert!(rtc.offset > -(u32::MAX as i64) && rtc.offset < u32::MAX as i64);
        // We're checking that this is not 0 because that's the value we're
        // setting in case the DR value does not fit in an u32.
        assert_ne!(u32::from_le_bytes(data), 0);

        // Reset the RTC value to 0 and confirm it was reset.
        let lr = 0u32;
        data = lr.to_le_bytes();
        rtc.write(RTCLR, &data);

        // Read the data register and verify it has been reset.
        rtc.read(RTCDR, &mut data);
        assert_eq!(lr, u32::from_le_bytes(data));
    }

    #[test]
    fn test_rtc_value_overflow() {
        // Verify that the RTC value will wrap on overflow instead of panic.
        let mut rtc = Rtc::new();
        let mut data: [u8; 4];

        // Write u32::MAX to the load register
        let lr_max = u32::MAX;
        data = lr_max.to_le_bytes();
        rtc.write(RTCLR, &data);

        // Read the load register and verify it matches the value just loaded.
        rtc.read(RTCLR, &mut data);
        assert_eq!(lr_max, u32::from_le_bytes(data));

        // Read the data register and verify it matches the value just loaded.
        // Note that this assumes less than 1 second has elapsed between
        // setting RTCLR and this read (based on the RTC counter
        // tick rate being 1Hz).
        rtc.read(RTCDR, &mut data);
        assert_eq!(lr_max, u32::from_le_bytes(data));

        // Sleep for 1.5 seconds to let the counter tick. This should
        // cause the RTC value to overflow and wrap.
        let delay = Duration::from_millis(1500);
        thread::sleep(delay);

        // Read the data register and verify it has wrapped around.
        rtc.read(RTCDR, &mut data);
        assert!(lr_max > u32::from_le_bytes(data));
    }

    #[test]
    fn test_interrupt_mask_set_clear_register() {
        // Test setting and clearing the interrupt mask bit.
        let mut rtc = Rtc::new();
        let mut data: [u8; 4];

        // Manually set the raw interrupt.
        rtc.ris = 1;

        // Set the mask bit.
        data = 1u32.to_le_bytes();
        rtc.write(RTCIMSC, &data);

        // Confirm the mask bit is set.
        rtc.read(RTCIMSC, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));

        // Confirm the raw and masked interrupts are set.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));
        rtc.read(RTCMIS, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));

        // Clear the mask bit.
        data = 0u32.to_le_bytes();
        rtc.write(RTCIMSC, &data);

        // Confirm the mask bit is cleared.
        rtc.read(RTCIMSC, &mut data);
        assert_eq!(0, u32::from_le_bytes(data));

        // Confirm the raw interrupt is set and the masked
        // interrupt is not.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));
        rtc.read(RTCMIS, &mut data);
        assert_eq!(0, u32::from_le_bytes(data));
    }

    #[test]
    fn test_interrupt_clear_register() {
        // Test clearing the interrupt. This also tests
        // that the invalid read and write metrics are incremented.
        let metrics = Arc::new(ExampleRtcMetrics::default());
        let mut rtc = Rtc::with_events(metrics);
        let mut data = [0; 4];

        // Check metrics are equal to 0 at the beginning.
        assert_eq!(rtc.events.invalid_read_count.count(), 0);
        assert_eq!(rtc.events.invalid_write_count.count(), 0);

        // Manually set the raw interrupt and interrupt mask.
        rtc.ris = 1;
        rtc.imsc = 1;

        // Confirm the raw and masked interrupts are set.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));
        rtc.read(RTCMIS, &mut data);
        assert_eq!(1, u32::from_le_bytes(data));

        // Write to the interrupt clear register.
        data = 1u32.to_le_bytes();
        rtc.write(RTCICR, &data);

        // Metrics should not change.
        assert_eq!(rtc.events.invalid_read_count.count(), 0);
        assert_eq!(rtc.events.invalid_write_count.count(), 0);

        // Confirm the raw and masked interrupts are cleared.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(0, u32::from_le_bytes(data));
        rtc.read(RTCMIS, &mut data);
        assert_eq!(0, u32::from_le_bytes(data));

        // Confirm reading from RTCICR has no effect.
        data = 123u32.to_le_bytes();
        rtc.read(RTCICR, &mut data);
        let v = u32::from_le_bytes(data);
        assert_eq!(v, 123);

        // Invalid read should increment.  All others should not change.
        assert_eq!(rtc.events.invalid_read_count.count(), 1);
        assert_eq!(rtc.events.invalid_write_count.count(), 0);
    }

    #[test]
    fn test_control_register() {
        // Writing 1 to the Control Register should reset the RTC value.
        // Writing 0 should have no effect.
        let mut rtc = Rtc::new();
        let mut data: [u8; 4];

        // Let's move the guest time in the future.
        let lr = get_current_time() + 100;
        data = lr.to_le_bytes();
        rtc.write(RTCLR, &data);

        // Get the RTC value.
        rtc.read(RTCDR, &mut data);
        let old_val = u32::from_le_bytes(data);

        // Reset the RTC value by writing 1 to RTCCR.
        data = 1u32.to_le_bytes();
        rtc.write(RTCCR, &data);

        // Get the RTC value.
        rtc.read(RTCDR, &mut data);
        let new_val = u32::from_le_bytes(data);

        // The new value should be less than the old value.
        assert!(new_val < old_val);

        // Attempt to clear the control register should have no effect on
        // either the RTCCR value or the RTC value.
        data = 0u32.to_le_bytes();
        rtc.write(RTCCR, &data);

        // Read the RTCCR value and confirm it's still 1.
        rtc.read(RTCCR, &mut data);
        let v = u32::from_le_bytes(data);
        assert_eq!(v, 1);

        // Sleep for 1.5 seconds to let the counter tick.
        let delay = Duration::from_millis(1500);
        thread::sleep(delay);

        // Read the RTC value and confirm it has incremented.
        let old_val = new_val;
        rtc.read(RTCDR, &mut data);
        let new_val = u32::from_le_bytes(data);
        assert!(new_val > old_val);
    }

    #[test]
    fn test_raw_interrupt_status_register() {
        // Writing to the Raw Interrupt Status Register should have no effect,
        // and reading should return the value of RTCRIS.
        let mut rtc = Rtc::new();
        let mut data = [0; 4];

        // Set the raw interrupt for testing.
        rtc.ris = 1u32;

        // Read the current value of RTCRIS.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(u32::from_le_bytes(data), 1);

        // Attempt to write to RTCRIS.
        data = 0u32.to_le_bytes();
        rtc.write(RTCRIS, &data);

        // Read the current value of RTCRIS and confirm it's unchanged.
        rtc.read(RTCRIS, &mut data);
        assert_eq!(u32::from_le_bytes(data), 1);
    }

    #[test]
    fn test_mask_interrupt_status_register() {
        // Writing to the Masked Interrupt Status Register should have no effect,
        // and reading should return the value of RTCRIS & RTCIMSC.
        let mut rtc = Rtc::new();
        let mut data = [0; 4];

        // Set the raw interrupt for testing.
        rtc.ris = 1u32;

        // Confirm the mask bit is not set.
        rtc.read(RTCIMSC, &mut data);
        assert_eq!(0, u32::from_le_bytes(data));

        // Read the current value of RTCMIS. Since the interrupt mask is
        // initially 0, the interrupt should not be masked and reading RTCMIS
        // should return 0.
        rtc.read(RTCMIS, &mut data);
        assert_eq!(u32::from_le_bytes(data), 0);

        // Set the mask bit.
        data = 1u32.to_le_bytes();
        rtc.write(RTCIMSC, &data);

        // Read the current value of RTCMIS. Since the interrupt mask is
        // now set, the masked interrupt should be set.
        rtc.read(RTCMIS, &mut data);
        assert_eq!(u32::from_le_bytes(data), 1);

        // Attempt to write to RTCMIS should have no effect.
        data = 0u32.to_le_bytes();
        rtc.write(RTCMIS, &data);

        // Read the current value of RTCMIS and confirm it's unchanged.
        rtc.read(RTCMIS, &mut data);
        assert_eq!(u32::from_le_bytes(data), 1);
    }

    #[test]
    fn test_read_only_register_addresses() {
        let mut rtc = Rtc::new();
        let mut data = [0; 4];

        // Read the current value of AMBA_ID_LOW.
        rtc.read(AMBA_ID_LOW, &mut data);
        assert_eq!(data[0], AMBA_IDS[0]);

        // Attempts to write to read-only registers (AMBA_ID_LOW in this case)
        // should have no effect.
        data = 123u32.to_le_bytes();
        rtc.write(AMBA_ID_LOW, &data);

        // Reread the current value of AMBA_ID_LOW and confirm it's unchanged.
        rtc.read(AMBA_ID_LOW, &mut data);
        assert_eq!(data[0], AMBA_IDS[0]);

        // Reading from the AMBA_ID registers should succeed.
        // Becuase we compute the index of the AMBA_IDS array by a logical bit
        // shift of (offset - AMBA_ID_LOW) >> 2, we want to make sure that
        // we correctly align down to a 4-byte register boundary, and that we
        // don't overflow (we shouldn't, since offset provided to read()
        // is unsigned).

        // Verify that we can read from AMBA_ID_LOW and that the logical shift
        // doesn't overflow.
        data = [0; 4];
        rtc.read(AMBA_ID_LOW, &mut data);
        assert_eq!(data[0], AMBA_IDS[0]);

        // Verify that attempts to read from AMBA_ID_LOW + 5 align down to
        // AMBA_ID_LOW + 4, corresponding to AMBA_IDS[1].
        data = [0; 4];
        rtc.read(AMBA_ID_LOW + 5, &mut data);
        assert_eq!(data[0], AMBA_IDS[1]);
    }

    #[test]
    fn test_invalid_write_offset() {
        // Test that writing to an invalid register offset has no effect
        // on the RTC value (as read from the data register), and confirm
        // the invalid write metric increments.
        let metrics = Arc::new(ExampleRtcMetrics::default());
        let mut rtc = Rtc::with_events(metrics);
        let mut data = [0; 4];

        // Check metrics are equal to 0 at the beginning.
        assert_eq!(rtc.events.invalid_read_count.count(), 0);
        assert_eq!(rtc.events.invalid_write_count.count(), 0);

        // First test: Write to an address outside the expected range of
        // register memory.

        // Read the data register.
        rtc.read(RTCDR, &mut data);
        let first_read = u32::from_le_bytes(data);

        // Attempt to write to an address outside the expected range of
        // register memory.
        data = 123u32.to_le_bytes();
        rtc.write(AMBA_ID_HIGH + 4, &data);

        // Invalid write should increment.  All others should not change.
        assert_eq!(rtc.events.invalid_read_count.count(), 0);
        assert_eq!(rtc.events.invalid_write_count.count(), 1);

        // Read the data register again.
        rtc.read(RTCDR, &mut data);
        let second_read = u32::from_le_bytes(data);

        // RTCDR should be unchanged.
        // Note that this assumes less than 1 second has elapsed between
        // the first and second read of RTCDR (based on the RTC counter
        // tick rate being 1Hz).
        assert_eq!(second_read, first_read);

        // Second test: Attempt to write to a register address similar to the
        // load register, but not actually valid.

        // Read the data register.
        rtc.read(RTCDR, &mut data);
        let first_read = u32::from_le_bytes(data);

        // Attempt to write to an invalid register address close to the load
        // register's address.
        data = 123u32.to_le_bytes();
        rtc.write(RTCLR + 1, &data);

        // Invalid write should increment again.  All others should not change.
        assert_eq!(rtc.events.invalid_read_count.count(), 0);
        assert_eq!(rtc.events.invalid_write_count.count(), 2);

        // Read the data register again.
        rtc.read(RTCDR, &mut data);
        let second_read = u32::from_le_bytes(data);

        // RTCDR should be unchanged
        // Note that this assumes less than 1 second has elapsed between
        // the first and second read of RTCDR (based on the RTC counter
        // tick rate being 1Hz).
        assert_eq!(second_read, first_read);

        // Confirm neither metric has changed.
        assert_eq!(rtc.events.invalid_read_count.count(), 0);
        assert_eq!(rtc.events.invalid_write_count.count(), 2);
    }

    #[test]
    fn test_invalid_read_offset() {
        // Test that reading from an invalid register offset has no effect,
        // and confirm the invalid read metric increments.
        let metrics = Arc::new(ExampleRtcMetrics::default());
        let mut rtc = Rtc::with_events(metrics);
        let mut data: [u8; 4];

        // Check metrics are equal to 0 at the beginning.
        assert_eq!(rtc.events.invalid_read_count.count(), 0);
        assert_eq!(rtc.events.invalid_write_count.count(), 0);

        // Reading from a non-existent register should have no effect.
        data = 123u32.to_le_bytes();
        rtc.read(AMBA_ID_HIGH + 4, &mut data);
        assert_eq!(123, u32::from_le_bytes(data));

        // Invalid read should increment. All others should not change.
        assert_eq!(rtc.events.invalid_read_count.count(), 1);
        assert_eq!(rtc.events.invalid_write_count.count(), 0);

        // Just to prove that AMBA_ID_HIGH + 4 doesn't contain 123...
        data = 321u32.to_le_bytes();
        rtc.read(AMBA_ID_HIGH + 4, &mut data);
        assert_eq!(321, u32::from_le_bytes(data));

        // Invalid read should increment again. All others should not change.
        assert_eq!(rtc.events.invalid_read_count.count(), 2);
        assert_eq!(rtc.events.invalid_write_count.count(), 0);
    }

    #[test]
    fn test_state() {
        let metrics = Arc::new(ExampleRtcMetrics::default());
        let mut rtc = Rtc::with_events(metrics);
        let mut data = [0; 4];

        // Get the RTC value with a load register of 0 (the initial value).
        rtc.read(RTCDR, &mut data);
        let first_read = u32::from_le_bytes(data);

        // Increment LR and verify that the value was updated.
        let lr = get_current_time() + 100;
        data = lr.to_le_bytes();
        rtc.write(RTCLR, &data);

        let state = rtc.state();
        rtc.read(RTCLR, &mut data);
        assert_eq!(state.lr.to_le_bytes(), data);

        // Do an invalid `write` in order to increment a metric.
        let mut data2 = 123u32.to_le_bytes();
        rtc.write(AMBA_ID_HIGH + 4, &data2);
        assert_eq!(rtc.events.invalid_write_count.count(), 1);

        let metrics = Arc::new(ExampleRtcMetrics::default());
        let mut rtc_from_state = Rtc::from_state(&state, metrics.clone());
        let state_after_restore = rtc_from_state.state();

        // Check that the old and the new state are identical.
        assert_eq!(state, state_after_restore);

        // Read the data register again.
        rtc.read(RTCDR, &mut data);
        let second_read = u32::from_le_bytes(data);
        // The RTC values should be different.
        assert!(second_read > first_read);

        // Reading from the LR register should return the same value as before saving the state.
        rtc_from_state.read(RTCLR, &mut data2);
        assert_eq!(data, data2);

        // Check that the restored `Rtc` doesn't keep the state of the old `metrics` object.
        assert_eq!(rtc_from_state.events.invalid_write_count.count(), 0);

        // Let's increment again a metric, and this time save the state of events as well (separate
        // from the base state).
        // Do an invalid `write` in order to increment a metric.
        let data3 = 123u32.to_le_bytes();
        rtc_from_state.write(AMBA_ID_HIGH + 4, &data3);
        assert_eq!(rtc_from_state.events.invalid_write_count.count(), 1);

        let state2 = rtc_from_state.state();
        // Mimic saving the metrics for the sake of the example.
        let saved_metrics = metrics;
        let rtc = Rtc::from_state(&state2, saved_metrics);

        // Check that the restored `Rtc` keeps the state of the old `metrics` object.
        assert_eq!(rtc.events.invalid_write_count.count(), 1);
    }

    #[test]
    fn test_overflow_offset() {
        // Test that an invalid offset (too big) does not cause an overflow.
        let rtc_state = RtcState {
            lr: 65535,
            offset: 9223372036854710636,
            mr: 0,
            imsc: 0,
            ris: 0,
        };
        let mut rtc = Rtc::from_state(&rtc_state, NoEvents);
        let mut data = [0u8; 4];
        rtc.read(RTCDR, &mut data);
    }
}
