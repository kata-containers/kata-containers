//! `log` crate adapter for slog-rs
//!
//! This crate provides two way compatibility with Rust standard `log` crate.
//!
//! ### `log` -> `slog`
//!
//! After calling `init()` `slog-stdlog` will take a role of `log` crate
//! back-end, forwarding all the `log` logging to `slog_scope::logger()`.
//! In other words, any `log` crate logging statement will behave like it was `slog`
//! logging statement executed with logger returned by `slog_scope::logger()`.
//!
//! See documentation of `slog-scope` for more information about logging scopes.
//!
//! See [`init` documentation](fn.init.html) for an example.
//!
//! ### `slog` -> `log`
//!
//! `StdLog` is `slog::Drain` that will pass all `Record`s passing through it to
//! `log` crate just like they were crated with `log` crate logging macros in
//! the first place.
//!
//! ## `slog-scope`
//!
//! Since `log` does not have any form of context, and does not support `Logger`
//! `slog-stdlog` relies on "logging scopes" to establish it.
//!
//! You must set up logging context for `log` -> `slog` via `slog_scope::scope`
//! or `slog_scope::set_global_logger`. Setting a global logger upfront via
//! `slog_scope::set_global_logger` is highly recommended.
//!
//! Note: Since `slog-stdlog` v2, unlike previous releases, `slog-stdlog` uses
//! logging scopes provided by `slog-scope` crate instead of it's own.
//!
//! Refer to `slog-scope` crate documentation for more information.
//!
//! ### Warning
//!
//! Be careful when using both methods at the same time, as a loop can be easily
//! created: `log` -> `slog` -> `log` -> ...
//!
//! ## Compile-time log level filtering
//!
//! For filtering `debug!` and other `log` statements at compile-time, configure
//! the features on the `log` crate in your `Cargo.toml`:
//!
//! ```norust
//! log = { version = "*", features = ["max_level_trace", "release_max_level_warn"] }
//! ```
#![warn(missing_docs)]

extern crate log;

#[cfg(feature = "kv_unstable")]
mod kv;

use slog::{b, Level, KV};
use std::{fmt, io};

struct Logger;

fn log_to_slog_level(level: log::Level) -> Level {
    match level {
        log::Level::Trace => Level::Trace,
        log::Level::Debug => Level::Debug,
        log::Level::Info => Level::Info,
        log::Level::Warn => Level::Warning,
        log::Level::Error => Level::Error,
    }
}

fn record_as_location(r: &log::Record) -> slog::RecordLocation {
    let module = r.module_path_static().unwrap_or("<unknown>");
    let file = r.file_static().unwrap_or("<unknown>");
    let line = r.line().unwrap_or_default();

    slog::RecordLocation {
        file,
        line,
        column: 0,
        function: "",
        module,
    }
}

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, r: &log::Record) {
        let level = log_to_slog_level(r.metadata().level());

        let args = r.args();
        let target = r.target();
        let location = &record_as_location(r);
        let s = slog::RecordStatic {
            location,
            level,
            tag: target,
        };
        #[cfg(feature = "kv_unstable")]
        {
            let key_values = r.key_values();
            let mut visitor = kv::Visitor::new();
            key_values.visit(&mut visitor).unwrap();
            slog_scope::with_logger(|logger| logger.log(&slog::Record::new(&s, args, b!(visitor))))
        }
        #[cfg(not(feature = "kv_unstable"))]
        slog_scope::with_logger(|logger| logger.log(&slog::Record::new(&s, args, b!())))
    }

    fn flush(&self) {}
}

/// Register `slog-stdlog` as `log` backend.
///
/// This will pass all logging statements crated with `log`
/// crate to current `slog-scope::logger()`.
///
/// ```
/// #[macro_use]
/// extern crate log;
/// #[macro_use(slog_o, slog_kv)]
/// extern crate slog;
/// extern crate slog_stdlog;
/// extern crate slog_scope;
/// extern crate slog_term;
/// extern crate slog_async;
///
/// use slog::Drain;
///
/// fn main() {
///     let decorator = slog_term::TermDecorator::new().build();
///     let drain = slog_term::FullFormat::new(decorator).build().fuse();
///     let drain = slog_async::Async::new(drain).build().fuse();
///     let logger = slog::Logger::root(drain, slog_o!("version" => env!("CARGO_PKG_VERSION")));
///
///     let _scope_guard = slog_scope::set_global_logger(logger);
///     let _log_guard = slog_stdlog::init().unwrap();
///     // Note: this `info!(...)` macro comes from `log` crate
///     info!("standard logging redirected to slog");
/// }
/// ```
pub fn init() -> Result<(), log::SetLoggerError> {
    init_with_level(log::Level::max())
}

/// Register `slog-stdlog` as `log` backend.
/// Pass a log::Level to do the log filter explicitly.
///
/// This will pass all logging statements crated with `log`
/// crate to current `slog-scope::logger()`.
///
/// ```
/// #[macro_use]
/// extern crate log;
/// #[macro_use(slog_o, slog_kv)]
/// extern crate slog;
/// extern crate slog_stdlog;
/// extern crate slog_scope;
/// extern crate slog_term;
/// extern crate slog_async;
///
/// use slog::Drain;
///
/// fn main() {
///     let decorator = slog_term::TermDecorator::new().build();
///     let drain = slog_term::FullFormat::new(decorator).build().fuse();
///     let drain = slog_async::Async::new(drain).build().fuse();
///     let logger = slog::Logger::root(drain, slog_o!("version" => env!("CARGO_PKG_VERSION")));
///
///     let _scope_guard = slog_scope::set_global_logger(logger);
///     let _log_guard = slog_stdlog::init_with_level(log::Level::Error).unwrap();
///     // Note: this `info!(...)` macro comes from `log` crate
///     info!("standard logging redirected to slog");
///     error!("standard logging redirected to slog");
/// }
/// ```
pub fn init_with_level(level: log::Level) -> Result<(), log::SetLoggerError> {
    log::set_boxed_logger(Box::new(Logger))?;
    log::set_max_level(level.to_level_filter());

    Ok(())
}

/// Drain logging `Record`s into `log` crate
///
/// Any `Record` passing through this `Drain` will be forwarded
/// to `log` crate, just like it was created with `log` crate macros
/// in the first place. The message and key-value pairs will be formated
/// to be one string.
///
/// Caution needs to be taken to prevent circular loop where `Logger`
/// installed via `slog-stdlog::set_logger` would log things to a `StdLog`
/// drain, which would again log things to the global `Logger` and so on
/// leading to an infinite recursion.
pub struct StdLog;

struct LazyLogString<'a> {
    info: &'a slog::Record<'a>,
    logger_values: &'a slog::OwnedKVList,
}

impl<'a> LazyLogString<'a> {
    fn new(info: &'a slog::Record, logger_values: &'a slog::OwnedKVList) -> Self {
        LazyLogString {
            info,
            logger_values,
        }
    }
}

impl<'a> fmt::Display for LazyLogString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.info.msg())?;

        let io = io::Cursor::new(Vec::new());
        let mut ser = Ksv::new(io);

        self.logger_values
            .serialize(self.info, &mut ser)
            .map_err(|_| fmt::Error)?;
        self.info
            .kv()
            .serialize(self.info, &mut ser)
            .map_err(|_| fmt::Error)?;

        let values = ser.into_inner().into_inner();

        write!(f, "{}", String::from_utf8_lossy(&values))
    }
}

impl slog::Drain for StdLog {
    type Ok = ();
    type Err = io::Error;

    fn log(&self, info: &slog::Record, logger_values: &slog::OwnedKVList) -> io::Result<()> {
        let level = match info.level() {
            slog::Level::Critical | slog::Level::Error => log::Level::Error,
            slog::Level::Warning => log::Level::Warn,
            slog::Level::Info => log::Level::Info,
            slog::Level::Debug => log::Level::Debug,
            slog::Level::Trace => log::Level::Trace,
        };

        let mut target = info.tag();
        if target.is_empty() {
            target = info.module();
        }

        let lazy = LazyLogString::new(info, logger_values);
        /*
         * TODO: Support `log` crate key_values here.
         *
         * This requires the log/kv_unstable feature here.
         *
         * Not supporting this feature is backwards compatible
         * and it shouldn't break anything (because we've never had),
         * but is undesirable from a feature-completeness point of view.
         *
         * However, this is most likely not as powerful as slog's own
         * notion of key/value pairs, so I would humbly suggest using `slog`
         * directly if this feature is important to you ;)
         *
         * This avoids using the private log::__private_api_log api function,
         * which is just a thin wrapper around a `RecordBuilder`.
         */
        log::logger().log(
            &log::Record::builder()
                .args(format_args!("{}", lazy))
                .level(level)
                .target(target)
                .module_path_static(Some(info.module()))
                .file_static(Some(info.file()))
                .line(Some(info.line()))
                .build(),
        );

        Ok(())
    }
}

/// Key-Separator-Value serializer
struct Ksv<W: io::Write> {
    io: W,
}

impl<W: io::Write> Ksv<W> {
    fn new(io: W) -> Self {
        Ksv { io }
    }

    fn into_inner(self) -> W {
        self.io
    }
}

impl<W: io::Write> slog::Serializer for Ksv<W> {
    fn emit_arguments(&mut self, key: slog::Key, val: &fmt::Arguments) -> slog::Result {
        write!(self.io, ", {}: {}", key, val)?;
        Ok(())
    }
}
