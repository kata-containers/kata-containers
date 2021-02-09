//! Cleaning up signals.
//!
//! The routines in this module allow resetting the signals of an application back to defaults.
//! This is intended for the following situation:
//!
//! * A terminal signal (eg. a `SIGTERM`, `SIGINT` or something similar) is received.
//! * The application resets the signal handlers to defaults.
//! * The application proceeds to perform some kind of shutdown (saving data, cleaning up, ...).
//! * If another such signal is received, the application is terminated right away the hard way,
//!   without finishing the shutdown.
//!
//! The alternative of leaving the original signals in place might be problematic in case the
//! shutdown takes a long time or when it gets stuck. In such case the application would appear to
//! ignore the signal and just refuse to die.
//!
//! There are two ways to perform the reset:
//! * Registering the reset as part of the signal handlers. This is more reliable (even in case the
//!   application is already stuck in some kind of infinite loop, it would still work). This is
//!   done by [register].
//! * Manually resetting the handlers just before the shutdown. This is done with [cleanup_signal].

use std::io::Error;
#[cfg(not(windows))]
use std::ptr;

use libc::{c_int, sighandler_t, SIG_ERR};

#[cfg(not(windows))]
use libc::SIG_DFL;
// Unfortunately, not exported on windows :-(. Checked this actually works by tests/default.rs.
#[cfg(windows)]
const SIG_DFL: sighandler_t = 0;

pub use signal_hook_registry::unregister_signal;

use crate::SigId;

/// Resets the signal handler to the default one.
///
/// This is the lowest level wrapper around resetting the signal handler to the OS default. It
/// doesn't remove the hooks (though they will not get called), it doesn't handle errors and it
/// doesn't return any previous chained signals. The hooks will simply stay registered but dormant.
///
/// This function is async-signal-safe. However, you might prefer to use either [cleanup_signal] or
/// [register].
///
/// # Warning
///
/// This action is irreversible, once called, registering more hooks for the same signal will have
/// no effect (neither the old nor the new ones will be active but the registration will appear to
/// have succeeded).
///
/// This behaviour **can change** in future versions without considering it a breaking change.
///
/// In other words, this is expected to be called only before terminating the application and no
/// further manipulation of the given signal is supported in any way. While it won't cause UB, it
/// *will* produce unexpected results.
pub fn cleanup_raw(signal: c_int) -> sighandler_t {
    unsafe { ::libc::signal(signal, SIG_DFL) }
}

/// Resets the signal handler to the default one and removes all its hooks.
///
/// This resets the signal to the OS default. It doesn't revert to calling any previous signal
/// handlers (the ones not handled by `signal-hook`). All the hooks registered for this signal are
/// removed.
///
/// The intended use case is making sure further instances of a terminal signal have immediate
/// effect. If eg. a CTRL+C is pressed, the application removes all signal handling and proceeds to
/// its own shutdown phase. If the shutdown phase takes too long or gets stuck, the user may press
/// CTRL+C again which will then kill the application immediately, by a default signal action.
///
/// # Warning
///
/// This action is *global* (affecting hooks some other library or unrelated part of program
/// registered) and *irreversible*. Once called, registering new hooks for this signal has no
/// further effect (they'll appear to be registered, but they won't be called by the signal). The
/// latter may change in the future and it won't be considered a breaking change.
///
/// In other words, this is expected to be called only once the application enters its terminal
/// state and is not supported otherwise.
///
/// The function is **not** async-signal-safe. See [register] and [cleanup_raw] if you intend to
/// reset the signal directly from inside the signal handler itself.
///
/// # Examples
///
/// ```rust
/// # extern crate libc;
/// # extern crate signal_hook;
/// #
/// # use std::io::Error;
/// # use std::sync::atomic::{AtomicBool, Ordering};
/// # use std::sync::Arc;
/// #
/// # fn keep_processing() { std::thread::sleep(std::time::Duration::from_millis(50)); }
/// # fn app_cleanup() {}
/// use signal_hook::{cleanup, flag, SIGTERM};
///
/// fn main() -> Result<(), Error> {
///     let terminated = Arc::new(AtomicBool::new(false));
///     flag::register(SIGTERM, Arc::clone(&terminated))?;
/// #   unsafe { libc::raise(SIGTERM) };
///
///     while !terminated.load(Ordering::Relaxed) {
///         keep_processing();
///     }
///
///     cleanup::cleanup_signal(SIGTERM)?;
///     app_cleanup();
///     Ok(())
/// }
/// ```
pub fn cleanup_signal(signal: c_int) -> Result<(), Error> {
    // We use `signal` both on unix and windows here. Unlike with regular functions, usage of
    // SIG_DFL is portable and much more convenient to use.
    let result = cleanup_raw(signal);
    // The cast is needed on windows :-|.
    if result == SIG_ERR as _ {
        return Err(Error::last_os_error());
    }
    unregister_signal(signal);
    Ok(())
}

#[cfg(not(windows))]
fn verify_signals_exist(signals: &[c_int]) -> Result<(), Error> {
    signals
        .iter()
        .map(|s| -> Result<(), Error> {
            if unsafe { ::libc::sigaction(*s, ptr::null(), ptr::null_mut()) } == -1 {
                Err(Error::last_os_error())
            } else {
                Ok(())
            }
        })
        .collect()
}

#[cfg(windows)]
fn verify_signals_exist(_: &[c_int]) -> Result<(), Error> {
    // TODO: Do we have a way to check if the signals are valid on windows too?
    Ok(())
}

/// Register a cleanup after receiving a signal.
///
/// Registers an action that, after receiving `signal`, will reset all signals specified in
/// `cleanup` to their OS defaults. The reset is done as part of the signal handler.
///
/// The intended use case is that at CTRL+C (or something similar), the application starts shutting
/// down. This might take some time so by resetting all terminal signals to the defaults at that
/// time makes sure a second CTRL+C results in immediate (hard) termination of the application.
///
/// The hooks are still left inside and any following hooks after the reset are still run. Only the
/// next signal will be affected (and the hooks will be inert).
///
/// # Warning
///
/// The reset as part of the action is *global* and *irreversible*. All signal hooks and all
/// signals registered outside of `signal-hook` are affected and won't be run any more. Registering
/// more hooks for the same signals as cleaned will have no effect.
///
/// The latter part of having no effect may be changed in the future, do not rely on it.
/// Preferably, don't manipulate the signal any longer.
///
/// # Examples
///
/// ```rust
/// # extern crate libc;
/// # extern crate signal_hook;
/// #
/// # use std::io::Error;
/// # use std::sync::atomic::{AtomicBool, Ordering};
/// # use std::sync::Arc;
/// #
/// # fn keep_processing() { std::thread::sleep(std::time::Duration::from_millis(50)); }
/// # fn app_cleanup() {}
/// use signal_hook::{cleanup, flag, SIGINT, SIGTERM};
///
/// fn main() -> Result<(), Error> {
///     let terminated = Arc::new(AtomicBool::new(false));
///     flag::register(SIGTERM, Arc::clone(&terminated))?;
///     cleanup::register(SIGTERM, vec![SIGTERM, SIGINT])?;
/// #   unsafe { libc::raise(SIGTERM) };
///
///     while !terminated.load(Ordering::Relaxed) {
///         keep_processing();
///     }
///
///     app_cleanup();
///     Ok(())
/// }
/// ```
pub fn register(signal: c_int, cleanup: Vec<c_int>) -> Result<SigId, Error> {
    verify_signals_exist(&cleanup)?;
    let hook = move || {
        for sig in &cleanup {
            // Note: we are ignoring the errors here. We have no way to handle them and the only
            // possible ones are invalid signals ‒ which we should have handled by
            // verify_signals_exist above.
            cleanup_raw(*sig);
        }
    };
    unsafe { crate::register(signal, hook) }
}
