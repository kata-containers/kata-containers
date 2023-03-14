// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
//
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! Emulation for legacy devices.
//!
//! For now, it offers emulation support only for the Linux serial console,
//! an Arm PL031 Real Time Clock (RTC), and an i8042 PS/2 controller that only
//! handles the CPU reset.
//!
//! It also provides a [Trigger](trait.Trigger.html) interface for an object
//! that can generate an event.

#![deny(missing_docs)]

pub mod i8042;
pub mod rtc_pl031;
pub mod serial;

pub use i8042::I8042Device;
pub use rtc_pl031::{Rtc, RtcState};
pub use serial::Serial;

use std::result::Result;

/// Abstraction for a simple, push-button like interrupt mechanism.
/// This helps in abstracting away how events/interrupts are generated when
/// working with the emulated devices.
///
/// The user has to provide a `Trigger` object to the device's constructor when
/// initializing that device. The generic type `T: Trigger` is used in the
/// device's structure definition to mark the fact that the events notification
/// mechanism is done via the Trigger interface.
/// An example of how to implement the `Trigger` interface for
/// [an eventfd wrapper](https://docs.rs/vmm-sys-util/latest/vmm_sys_util/eventfd/index.html)
/// can be found in the
/// [`Example` section from `Serial`](../vm_superio/serial/struct.Serial.html#example).
/// The `EventFd` is wrapped in the `EventFdTrigger` newtype because Rust
/// doesn't allow implementing an external trait on external types. To get
/// around this restriction, the newtype pattern can be used. More details
/// about this,
/// [here](https://doc.rust-lang.org/book/ch19-03-advanced-traits.html#using-the-newtype-pattern-to-implement-external-traits-on-external-types).
pub trait Trigger {
    /// Underlying type for the potential error conditions returned by `Self::trigger`.
    type E;

    /// Trigger an event.
    fn trigger(&self) -> Result<(), Self::E>;
}
