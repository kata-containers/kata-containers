// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Type-safe bindings for Zircon event pairs.

use {AsHandleRef, Cookied, HandleBased, Handle, HandleRef, Peered, Status};
use {sys, ok};

/// An object representing a Zircon
/// [event pair](https://fuchsia.googlesource.com/zircon/+/master/docs/concepts.md#Other-IPC_Events_Event-Pairs_and-User-Signals).
///
/// As essentially a subtype of `Handle`, it can be freely interconverted.
#[derive(Debug, Eq, PartialEq)]
pub struct EventPair(Handle);
impl_handle_based!(EventPair);
impl Peered for EventPair {}
impl Cookied for EventPair {}

impl EventPair {
    /// Create an event pair, a pair of objects which can signal each other. Wraps the
    /// [zx_eventpair_create](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/eventpair_create.md)
    /// syscall.
    pub fn create() -> Result<(EventPair, EventPair), Status> {
        let mut out0 = 0;
        let mut out1 = 0;
        let options = 0;
        let status = unsafe { sys::zx_eventpair_create(options, &mut out0, &mut out1) };
        ok(status)?;
        unsafe {
            Ok((
                Self::from(Handle::from_raw(out0)),
                Self::from(Handle::from_raw(out1))
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use {DurationNum, Signals};

    #[test]
    fn wait_and_signal_peer() {
        let (p1, p2) = EventPair::create().unwrap();
        let eighty_ms = 80.millis();

        // Waiting on one without setting any signal should time out.
        assert_eq!(p2.wait_handle(Signals::USER_0, eighty_ms.after_now()), Err(Status::TIMED_OUT));

        // If we set a signal, we should be able to wait for it.
        assert!(p1.signal_peer(Signals::NONE, Signals::USER_0).is_ok());
        assert_eq!(p2.wait_handle(Signals::USER_0, eighty_ms.after_now()).unwrap(),
            Signals::USER_0);

        // Should still work, signals aren't automatically cleared.
        assert_eq!(p2.wait_handle(Signals::USER_0, eighty_ms.after_now()).unwrap(),
            Signals::USER_0);

        // Now clear it, and waiting should time out again.
        assert!(p1.signal_peer(Signals::USER_0, Signals::NONE).is_ok());
        assert_eq!(p2.wait_handle(Signals::USER_0, eighty_ms.after_now()), Err(Status::TIMED_OUT));
    }
}
