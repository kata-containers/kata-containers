// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Type-safe bindings for Zircon kernel
//! [syscalls](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls.md).

#![deny(warnings)]

#[macro_use]
extern crate bitflags;

pub extern crate fuchsia_zircon_sys as sys;

#[deprecated(note="use fuchsia_zircon::sys::ZX_CPRNG_DRAW_MAX_LEN instead")]
#[doc(hidden)]
pub use sys::ZX_CPRNG_DRAW_MAX_LEN;

// Implements the HandleBased traits for a Handle newtype struct
macro_rules! impl_handle_based {
    ($type_name:path) => {
        impl AsHandleRef for $type_name {
            fn as_handle_ref(&self) -> HandleRef {
                self.0.as_handle_ref()
            }
        }

        impl From<Handle> for $type_name {
            fn from(handle: Handle) -> Self {
                $type_name(handle)
            }
        }

        impl From<$type_name> for Handle {
            fn from(x: $type_name) -> Handle {
                x.0
            }
        }

        impl HandleBased for $type_name {}
    }
}

// Creates associated constants of TypeName of the form
// `pub const NAME: TypeName = TypeName(value);`
macro_rules! assoc_consts {
    ($typename:ident, [$($name:ident = $num:expr;)*]) => {
        #[allow(non_upper_case_globals)]
        impl $typename {
            $(
                pub const $name: $typename = $typename($num);
            )*
        }
    }
}

mod channel;
mod cprng;
mod event;
mod eventpair;
mod fifo;
mod handle;
mod job;
mod port;
mod process;
mod rights;
mod socket;
mod signals;
mod status;
mod time;
mod thread;
mod vmar;
mod vmo;

pub use channel::*;
pub use cprng::*;
pub use event::*;
pub use eventpair::*;
pub use fifo::*;
pub use handle::*;
pub use job::*;
pub use port::*;
pub use process::*;
pub use rights::*;
pub use socket::*;
pub use signals::*;
pub use status::*;
pub use thread::*;
pub use time::*;
pub use vmar::*;
pub use vmo::*;

/// Prelude containing common utility traits.
/// Designed for use like `use fuchsia_zircon::prelude::*;`
pub mod prelude {
    pub use {
        AsHandleRef,
        Cookied,
        DurationNum,
        HandleBased,
        Peered,
    };
}

/// Convenience re-export of `Status::ok`.
pub fn ok(raw: sys::zx_status_t) -> Result<(), Status> {
    Status::ok(raw)
}

/// A "wait item" containing a handle reference and information about what signals
/// to wait on, and, on return from `object_wait_many`, which are pending.
#[repr(C)]
#[derive(Debug)]
pub struct WaitItem<'a> {
    /// The handle to wait on.
    pub handle: HandleRef<'a>,
    /// A set of signals to wait for.
    pub waitfor: Signals,
    /// The set of signals pending, on return of `object_wait_many`.
    pub pending: Signals,
}

/// An identifier to select a particular clock. See
/// [zx_time_get](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/time_get.md)
/// for more information about the possible values.
#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ClockId {
    /// The number of nanoseconds since the system was powered on. Corresponds to
    /// `ZX_CLOCK_MONOTONIC`.
    Monotonic = 0,
    /// The number of wall clock nanoseconds since the Unix epoch (midnight on January 1 1970) in
    /// UTC. Corresponds to ZX_CLOCK_UTC.
    UTC = 1,
    /// The number of nanoseconds the current thread has been running for. Corresponds to
    /// ZX_CLOCK_THREAD.
    Thread = 2,
}

/// Wait on multiple handles.
/// The success return value is a bool indicating whether one or more of the
/// provided handle references was closed during the wait.
///
/// Wraps the
/// [zx_object_wait_many](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/object_wait_many.md)
/// syscall.
pub fn object_wait_many(items: &mut [WaitItem], deadline: Time) -> Result<bool, Status>
{
    let len = try!(usize_into_u32(items.len()).map_err(|_| Status::OUT_OF_RANGE));
    let items_ptr = items.as_mut_ptr() as *mut sys::zx_wait_item_t;
    let status = unsafe { sys::zx_object_wait_many( items_ptr, len, deadline.nanos()) };
    if status == sys::ZX_ERR_CANCELED {
        return Ok(true)
    }
    ok(status).map(|()| false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use super::prelude::*;

    #[test]
    fn monotonic_time_increases() {
        let time1 = Time::get(ClockId::Monotonic);
        1_000.nanos().sleep();
        let time2 = Time::get(ClockId::Monotonic);
        assert!(time2 > time1);
    }

    #[test]
    fn utc_time_increases() {
        let time1 = Time::get(ClockId::UTC);
        1_000.nanos().sleep();
        let time2 = Time::get(ClockId::UTC);
        assert!(time2 > time1);
    }

    #[test]
    fn thread_time_increases() {
        let time1 = Time::get(ClockId::Thread);
        1_000.nanos().sleep();
        let time2 = Time::get(ClockId::Thread);
        assert!(time2 > time1);
    }

    #[test]
    fn ticks_increases() {
        let ticks1 = ticks_get();
        1_000.nanos().sleep();
        let ticks2 = ticks_get();
        assert!(ticks2 > ticks1);
    }

    #[test]
    fn tick_length() {
        let sleep_time = 1.milli();
        let ticks1 = ticks_get();
        sleep_time.sleep();
        let ticks2 = ticks_get();

        // The number of ticks should have increased by at least 1 ms worth
        let sleep_ticks = sleep_time.millis() * ticks_per_second() / 1000;
        assert!(ticks2 >= (ticks1 + sleep_ticks));
    }

    #[test]
    fn into_raw() {
        let vmo = Vmo::create(1).unwrap();
        let h = vmo.into_raw();
        let vmo2 = Vmo::from(unsafe { Handle::from_raw(h) });
        assert!(vmo2.write(b"1", 0).is_ok());
    }

    #[test]
    fn sleep() {
        let sleep_ns = 1.millis();
        let time1 = Time::get(ClockId::Monotonic);
        sleep_ns.sleep();
        let time2 = Time::get(ClockId::Monotonic);
        assert!(time2 > time1 + sleep_ns);
    }

    /// Test duplication by means of a VMO
    #[test]
    fn duplicate() {
        let hello_length: usize = 5;

        // Create a VMO and write some data to it.
        let vmo = Vmo::create(hello_length as u64).unwrap();
        assert!(vmo.write(b"hello", 0).is_ok());

        // Replace, reducing rights to read.
        let readonly_vmo = vmo.duplicate_handle(Rights::READ).unwrap();
        // Make sure we can read but not write.
        let mut read_vec = vec![0; hello_length];
        assert_eq!(readonly_vmo.read(&mut read_vec, 0).unwrap(), hello_length);
        assert_eq!(read_vec, b"hello");
        assert_eq!(readonly_vmo.write(b"", 0), Err(Status::ACCESS_DENIED));

        // Write new data to the original handle, and read it from the new handle
        assert!(vmo.write(b"bye", 0).is_ok());
        assert_eq!(readonly_vmo.read(&mut read_vec, 0).unwrap(), hello_length);
        assert_eq!(read_vec, b"byelo");
    }

    // Test replace by means of a VMO
    #[test]
    fn replace() {
        let hello_length: usize = 5;

        // Create a VMO and write some data to it.
        let vmo = Vmo::create(hello_length as u64).unwrap();
        assert!(vmo.write(b"hello", 0).is_ok());

        // Replace, reducing rights to read.
        let readonly_vmo = vmo.replace_handle(Rights::READ).unwrap();
        // Make sure we can read but not write.
        let mut read_vec = vec![0; hello_length];
        assert_eq!(readonly_vmo.read(&mut read_vec, 0).unwrap(), hello_length);
        assert_eq!(read_vec, b"hello");
        assert_eq!(readonly_vmo.write(b"", 0), Err(Status::ACCESS_DENIED));
    }

    #[test]
    fn wait_and_signal() {
        let event = Event::create().unwrap();
        let ten_ms = 10.millis();

        // Waiting on it without setting any signal should time out.
        assert_eq!(event.wait_handle(
            Signals::USER_0, ten_ms.after_now()), Err(Status::TIMED_OUT));

        // If we set a signal, we should be able to wait for it.
        assert!(event.signal_handle(Signals::NONE, Signals::USER_0).is_ok());
        assert_eq!(event.wait_handle(Signals::USER_0, ten_ms.after_now()).unwrap(),
            Signals::USER_0);

        // Should still work, signals aren't automatically cleared.
        assert_eq!(event.wait_handle(Signals::USER_0, ten_ms.after_now()).unwrap(),
            Signals::USER_0);

        // Now clear it, and waiting should time out again.
        assert!(event.signal_handle(Signals::USER_0, Signals::NONE).is_ok());
        assert_eq!(event.wait_handle(
            Signals::USER_0, ten_ms.after_now()), Err(Status::TIMED_OUT));
    }

    #[test]
    fn wait_many_and_signal() {
        let ten_ms = 10.millis();
        let e1 = Event::create().unwrap();
        let e2 = Event::create().unwrap();

        // Waiting on them now should time out.
        let mut items = vec![
          WaitItem { handle: e1.as_handle_ref(), waitfor: Signals::USER_0, pending: Signals::NONE },
          WaitItem { handle: e2.as_handle_ref(), waitfor: Signals::USER_1, pending: Signals::NONE },
        ];
        assert_eq!(object_wait_many(&mut items, ten_ms.after_now()), Err(Status::TIMED_OUT));
        assert_eq!(items[0].pending, Signals::NONE);
        assert_eq!(items[1].pending, Signals::NONE);

        // Signal one object and it should return success.
        assert!(e1.signal_handle(Signals::NONE, Signals::USER_0).is_ok());
        assert!(object_wait_many(&mut items, ten_ms.after_now()).is_ok());
        assert_eq!(items[0].pending, Signals::USER_0);
        assert_eq!(items[1].pending, Signals::NONE);

        // Signal the other and it should return both.
        assert!(e2.signal_handle(Signals::NONE, Signals::USER_1).is_ok());
        assert!(object_wait_many(&mut items, ten_ms.after_now()).is_ok());
        assert_eq!(items[0].pending, Signals::USER_0);
        assert_eq!(items[1].pending, Signals::USER_1);

        // Clear signals on both; now it should time out again.
        assert!(e1.signal_handle(Signals::USER_0, Signals::NONE).is_ok());
        assert!(e2.signal_handle(Signals::USER_1, Signals::NONE).is_ok());
        assert_eq!(object_wait_many(&mut items, ten_ms.after_now()), Err(Status::TIMED_OUT));
        assert_eq!(items[0].pending, Signals::NONE);
        assert_eq!(items[1].pending, Signals::NONE);
    }

    #[test]
    fn cookies() {
        let event = Event::create().unwrap();
        let scope = Event::create().unwrap();

        // Getting a cookie when none has been set should fail.
        assert_eq!(event.get_cookie(&scope.as_handle_ref()), Err(Status::ACCESS_DENIED));

        // Set a cookie.
        assert_eq!(event.set_cookie(&scope.as_handle_ref(), 42), Ok(()));

        // Should get it back....
        assert_eq!(event.get_cookie(&scope.as_handle_ref()), Ok(42));

        // but not with the wrong scope!
        assert_eq!(event.get_cookie(&event.as_handle_ref()), Err(Status::ACCESS_DENIED));

        // Can change it, with the same scope...
        assert_eq!(event.set_cookie(&scope.as_handle_ref(), 123), Ok(()));

        // but not with a different scope.
        assert_eq!(event.set_cookie(&event.as_handle_ref(), 123), Err(Status::ACCESS_DENIED));
    }
}

pub fn usize_into_u32(n: usize) -> Result<u32, ()> {
    if n > ::std::u32::MAX as usize || n < ::std::u32::MIN as usize {
        return Err(())
    }
    Ok(n as u32)
}

pub fn size_to_u32_sat(n: usize) -> u32 {
    if n > ::std::u32::MAX as usize {
        return ::std::u32::MAX;
    }
    if n < ::std::u32::MIN as usize {
        return ::std::u32::MIN;
    }
    n as u32
}
