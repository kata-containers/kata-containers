//! Logging scopes for slog-rs
//!
//! Logging scopes are convenience functionality for slog-rs to free user from manually passing
//! `Logger` objects around.
//!
//! Set of macros is also provided as an alternative to original `slog` crate macros, for logging
//! directly to `Logger` of the current logging scope.
//!
//! # Set global logger upfront
//!
//! **Warning**: Since `slog-scope` version 4.0.0, `slog-scope` defaults to
//! panicking on logging if no scope or global logger was set. Because of it, it
//! is advised to always set a global logger upfront with `set_global_logger`.
//!
//! # Using `slog-scope` as a part of API is not advised
//!
//! Part of a `slog` logging philosophy is ability to freely express logging contexts
//! according to logical structure, rather than callstack structure. By using
//! logging scopes the logging context is tied to code flow again, which is less
//! expressive.
//!
//! It is generally advised **NOT** to use `slog_scope` in libraries. Read more in
//! [slog-rs FAQ](https://github.com/slog-rs/slog/wiki/FAQ#do-i-have-to-pass-logger-around)
//!
//! ```
//! #[macro_use(slog_o, slog_info, slog_log, slog_record, slog_record_static, slog_b, slog_kv)]
//! extern crate slog;
//! #[macro_use]
//! extern crate slog_scope;
//! extern crate slog_term;
//!
//! use slog::Drain;
//!
//! fn foo() {
//!     slog_info!(slog_scope::logger(), "foo");
//!     info!("foo"); // Same as above, but more ergonomic and a bit faster
//!                   // since it uses `with_logger`
//! }
//!
//! fn main() {
//!     let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
//!     let log = slog::Logger::root(
//!         slog_term::FullFormat::new(plain)
//!         .build().fuse(), slog_o!()
//!     );
//!
//!     // Make sure to save the guard, see documentation for more information
//!     let _guard = slog_scope::set_global_logger(log);
//!     slog_scope::scope(&slog_scope::logger().new(slog_o!("scope" => "1")),
//!         || foo()
//!     );
//! }

#![warn(missing_docs)]

#[macro_use(o)]
extern crate slog;
#[macro_use]
extern crate lazy_static;
extern crate arc_swap;

use slog::{Logger, Record, OwnedKVList};

use std::sync::Arc;
use std::cell::RefCell;
use arc_swap::ArcSwap;

use std::result;

pub use slog::{slog_crit, slog_debug, slog_error, slog_info, slog_trace, slog_warn};

/// Log a critical level message using current scope logger
#[macro_export]
macro_rules! crit( ($($args:tt)+) => {
    $crate::with_logger(|logger| $crate::slog_crit![logger, $($args)+])
};);
/// Log a error level message using current scope logger
#[macro_export]
macro_rules! error( ($($args:tt)+) => {
    $crate::with_logger(|logger| $crate::slog_error![logger, $($args)+])
};);
/// Log a warning level message using current scope logger
#[macro_export]
macro_rules! warn( ($($args:tt)+) => {
    $crate::with_logger(|logger| $crate::slog_warn![logger, $($args)+])
};);
/// Log a info level message using current scope logger
#[macro_export]
macro_rules! info( ($($args:tt)+) => {
    $crate::with_logger(|logger| $crate::slog_info![logger, $($args)+])
};);
/// Log a debug level message using current scope logger
#[macro_export]
macro_rules! debug( ($($args:tt)+) => {
    $crate::with_logger(|logger| $crate::slog_debug![logger, $($args)+])
};);
/// Log a trace level message using current scope logger
#[macro_export]
macro_rules! trace( ($($args:tt)+) => {
    $crate::with_logger(|logger| $crate::slog_trace![logger, $($args)+])
};);

thread_local! {
    static TL_SCOPES: RefCell<Vec<*const slog::Logger>> = RefCell::new(Vec::with_capacity(8))
}

lazy_static! {
    static ref GLOBAL_LOGGER : ArcSwap<slog::Logger> = ArcSwap::from(
        Arc::new(
            slog::Logger::root(slog::Discard, o!())
        )
    );
}

struct NoGlobalLoggerSet;

impl slog::Drain for NoGlobalLoggerSet {
    type Ok = ();
    type Err = slog::Never;

    fn log(&self,
           _record: &Record,
           _values: &OwnedKVList)
        -> result::Result<Self::Ok, Self::Err> {
            panic!(
            "slog-scope: No logger set. Use `slog_scope::set_global_logger` or `slog_scope::scope`."
            )
        }
}


/// Guard resetting global logger
///
/// On drop it will reset global logger to `slog::Discard`.
/// This will `drop` any existing global logger.
#[must_use]
pub struct GlobalLoggerGuard {
    canceled : bool,
}

impl GlobalLoggerGuard {
    /// Getter for canceled to check status
    pub fn is_canceled(&self) -> bool {
        self.canceled
    }

    fn new() -> Self {
        GlobalLoggerGuard {
            canceled: false,
        }
    }

    /// Cancel resetting global logger
    pub fn cancel_reset(mut self) {
        self.canceled = true;
    }
}

impl Drop for GlobalLoggerGuard {
    fn drop(&mut self) {
        if !self.canceled {
            GLOBAL_LOGGER.store(
                Arc::new(
                    slog::Logger::root(NoGlobalLoggerSet, o!())
                    )
                );
        }
    }
}


/// Set global `Logger` that is returned by calls like `logger()` outside of any logging scope.
pub fn set_global_logger(l: slog::Logger) -> GlobalLoggerGuard {
    GLOBAL_LOGGER.store(Arc::new(l));

    GlobalLoggerGuard::new()
}

struct ScopeGuard;


impl ScopeGuard {
    fn new(logger: &slog::Logger) -> Self {
        TL_SCOPES.with(|s| { s.borrow_mut().push(logger as *const Logger); });

        ScopeGuard
    }
}

impl Drop for ScopeGuard {
    fn drop(&mut self) {
        TL_SCOPES.with(|s| { s.borrow_mut().pop().expect("TL_SCOPES should contain a logger"); })
    }
}

/// Access the `Logger` for the current logging scope
///
/// This function needs to clone an underlying scoped
/// `Logger`. If performance is of vital importance,
/// use `with_logger`.
pub fn logger() -> Logger {
    TL_SCOPES.with(|s| {
        let s = s.borrow();
        match s.last() {
            Some(logger) => (unsafe {&**logger}).clone(),
            None => Logger::clone(&GLOBAL_LOGGER.load())
        }
    })
}

/// Access the `Logger` for the current logging scope
///
/// This function doesn't have to clone the Logger
/// so it might be a bit faster.
pub fn with_logger<F, R>(f : F) -> R
where F : FnOnce(&Logger) -> R {
    TL_SCOPES.with(|s| {
        let s = s.borrow();
        match s.last() {
            Some(logger) => f(unsafe {&**logger}),
            None => f(&GLOBAL_LOGGER.load()),
        }
    })
}

/// Execute code in a logging scope
///
/// Logging scopes allow using a `slog::Logger` without explicitly
/// passing it in the code.
///
/// At any time current active `Logger` for a given thread can be retrived
/// with `logger()` call.
///
/// Logging scopes can be nested and are panic safe.
///
/// `logger` is the `Logger` to use during the duration of `f`.
/// `with_current_logger` can be used to build it as a child of currently active
/// logger.
///
/// `f` is a code to be executed in the logging scope.
///
/// Note: Thread scopes are thread-local. Each newly spawned thread starts
/// with a global logger, as a current logger.
pub fn scope<SF, R>(logger: &slog::Logger, f: SF) -> R
    where SF: FnOnce() -> R
{
    let _guard = ScopeGuard::new(&logger);
    f()
}
