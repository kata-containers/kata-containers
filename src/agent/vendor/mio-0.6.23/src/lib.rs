#![doc(html_root_url = "https://docs.rs/mio/0.6.23")]
// Mio targets old versions of the Rust compiler. In order to do this, uses
// deprecated APIs.
#![allow(bare_trait_objects, deprecated, unknown_lints)]
#![deny(missing_docs, missing_debug_implementations)]
#![cfg_attr(test, deny(warnings))]

// Many of mio's public methods violate this lint, but they can't be fixed
// without a breaking change.
#![cfg_attr(feature = "cargo-clippy", allow(clippy::trivially_copy_pass_by_ref))]

//! A fast, low-level IO library for Rust focusing on non-blocking APIs, event
//! notification, and other useful utilities for building high performance IO
//! apps.
//!
//! # Features
//!
//! * Non-blocking TCP, UDP
//! * I/O event notification queue backed by epoll, kqueue, and IOCP
//! * Zero allocations at runtime
//! * Platform specific extensions
//!
//! # Non-goals
//!
//! The following are specifically omitted from Mio and are left to the user or higher-level libraries.
//!
//! * File operations
//! * Thread pools / multi-threaded event loop
//! * Timers
//!
//! # Platforms
//!
//! Currently supported platforms:
//!
//! * Linux
//! * OS X
//! * Windows
//! * FreeBSD
//! * NetBSD
//! * Android
//! * iOS
//!
//! mio can handle interfacing with each of the event notification systems of the aforementioned platforms. The details of
//! their implementation are further discussed in [`Poll`].
//!
//! # Usage
//!
//! Using mio starts by creating a [`Poll`], which reads events from the OS and
//! put them into [`Events`]. You can handle IO events from the OS with it.
//!
//! For more detail, see [`Poll`].
//!
//! [`Poll`]: struct.Poll.html
//! [`Events`]: struct.Events.html
//!
//! # Example
//!
//! ```
//! use mio::*;
//! use mio::net::{TcpListener, TcpStream};
//!
//! // Setup some tokens to allow us to identify which event is
//! // for which socket.
//! const SERVER: Token = Token(0);
//! const CLIENT: Token = Token(1);
//!
//! let addr = "127.0.0.1:13265".parse().unwrap();
//!
//! // Setup the server socket
//! let server = TcpListener::bind(&addr).unwrap();
//!
//! // Create a poll instance
//! let poll = Poll::new().unwrap();
//!
//! // Start listening for incoming connections
//! poll.register(&server, SERVER, Ready::readable(),
//!               PollOpt::edge()).unwrap();
//!
//! // Setup the client socket
//! let sock = TcpStream::connect(&addr).unwrap();
//!
//! // Register the socket
//! poll.register(&sock, CLIENT, Ready::readable(),
//!               PollOpt::edge()).unwrap();
//!
//! // Create storage for events
//! let mut events = Events::with_capacity(1024);
//!
//! loop {
//!     poll.poll(&mut events, None).unwrap();
//!
//!     for event in events.iter() {
//!         match event.token() {
//!             SERVER => {
//!                 // Accept and drop the socket immediately, this will close
//!                 // the socket and notify the client of the EOF.
//!                 let _ = server.accept();
//!             }
//!             CLIENT => {
//!                 // The server just shuts down the socket, let's just exit
//!                 // from our event loop.
//!                 return;
//!             }
//!             _ => unreachable!(),
//!         }
//!     }
//! }
//!
//! ```

extern crate net2;
extern crate iovec;
extern crate slab;

#[cfg(target_os = "fuchsia")]
extern crate fuchsia_zircon as zircon;
#[cfg(target_os = "fuchsia")]
extern crate fuchsia_zircon_sys as zircon_sys;

#[cfg(unix)]
extern crate libc;

#[cfg(windows)]
extern crate miow;

#[cfg(windows)]
extern crate winapi;

#[cfg(windows)]
extern crate kernel32;

#[macro_use]
extern crate log;

mod event_imp;
mod io;
mod poll;
mod sys;
mod token;
mod lazycell;

pub mod net;

#[deprecated(since = "0.6.5", note = "use mio-extras instead")]
#[cfg(feature = "with-deprecated")]
#[doc(hidden)]
pub mod channel;

#[deprecated(since = "0.6.5", note = "use mio-extras instead")]
#[cfg(feature = "with-deprecated")]
#[doc(hidden)]
pub mod timer;

#[deprecated(since = "0.6.5", note = "update to use `Poll`")]
#[cfg(feature = "with-deprecated")]
#[doc(hidden)]
pub mod deprecated;

#[deprecated(since = "0.6.5", note = "use iovec crate directly")]
#[cfg(feature = "with-deprecated")]
#[doc(hidden)]
pub use iovec::IoVec;

#[deprecated(since = "0.6.6", note = "use net module instead")]
#[cfg(feature = "with-deprecated")]
#[doc(hidden)]
pub mod tcp {
    pub use net::{TcpListener, TcpStream};
    pub use std::net::Shutdown;
}

#[deprecated(since = "0.6.6", note = "use net module instead")]
#[cfg(feature = "with-deprecated")]
#[doc(hidden)]
pub mod udp;

pub use poll::{
    Poll,
    Registration,
    SetReadiness,
};
pub use event_imp::{
    PollOpt,
    Ready,
};
pub use token::Token;

pub mod event {
    //! Readiness event types and utilities.

    pub use super::poll::{Events, Iter};
    pub use super::event_imp::{Event, Evented};
}

pub use event::{
    Events,
};

#[deprecated(since = "0.6.5", note = "use events:: instead")]
#[cfg(feature = "with-deprecated")]
#[doc(hidden)]
pub use event::{Event, Evented};

#[deprecated(since = "0.6.5", note = "use events::Iter instead")]
#[cfg(feature = "with-deprecated")]
#[doc(hidden)]
pub use poll::Iter as EventsIter;

#[deprecated(since = "0.6.5", note = "std::io::Error can avoid the allocation now")]
#[cfg(feature = "with-deprecated")]
#[doc(hidden)]
pub use io::deprecated::would_block;

#[cfg(all(unix, not(target_os = "fuchsia")))]
pub mod unix {
    //! Unix only extensions
    pub use sys::{
        EventedFd,
    };
    pub use sys::unix::UnixReady;
}

#[cfg(target_os = "fuchsia")]
pub mod fuchsia {
    //! Fuchsia-only extensions
    //!
    //! # Stability
    //!
    //! This module depends on the [magenta-sys crate](https://crates.io/crates/magenta-sys)
    //! and so might introduce breaking changes, even on minor releases,
    //! so long as that crate remains unstable.
    pub use sys::{
        EventedHandle,
    };
    pub use sys::fuchsia::{FuchsiaReady, zx_signals_t};
}

/// Windows-only extensions to the mio crate.
///
/// Mio on windows is currently implemented with IOCP for a high-performance
/// implementation of asynchronous I/O. Mio then provides TCP and UDP as sample
/// bindings for the system to connect networking types to asynchronous I/O. On
/// Unix this scheme is then also extensible to all other file descriptors with
/// the `EventedFd` type, but on Windows no such analog is available. The
/// purpose of this module, however, is to similarly provide a mechanism for
/// foreign I/O types to get hooked up into the IOCP event loop.
///
/// This module provides two types for interfacing with a custom IOCP handle:
///
/// * `Binding` - this type is intended to govern binding with mio's `Poll`
///   type. Each I/O object should contain an instance of `Binding` that's
///   interfaced with for the implementation of the `Evented` trait. The
///   `register`, `reregister`, and `deregister` methods for the `Evented` trait
///   all have rough analogs with `Binding`.
///
///   Note that this type **does not handle readiness**. That is, this type does
///   not handle whether sockets are readable/writable/etc. It's intended that
///   IOCP types will internally manage this state with a `SetReadiness` type
///   from the `poll` module. The `SetReadiness` is typically lazily created on
///   the first time that `Evented::register` is called and then stored in the
///   I/O object.
///
///   Also note that for types which represent streams of bytes the mio
///   interface of *readiness* doesn't map directly to the Windows model of
///   *completion*. This means that types will have to perform internal
///   buffering to ensure that a readiness interface can be provided. For a
///   sample implementation see the TCP/UDP modules in mio itself.
///
/// * `Overlapped` - this type is intended to be used as the concrete instances
///   of the `OVERLAPPED` type that most win32 methods expect. It's crucial, for
///   safety, that all asynchronous operations are initiated with an instance of
///   `Overlapped` and not another instantiation of `OVERLAPPED`.
///
///   Mio's `Overlapped` type is created with a function pointer that receives
///   a `OVERLAPPED_ENTRY` type when called. This `OVERLAPPED_ENTRY` type is
///   defined in the `winapi` crate. Whenever a completion is posted to an IOCP
///   object the `OVERLAPPED` that was signaled will be interpreted as
///   `Overlapped` in the mio crate and this function pointer will be invoked.
///   Through this function pointer, and through the `OVERLAPPED` pointer,
///   implementations can handle management of I/O events.
///
/// When put together these two types enable custom Windows handles to be
/// registered with mio's event loops. The `Binding` type is used to associate
/// handles and the `Overlapped` type is used to execute I/O operations. When
/// the I/O operations are completed a custom function pointer is called which
/// typically modifies a `SetReadiness` set by `Evented` methods which will get
/// later hooked into the mio event loop.
#[cfg(windows)]
pub mod windows {

    pub use sys::{Overlapped, Binding};
}

#[cfg(feature = "with-deprecated")]
mod convert {
    use std::time::Duration;

    const NANOS_PER_MILLI: u32 = 1_000_000;
    const MILLIS_PER_SEC: u64 = 1_000;

    /// Convert a `Duration` to milliseconds, rounding up and saturating at
    /// `u64::MAX`.
    ///
    /// The saturating is fine because `u64::MAX` milliseconds are still many
    /// million years.
    pub fn millis(duration: Duration) -> u64 {
        // Round up.
        let millis = (duration.subsec_nanos() + NANOS_PER_MILLI - 1) / NANOS_PER_MILLI;
        duration.as_secs().saturating_mul(MILLIS_PER_SEC).saturating_add(u64::from(millis))
    }
}
