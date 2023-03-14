// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Provides emulation for a super minimal i8042 controller.
//!
//! This emulates just the CPU reset command.

use std::result::Result;

use crate::Trigger;

// Offset of the command register, for write accesses (port 0x64). The same
// offset can be used, in case of read operations, to access the status
// register (in which we are not interested for an i8042 that only knows
// about reset).
const COMMAND_OFFSET: u8 = 4;

// Reset CPU command.
const CMD_RESET_CPU: u8 = 0xFE;

/// An i8042 PS/2 controller that emulates just enough to shutdown the machine.
///
/// A [`Trigger`](../trait.Trigger.html) object is used for notifying the VMM
/// about the CPU reset event.
///
/// # Example
///
/// ```rust
/// # use std::io::{Error, Result};
/// # use std::ops::Deref;
/// # use vm_superio::Trigger;
/// # use vm_superio::I8042Device;
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
/// }
///
/// let reset_evt = EventFdTrigger::new(libc::EFD_NONBLOCK);
/// let mut i8042 = I8042Device::new(reset_evt);
///
/// // Check read/write operations.
/// assert_eq!(i8042.read(0), 0);
/// i8042.write(4, 0xFE).unwrap();
/// ```
pub struct I8042Device<T: Trigger> {
    /// CPU reset event object. We will trigger this event when the guest issues
    /// the reset CPU command.
    reset_evt: T,
}

impl<T: Trigger> I8042Device<T> {
    /// Constructs an i8042 device that will signal the given event when the
    /// guest requests it.
    ///
    /// # Arguments
    /// * `reset_evt` - A Trigger object that will be used to notify the driver
    ///                 about the reset event.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `I8042Device`](struct.I8042Device.html#example).
    pub fn new(reset_evt: T) -> I8042Device<T> {
        I8042Device { reset_evt }
    }

    /// Handles a read request from the driver at `_offset` offset from the
    /// base I/O address.
    ///
    /// Returns the read value, which at this moment is 0x00, since we're not
    /// interested in an i8042 operation other than CPU reset.
    ///
    /// # Arguments
    /// * `_offset` - The offset that will be added to the base address
    ///              for writing to a specific register.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `I8042Device`](struct.I8042Device.html#example).
    pub fn read(&mut self, _offset: u8) -> u8 {
        0x00
    }

    /// Handles a write request from the driver at `offset` offset from the
    /// base I/O address.
    ///
    /// # Arguments
    /// * `offset` - The offset that will be added to the base address
    ///              for writing to a specific register.
    /// * `value` - The byte that should be written.
    ///
    /// # Example
    ///
    /// You can see an example of how to use this function in the
    /// [`Example` section from `I8042Device`](struct.I8042Device.html#example).
    pub fn write(&mut self, offset: u8, value: u8) -> Result<(), T::E> {
        match offset {
            COMMAND_OFFSET if value == CMD_RESET_CPU => {
                // Trigger the exit event.
                self.reset_evt.trigger()
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vmm_sys_util::eventfd::EventFd;

    #[test]
    fn test_i8042_valid_ops() {
        let reset_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut i8042 = I8042Device::new(reset_evt.try_clone().unwrap());

        assert_eq!(i8042.read(0), 0);

        // Check if reset works.
        i8042.write(COMMAND_OFFSET, CMD_RESET_CPU).unwrap();
        assert_eq!(reset_evt.read().unwrap(), 1);
    }

    #[test]
    fn test_i8042_invalid_reset() {
        let reset_evt = EventFd::new(libc::EFD_NONBLOCK).unwrap();
        let mut i8042 = I8042Device::new(reset_evt.try_clone().unwrap());

        // Write something different than CPU reset and check that the reset event
        // was not triggered. For this we have to write 1 to the reset event fd, so
        // that read doesn't block.
        assert!(reset_evt.write(1).is_ok());
        i8042.write(COMMAND_OFFSET, CMD_RESET_CPU + 1).unwrap();
        assert_eq!(reset_evt.read().unwrap(), 1);

        // Write the CPU reset to a different offset than COMMAND_OFFSET and check that
        // the reset event was not triggered.
        // For such accesses we should increment some metric (COMMAND_OFFSET + 1 is not
        // even a valid i8042 offset), see
        // [tracking issue](https://github.com/rust-vmm/vm-superio/issues/13).
        assert!(reset_evt.write(1).is_ok());
        i8042.write(COMMAND_OFFSET + 1, CMD_RESET_CPU).unwrap();
        assert_eq!(reset_evt.read().unwrap(), 1);
    }
}
