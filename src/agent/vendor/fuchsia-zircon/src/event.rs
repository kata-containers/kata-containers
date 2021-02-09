// Copyright 2017 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Type-safe bindings for Zircon event objects.

use {AsHandleRef, Cookied, HandleBased, Handle, HandleRef, Status};
use {sys, ok};

/// An object representing a Zircon
/// [event object](https://fuchsia.googlesource.com/zircon/+/master/docs/objects/event.md).
///
/// As essentially a subtype of `Handle`, it can be freely interconverted.
#[derive(Debug, Eq, PartialEq)]
pub struct Event(Handle);
impl_handle_based!(Event);
impl Cookied for Event {}

impl Event {
    /// Create an event object, an object which is signalable but nothing else. Wraps the
    /// [zx_event_create](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/event_create.md)
    /// syscall.
    pub fn create() -> Result<Event, Status> {
        let mut out = 0;
        let opts = 0;
        let status = unsafe { sys::zx_event_create(opts, &mut out) };
        ok(status)?;
        unsafe {
            Ok(Self::from(Handle::from_raw(out)))
        }
    }
}
