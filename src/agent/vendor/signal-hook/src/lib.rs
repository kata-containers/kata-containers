#![doc(
    html_root_url = "https://docs.rs/signal-hook/0.1.16/signal-hook/",
    test(attr(deny(warnings))),
    test(attr(allow(bare_trait_objects, unknown_lints)))
)]
#![deny(missing_docs, warnings)]
// Don't fail on links to things not enabled in features
#![allow(unknown_lints, intra_doc_link_resolution_failure)]
//! Library for easier and safe Unix signal handling
//!
//! Unix signals are inherently hard to handle correctly, for several reasons:
//!
//! * They are a global resource. If a library wants to set its own signal handlers, it risks
//!   disturbing some other library. It is possible to chain the previous signal handler, but then
//!   it is impossible to remove the old signal handlers from the chains in any practical manner.
//! * They can be called from whatever thread, requiring synchronization. Also, as they can
//!   interrupt a thread at any time, making most handling race-prone.
//! * According to the POSIX standard, the set of functions one may call inside a signal handler is
//!   limited to very few of them. To highlight, mutexes (or other locking mechanisms) and memory
//!   allocation and deallocation is *not* allowed.
//!
//! This library aims to solve some of the problems. It provides a global registry of actions
//! performed on arrival of signals. It is possible to register multiple actions for the same
//! signal and it is possible to remove the actions later on. If there was a previous signal
//! handler when the first action for a signal is registered, it is chained (but the original one
//! can't be removed).
//!
//! The main function of the library is [`register`](fn.register.html).
//!
//! It also offers several common actions one might want to register, implemented in the correct
//! way. They are scattered through submodules and have the same limitations and characteristics as
//! the [`register`](fn.register.html) function. Generally, they work to postpone the action taken
//! outside of the signal handler, where the full freedom and power of rust is available.
//!
//! Unlike other Rust libraries for signal handling, this should be flexible enough to handle all
//! the common and useful patterns.
//!
//! The library avoids all the newer fancy signal-handling routines. These generally have two
//! downsides:
//!
//! * They are not fully portable, therefore the library would have to contain *both* the
//!   implementation using the basic routines and the fancy ones. As signal handling is not on the
//!   hot path of most programs, this would not bring any actual benefit.
//! * The other routines require that the given signal is masked in all application's threads. As
//!   the signals are not masked by default and a new thread inherits the signal mask of its
//!   parent, it is possible to guarantee such global mask by masking them before any threads
//!   start. While this is possible for an application developer to do, it is not possible for a
//!   a library.
//!
//! # Warning
//!
//! Even with this library, you should thread with care. It does not eliminate all the problems
//! mentioned above.
//!
//! Also, note that the OS may collate multiple instances of the same signal into just one call of
//! the signal handler. Furthermore, some abstractions implemented here also naturally collate
//! multiple instances of the same signal. The general guarantee is, if there was at least one
//! signal of the given number delivered, an action will be taken, but it is not specified how many
//! times ‒ signals work mostly as kind of „wake up now“ nudge, if the application is slow to wake
//! up, it may be nudged multiple times before it does so.
//!
//! # Signal limitations
//!
//! OS limits still apply ‒ it is not possible to redefine certain signals (eg. `SIGKILL` or
//! `SIGSTOP`) and it is probably a *very* stupid idea to touch certain other ones (`SIGSEGV`,
//! `SIGFPE`, `SIGILL`). Therefore, this library will panic if any attempt at manipulating these is
//! made. There are some use cases for redefining the latter ones, but these are not well served by
//! this library and you really *really* have to know what you're doing and are generally on your
//! own doing that.
//!
//! # Signal masks
//!
//! As the library uses `sigaction` under the hood, signal masking works as expected (eg. with
//! `pthread_sigmask`). This means, signals will *not* be delivered if the signal is masked in all
//! program's threads.
//!
//! By the way, if you do want to modify the signal mask (or do other Unix-specific magic), the
//! [nix](https://crates.io/crates/nix) crate offers safe interface to many low-level functions,
//! including
//! [`pthread_sigmask`](https://docs.rs/nix/0.11.0/nix/sys/signal/fn.pthread_sigmask.html).
//!
//! # Portability
//!
//! It should work on any POSIX.1-2001 system, which are all the major big OSes with the notable
//! exception of Windows.
//!
//! Non-standard signals are also supported. Pass the signal value directly from `libc` or use
//! the numeric value directly.
//!
//! ```rust
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicBool};
//! let term = Arc::new(AtomicBool::new(false));
//! let _ = signal_hook::flag::register(libc::SIGINT, Arc::clone(&term));
//! ```
//!
//! This crate includes a limited support for Windows, based on `signal`/`raise` in the CRT.
//! There are differences in both API and behavior:
//!
//! - `iterator` and `pipe` are not yet implemented.
//! - We have only a few signals: `SIGABRT`, `SIGABRT_COMPAT`, `SIGBREAK`,
//!   `SIGFPE`, `SIGILL`, `SIGINT`, `SIGSEGV` and `SIGTERM`.
//! - Due to lack of signal blocking, there's a race condition.
//!   After the call to `signal`, there's a moment where we miss a signal.
//!   That means when you register a handler, there may be a signal which invokes
//!   neither the default handler or the handler you register.
//! - Handlers registered by `signal` in Windows are cleared on first signal.
//!   To match behavior in other platforms, we re-register the handler each time the handler is
//!   called, but there's a moment where we miss a handler.
//!   That means when you receive two signals in a row, there may be a signal which invokes
//!   the default handler, nevertheless you certainly have registered the handler.
//!
//! Moreover, signals won't work as you expected. `SIGTERM` isn't actually used and
//! not all `Ctrl-C`s are turned into `SIGINT`.
//!
//! Patches to improve Windows support in this library are welcome.
//!
//! # Examples
//!
//! ```rust
//! extern crate signal_hook;
//!
//! use std::io::Error;
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicBool, Ordering};
//!
//! fn main() -> Result<(), Error> {
//!     let term = Arc::new(AtomicBool::new(false));
//!     signal_hook::flag::register(signal_hook::SIGTERM, Arc::clone(&term))?;
//!     while !term.load(Ordering::Relaxed) {
//!         // Do some time-limited stuff here
//!         // (if this could block forever, then there's no guarantee the signal will have any
//!         // effect).
//! #
//! #       // Hack to terminate the example, not part of the real code.
//! #       term.store(true, Ordering::Relaxed);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! * `mio-support`: The [`Signals` iterator](iterator/struct.Signals.html) becomes pluggable into
//!   mio 0.6.
//! * `mio-0_7-support`: The [`Signals` iterator](iterator/struct.Signals.html) becomes pluggable into
//!   mio 0.7.
//! * `tokio-support`: The [`Signals`](iterator/struct.Signals.html) can be turned into
//!   [`Async`](iterator/struct.Async.html), which provides a `Stream` interface for integration in
//!   the asynchronous world.

#[cfg(feature = "tokio-support")]
extern crate futures;
extern crate libc;
#[cfg(feature = "mio-support")]
extern crate mio;
#[cfg(any(test, feature = "mio-0_7-support"))]
extern crate mio_0_7;
extern crate signal_hook_registry;
#[cfg(feature = "tokio-support")]
extern crate tokio_reactor;

pub mod cleanup;
pub mod flag;
#[cfg(not(windows))]
pub mod iterator;
#[cfg(not(windows))]
pub mod pipe;

#[cfg(not(windows))]
pub use libc::{
    SIGABRT, SIGALRM, SIGBUS, SIGCHLD, SIGCONT, SIGFPE, SIGHUP, SIGILL, SIGINT, SIGIO, SIGKILL,
    SIGPIPE, SIGPROF, SIGQUIT, SIGSEGV, SIGSTOP, SIGSYS, SIGTERM, SIGTRAP, SIGUSR1, SIGUSR2,
    SIGWINCH,
};

#[cfg(windows)]
pub use libc::{SIGABRT, SIGFPE, SIGILL, SIGINT, SIGSEGV, SIGTERM};

// NOTE: they perhaps deserve backport to libc.
#[cfg(windows)]
/// Same as `SIGABRT`, but the number is compatible to other platforms.
pub const SIGABRT_COMPAT: libc::c_int = 6;
#[cfg(windows)]
/// Ctrl-Break is pressed for Windows Console processes.
pub const SIGBREAK: libc::c_int = 21;

pub use signal_hook_registry::{register, unregister, SigId, FORBIDDEN};
