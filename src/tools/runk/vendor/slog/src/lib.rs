//! # Slog -  Structured, extensible, composable logging for Rust
//!
//! `slog-rs` is an ecosystem of reusable components for structured, extensible,
//! composable logging for Rust.
//!
//! `slog` is `slog-rs`'s main crate providing core components shared between
//! all other parts of `slog-rs` ecosystem.
//!
//! This is auto-generated technical documentation of `slog`. For information
//! about project organization, development, help, etc. please see
//! [slog github page](https://github.com/slog-rs/slog)
//!
//! ## Core advantages over `log` crate
//!
//! * **extensible** - `slog` crate provides core functionality: a very basic
//!   and portable standard feature-set based on open `trait`s. This allows
//!   implementing new features that can be independently published.
//! * **composable** - `trait`s that `slog` exposes to provide extensibility
//!   are designed to be easy to efficiently reuse and combine. By combining
//!   different functionalities, each application can specify precisely when,
//!   where, and how to process logging data from an application and its
//!   dependencies.
//! * **flexible** - `slog` does not constrain logging to just one globally
//!   registered backend. Parts of your application can handle logging
//!   in a customized way, or completely independently.
//! * **structured** and both **human and machine readable** - By using the
//!   key-value data format and retaining its type information, the meaning of logging
//!   data is preserved.  Data can be serialized to machine readable formats like
//!   JSON and sent to data-mining systems for further analysis, etc. On the
//!   other hand, when presenting on screen, logging information can be presented
//!   in aesthetically pleasing and easy to understand ways.
//! * **contextual** - `slog`'s `Logger` objects carry set of key-value data
//!   pairs that contain the context of logging - information that otherwise
//!   would have to be repeated in every logging statement.
//!
//! ## `slog` features
//!
//! * performance oriented; read [what makes slog
//!   fast](https://github.com/slog-rs/slog/wiki/What-makes-slog-fast) and see:
//!   [slog bench log](https://github.com/dpc/slog-rs/wiki/Bench-log)
//!   * lazily evaluation through closure values
//!   * async IO support included: see [`slog-async`
//!     crate](https://docs.rs/slog-async)
//! * `#![no_std]` support (with opt-out `std` cargo feature flag)
//! * support for named format arguments (e.g. `info!(logger, "printed {line_count} lines", line_count = 2);`)
//!   for easy bridging between the human readable and machine-readable outputs
//! * tree-structured loggers
//! * modular, lightweight and very extensible
//!   * tiny core crate that does not pull any dependencies
//!   * feature-crates for specific functionality
//!   * using `slog` in library does not force users of the library to use slog
//!     (but provides additional functionality); see [example how to use
//!     `slog` in library](https://github.com/slog-rs/example-lib)
//! * backwards and forwards compatibility with `log` crate:
//!   see [`slog-stdlog` crate](https://docs.rs/slog-stdlog)
//! * convenience crates:
//!   * logging-scopes for implicit `Logger` passing: see
//!     [slog-scope crate](https://docs.rs/slog-scope)
//! * many existing core & community provided features:
//!   * multiple outputs
//!   * filtering control
//!       * compile-time log level filter using cargo features (same as in `log`
//!         crate)
//!       * by level, msg, and any other meta-data
//!       * [`slog-envlogger`](https://github.com/slog-rs/envlogger) - port of
//!         `env_logger`
//!       * terminal output, with color support: see [`slog-term`
//!         crate](https://docs.rs/slog-term)
//!  * [json](https://docs.rs/slog-json)
//!      * [bunyan](https://docs.rs/slog-bunyan)
//!  * [syslog](https://docs.rs/slog-syslog)
//!    and [journald](https://docs.rs/slog-journald) support
//!  * run-time configuration:
//!      * run-time behavior change;
//!        see [slog-atomic](https://docs.rs/slog-atomic)
//!      * run-time configuration; see
//!        [slog-config crate](https://docs.rs/slog-config)
//!
//!
//! [env_logger]: https://crates.io/crates/env_logger
//!
//! ## Notable details
//!
//! **Note:** At compile time `slog` by default removes trace and debug level
//! statements in release builds, and trace level records in debug builds. This
//! makes `trace` and `debug` level logging records practically free, which
//! should encourage using them freely. If you want to enable trace/debug
//! messages or raise the compile time logging level limit, use the following in
//! your `Cargo.toml`:
//!
//! ```norust
//! slog = { version = ... ,
//!          features = ["max_level_trace", "release_max_level_warn"] }
//! ```
//!
//! Root drain (passed to `Logger::root`) must be one that does not ever return
//! errors. This forces user to pick error handing strategy.
//! `Drain::fuse()` or `Drain::ignore_res()`.
//!
//! [env_logger]: https://crates.io/crates/env_logger
//! [fn-overv]: https://github.com/dpc/slog-rs/wiki/Functional-overview
//! [atomic-switch]: https://docs.rs/slog-atomic/
//!
//! ## Where to start
//!
//! [`Drain`](trait.Drain.html), [`Logger`](struct.Logger.html) and
//! [`log` macro](macro.log.html) are the most important elements of
//! slog. Make sure to read their respective documentation
//!
//! Typically the biggest problem is creating a `Drain`
//!
//!
//! ### Logging to the terminal
//!
//! ```ignore
//! #[macro_use]
//! extern crate slog;
//! extern crate slog_term;
//! extern crate slog_async;
//!
//! use slog::Drain;
//!
//! fn main() {
//!     let decorator = slog_term::TermDecorator::new().build();
//!     let drain = slog_term::FullFormat::new(decorator).build().fuse();
//!     let drain = slog_async::Async::new(drain).build().fuse();
//!
//!     let _log = slog::Logger::root(drain, o!());
//! }
//! ```
//!
//! ### Logging to a file
//!
//! ```ignore
//! #[macro_use]
//! extern crate slog;
//! extern crate slog_term;
//! extern crate slog_async;
//!
//! use std::fs::OpenOptions;
//! use slog::Drain;
//!
//! fn main() {
//!    let log_path = "target/your_log_file_path.log";
//!    let file = OpenOptions::new()
//!       .create(true)
//!       .write(true)
//!       .truncate(true)
//!       .open(log_path)
//!       .unwrap();
//!
//!     let decorator = slog_term::PlainDecorator::new(file);
//!     let drain = slog_term::FullFormat::new(decorator).build().fuse();
//!     let drain = slog_async::Async::new(drain).build().fuse();
//!
//!     let _log = slog::Logger::root(drain, o!());
//! }
//! ```
//!
//! You can consider using `slog-json` instead of `slog-term`.
//! `slog-term` only coincidently fits the role of a file output format. A
//! proper `slog-file` crate with suitable format, log file rotation and other
//! file-logging related features would be awesome. Contributions are welcome!
//!
//! ### Change logging level at runtime
//!
//! ```ignore
//! #[macro_use]
//! extern crate slog;
//! extern crate slog_term;
//! extern crate slog_async;
//!
//! use slog::Drain;
//!
//! use std::sync::{Arc, atomic};
//! use std::sync::atomic::Ordering;
//! use std::result;
//!
//! /// Custom Drain logic
//! struct RuntimeLevelFilter<D>{
//!    drain: D,
//!    on: Arc<atomic::AtomicBool>,
//! }
//!
//! impl<D> Drain for RuntimeLevelFilter<D>
//!     where D : Drain {
//!     type Ok = Option<D::Ok>;
//!     type Err = Option<D::Err>;
//!
//!     fn log(&self,
//!           record: &slog::Record,
//!           values: &slog::OwnedKVList)
//!           -> result::Result<Self::Ok, Self::Err> {
//!           let current_level = if self.on.load(Ordering::Relaxed) {
//!               slog::Level::Trace
//!           } else {
//!               slog::Level::Info
//!           };
//!
//!           if record.level().is_at_least(current_level) {
//!               self.drain.log(
//!                   record,
//!                   values
//!               )
//!               .map(Some)
//!               .map_err(Some)
//!           } else {
//!               Ok(None)
//!           }
//!       }
//!   }
//!
//! fn main() {
//!     // atomic variable controlling logging level
//!     let on = Arc::new(atomic::AtomicBool::new(false));
//!
//!     let decorator = slog_term::TermDecorator::new().build();
//!     let drain = slog_term::FullFormat::new(decorator).build();
//!     let drain = RuntimeLevelFilter {
//!         drain: drain,
//!         on: on.clone(),
//!     }.fuse();
//!     let drain = slog_async::Async::new(drain).build().fuse();
//!
//!     let _log = slog::Logger::root(drain, o!());
//!
//!     // switch level in your code
//!     on.store(true, Ordering::Relaxed);
//! }
//! ```
//!
//! Why is this not an existing crate? Because there are multiple ways to
//! achieve the same result, and each application might come with its own
//! variation. Supporting a more general solution is a maintenance effort.
//! There is also nothing stopping anyone from publishing their own crate
//! implementing it.
//!
//! Alternative to the above approach is `slog-atomic` crate. It implements
//! swapping whole parts of `Drain` logging hierarchy.
//!
//! ## Examples & help
//!
//! Basic examples that are kept up-to-date are typically stored in
//! respective git repository, under `examples/` subdirectory. Eg.
//! [slog-term examples](https://github.com/slog-rs/term/tree/master/examples).
//!
//! [slog-rs wiki pages](https://github.com/slog-rs/slog/wiki) contain
//! some pages about `slog-rs` technical details.
//!
//! Source code of other [software using
//! slog-rs](https://crates.io/crates/slog/reverse_dependencies) can
//! be an useful reference.
//!
//! Visit [slog-rs gitter channel](https://gitter.im/slog-rs/slog) for immediate
//! help.
//!
//! ## Migrating from slog v1 to slog v2
//!
//! ### Key-value pairs come now after format string
//!
//! ```
//! #[macro_use]
//! extern crate slog;
//!
//! fn main() {
//!     let drain = slog::Discard;
//!     let root = slog::Logger::root(drain, o!());
//!     info!(root, "formatted: {}", 1; "log-key" => true);
//! }
//! ```
//!
//! See more information about format at [`log`](macro.log.html).
//!
//! ### `slog-streamer` is gone
//!
//! Create simple terminal logger like this:
//!
//! ```ignore
//! #[macro_use]
//! extern crate slog;
//! extern crate slog_term;
//! extern crate slog_async;
//!
//! use slog::Drain;
//!
//! fn main() {
//!     let decorator = slog_term::TermDecorator::new().build();
//!     let drain = slog_term::FullFormat::new(decorator).build().fuse();
//!     let drain = slog_async::Async::new(drain).build().fuse();
//!
//!     let _log = slog::Logger::root(drain, o!());
//! }
//! ```
//!
//!
//! ### Logging macros now takes ownership of values.
//!
//! Pass them by reference: `&x`.
//!
// }}}

// {{{ Imports & meta
#![warn(missing_docs)]
#![no_std]

#[cfg(not(feature = "std"))]
extern crate alloc;
#[macro_use]
#[cfg(feature = "std")]
extern crate std;

mod key;
pub use self::key::Key;
#[cfg(not(feature = "std"))]
use alloc::sync::Arc;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
#[cfg(not(feature = "std"))]
use alloc::rc::Rc;
#[cfg(not(feature = "std"))]
use alloc::string::String;

#[cfg(feature = "nested-values")]
extern crate erased_serde;

use core::{convert, fmt, result};
use core::str::FromStr;
#[cfg(feature = "std")]
use std::boxed::Box;
#[cfg(feature = "std")]
use std::panic::{RefUnwindSafe, UnwindSafe};
#[cfg(feature = "std")]
use std::rc::Rc;
#[cfg(feature = "std")]
use std::string::String;
#[cfg(feature = "std")]
use std::sync::Arc;
// }}}

// {{{ Macros
/// Macro for building group of key-value pairs:
/// [`OwnedKV`](struct.OwnedKV.html)
///
/// ```
/// #[macro_use]
/// extern crate slog;
///
/// fn main() {
///     let drain = slog::Discard;
///     let _root = slog::Logger::root(
///         drain,
///         o!("key1" => "value1", "key2" => "value2")
///     );
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! o(
    ($($args:tt)*) => {
        $crate::OwnedKV(kv!($($args)*))
    };
);

/// Macro for building group of key-value pairs (alias)
///
/// Use in case of macro name collisions
#[macro_export(local_inner_macros)]
macro_rules! slog_o(
    ($($args:tt)*) => {
        $crate::OwnedKV(slog_kv!($($args)*))
    };
);

/// Macro for building group of key-value pairs in
/// [`BorrowedKV`](struct.BorrowedKV.html)
///
/// In most circumstances using this macro directly is unnecessary and `info!`
/// and other wrappers over `log!` should be used instead.
#[macro_export(local_inner_macros)]
macro_rules! b(
    ($($args:tt)*) => {
        $crate::BorrowedKV(&kv!($($args)*))
    };
);

/// Alias of `b`
#[macro_export(local_inner_macros)]
macro_rules! slog_b(
    ($($args:tt)*) => {
        $crate::BorrowedKV(&slog_kv!($($args)*))
    };
);

/// Macro for build `KV` implementing type
///
/// You probably want to use `o!` or `b!` instead.
#[macro_export(local_inner_macros)]
macro_rules! kv(
    (@ $args_ready:expr; $k:expr => %$v:expr) => {
        kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{}", $v))), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => %$v:expr, $($args:tt)* ) => {
        kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{}", $v))), $args_ready); $($args)* )
    };
    (@ $args_ready:expr; $k:expr => #%$v:expr) => {
        kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:#}", $v))), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => #%$v:expr, $($args:tt)* ) => {
        kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:#}", $v))), $args_ready); $($args)* )
    };
    (@ $args_ready:expr; $k:expr => ?$v:expr) => {
        kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:?}", $v))), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => ?$v:expr, $($args:tt)* ) => {
        kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:?}", $v))), $args_ready); $($args)* )
    };
    (@ $args_ready:expr; $k:expr => #?$v:expr) => {
        kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:#?}", $v))), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => #?$v:expr, $($args:tt)* ) => {
        kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:#?}", $v))), $args_ready); $($args)* )
    };
    (@ $args_ready:expr; $k:expr => #$v:expr) => {
        kv!(@ ($crate::SingleKV::from(($k, $crate::ErrorValue($v))), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => $v:expr) => {
        kv!(@ ($crate::SingleKV::from(($k, $v)), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => $v:expr, $($args:tt)* ) => {
        kv!(@ ($crate::SingleKV::from(($k, $v)), $args_ready); $($args)* )
    };
    (@ $args_ready:expr; $kv:expr) => {
        kv!(@ ($kv, $args_ready); )
    };
    (@ $args_ready:expr; $kv:expr, $($args:tt)* ) => {
        kv!(@ ($kv, $args_ready); $($args)* )
    };
    (@ $args_ready:expr; ) => {
        $args_ready
    };
    (@ $args_ready:expr;, ) => {
        $args_ready
    };
    ($($args:tt)*) => {
        kv!(@ (); $($args)*)
    };
);

/// Alias of `kv`
#[macro_export(local_inner_macros)]
macro_rules! slog_kv(
    (@ $args_ready:expr; $k:expr => %$v:expr) => {
        slog_kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{}", $v))), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => %$v:expr, $($args:tt)* ) => {
        slog_kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{}", $v))), $args_ready); $($args)* )
    };
    (@ $args_ready:expr; $k:expr => #%$v:expr) => {
        slog_kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:#}", $v))), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => #%$v:expr, $($args:tt)* ) => {
        slog_kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:#}", $v))), $args_ready); $($args)* )
    };
    (@ $args_ready:expr; $k:expr => ?$v:expr) => {
        slog_kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:?}", $v))), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => ?$v:expr, $($args:tt)* ) => {
        slog_kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:?}", $v))), $args_ready); $($args)* )
    };
    (@ $args_ready:expr; $k:expr => #?$v:expr) => {
        slog_kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:#?}", $v))), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => #?$v:expr, $($args:tt)* ) => {
        slog_kv!(@ ($crate::SingleKV::from(($k, __slog_builtin!(@format_args "{:#?}", $v))), $args_ready); $($args)* )
    };
    (@ $args_ready:expr; $k:expr => $v:expr) => {
        slog_kv!(@ ($crate::SingleKV::from(($k, $v)), $args_ready); )
    };
    (@ $args_ready:expr; $k:expr => $v:expr, $($args:tt)* ) => {
        slog_kv!(@ ($crate::SingleKV::from(($k, $v)), $args_ready); $($args)* )
    };
    (@ $args_ready:expr; $slog_kv:expr) => {
        slog_kv!(@ ($slog_kv, $args_ready); )
    };
    (@ $args_ready:expr; $slog_kv:expr, $($args:tt)* ) => {
        slog_kv!(@ ($slog_kv, $args_ready); $($args)* )
    };
    (@ $args_ready:expr; ) => {
        $args_ready
    };
    (@ $args_ready:expr;, ) => {
        $args_ready
    };
    ($($args:tt)*) => {
        slog_kv!(@ (); $($args)*)
    };
);

#[macro_export(local_inner_macros)]
/// Create `RecordStatic` at the given code location
macro_rules! record_static(
    ($lvl:expr, $tag:expr,) => { record_static!($lvl, $tag) };
    ($lvl:expr, $tag:expr) => {{
        static LOC : $crate::RecordLocation = $crate::RecordLocation {
            file: __slog_builtin!(@file),
            line: __slog_builtin!(@line),
            column: __slog_builtin!(@column),
            function: "",
            module: __slog_builtin!(@module_path),
        };
        $crate::RecordStatic {
            location : &LOC,
            level: $lvl,
            tag : $tag,
        }
    }};
);

#[macro_export(local_inner_macros)]
/// Create `RecordStatic` at the given code location (alias)
macro_rules! slog_record_static(
    ($lvl:expr, $tag:expr,) => { slog_record_static!($lvl, $tag) };
    ($lvl:expr, $tag:expr) => {{
        static LOC : $crate::RecordLocation = $crate::RecordLocation {
            file: __slog_builtin!(@file),
            line: __slog_builtin!(@line),
            column: __slog_builtin!(@column),
            function: "",
            module: __slog_builtin!(@module_path),
        };
        $crate::RecordStatic {
            location : &LOC,
            level: $lvl,
            tag: $tag,
        }
    }};
);

#[macro_export(local_inner_macros)]
/// Create `Record` at the given code location
///
/// Note that this requires that `lvl` and `tag` are compile-time constants. If
/// you need them to *not* be compile-time, such as when recreating a `Record`
/// from a serialized version, use `Record::new` instead.
macro_rules! record(
    ($lvl:expr, $tag:expr, $args:expr, $b:expr,) => {
        record!($lvl, $tag, $args, $b)
    };
    ($lvl:expr, $tag:expr, $args:expr, $b:expr) => {{
        #[allow(dead_code)]
        static RS : $crate::RecordStatic<'static> = record_static!($lvl, $tag);
        $crate::Record::new(&RS, $args, $b)
    }};
);

#[macro_export(local_inner_macros)]
/// Create `Record` at the given code location (alias)
macro_rules! slog_record(
    ($lvl:expr, $tag:expr, $args:expr, $b:expr,) => {
        slog_record!($lvl, $tag, $args, $b)
    };
    ($lvl:expr, $tag:expr, $args:expr, $b:expr) => {{
        static RS : $crate::RecordStatic<'static> = slog_record_static!($lvl,
                                                                        $tag);
        $crate::Record::new(&RS, $args, $b)
    }};
);

/// Log message a logging record
///
/// Use wrappers `error!`, `warn!` etc. instead
///
/// The `max_level_*` and `release_max_level*` cargo features can be used to
/// statically disable logging at various levels. See [slog notable
/// details](index.html#notable-details)
///
/// Use [version with longer name](macro.slog_log.html) if you want to prevent
/// clash with legacy `log` crate macro names.
///
/// ## Supported invocations
///
/// ### Simple
///
/// ```
/// #[macro_use]
/// extern crate slog;
///
/// fn main() {
///     let drain = slog::Discard;
///     let root = slog::Logger::root(
///         drain,
///         o!("key1" => "value1", "key2" => "value2")
///     );
///     info!(root, "test info log"; "log-key" => true);
/// }
/// ```
///
/// Note that `"key" => value` part is optional:
///
/// ```
/// #[macro_use]
/// extern crate slog;
///
/// fn main() {
///     let drain = slog::Discard;
///     let root = slog::Logger::root(
///         drain, o!("key1" => "value1", "key2" => "value2")
///     );
///     info!(root, "test info log");
/// }
/// ```
///
/// ### Formatting support:
///
/// ```
/// #[macro_use]
/// extern crate slog;
///
/// fn main() {
///     let drain = slog::Discard;
///     let root = slog::Logger::root(drain,
///         o!("key1" => "value1", "key2" => "value2")
///     );
///     info!(root, "formatted {num_entries} entries of {}", "something", num_entries = 2; "log-key" => true);
/// }
/// ```
///
/// Note:
///
/// * `;` is used to separate message arguments and key value pairs.
/// * message behaves like `format!`/`format_args!`
/// * Named arguments to messages will be added to key-value pairs as well!
///
/// `"key" => value` part is optional:
///
/// ```
/// #[macro_use]
/// extern crate slog;
///
/// fn main() {
///     let drain = slog::Discard;
///     let root = slog::Logger::root(
///         drain, o!("key1" => "value1", "key2" => "value2")
///     );
///     info!(root, "formatted: {}", 1);
/// }
/// ```
///
/// Use formatting support wisely. Prefer named arguments, so the associated
/// data is not "lost" by becoming an untyped string in the message.
///
/// ### Tags
///
/// All above versions can be supplemented with a tag - string literal prefixed
/// with `#`.
///
/// ```
/// #[macro_use]
/// extern crate slog;
///
/// fn main() {
///     let drain = slog::Discard;
///     let root = slog::Logger::root(drain,
///         o!("key1" => "value1", "key2" => "value2")
///     );
///     let ops = 3;
///     info!(
///         root,
///         #"performance-metric", "thread speed"; "ops_per_sec" => ops
///     );
/// }
/// ```
///
/// See `Record::tag()` for more information about tags.
///
/// ### Own implementations of `KV` and `Value`
///
/// List of key value pairs is a comma separated list of key-values. Typically,
/// a designed syntax is used in form of `k => v` where `k` can be any type
/// that implements `Value` type.
///
/// It's possible to directly specify type that implements `KV` trait without
/// `=>` syntax.
///
/// ```
/// #[macro_use]
/// extern crate slog;
///
/// use slog::*;
///
/// fn main() {
///     struct MyKV;
///     struct MyV;
///
///     impl KV for MyKV {
///        fn serialize(&self,
///                     _record: &Record,
///                     serializer: &mut Serializer)
///                    -> Result {
///            serializer.emit_u32("MyK", 16)
///        }
///     }
///
///     impl Value for MyV {
///        fn serialize(&self,
///                     _record: &Record,
///                     key : Key,
///                     serializer: &mut Serializer)
///                    -> Result {
///            serializer.emit_u32("MyKV", 16)
///        }
///     }
///
///     let drain = slog::Discard;
///
///     let root = slog::Logger::root(drain, o!(MyKV));
///
///     info!(
///         root,
///         "testing MyV"; "MyV" => MyV
///     );
/// }
/// ```
///
/// ### `fmt::Display` and `fmt::Debug` values
///
/// Value of any type that implements `std::fmt::Display` can be prefixed with
/// `%` in `k => v` expression to use its text representation returned by
/// `format_args!("{}", v)`. This is especially useful for errors. Not that
/// this does not allocate any `String` since it operates on `fmt::Arguments`.
/// You can also use the `#%` prefix to use the "alternate" form of formatting,
/// represented by the `{:#}` formatting specifier.
///
/// Similarly to use `std::fmt::Debug` value can be prefixed with `?`,
/// or pretty-printed with `#?`.
///
/// ```
/// #[macro_use]
/// extern crate slog;
/// use std::fmt::Write;
///
/// fn main() {
///     let drain = slog::Discard;
///     let log  = slog::Logger::root(drain, o!());
///
///     let mut output = String::new();
///
///     if let Err(e) = write!(&mut output, "write to string") {
///         error!(log, "write failed"; "err" => %e);
///     }
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! log(
    // `2` means that `;` was already found
   (2 @ { $($fmt:tt)* }, { $($kv:tt)* },  $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr) => {
      $crate::Logger::log(&$l, &record!($lvl, $tag, &__slog_builtin!(@format_args $msg_fmt, $($fmt)*), b!($($kv)*)))
   };
   (2 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr,) => {
       log!(2 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt)
   };
   (2 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr;) => {
       log!(2 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt)
   };
   (2 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $($args:tt)*) => {
       log!(2 @ { $($fmt)* }, { $($kv)* $($args)*}, $l, $lvl, $tag, $msg_fmt)
   };
    // `1` means that we are still looking for `;`
    // -- handle named arguments to format string
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $k:ident = $v:expr) => {
       log!(2 @ { $($fmt)* $k = $v }, { $($kv)* __slog_builtin!(@stringify $k) => $v, }, $l, $lvl, $tag, $msg_fmt)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $k:ident = $v:expr;) => {
       log!(2 @ { $($fmt)* $k = $v }, { $($kv)* __slog_builtin!(@stringify $k) => $v, }, $l, $lvl, $tag, $msg_fmt)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $k:ident = $v:expr,) => {
       log!(2 @ { $($fmt)* $k = $v }, { $($kv)* __slog_builtin!(@stringify $k) => $v, }, $l, $lvl, $tag, $msg_fmt)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $k:ident = $v:expr; $($args:tt)*) => {
       log!(2 @ { $($fmt)* $k = $v }, { $($kv)* __slog_builtin!(@stringify $k) => $v, }, $l, $lvl, $tag, $msg_fmt, $($args)*)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $k:ident = $v:expr, $($args:tt)*) => {
       log!(1 @ { $($fmt)* $k = $v, }, { $($kv)* __slog_builtin!(@stringify $k) => $v, }, $l, $lvl, $tag, $msg_fmt, $($args)*)
   };
    // -- look for `;` termination
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr,) => {
       log!(2 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr) => {
       log!(2 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, ; $($args:tt)*) => {
       log!(1 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt; $($args)*)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr; $($args:tt)*) => {
       log!(2 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt, $($args)*)
   };
    // -- must be normal argument to format string
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $f:tt $($args:tt)*) => {
       log!(1 @ { $($fmt)* $f }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt, $($args)*)
   };
   ($l:expr, $lvl:expr, $tag:expr, $($args:tt)*) => {
       if $lvl.as_usize() <= $crate::__slog_static_max_level().as_usize() {
           log!(1 @ { }, { }, $l, $lvl, $tag, $($args)*)
       }
   };
);

/// Log message a logging record (alias)
///
/// Prefer [shorter version](macro.log.html), unless it clashes with
/// existing `log` crate macro.
///
/// See [`log`](macro.log.html) for documentation.
///
/// ```
/// #[macro_use(slog_o,slog_b,slog_record,slog_record_static,slog_log,slog_info,slog_kv,__slog_builtin)]
/// extern crate slog;
///
/// fn main() {
///     let log = slog::Logger::root(slog::Discard, slog_o!());
///
///     slog_info!(log, "some interesting info"; "where" => "right here");
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! slog_log(
    // `2` means that `;` was already found
   (2 @ { $($fmt:tt)* }, { $($kv:tt)* },  $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr) => {
      $crate::Logger::log(&$l, &slog_record!($lvl, $tag, &__slog_builtin!(@format_args $msg_fmt, $($fmt)*), slog_b!($($kv)*)))
   };
   (2 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr,) => {
       slog_log!(2 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt)
   };
   (2 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr;) => {
       slog_log!(2 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt)
   };
   (2 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $($args:tt)*) => {
       slog_log!(2 @ { $($fmt)* }, { $($kv)* $($args)*}, $l, $lvl, $tag, $msg_fmt)
   };
    // `1` means that we are still looking for `;`
    // -- handle named arguments to format string
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $k:ident = $v:expr) => {
       slog_log!(2 @ { $($fmt)* $k = $v }, { $($kv)* __slog_builtin!(@stringify $k) => $v, }, $l, $lvl, $tag, $msg_fmt)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $k:ident = $v:expr;) => {
       slog_log!(2 @ { $($fmt)* $k = $v }, { $($kv)* __slog_builtin!(@stringify $k) => $v, }, $l, $lvl, $tag, $msg_fmt)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $k:ident = $v:expr,) => {
       slog_log!(2 @ { $($fmt)* $k = $v }, { $($kv)* __slog_builtin!(@stringify $k) => $v, }, $l, $lvl, $tag, $msg_fmt)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $k:ident = $v:expr; $($args:tt)*) => {
       slog_log!(2 @ { $($fmt)* $k = $v }, { $($kv)* __slog_builtin!(@stringify $k) => $v, }, $l, $lvl, $tag, $msg_fmt, $($args)*)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $k:ident = $v:expr, $($args:tt)*) => {
       slog_log!(1 @ { $($fmt)* $k = $v, }, { $($kv)* __slog_builtin!(@stringify $k) => $v, }, $l, $lvl, $tag, $msg_fmt, $($args)*)
   };
    // -- look for `;` termination
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr,) => {
       slog_log!(2 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr) => {
       slog_log!(2 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, ; $($args:tt)*) => {
       slog_log!(1 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt; $($args)*)
   };
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr; $($args:tt)*) => {
       slog_log!(2 @ { $($fmt)* }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt, $($args)*)
   };
    // -- must be normal argument to format string
   (1 @ { $($fmt:tt)* }, { $($kv:tt)* }, $l:expr, $lvl:expr, $tag:expr, $msg_fmt:expr, $f:tt $($args:tt)*) => {
       slog_log!(1 @ { $($fmt)* $f }, { $($kv)* }, $l, $lvl, $tag, $msg_fmt, $($args)*)
   };
   ($l:expr, $lvl:expr, $tag:expr, $($args:tt)*) => {
       if $lvl.as_usize() <= $crate::__slog_static_max_level().as_usize() {
           slog_log!(1 @ { }, { }, $l, $lvl, $tag, $($args)*)
       }
   };
);
/// Log critical level record
///
/// See `log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! crit(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        log!($l, $crate::Level::Critical, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        log!($l, $crate::Level::Critical, "", $($args)+)
    };
);

/// Log critical level record (alias)
///
/// Prefer shorter version, unless it clashes with
/// existing `log` crate macro.
///
/// See `slog_log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! slog_crit(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Critical, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Critical, "", $($args)+)
    };
);

/// Log error level record
///
/// See `log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! error(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        log!($l, $crate::Level::Error, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        log!($l, $crate::Level::Error, "", $($args)+)
    };
);

/// Log error level record
///
/// Prefer shorter version, unless it clashes with
/// existing `log` crate macro.
///
/// See `slog_log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! slog_error(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Error, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Error, "", $($args)+)
    };
);

/// Log warning level record
///
/// See `log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! warn(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        log!($l, $crate::Level::Warning, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        log!($l, $crate::Level::Warning, "", $($args)+)
    };
);

/// Log warning level record (alias)
///
/// Prefer shorter version, unless it clashes with
/// existing `log` crate macro.
///
/// See `slog_log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! slog_warn(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Warning, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Warning, "", $($args)+)
    };
);

/// Log info level record
///
/// See `slog_log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! info(
    ($l:expr, #$tag:expr, $($args:tt)*) => {
        log!($l, $crate::Level::Info, $tag, $($args)*)
    };
    ($l:expr, $($args:tt)*) => {
        log!($l, $crate::Level::Info, "", $($args)*)
    };
);

/// Log info level record (alias)
///
/// Prefer shorter version, unless it clashes with
/// existing `log` crate macro.
///
/// See `slog_log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! slog_info(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Info, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Info, "", $($args)+)
    };
);

/// Log debug level record
///
/// See `log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! debug(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        log!($l, $crate::Level::Debug, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        log!($l, $crate::Level::Debug, "", $($args)+)
    };
);

/// Log debug level record (alias)
///
/// Prefer shorter version, unless it clashes with
/// existing `log` crate macro.
///
/// See `slog_log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! slog_debug(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Debug, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Debug, "", $($args)+)
    };
);

/// Log trace level record
///
/// See `log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! trace(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        log!($l, $crate::Level::Trace, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        log!($l, $crate::Level::Trace, "", $($args)+)
    };
);

/// Log trace level record (alias)
///
/// Prefer shorter version, unless it clashes with
/// existing `log` crate macro.
///
/// See `slog_log` for documentation.
#[macro_export(local_inner_macros)]
macro_rules! slog_trace(
    ($l:expr, #$tag:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Trace, $tag, $($args)+)
    };
    ($l:expr, $($args:tt)+) => {
        slog_log!($l, $crate::Level::Trace, "", $($args)+)
    };
    ($($args:tt)+) => {
        slog_log!($crate::Level::Trace, $($args)+)
    };
);

/// Helper macro for using the built-in macros inside of
/// exposed macros with `local_inner_macros` attribute.
#[doc(hidden)]
#[macro_export]
macro_rules! __slog_builtin {
    (@format_args $($t:tt)*) => ( format_args!($($t)*) );
    (@stringify $($t:tt)*) => ( stringify!($($t)*) );
    (@file) => ( file!() );
    (@line) => ( line!() );
    (@column) => ( column!() );
    (@module_path) => ( module_path!() );
}

// }}}

// {{{ Logger
/// Logging handle used to execute logging statements
///
/// In an essence `Logger` instance holds two pieces of information:
///
/// * drain - destination where to forward logging `Record`s for
/// processing.
/// * context - list of key-value pairs associated with it.
///
/// The root `Logger` is created with a `Drain` that will be cloned to every
/// member of its hierarchy.
///
/// Child `Logger`s are built from existing ones, and inherit their key-value
/// pairs, which can be supplemented with additional pairs.
///
/// Cloning existing loggers and creating new ones is cheap. Loggers can be
/// freely passed around the code and between threads.
///
/// `Logger`s are `Sync+Send` - there's no need to synchronize accesses to them,
/// as they can accept logging records from multiple threads at once. They can
/// be sent to any thread. Because of that they require the `Drain` to be
/// `Sync+Send` as well. Not all `Drain`s are `Sync` or `Send` but they can
/// often be made so by wrapping in a `Mutex` and/or `Arc`.
///
/// `Logger` implements `Drain` trait. Any logging `Record` delivered to
/// a `Logger` functioning as a `Drain` will be delivered to its `Drain`
/// with existing key-value pairs appended to the `Logger`'s key-value pairs.
/// By itself, it is effectively very similar to `Logger` being an ancestor
/// of `Logger` that originated the logging `Record`. Combined with other
/// `Drain`s, this allows custom processing logic for a sub-tree of a whole logging
/// tree.
///
/// Logger is parametrized over type of a `Drain` associated with it (`D`). It
/// default to type-erased version so `Logger` without any type annotation
/// means `Logger<Arc<SendSyncRefUnwindSafeDrain<Ok = (), Err = Never>>>`. See
/// `Logger::root_typed` and `Logger::to_erased` for more information.
#[derive(Clone)]
pub struct Logger<D = Arc<SendSyncRefUnwindSafeDrain<Ok = (), Err = Never>>>
where
    D: SendSyncUnwindSafeDrain<Ok = (), Err = Never>,
{
    drain: D,
    list: OwnedKVList,
}

impl<D> Logger<D>
where
    D: SendSyncUnwindSafeDrain<Ok = (), Err = Never>,
{
    /// Build a root `Logger`
    ///
    /// Root logger starts a new tree associated with a given `Drain`. Root
    /// logger drain must return no errors. See `Drain::ignore_res()` and
    /// `Drain::fuse()`.
    ///
    /// All children and their children (and so on), form one logging tree
    /// sharing a common drain. See `Logger::new`.
    ///
    /// This version (as opposed to `Logger:root_typed`) will take `drain` and
    /// made it into `Arc<SendSyncRefUnwindSafeDrain<Ok = (), Err = Never>>`.
    /// This is typically the most convenient way to work with `Logger`s.
    ///
    /// Use `o!` macro to build `OwnedKV` object.
    ///
    /// ```
    /// #[macro_use]
    /// extern crate slog;
    ///
    /// fn main() {
    ///     let _root = slog::Logger::root(
    ///         slog::Discard,
    ///         o!("key1" => "value1", "key2" => "value2"),
    ///     );
    /// }
    /// ```
    pub fn root<T>(drain: D, values: OwnedKV<T>) -> Logger
    where
        D: 'static + SendSyncRefUnwindSafeDrain<Err = Never, Ok = ()>,
        T: SendSyncRefUnwindSafeKV + 'static,
    {
        Logger {
            drain: Arc::new(drain)
                as Arc<SendSyncRefUnwindSafeDrain<Ok = (), Err = Never>>,
            list: OwnedKVList::root(values),
        }
    }

    /// Build a root `Logger` that retains `drain` type
    ///
    /// Unlike `Logger::root`, this constructor retains the type of a `drain`,
    /// which allows highest performance possible by eliminating indirect call
    /// on `Drain::log`, and allowing monomorphization of `Logger` and `Drain`
    /// objects.
    ///
    /// If you don't understand the implications, you should probably just
    /// ignore it.
    ///
    /// See `Logger:into_erased` and `Logger::to_erased` for conversion from
    /// type returned by this function to version that would be returned by
    /// `Logger::root`.
    pub fn root_typed<T>(drain: D, values: OwnedKV<T>) -> Logger<D>
    where
        D: 'static + SendSyncUnwindSafeDrain<Err = Never, Ok = ()> + Sized,
        T: SendSyncRefUnwindSafeKV + 'static,
    {
        Logger {
            drain: drain,
            list: OwnedKVList::root(values),
        }
    }

    /// Build a child logger
    ///
    /// Child logger inherits all existing key-value pairs from its parent and
    /// supplements them with additional ones.
    ///
    /// Use `o!` macro to build `OwnedKV` object.
    ///
    /// ### Drain cloning (`D : Clone` requirement)
    ///
    /// All children, their children and so on, form one tree sharing a
    /// common drain. This drain, will be `Clone`d when this method is called.
    /// That is why `Clone` must be implemented for `D` in `Logger<D>::new`.
    ///
    /// For some `Drain` types `Clone` is cheap or even free (a no-op). This is
    /// the case for any `Logger` returned by `Logger::root` and its children.
    ///
    /// When using `Logger::root_typed`, it's possible that cloning might be
    /// expensive, or even impossible.
    ///
    /// The reason why wrapping in an `Arc` is not done internally, and exposed
    /// to the user is performance. Calling `Drain::log` through an `Arc` is
    /// tiny bit slower than doing it directly.
    ///
    /// ```
    /// #[macro_use]
    /// extern crate slog;
    ///
    /// fn main() {
    ///     let root = slog::Logger::root(slog::Discard,
    ///         o!("key1" => "value1", "key2" => "value2"));
    ///     let _log = root.new(o!("key" => "value"));
    /// }
    #[cfg_attr(feature = "cargo-clippy", allow(wrong_self_convention))]
    pub fn new<T>(&self, values: OwnedKV<T>) -> Logger<D>
    where
        T: SendSyncRefUnwindSafeKV + 'static,
        D: Clone,
    {
        Logger {
            drain: self.drain.clone(),
            list: OwnedKVList::new(values, self.list.node.clone()),
        }
    }

    /// Log one logging `Record`
    ///
    /// Use specific logging functions instead. See `log!` macro
    /// documentation.
    #[inline]
    pub fn log(&self, record: &Record) {
        let _ = self.drain.log(record, &self.list);
    }

    /// Get list of key-value pairs assigned to this `Logger`
    pub fn list(&self) -> &OwnedKVList {
        &self.list
    }

    /// Convert to default, "erased" type:
    /// `Logger<Arc<SendSyncUnwindSafeDrain>>`
    ///
    /// Useful to adapt `Logger<D : Clone>` to an interface expecting
    /// `Logger<Arc<...>>`.
    ///
    /// Note that calling on a `Logger<Arc<...>>` will convert it to
    /// `Logger<Arc<Arc<...>>>` which is not optimal. This might be fixed when
    /// Rust gains trait implementation specialization.
    pub fn into_erased(
        self,
    ) -> Logger<Arc<SendSyncRefUnwindSafeDrain<Ok = (), Err = Never>>>
    where
        D: SendRefUnwindSafeDrain + 'static,
    {
        Logger {
            drain: Arc::new(self.drain)
                as Arc<SendSyncRefUnwindSafeDrain<Ok = (), Err = Never>>,
            list: self.list,
        }
    }

    /// Create a copy with "erased" type
    ///
    /// See `into_erased`
    pub fn to_erased(
        &self,
    ) -> Logger<Arc<SendSyncRefUnwindSafeDrain<Ok = (), Err = Never>>>
    where
        D: SendRefUnwindSafeDrain + 'static + Clone,
    {
        self.clone().into_erased()
    }
}

impl<D> fmt::Debug for Logger<D>
where
    D: SendSyncUnwindSafeDrain<Ok = (), Err = Never>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Logger{:?}", self.list));
        Ok(())
    }
}

impl<D> Drain for Logger<D>
where
    D: SendSyncUnwindSafeDrain<Ok = (), Err = Never>,
{
    type Ok = ();
    type Err = Never;

    fn log(
        &self,
        record: &Record,
        values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        let chained = OwnedKVList {
            node: Arc::new(MultiListNode {
                next_node: values.node.clone(),
                node: self.list.node.clone(),
            }),
        };
        self.drain.log(record, &chained)
    }

    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        self.drain.is_enabled(level)
    }
}

// {{{ Drain
/// Logging drain
///
/// `Drain`s typically mean destination for logs, but `slog` generalizes the
/// term.
///
/// `Drain`s are responsible for handling logging statements (`Record`s) from
/// `Logger`s associated with them: filtering, modifying, formatting
/// and writing the log records into given destination(s).
///
/// It's a typical pattern to parametrize `Drain`s over `Drain` traits to allow
/// composing `Drain`s.
///
/// Implementing this trait allows writing custom `Drain`s. Slog users should
/// not be afraid of implementing their own `Drain`s. Any custom log handling
/// logic should be implemented as a `Drain`.
pub trait Drain {
    /// Type returned by this drain
    ///
    /// It can be useful in some circumstances, but rarely. It will probably
    /// default to `()` once https://github.com/rust-lang/rust/issues/29661 is
    /// stable.
    type Ok;
    /// Type of potential errors that can be returned by this `Drain`
    type Err;
    /// Handle one logging statement (`Record`)
    ///
    /// Every logging `Record` built from a logging statement (eg.
    /// `info!(...)`), and key-value lists of a `Logger` it was executed on
    /// will be passed to the root drain registered during `Logger::root`.
    ///
    /// Typically `Drain`s:
    ///
    /// * pass this information (or not) to the sub-logger(s) (filters)
    /// * format and write the information to a destination (writers)
    /// * deal with the errors returned from the sub-logger(s)
    fn log(
        &self,
        record: &Record,
        values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err>;

    /// **Avoid**: Check if messages at the specified log level are **maybe**
    /// enabled for this logger.
    ///
    /// The purpose of it so to allow **imprecise** detection if a given logging
    /// level has any chance of actually being logged. This might be used
    /// to explicitly skip needless computation.
    ///
    /// **It is best effort, can return false positives, but not false negatives.**
    ///
    /// The logger is still free to ignore records even if the level is enabled,
    /// so an enabled level doesn't necessarily guarantee that the record will
    /// actually be logged.
    ///
    /// This function is somewhat needless, and is better expressed by using
    /// lazy values (see `FnValue`).  A `FnValue` is more precise and does not
    /// require additional (potentially recursive) calls to do something that
    /// `log` will already do anyways (making decision if something should be
    /// logged or not).
    ///
    /// ```
    /// # #[macro_use]
    /// # extern crate slog;
    /// # use slog::*;
    /// # fn main() {
    /// let logger = Logger::root(Discard, o!());
    /// if logger.is_enabled(Level::Debug) {
    ///     let num = 5.0f64;
    ///     let sqrt = num.sqrt();
    ///     debug!(logger, "Sqrt"; "num" => num, "sqrt" => sqrt);
    /// }
    /// # }
    /// ```
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        level.as_usize() <= ::__slog_static_max_level().as_usize()
    }

    /// **Avoid**: See `is_enabled`
    #[inline]
    fn is_critical_enabled(&self) -> bool {
        self.is_enabled(Level::Critical)
    }

    /// **Avoid**: See `is_enabled`
    #[inline]
    fn is_error_enabled(&self) -> bool {
        self.is_enabled(Level::Error)
    }

    /// **Avoid**: See `is_enabled`
    #[inline]
    fn is_warning_enabled(&self) -> bool {
        self.is_enabled(Level::Warning)
    }

    /// **Avoid**: See `is_enabled`
    #[inline]
    fn is_info_enabled(&self) -> bool {
        self.is_enabled(Level::Info)
    }

    /// **Avoid**: See `is_enabled`
    #[inline]
    fn is_debug_enabled(&self) -> bool {
        self.is_enabled(Level::Debug)
    }

    /// **Avoid**: See `is_enabled`
    #[inline]
    fn is_trace_enabled(&self) -> bool {
        self.is_enabled(Level::Trace)
    }

    /// Pass `Drain` through a closure, eg. to wrap
    /// into another `Drain`.
    ///
    /// ```
    /// #[macro_use]
    /// extern crate slog;
    /// use slog::*;
    ///
    /// fn main() {
    ///     let _drain = Discard.map(Fuse);
    /// }
    /// ```
    fn map<F, R>(self, f: F) -> R
    where
        Self: Sized,
        F: FnOnce(Self) -> R,
    {
        f(self)
    }

    /// Filter logging records passed to `Drain`
    ///
    /// Wrap `Self` in `Filter`
    ///
    /// This will convert `self` to a `Drain` that ignores `Record`s
    /// for which `f` returns false.
    fn filter<F>(self, f: F) -> Filter<Self, F>
    where
        Self: Sized,
        F: FilterFn,
    {
        Filter::new(self, f)
    }

    /// Filter logging records passed to `Drain` (by level)
    ///
    /// Wrap `Self` in `LevelFilter`
    ///
    /// This will convert `self` to a `Drain` that ignores `Record`s of
    /// logging lever smaller than `level`.
    fn filter_level(self, level: Level) -> LevelFilter<Self>
    where
        Self: Sized,
    {
        LevelFilter(self, level)
    }

    /// Map logging errors returned by this drain
    ///
    /// `f` is a closure that takes `Drain::Err` returned by a given
    /// drain, and returns new error of potentially different type
    fn map_err<F, E>(self, f: F) -> MapError<Self, E>
    where
        Self: Sized,
        F: MapErrFn<Self::Err, E>,
    {
        MapError::new(self, f)
    }

    /// Ignore results returned by this drain
    ///
    /// Wrap `Self` in `IgnoreResult`
    fn ignore_res(self) -> IgnoreResult<Self>
    where
        Self: Sized,
    {
        IgnoreResult::new(self)
    }

    /// Make `Self` panic when returning any errors
    ///
    /// Wrap `Self` in `Map`
    fn fuse(self) -> Fuse<Self>
    where
        Self::Err: fmt::Debug,
        Self: Sized,
    {
        self.map(Fuse)
    }
}

impl<'a, D: Drain + 'a> Drain for &'a D {
    type Ok = D::Ok;
    type Err = D::Err;
    #[inline]
    fn log(
        &self,
        record: &Record,
        values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        (**self).log(record, values)
    }
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        (**self).is_enabled(level)
    }
}

impl<'a, D: Drain + 'a> Drain for &'a mut D {
    type Ok = D::Ok;
    type Err = D::Err;
    #[inline]
    fn log(
        &self,
        record: &Record,
        values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        (**self).log(record, values)
    }
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        (**self).is_enabled(level)
    }
}

#[cfg(feature = "std")]
/// `Send + Sync + UnwindSafe` bound
///
/// This type is used to enforce `Drain`s associated with `Logger`s
/// are thread-safe.
pub trait SendSyncUnwindSafe: Send + Sync + UnwindSafe {}

#[cfg(feature = "std")]
impl<T> SendSyncUnwindSafe for T
where
    T: Send + Sync + UnwindSafe + ?Sized,
{
}

#[cfg(feature = "std")]
/// `Drain + Send + Sync + UnwindSafe` bound
///
/// This type is used to enforce `Drain`s associated with `Logger`s
/// are thread-safe.
pub trait SendSyncUnwindSafeDrain: Drain + Send + Sync + UnwindSafe {}

#[cfg(feature = "std")]
impl<T> SendSyncUnwindSafeDrain for T
where
    T: Drain + Send + Sync + UnwindSafe + ?Sized,
{
}

#[cfg(feature = "std")]
/// `Drain + Send + Sync + RefUnwindSafe` bound
///
/// This type is used to enforce `Drain`s associated with `Logger`s
/// are thread-safe.
pub trait SendSyncRefUnwindSafeDrain: Drain + Send + Sync + RefUnwindSafe {}

#[cfg(feature = "std")]
impl<T> SendSyncRefUnwindSafeDrain for T
where
    T: Drain + Send + Sync + RefUnwindSafe + ?Sized,
{
}

#[cfg(feature = "std")]
/// Function that can be used in `MapErr` drain
pub trait MapErrFn<EI, EO>
    : 'static + Sync + Send + UnwindSafe + RefUnwindSafe + Fn(EI) -> EO {
}

#[cfg(feature = "std")]
impl<T, EI, EO> MapErrFn<EI, EO> for T
where
    T: 'static
        + Sync
        + Send
        + ?Sized
        + UnwindSafe
        + RefUnwindSafe
        + Fn(EI) -> EO,
{
}

#[cfg(feature = "std")]
/// Function that can be used in `Filter` drain
pub trait FilterFn
    : 'static + Sync + Send + UnwindSafe + RefUnwindSafe + Fn(&Record) -> bool {
}

#[cfg(feature = "std")]
impl<T> FilterFn for T
where
    T: 'static
        + Sync
        + Send
        + ?Sized
        + UnwindSafe
        + RefUnwindSafe
        + Fn(&Record) -> bool,
{
}

#[cfg(not(feature = "std"))]
/// `Drain + Send + Sync + UnwindSafe` bound
///
/// This type is used to enforce `Drain`s associated with `Logger`s
/// are thread-safe.
pub trait SendSyncUnwindSafeDrain: Drain + Send + Sync {}

#[cfg(not(feature = "std"))]
impl<T> SendSyncUnwindSafeDrain for T
where
    T: Drain + Send + Sync + ?Sized,
{
}

#[cfg(not(feature = "std"))]
/// `Drain + Send + Sync + RefUnwindSafe` bound
///
/// This type is used to enforce `Drain`s associated with `Logger`s
/// are thread-safe.
pub trait SendSyncRefUnwindSafeDrain: Drain + Send + Sync {}

#[cfg(not(feature = "std"))]
impl<T> SendSyncRefUnwindSafeDrain for T
where
    T: Drain + Send + Sync + ?Sized,
{
}

#[cfg(feature = "std")]
/// `Drain + Send + RefUnwindSafe` bound
pub trait SendRefUnwindSafeDrain: Drain + Send + RefUnwindSafe {}

#[cfg(feature = "std")]
impl<T> SendRefUnwindSafeDrain for T
where
    T: Drain + Send + RefUnwindSafe + ?Sized,
{
}

#[cfg(not(feature = "std"))]
/// `Drain + Send + RefUnwindSafe` bound
pub trait SendRefUnwindSafeDrain: Drain + Send {}

#[cfg(not(feature = "std"))]
impl<T> SendRefUnwindSafeDrain for T
where
    T: Drain + Send + ?Sized,
{
}

#[cfg(not(feature = "std"))]
/// Function that can be used in `MapErr` drain
pub trait MapErrFn<EI, EO>: 'static + Sync + Send + Fn(EI) -> EO {}

#[cfg(not(feature = "std"))]
impl<T, EI, EO> MapErrFn<EI, EO> for T
where
    T: 'static + Sync + Send + ?Sized + Fn(EI) -> EO,
{
}

#[cfg(not(feature = "std"))]
/// Function that can be used in `Filter` drain
pub trait FilterFn: 'static + Sync + Send + Fn(&Record) -> bool {}

#[cfg(not(feature = "std"))]
impl<T> FilterFn for T
where
    T: 'static + Sync + Send + ?Sized + Fn(&Record) -> bool,
{
}

impl<D: Drain + ?Sized> Drain for Box<D> {
    type Ok = D::Ok;
    type Err = D::Err;
    fn log(
        &self,
        record: &Record,
        o: &OwnedKVList,
    ) -> result::Result<Self::Ok, D::Err> {
        (**self).log(record, o)
    }
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        (**self).is_enabled(level)
    }
}

impl<D: Drain + ?Sized> Drain for Arc<D> {
    type Ok = D::Ok;
    type Err = D::Err;
    fn log(
        &self,
        record: &Record,
        o: &OwnedKVList,
    ) -> result::Result<Self::Ok, D::Err> {
        (**self).log(record, o)
    }
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        (**self).is_enabled(level)
    }
}

/// `Drain` discarding everything
///
/// `/dev/null` of `Drain`s
#[derive(Debug, Copy, Clone)]
pub struct Discard;

impl Drain for Discard {
    type Ok = ();
    type Err = Never;
    fn log(&self, _: &Record, _: &OwnedKVList) -> result::Result<(), Never> {
        Ok(())
    }
    #[inline]
    fn is_enabled(&self, _1: Level) -> bool {
        false
    }
}

/// `Drain` filtering records
///
/// Wraps another `Drain` and passes `Record`s to it, only if they satisfy a
/// given condition.
#[derive(Debug, Clone)]
pub struct Filter<D: Drain, F>(pub D, pub F)
where
    F: Fn(&Record) -> bool + 'static + Send + Sync;

impl<D: Drain, F> Filter<D, F>
where
    F: FilterFn,
{
    /// Create `Filter` wrapping given `drain`
    pub fn new(drain: D, cond: F) -> Self {
        Filter(drain, cond)
    }
}

impl<D: Drain, F> Drain for Filter<D, F>
where
    F: FilterFn,
{
    type Ok = Option<D::Ok>;
    type Err = D::Err;
    fn log(
        &self,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        if (self.1)(record) {
            Ok(Some(self.0.log(record, logger_values)?))
        } else {
            Ok(None)
        }
    }
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        /*
         * This is one of the reasons we can't guarantee the value is actually logged.
         * The filter function is given dynamic control over whether or not the record is logged
         * and could filter stuff out even if the log level is supposed to be enabled
         */
        self.0.is_enabled(level)
    }
}

/// `Drain` filtering records by `Record` logging level
///
/// Wraps a drain and passes records to it, only
/// if their level is at least given level.
///
/// TODO: Remove this type. This drain is a special case of `Filter`, but
/// because `Filter` can not use static dispatch ATM due to Rust limitations
/// that will be lifted in the future, it is a standalone type.
/// Reference: https://github.com/rust-lang/rust/issues/34511
#[derive(Debug, Clone)]
pub struct LevelFilter<D: Drain>(pub D, pub Level);

impl<D: Drain> LevelFilter<D> {
    /// Create `LevelFilter`
    pub fn new(drain: D, level: Level) -> Self {
        LevelFilter(drain, level)
    }
}

impl<D: Drain> Drain for LevelFilter<D> {
    type Ok = Option<D::Ok>;
    type Err = D::Err;
    fn log(
        &self,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        if record.level().is_at_least(self.1) {
            Ok(Some(self.0.log(record, logger_values)?))
        } else {
            Ok(None)
        }
    }
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        level.is_at_least(self.1) && self.0.is_enabled(level)
    }
}

/// `Drain` mapping error returned by another `Drain`
///
/// See `Drain::map_err` for convenience function.
pub struct MapError<D: Drain, E> {
    drain: D,
    // eliminated dynamic dispatch, after rust learns `-> impl Trait`
    map_fn: Box<MapErrFn<D::Err, E, Output = E>>,
}

impl<D: Drain, E> MapError<D, E> {
    /// Create `Filter` wrapping given `drain`
    pub fn new<F>(drain: D, map_fn: F) -> Self
    where
        F: MapErrFn<<D as Drain>::Err, E>,
    {
        MapError {
            drain: drain,
            map_fn: Box::new(map_fn),
        }
    }
}

impl<D: Drain, E> Drain for MapError<D, E> {
    type Ok = D::Ok;
    type Err = E;
    fn log(
        &self,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        self.drain
            .log(record, logger_values)
            .map_err(|e| (self.map_fn)(e))
    }
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        self.drain.is_enabled(level)
    }
}

/// `Drain` duplicating records into two other `Drain`s
///
/// Can be nested for more than two outputs.
#[derive(Debug, Clone)]
pub struct Duplicate<D1: Drain, D2: Drain>(pub D1, pub D2);

impl<D1: Drain, D2: Drain> Duplicate<D1, D2> {
    /// Create `Duplicate`
    pub fn new(drain1: D1, drain2: D2) -> Self {
        Duplicate(drain1, drain2)
    }
}

impl<D1: Drain, D2: Drain> Drain for Duplicate<D1, D2> {
    type Ok = (D1::Ok, D2::Ok);
    type Err = (
        result::Result<D1::Ok, D1::Err>,
        result::Result<D2::Ok, D2::Err>,
    );
    fn log(
        &self,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        let res1 = self.0.log(record, logger_values);
        let res2 = self.1.log(record, logger_values);

        match (res1, res2) {
            (Ok(o1), Ok(o2)) => Ok((o1, o2)),
            (r1, r2) => Err((r1, r2)),
        }
    }
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        self.0.is_enabled(level) || self.1.is_enabled(level)
    }
}

/// `Drain` panicking on error
///
/// `Logger` requires a root drain to handle all errors (`Drain::Error == ()`),
/// `Fuse` will wrap a `Drain` and panic if it returns any errors.
///
/// Note: `Drain::Err` must implement `Display` (for displaying on panic). It's
/// easy to create your own `Fuse` drain if this requirement can't be fulfilled.
#[derive(Debug, Clone)]
pub struct Fuse<D: Drain>(pub D)
where
    D::Err: fmt::Debug;

impl<D: Drain> Fuse<D>
where
    D::Err: fmt::Debug,
{
    /// Create `Fuse` wrapping given `drain`
    pub fn new(drain: D) -> Self {
        Fuse(drain)
    }
}

impl<D: Drain> Drain for Fuse<D>
where
    D::Err: fmt::Debug,
{
    type Ok = ();
    type Err = Never;
    fn log(
        &self,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Never> {
        let _ = self.0
            .log(record, logger_values)
            .unwrap_or_else(|e| panic!("slog::Fuse Drain: {:?}", e));
        Ok(())
    }
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        self.0.is_enabled(level)
    }
}

/// `Drain` ignoring result
///
/// `Logger` requires a root drain to handle all errors (`Drain::Err=()`), and
/// returns nothing (`Drain::Ok=()`) `IgnoreResult` will ignore any result
/// returned by the `Drain` it wraps.
#[derive(Clone)]
pub struct IgnoreResult<D: Drain> {
    drain: D,
}

impl<D: Drain> IgnoreResult<D> {
    /// Create `IgnoreResult` wrapping `drain`
    pub fn new(drain: D) -> Self {
        IgnoreResult { drain: drain }
    }
}

impl<D: Drain> Drain for IgnoreResult<D> {
    type Ok = ();
    type Err = Never;
    fn log(
        &self,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> result::Result<(), Never> {
        let _ = self.drain.log(record, logger_values);
        Ok(())
    }

    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        self.drain.is_enabled(level)
    }
}

/// Error returned by `Mutex<D : Drain>`
#[cfg(feature = "std")]
#[derive(Clone)]
pub enum MutexDrainError<D: Drain> {
    /// Error acquiring mutex
    Mutex,
    /// Error returned by drain
    Drain(D::Err),
}

#[cfg(feature = "std")]
impl<D> fmt::Debug for MutexDrainError<D>
where
    D: Drain,
    D::Err: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            MutexDrainError::Mutex => write!(f, "MutexDrainError::Mutex"),
            MutexDrainError::Drain(ref e) => e.fmt(f),
        }
    }
}

#[cfg(feature = "std")]
impl<D> std::error::Error for MutexDrainError<D>
where
    D: Drain,
    D::Err: fmt::Debug + fmt::Display + std::error::Error,
{
    fn description(&self) -> &str {
        match *self {
            MutexDrainError::Mutex => "Mutex acquire failed",
            MutexDrainError::Drain(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&std::error::Error> {
        match *self {
            MutexDrainError::Mutex => None,
            MutexDrainError::Drain(ref e) => Some(e),
        }
    }
}

#[cfg(feature = "std")]
impl<'a, D: Drain> From<std::sync::PoisonError<std::sync::MutexGuard<'a, D>>>
    for MutexDrainError<D> {
    fn from(
        _: std::sync::PoisonError<std::sync::MutexGuard<'a, D>>,
    ) -> MutexDrainError<D> {
        MutexDrainError::Mutex
    }
}

#[cfg(feature = "std")]
impl<D: Drain> fmt::Display for MutexDrainError<D>
where
    D::Err: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            MutexDrainError::Mutex => write!(f, "MutexError"),
            MutexDrainError::Drain(ref e) => write!(f, "{}", e),
        }
    }
}

#[cfg(feature = "std")]
impl<D: Drain> Drain for std::sync::Mutex<D> {
    type Ok = D::Ok;
    type Err = MutexDrainError<D>;
    fn log(
        &self,
        record: &Record,
        logger_values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        let d = self.lock()?;
        d.log(record, logger_values).map_err(MutexDrainError::Drain)
    }
    #[inline]
    fn is_enabled(&self, level: Level) -> bool {
        self.lock().ok().map_or(true, |lock| lock.is_enabled(level))
    }
}
// }}}

// {{{ Level & FilterLevel
/// Official capitalized logging (and logging filtering) level names
///
/// In order of `as_usize()`.
pub static LOG_LEVEL_NAMES: [&'static str; 7] =
    ["OFF", "CRITICAL", "ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

/// Official capitalized logging (and logging filtering) short level names
///
/// In order of `as_usize()`.
pub static LOG_LEVEL_SHORT_NAMES: [&'static str; 7] =
    ["OFF", "CRIT", "ERRO", "WARN", "INFO", "DEBG", "TRCE"];

/// Logging level associated with a logging `Record`
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Level {
    /// Critical
    Critical,
    /// Error
    Error,
    /// Warning
    Warning,
    /// Info
    Info,
    /// Debug
    Debug,
    /// Trace
    Trace,
}

/// Logging filtering level
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum FilterLevel {
    /// Log nothing
    Off,
    /// Log critical level only
    Critical,
    /// Log only error level and above
    Error,
    /// Log only warning level and above
    Warning,
    /// Log only info level and above
    Info,
    /// Log only debug level and above
    Debug,
    /// Log everything
    Trace,
}

impl Level {
    /// Convert to `str` from `LOG_LEVEL_SHORT_NAMES`
    pub fn as_short_str(&self) -> &'static str {
        LOG_LEVEL_SHORT_NAMES[self.as_usize()]
    }

    /// Convert to `str` from `LOG_LEVEL_NAMES`
    pub fn as_str(&self) -> &'static str {
        LOG_LEVEL_NAMES[self.as_usize()]
    }

    /// Cast `Level` to ordering integer
    ///
    /// `Critical` is the smallest and `Trace` the biggest value
    #[inline]
    pub fn as_usize(&self) -> usize {
        match *self {
            Level::Critical => 1,
            Level::Error => 2,
            Level::Warning => 3,
            Level::Info => 4,
            Level::Debug => 5,
            Level::Trace => 6,
        }
    }

    /// Get a `Level` from an `usize`
    ///
    /// This complements `as_usize`
    #[inline]
    pub fn from_usize(u: usize) -> Option<Level> {
        match u {
            1 => Some(Level::Critical),
            2 => Some(Level::Error),
            3 => Some(Level::Warning),
            4 => Some(Level::Info),
            5 => Some(Level::Debug),
            6 => Some(Level::Trace),
            _ => None,
        }
    }
}

impl FilterLevel {
    /// Convert to `str` from `LOG_LEVEL_SHORT_NAMES`
    pub fn as_short_str(&self) -> &'static str {
        LOG_LEVEL_SHORT_NAMES[self.as_usize()]
    }

    /// Convert to `str` from `LOG_LEVEL_NAMES`
    pub fn as_str(&self) -> &'static str {
        LOG_LEVEL_NAMES[self.as_usize()]
    }

    /// Convert to `usize` value
    ///
    /// `Off` is 0, and `Trace` 6
    #[inline]
    pub fn as_usize(&self) -> usize {
        match *self {
            FilterLevel::Off => 0,
            FilterLevel::Critical => 1,
            FilterLevel::Error => 2,
            FilterLevel::Warning => 3,
            FilterLevel::Info => 4,
            FilterLevel::Debug => 5,
            FilterLevel::Trace => 6,
        }
    }

    /// Get a `FilterLevel` from an `usize`
    ///
    /// This complements `as_usize`
    #[inline]
    pub fn from_usize(u: usize) -> Option<FilterLevel> {
        match u {
            0 => Some(FilterLevel::Off),
            1 => Some(FilterLevel::Critical),
            2 => Some(FilterLevel::Error),
            3 => Some(FilterLevel::Warning),
            4 => Some(FilterLevel::Info),
            5 => Some(FilterLevel::Debug),
            6 => Some(FilterLevel::Trace),
            _ => None,
        }
    }

    /// Maximum logging level (log everything)
    #[inline]
    pub fn max() -> Self {
        FilterLevel::Trace
    }

    /// Minimum logging level (log nothing)
    #[inline]
    pub fn min() -> Self {
        FilterLevel::Off
    }

    /// Check if message with given level should be logged
    pub fn accepts(self, level: Level) -> bool {
        self.as_usize() >= level.as_usize()
    }
}

impl FromStr for Level {
    type Err = ();
    fn from_str(name: &str) -> core::result::Result<Level, ()> {
        index_of_log_level_name(name)
            .and_then(|idx| Level::from_usize(idx))
            .ok_or(())
    }
}

impl FromStr for FilterLevel {
    type Err = ();
    fn from_str(name: &str) -> core::result::Result<FilterLevel, ()> {
        index_of_log_level_name(name)
            .and_then(|idx| FilterLevel::from_usize(idx))
            .ok_or(())
    }
}

fn index_of_log_level_name(name: &str) -> Option<usize> {
    index_of_str_ignore_case(&LOG_LEVEL_NAMES, name)
        .or_else(|| index_of_str_ignore_case(&LOG_LEVEL_SHORT_NAMES, name))
}

fn index_of_str_ignore_case(haystack: &[&str], needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack.iter()
        // This will never panic because haystack has only ASCII characters
        .map(|hay| &hay[..needle.len().min(hay.len())])
        .position(|hay| hay.eq_ignore_ascii_case(needle))
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_short_str())
    }
}

impl fmt::Display for FilterLevel {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_short_str())
    }
}

impl Level {
    /// Returns true if `self` is at least `level` logging level
    #[inline]
    pub fn is_at_least(&self, level: Self) -> bool {
        self.as_usize() <= level.as_usize()
    }
}

#[test]
fn level_at_least() {
    assert!(Level::Debug.is_at_least(Level::Debug));
    assert!(Level::Debug.is_at_least(Level::Trace));
    assert!(!Level::Debug.is_at_least(Level::Info));
}

#[test]
fn filter_level_sanity() {
    assert!(Level::Critical.as_usize() > FilterLevel::Off.as_usize());
    assert!(Level::Critical.as_usize() == FilterLevel::Critical.as_usize());
    assert!(Level::Trace.as_usize() == FilterLevel::Trace.as_usize());
}

#[test]
fn level_from_str() {
    refute_from_str::<Level>("off");
    assert_from_str(Level::Critical, "critical");
    assert_from_str(Level::Critical, "crit");
    assert_from_str(Level::Error, "error");
    assert_from_str(Level::Error, "erro");
    assert_from_str(Level::Warning, "warn");
    assert_from_str(Level::Info, "info");
    assert_from_str(Level::Debug, "debug");
    assert_from_str(Level::Debug, "debg");
    assert_from_str(Level::Trace, "trace");
    assert_from_str(Level::Trace, "trce");

    assert_from_str(Level::Info, "Info");
    assert_from_str(Level::Info, "INFO");
    assert_from_str(Level::Info, "iNfO");

    refute_from_str::<Level>("");
    assert_from_str(Level::Info, "i");
    assert_from_str(Level::Info, "in");
    assert_from_str(Level::Info, "inf");
    refute_from_str::<Level>("infor");

    refute_from_str::<Level>("?");
    refute_from_str::<Level>("info ");
    refute_from_str::<Level>(" info");
    refute_from_str::<Level>("desinfo");
}

#[test]
fn filter_level_from_str() {
    assert_from_str(FilterLevel::Off, "off");
    assert_from_str(FilterLevel::Critical, "critical");
    assert_from_str(FilterLevel::Critical, "crit");
    assert_from_str(FilterLevel::Error, "error");
    assert_from_str(FilterLevel::Error, "erro");
    assert_from_str(FilterLevel::Warning, "warn");
    assert_from_str(FilterLevel::Info, "info");
    assert_from_str(FilterLevel::Debug, "debug");
    assert_from_str(FilterLevel::Debug, "debg");
    assert_from_str(FilterLevel::Trace, "trace");
    assert_from_str(FilterLevel::Trace, "trce");

    assert_from_str(FilterLevel::Info, "Info");
    assert_from_str(FilterLevel::Info, "INFO");
    assert_from_str(FilterLevel::Info, "iNfO");

    refute_from_str::<FilterLevel>("");
    assert_from_str(FilterLevel::Info, "i");
    assert_from_str(FilterLevel::Info, "in");
    assert_from_str(FilterLevel::Info, "inf");
    refute_from_str::<FilterLevel>("infor");

    refute_from_str::<FilterLevel>("?");
    refute_from_str::<FilterLevel>("info ");
    refute_from_str::<FilterLevel>(" info");
    refute_from_str::<FilterLevel>("desinfo");
}

#[cfg(test)]
fn assert_from_str<T>(expected: T, level_str: &str)
    where
        T: FromStr + fmt::Debug + PartialEq,
        T::Err: fmt::Debug {
    let result = T::from_str(level_str);

    let actual = result.unwrap_or_else(|e| {
        panic!("Failed to parse filter level '{}': {:?}", level_str, e)
    });
    assert_eq!(expected, actual, "Invalid filter level parsed from '{}'", level_str);
}

#[cfg(test)]
fn refute_from_str<T>(level_str: &str)
    where
        T: FromStr + fmt::Debug {
    let result = T::from_str(level_str);

    if let Ok(level) = result {
        panic!("Parsing filter level '{}' succeeded: {:?}", level_str, level)
    }
}

#[cfg(feature = "std")]
#[test]
fn level_to_string_and_from_str_are_compatible() {
    assert_to_string_from_str(Level::Critical);
    assert_to_string_from_str(Level::Error);
    assert_to_string_from_str(Level::Warning);
    assert_to_string_from_str(Level::Info);
    assert_to_string_from_str(Level::Debug);
    assert_to_string_from_str(Level::Trace);
}

#[cfg(feature = "std")]
#[test]
fn filter_level_to_string_and_from_str_are_compatible() {
    assert_to_string_from_str(FilterLevel::Off);
    assert_to_string_from_str(FilterLevel::Critical);
    assert_to_string_from_str(FilterLevel::Error);
    assert_to_string_from_str(FilterLevel::Warning);
    assert_to_string_from_str(FilterLevel::Info);
    assert_to_string_from_str(FilterLevel::Debug);
    assert_to_string_from_str(FilterLevel::Trace);
}

#[cfg(all(test, feature = "std"))]
fn assert_to_string_from_str<T>(expected: T)
    where
        T: std::string::ToString + FromStr + PartialEq + fmt::Debug,
        <T as FromStr>::Err: fmt::Debug {
    let string = expected.to_string();

    let actual = T::from_str(&string)
        .expect(&format!("Failed to parse string representation of {:?}", expected));

    assert_eq!(expected, actual, "Invalid value parsed from string representation of {:?}", actual);
}

#[test]
fn filter_level_accepts_tests() {
    assert_eq!(true, FilterLevel::Warning.accepts(Level::Error));
    assert_eq!(true, FilterLevel::Warning.accepts(Level::Warning));
    assert_eq!(false, FilterLevel::Warning.accepts(Level::Info));
    assert_eq!(false, FilterLevel::Off.accepts(Level::Critical));
}
// }}}

// {{{ Record
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct RecordLocation {
    /// File
    pub file: &'static str,
    /// Line
    pub line: u32,
    /// Column (currently not implemented)
    pub column: u32,
    /// Function (currently not implemented)
    pub function: &'static str,
    /// Module
    pub module: &'static str,
}
/// Information that can be static in the given record thus allowing to optimize
/// record creation to be done mostly at compile-time.
///
/// This should be constructed via the `record_static!` macro.
pub struct RecordStatic<'a> {
    /// Code location
    #[doc(hidden)]
    pub location: &'a RecordLocation,
    /// Tag
    #[doc(hidden)]
    pub tag: &'a str,
    /// Logging level
    #[doc(hidden)]
    pub level: Level,
}

/// One logging record
///
/// Corresponds to one logging statement like `info!(...)` and carries all its
/// data: eg. message, immediate key-value pairs and key-value pairs of `Logger`
/// used to execute it.
///
/// Record is passed to a `Logger`, which delivers it to its own `Drain`,
/// where actual logging processing is implemented.
pub struct Record<'a> {
    rstatic: &'a RecordStatic<'a>,
    msg: &'a fmt::Arguments<'a>,
    kv: BorrowedKV<'a>,
}

impl<'a> Record<'a> {
    /// Create a new `Record`
    ///
    /// Most of the time, it is slightly more performant to construct a `Record`
    /// via the `record!` macro because it enforces that the *entire*
    /// `RecordStatic` is built at compile-time.
    ///
    /// Use this if runtime record creation is a requirement, as is the case with
    /// [slog-async](https://docs.rs/slog-async/latest/slog_async/struct.Async.html),
    /// for example.
    #[inline]
    pub fn new(
        s: &'a RecordStatic<'a>,
        msg: &'a fmt::Arguments<'a>,
        kv: BorrowedKV<'a>,
    ) -> Self {
        Record {
            rstatic: s,
            msg: msg,
            kv: kv,
        }
    }

    /// Get a log record message
    pub fn msg(&self) -> &fmt::Arguments {
        self.msg
    }

    /// Get record logging level
    pub fn level(&self) -> Level {
        self.rstatic.level
    }

    /// Get line number
    pub fn line(&self) -> u32 {
        self.rstatic.location.line
    }

    /// Get line number
    pub fn location(&self) -> &RecordLocation {
        self.rstatic.location
    }

    /// Get error column
    pub fn column(&self) -> u32 {
        self.rstatic.location.column
    }

    /// Get file path
    pub fn file(&self) -> &'static str {
        self.rstatic.location.file
    }

    /// Get tag
    ///
    /// Tag is information that can be attached to `Record` that is not meant
    /// to be part of the normal key-value pairs, but only as an ad-hoc control
    /// flag for quick lookup in the `Drain`s. As such should be used carefully
    /// and mostly in application code (as opposed to libraries) - where tag
    /// meaning across the system can be coordinated. When used in libraries,
    /// make sure to prefix it with something reasonably distinct, like create
    /// name.
    pub fn tag(&self) -> &str {
        self.rstatic.tag
    }

    /// Get module
    pub fn module(&self) -> &'static str {
        self.rstatic.location.module
    }

    /// Get function (placeholder)
    ///
    /// There's currently no way to obtain that information
    /// in Rust at compile time, so it is not implemented.
    ///
    /// It will be implemented at first opportunity, and
    /// it will not be considered a breaking change.
    pub fn function(&self) -> &'static str {
        self.rstatic.location.function
    }

    /// Get key-value pairs
    pub fn kv(&self) -> BorrowedKV {
        BorrowedKV(self.kv.0)
    }
}
// }}}

// {{{ Serializer

#[cfg(macro_workaround)]
macro_rules! impl_default_as_fmt{
    (#[$m:meta] $($t:tt)+) => {
        #[$m]
        impl_default_as_fmt!($($t)*);
    };
    ($t:ty => $f:ident) => {
        #[allow(missing_docs)]
        fn $f(&mut self, key : Key, val : $t)
            -> Result {
                self.emit_arguments(key, &format_args!("{}", val))
            }
    };
}

#[cfg(not(macro_workaround))]
macro_rules! impl_default_as_fmt{
    ($(#[$m:meta])* $t:ty => $f:ident) => {
        $(#[$m])*
        fn $f(&mut self, key : Key, val : $t)
            -> Result {
                self.emit_arguments(key, &format_args!("{}", val))
            }
    };
}

/// This is a workaround to be able to pass &mut Serializer, from
/// `Serializer::emit_serde` default implementation. `&Self` can't be casted to
/// `&Serializer` (without : Sized, which break object safety), but it can be
/// used as <T: Serializer>.
#[cfg(feature = "nested-values")]
struct SerializerForward<'a, T: 'a + ?Sized>(&'a mut T);

#[cfg(feature = "nested-values")]
impl<'a, T: Serializer + 'a + ?Sized> Serializer for SerializerForward<'a, T> {
    fn emit_arguments(&mut self, key: Key, val: &fmt::Arguments) -> Result {
        self.0.emit_arguments(key, val)
    }

    #[cfg(feature = "nested-values")]
    fn emit_serde(&mut self, _key: Key, _value: &SerdeValue) -> Result {
        panic!();
    }
}

/// Serializer
///
/// Drains using `Format` will internally use
/// types implementing this trait.
pub trait Serializer {
    impl_default_as_fmt! {
        /// Emit `usize`
        usize => emit_usize
    }
    impl_default_as_fmt! {
        /// Emit `isize`
        isize => emit_isize
    }
    impl_default_as_fmt! {
        /// Emit `bool`
        bool => emit_bool
    }
    impl_default_as_fmt! {
        /// Emit `char`
        char => emit_char
    }
    impl_default_as_fmt! {
        /// Emit `u8`
        u8 => emit_u8
    }
    impl_default_as_fmt! {
        /// Emit `i8`
        i8 => emit_i8
    }
    impl_default_as_fmt! {
        /// Emit `u16`
        u16 => emit_u16
    }
    impl_default_as_fmt! {
        /// Emit `i16`
        i16 => emit_i16
    }
    impl_default_as_fmt! {
        /// Emit `u32`
        u32 => emit_u32
    }
    impl_default_as_fmt! {
        /// Emit `i32`
        i32 => emit_i32
    }
    impl_default_as_fmt! {
        /// Emit `f32`
        f32 => emit_f32
    }
    impl_default_as_fmt! {
        /// Emit `u64`
        u64 => emit_u64
    }
    impl_default_as_fmt! {
        /// Emit `i64`
        i64 => emit_i64
    }
    impl_default_as_fmt! {
        /// Emit `f64`
        f64 => emit_f64
    }
    impl_default_as_fmt! {
        /// Emit `u128`
        #[cfg(integer128)]
        u128 => emit_u128
    }
    impl_default_as_fmt! {
        /// Emit `i128`
        #[cfg(integer128)]
        i128 => emit_i128
    }
    impl_default_as_fmt! {
        /// Emit `&str`
        &str => emit_str
    }

    /// Emit `()`
    fn emit_unit(&mut self, key: Key) -> Result {
        self.emit_arguments(key, &format_args!("()"))
    }

    /// Emit `None`
    fn emit_none(&mut self, key: Key) -> Result {
        self.emit_arguments(key, &format_args!(""))
    }

    /// Emit `fmt::Arguments`
    ///
    /// This is the only method that has to implemented, but for performance and
    /// to retain type information most serious `Serializer`s will want to
    /// implement all other methods as well.
    fn emit_arguments(&mut self, key: Key, val: &fmt::Arguments) -> Result;

    /// Emit a value implementing
    /// [`serde::Serialize`](https://docs.rs/serde/1/serde/trait.Serialize.html)
    ///
    /// This is especially useful for composite values, eg. structs as Json values, or sequences.
    ///
    /// To prevent pulling-in `serde` dependency, this is an extension behind a
    /// `serde` feature flag.
    ///
    /// The value needs to implement `SerdeValue`.
    #[cfg(feature = "nested-values")]
    fn emit_serde(&mut self, key: Key, value: &SerdeValue) -> Result {
        value.serialize_fallback(key, &mut SerializerForward(self))
    }

    /// Emit a type implementing `std::error::Error`
    ///
    /// Error values are a bit special as their `Display` implementation doesn't show full
    /// information about the type but must be retrieved using `source()`. This can be used
    /// for formatting sources of errors differntly.
    ///
    /// The default implementation of this method formats the sources separated with `: `.
    /// Serializers are encouraged to take advantage of the type information and format it as
    /// appropriate.
    ///
    /// This method is only available in `std` because the `Error` trait is not available
    /// without `std`.
    #[cfg(feature = "std")]
    fn emit_error(&mut self, key: Key, error: &(std::error::Error + 'static)) -> Result {
        self.emit_arguments(key, &format_args!("{}", ErrorAsFmt(error)))
    }
}

/// Serializer to closure adapter.
///
/// Formats all arguments as `fmt::Arguments` and passes them to a given closure.
struct AsFmtSerializer<F>(pub F)
where
    F: for<'a> FnMut(Key, fmt::Arguments<'a>) -> Result;

impl<F> Serializer for AsFmtSerializer<F>
where
    F: for<'a> FnMut(Key, fmt::Arguments<'a>) -> Result,
{
    fn emit_arguments(&mut self, key: Key, val: &fmt::Arguments) -> Result {
        (self.0)(key, *val)
    }
}

/// A helper for formatting std::error::Error types by joining sources with `: `
///
/// This avoids allocation in the default implementation of `Serializer::emit_error()`.
/// This is only enabled with `std` as the trait is only available there.
#[cfg(feature = "std")]
struct ErrorAsFmt<'a>(pub &'a (std::error::Error + 'static));

#[cfg(feature = "std")]
impl<'a> fmt::Display for ErrorAsFmt<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // For backwards compatibility
        // This is fine because we don't need downcasting
        #![allow(deprecated)]
        write!(f, "{}", self.0)?;
        let mut error = self.0.cause();
        while let Some(source) = error {
            write!(f, ": {}", source)?;
            error = source.cause();
        }
        Ok(())
    }
}

// }}}

// {{{ serde
/// A value that can be serialized via serde
///
/// This is useful for implementing nested values, like sequences or structures.
#[cfg(feature = "nested-values")]
pub trait SerdeValue: erased_serde::Serialize + Value {
    /// Serialize the value in a way that is compatible with `slog::Serializer`s
    /// that do not support serde.
    ///
    /// The implementation should *not* call `slog::Serialize::serialize`
    /// on itself, as it will lead to infinite recursion.
    ///
    /// Default implementation is provided, but it returns error, so use it
    /// only for internal types in systems and libraries where `serde` is always
    /// enabled.
    fn serialize_fallback(
        &self,
        _key: Key,
        _serializer: &mut Serializer,
    ) -> Result<()> {
        Err(Error::Other)
    }

    /// Convert to `erased_serialize::Serialize` of the underlying value,
    /// so `slog::Serializer`s can use it to serialize via `serde`.
    fn as_serde(&self) -> &erased_serde::Serialize;

    /// Convert to a boxed value that can be sent across threads
    ///
    /// This enables functionality like `slog-async` and similar.
    fn to_sendable(&self) -> Box<SerdeValue + Send + 'static>;
}

// }}}

// {{{ Value
/// Value that can be serialized
///
/// Types that implement this type implement custom serialization in the
/// structured part of the log macros. Without an implementation of `Value` for
/// your type you must emit using either the `?` "debug", `#?` "pretty-debug",
/// `%` "display", `#%` "alternate display" or [`SerdeValue`](trait.SerdeValue.html)
/// (if you have the `nested-values` feature enabled) formatters.
///
/// # Example
///
/// ```
/// use slog::{Key, Value, Record, Result, Serializer};
/// struct MyNewType(i64);
///
/// impl Value for MyNewType {
///     fn serialize(&self, _rec: &Record, key: Key, serializer: &mut Serializer) -> Result {
///         serializer.emit_i64(key, self.0)
///     }
/// }
/// ```
///
/// See also [`KV`](trait.KV.html) for formatting both the key and value.
pub trait Value {
    /// Serialize self into `Serializer`
    ///
    /// Structs implementing this trait should generally
    /// only call respective methods of `serializer`.
    fn serialize(
        &self,
        record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result;
}

impl<'a, V> Value for &'a V
where
    V: Value + ?Sized,
{
    fn serialize(
        &self,
        record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        (*self).serialize(record, key, serializer)
    }
}

macro_rules! impl_value_for{
    ($t:ty, $f:ident) => {
        impl Value for $t {
            fn serialize(&self,
                         _record : &Record,
                         key : Key,
                         serializer : &mut Serializer
                         ) -> Result {
                serializer.$f(key, *self)
            }
        }
    };
}

impl_value_for!(usize, emit_usize);
impl_value_for!(isize, emit_isize);
impl_value_for!(bool, emit_bool);
impl_value_for!(char, emit_char);
impl_value_for!(u8, emit_u8);
impl_value_for!(i8, emit_i8);
impl_value_for!(u16, emit_u16);
impl_value_for!(i16, emit_i16);
impl_value_for!(u32, emit_u32);
impl_value_for!(i32, emit_i32);
impl_value_for!(f32, emit_f32);
impl_value_for!(u64, emit_u64);
impl_value_for!(i64, emit_i64);
impl_value_for!(f64, emit_f64);
#[cfg(integer128)]
impl_value_for!(u128, emit_u128);
#[cfg(integer128)]
impl_value_for!(i128, emit_i128);

impl Value for () {
    fn serialize(
        &self,
        _record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        serializer.emit_unit(key)
    }
}

impl Value for str {
    fn serialize(
        &self,
        _record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        serializer.emit_str(key, self)
    }
}

impl<'a> Value for fmt::Arguments<'a> {
    fn serialize(
        &self,
        _record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        serializer.emit_arguments(key, self)
    }
}

impl Value for String {
    fn serialize(
        &self,
        _record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        serializer.emit_str(key, self.as_str())
    }
}

impl<T: Value> Value for Option<T> {
    fn serialize(
        &self,
        record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        match *self {
            Some(ref s) => s.serialize(record, key, serializer),
            None => serializer.emit_none(key),
        }
    }
}

impl<T> Value for Box<T>
where
    T: Value + ?Sized,
{
    fn serialize(
        &self,
        record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        (**self).serialize(record, key, serializer)
    }
}
impl<T> Value for Arc<T>
where
    T: Value + ?Sized,
{
    fn serialize(
        &self,
        record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        (**self).serialize(record, key, serializer)
    }
}

impl<T> Value for Rc<T>
where
    T: Value,
{
    fn serialize(
        &self,
        record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        (**self).serialize(record, key, serializer)
    }
}

impl<T> Value for core::num::Wrapping<T>
where
    T: Value,
{
    fn serialize(
        &self,
        record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        self.0.serialize(record, key, serializer)
    }
}

#[cfg(feature = "std")]
impl<'a> Value for std::path::Display<'a> {
    fn serialize(
        &self,
        _record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        serializer.emit_arguments(key, &format_args!("{}", *self))
    }
}

#[cfg(feature = "std")]
impl Value for std::net::SocketAddr {
    fn serialize(
        &self,
        _record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

#[cfg(feature = "std")]
impl Value for std::io::Error {
    fn serialize(
        &self,
        _record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        serializer.emit_error(key, self)
    }
}

/// Explicit lazy-closure `Value`
pub struct FnValue<V: Value, F>(pub F)
where
    F: for<'c, 'd> Fn(&'c Record<'d>) -> V;

impl<'a, V: 'a + Value, F> Value for FnValue<V, F>
where
    F: 'a + for<'c, 'd> Fn(&'c Record<'d>) -> V,
{
    fn serialize(
        &self,
        record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        (self.0)(record).serialize(record, key, serializer)
    }
}

#[deprecated(note = "Renamed to `PushFnValueSerializer`")]
/// Old name of `PushFnValueSerializer`
pub type PushFnSerializer<'a> = PushFnValueSerializer<'a>;

/// Handle passed to `PushFnValue` closure
///
/// It makes sure only one value is serialized, and will automatically emit
/// `()` if nothing else was serialized.
pub struct PushFnValueSerializer<'a> {
    record: &'a Record<'a>,
    key: Key,
    serializer: &'a mut Serializer,
    done: bool,
}

impl<'a> PushFnValueSerializer<'a> {
    #[deprecated(note = "Renamed to `emit`")]
    /// Emit a value
    pub fn serialize<'b, S: 'b + Value>(self, s: S) -> Result {
        self.emit(s)
    }

    /// Emit a value
    ///
    /// This consumes `self` to prevent serializing one value multiple times
    pub fn emit<'b, S: 'b + Value>(mut self, s: S) -> Result {
        self.done = true;
        s.serialize(self.record, self.key.clone(), self.serializer)
    }
}

impl<'a> Drop for PushFnValueSerializer<'a> {
    fn drop(&mut self) {
        if !self.done {
            // unfortunately this gives no change to return serialization errors
            let _ = self.serializer.emit_unit(self.key.clone());
        }
    }
}

/// Lazy `Value` that writes to Serializer
///
/// It's more ergonomic for closures used as lazy values to return type
/// implementing `Serialize`, but sometimes that forces an allocation (eg.
/// `String`s)
///
/// In some cases it might make sense for another closure form to be used - one
/// taking a serializer as an argument, which avoids lifetimes / allocation
/// issues.
///
/// Generally this method should be used if it avoids a big allocation of
/// `Serialize`-implementing type in performance-critical logging statement.
///
/// ```
/// #[macro_use]
/// extern crate slog;
/// use slog::{PushFnValue, Logger, Discard};
///
/// fn main() {
///     // Create a logger with a key-value printing
///     // `file:line` string value for every logging statement.
///     // `Discard` `Drain` used for brevity.
///     let root = Logger::root(Discard, o!(
///         "source_location" => PushFnValue(|record , s| {
///              s.serialize(
///                   format_args!(
///                        "{}:{}",
///                        record.file(),
///                        record.line(),
///                   )
///              )
///         })
///     ));
/// }
/// ```
pub struct PushFnValue<F>(pub F)
where
    F: 'static
        + for<'c, 'd> Fn(&'c Record<'d>, PushFnValueSerializer<'c>) -> Result;

impl<F> Value for PushFnValue<F>
where
    F: 'static
        + for<'c, 'd> Fn(&'c Record<'d>, PushFnValueSerializer<'c>) -> Result,
{
    fn serialize(
        &self,
        record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        let ser = PushFnValueSerializer {
            record: record,
            key: key,
            serializer: serializer,
            done: false,
        };
        (self.0)(record, ser)
    }
}

/// A wrapper struct for serializing errors
///
/// This struct can be used to wrap types that don't implement `slog::Value` but
/// do implement `std::error::Error` so that they can be logged.
/// This is usually not used directly but using `#error` in the macros.
///
/// This struct is only available in `std` because the `Error` trait is not available
/// without `std`.
#[cfg(feature = "std")]
pub struct ErrorValue<E: std::error::Error>(pub E);

#[cfg(feature = "std")]
impl<E> Value for ErrorValue<E>
where
    E: 'static + std::error::Error,
{
    fn serialize(
        &self,
        _record: &Record,
        key: Key,
        serializer: &mut Serializer,
    ) -> Result {
        serializer.emit_error(key, &self.0)
    }
}

// }}}

// {{{ KV
/// Key-value pair(s) for log events
///
/// Zero, one or more key value pairs chained together
///
/// Any logging data must implement this trait for slog to be able to use it,
/// although slog comes with default implementations within its macros (the
/// `=>` and `kv!` portions of the log macros).
///
/// If you don't use this trait, you must emit your structured data by
/// specifying both key and value in each log event:
///
/// ```ignore
/// info!(logger, "my event"; "type_key" => %my_val);
/// ```
///
/// If you implement this trait, that can become:
///
/// ```ignore
/// info!(logger, "my event"; my_val);
/// ```
///
/// Types implementing this trait can emit multiple key-value pairs, and can
/// customize their structured representation. The order of emitting them
/// should be consistent with the way key-value pair hierarchy is traversed:
/// from data most specific to the logging context to the most general one. Or
/// in other words: from newest to oldest.
///
/// Implementers are are responsible for calling the `emit_*` methods on the
/// `Serializer` passed in, the `Record` can be used to make display decisions
/// based on context, but for most plain-value structs you will just call
/// `emit_*`.
///
/// # Example
///
/// ```
/// use slog::{KV, Record, Result, Serializer};
///
/// struct MyNewType(i64);
///
/// impl KV for MyNewType {
///    fn serialize(&self, _rec: &Record, serializer: &mut Serializer) -> Result {
///        serializer.emit_i64("my_new_type", self.0)
///    }
/// }
/// ```
///
/// See also [`Value`](trait.Value.html), which allows you to customize just
/// the right hand side of the `=>` structure macro, and (if you have the
/// `nested-values` feature enabled) [`SerdeValue`](trait.SerdeValue.html)
/// which allows emitting anything serde can emit.
pub trait KV {
    /// Serialize self into `Serializer`
    ///
    /// `KV` should call respective `Serializer` methods
    /// for each key-value pair it contains.
    fn serialize(&self, record: &Record, serializer: &mut Serializer)
        -> Result;
}

impl<'a, T> KV for &'a T
where
    T: KV,
{
    fn serialize(
        &self,
        record: &Record,
        serializer: &mut Serializer,
    ) -> Result {
        (**self).serialize(record, serializer)
    }
}

#[cfg(feature = "nothreads")]
/// Thread-local safety bound for `KV`
///
/// This type is used to enforce `KV`s stored in `Logger`s are thread-safe.
pub trait SendSyncRefUnwindSafeKV: KV {}

#[cfg(feature = "nothreads")]
impl<T> SendSyncRefUnwindSafeKV for T where T: KV + ?Sized {}

#[cfg(all(not(feature = "nothreads"), feature = "std"))]
/// This type is used to enforce `KV`s stored in `Logger`s are thread-safe.
pub trait SendSyncRefUnwindSafeKV: KV + Send + Sync + RefUnwindSafe {}

#[cfg(all(not(feature = "nothreads"), feature = "std"))]
impl<T> SendSyncRefUnwindSafeKV for T where T: KV + Send + Sync + RefUnwindSafe + ?Sized {}

#[cfg(all(not(feature = "nothreads"), not(feature = "std")))]
/// This type is used to enforce `KV`s stored in `Logger`s are thread-safe.
pub trait SendSyncRefUnwindSafeKV: KV + Send + Sync {}

#[cfg(all(not(feature = "nothreads"), not(feature = "std")))]
impl<T> SendSyncRefUnwindSafeKV for T where T: KV + Send + Sync + ?Sized {}

/// Single pair `Key` and `Value`
pub struct SingleKV<V>(pub Key, pub V)
where
    V: Value;

#[cfg(feature = "dynamic-keys")]
impl<V: Value> From<(String, V)> for SingleKV<V> {
    fn from(x: (String, V)) -> SingleKV<V> {
        SingleKV(Key::from(x.0), x.1)
    }
}
#[cfg(feature = "dynamic-keys")]
impl<V: Value> From<(&'static str, V)> for SingleKV<V> {
    fn from(x: (&'static str, V)) -> SingleKV<V> {
        SingleKV(Key::from(x.0), x.1)
    }
}
#[cfg(not(feature = "dynamic-keys"))]
impl<V: Value> From<(&'static str, V)> for SingleKV<V> {
    fn from(x: (&'static str, V)) -> SingleKV<V> {
        SingleKV(x.0, x.1)
    }
}

impl<V> KV for SingleKV<V>
where
    V: Value,
{
    fn serialize(
        &self,
        record: &Record,
        serializer: &mut Serializer,
    ) -> Result {
        self.1.serialize(record, self.0.clone(), serializer)
    }
}

impl KV for () {
    fn serialize(
        &self,
        _record: &Record,
        _serializer: &mut Serializer,
    ) -> Result {
        Ok(())
    }
}

impl<T: KV, R: KV> KV for (T, R) {
    fn serialize(
        &self,
        record: &Record,
        serializer: &mut Serializer,
    ) -> Result {
        try!(self.0.serialize(record, serializer));
        self.1.serialize(record, serializer)
    }
}

impl<T> KV for Box<T>
where
    T: KV + ?Sized,
{
    fn serialize(
        &self,
        record: &Record,
        serializer: &mut Serializer,
    ) -> Result {
        (**self).serialize(record, serializer)
    }
}

impl<T> KV for Arc<T>
where
    T: KV + ?Sized,
{
    fn serialize(
        &self,
        record: &Record,
        serializer: &mut Serializer,
    ) -> Result {
        (**self).serialize(record, serializer)
    }
}

impl<T> KV for OwnedKV<T>
where
    T: SendSyncRefUnwindSafeKV + ?Sized,
{
    fn serialize(
        &self,
        record: &Record,
        serializer: &mut Serializer,
    ) -> Result {
        self.0.serialize(record, serializer)
    }
}

impl<'a> KV for BorrowedKV<'a> {
    fn serialize(
        &self,
        record: &Record,
        serializer: &mut Serializer,
    ) -> Result {
        self.0.serialize(record, serializer)
    }
}
// }}}

// {{{ OwnedKV
/// Owned KV
///
/// "Owned" means that the contained data (key-value pairs) can belong
/// to a `Logger` and thus must be thread-safe (`'static`, `Send`, `Sync`)
///
/// Zero, one or more owned key-value pairs.
///
/// Can be constructed with [`o!` macro](macro.o.html).
pub struct OwnedKV<T>(
    #[doc(hidden)]
    /// The exact details of that it are not considered public
    /// and stable API. `slog_o` or `o` macro should be used
    /// instead to create `OwnedKV` instances.
    pub T,
)
where
    T: SendSyncRefUnwindSafeKV + ?Sized;
// }}}

// {{{ BorrowedKV
/// Borrowed `KV`
///
/// "Borrowed" means that the data is only a temporary
/// referenced (`&T`) and can't be stored directly.
///
/// Zero, one or more borrowed key-value pairs.
///
/// Can be constructed with [`b!` macro](macro.b.html).
pub struct BorrowedKV<'a>(
    /// The exact details of it function are not
    /// considered public and stable API. `log` and other
    /// macros should be used instead to create
    /// `BorrowedKV` instances.
    #[doc(hidden)]
    pub &'a KV,
);

// }}}

// {{{ OwnedKVList
struct OwnedKVListNode<T>
where
    T: SendSyncRefUnwindSafeKV + 'static,
{
    next_node: Arc<SendSyncRefUnwindSafeKV + 'static>,
    kv: T,
}

struct MultiListNode {
    next_node: Arc<SendSyncRefUnwindSafeKV + 'static>,
    node: Arc<SendSyncRefUnwindSafeKV + 'static>,
}

/// Chain of `SyncMultiSerialize`-s of a `Logger` and its ancestors
#[derive(Clone)]
pub struct OwnedKVList {
    node: Arc<SendSyncRefUnwindSafeKV + 'static>,
}

impl<T> KV for OwnedKVListNode<T>
where
    T: SendSyncRefUnwindSafeKV + 'static,
{
    fn serialize(
        &self,
        record: &Record,
        serializer: &mut Serializer,
    ) -> Result {
        try!(self.kv.serialize(record, serializer));
        try!(self.next_node.serialize(record, serializer));

        Ok(())
    }
}

impl KV for MultiListNode {
    fn serialize(
        &self,
        record: &Record,
        serializer: &mut Serializer,
    ) -> Result {
        try!(self.next_node.serialize(record, serializer));
        try!(self.node.serialize(record, serializer));

        Ok(())
    }
}

impl KV for OwnedKVList {
    fn serialize(
        &self,
        record: &Record,
        serializer: &mut Serializer,
    ) -> Result {
        try!(self.node.serialize(record, serializer));

        Ok(())
    }
}

impl fmt::Debug for OwnedKVList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "("));
        let mut i = 0;

        {
            let mut as_str_ser = AsFmtSerializer(|key, _val| {
                if i != 0 {
                    try!(write!(f, ", "));
                }

                try!(write!(f, "{}", key));
                i += 1;
                Ok(())
            });
            let record_static = record_static!(Level::Trace, "");

            try!(
                self.node
                    .serialize(
                        &Record::new(
                            &record_static,
                            &format_args!(""),
                            BorrowedKV(&STATIC_TERMINATOR_UNIT)
                        ),
                        &mut as_str_ser
                    )
                    .map_err(|_| fmt::Error)
            );
        }

        try!(write!(f, ")"));
        Ok(())
    }
}

impl OwnedKVList {
    /// New `OwnedKVList` node without a parent (root)
    fn root<T>(values: OwnedKV<T>) -> Self
    where
        T: SendSyncRefUnwindSafeKV + 'static,
    {
        OwnedKVList {
            node: Arc::new(OwnedKVListNode {
                next_node: Arc::new(()),
                kv: values.0,
            }),
        }
    }

    /// New `OwnedKVList` node with an existing parent
    fn new<T>(
        values: OwnedKV<T>,
        next_node: Arc<SendSyncRefUnwindSafeKV + 'static>,
    ) -> Self
    where
        T: SendSyncRefUnwindSafeKV + 'static,
    {
        OwnedKVList {
            node: Arc::new(OwnedKVListNode {
                next_node: next_node,
                kv: values.0,
            }),
        }
    }
}

impl<T> convert::From<OwnedKV<T>> for OwnedKVList
where
    T: SendSyncRefUnwindSafeKV + 'static,
{
    fn from(from: OwnedKV<T>) -> Self {
        OwnedKVList::root(from)
    }
}
// }}}

// {{{ Error
#[derive(Debug)]
#[cfg(feature = "std")]
/// Serialization Error
pub enum Error {
    /// `io::Error` (not available in ![no_std] mode)
    Io(std::io::Error),
    /// `fmt::Error`
    Fmt(std::fmt::Error),
    /// Other error
    Other,
}

#[derive(Debug)]
#[cfg(not(feature = "std"))]
/// Serialization Error
pub enum Error {
    /// `fmt::Error`
    Fmt(core::fmt::Error),
    /// Other error
    Other,
}

/// Serialization `Result`
pub type Result<T = ()> = result::Result<T, Error>;

#[cfg(feature = "std")]
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<core::fmt::Error> for Error {
    fn from(_: core::fmt::Error) -> Error {
        Error::Other
    }
}

#[cfg(feature = "std")]
impl From<Error> for std::io::Error {
    fn from(e: Error) -> std::io::Error {
        match e {
            Error::Io(e) => e,
            Error::Fmt(_) => std::io::Error::new(
                std::io::ErrorKind::Other,
                "formatting error",
            ),
            Error::Other => {
                std::io::Error::new(std::io::ErrorKind::Other, "other error")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Io(ref e) => e.description(),
            Error::Fmt(_) => "formatting error",
            Error::Other => "serialization error",
        }
    }

    fn cause(&self) -> Option<&std::error::Error> {
        match *self {
            Error::Io(ref e) => Some(e),
            Error::Fmt(ref e) => Some(e),
            Error::Other => None,
        }
    }
}

#[cfg(feature = "std")]
impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Error::Io(ref e) => e.fmt(fmt),
            Error::Fmt(ref e) => e.fmt(fmt),
            Error::Other => fmt.write_str("Other serialization error"),
        }
    }
}
// }}}

// {{{ Misc
/// This type is here just to abstract away lack of `!` type support in stable
/// rust during time of the release. It will be switched to `!` at some point
/// and `Never` should not be considered "stable" API.
#[doc(hidden)]
pub type Never = private::NeverStruct;

mod private {
    #[doc(hidden)]
    #[derive(Clone, Debug)]
    pub struct NeverStruct(());
}

/// This is not part of "stable" API
#[doc(hidden)]
pub static STATIC_TERMINATOR_UNIT: () = ();

#[allow(unknown_lints)]
#[allow(inline_always)]
#[inline(always)]
#[doc(hidden)]
/// Not an API
///
/// Generally it's a bad idea to depend on static logging level
/// in your code. Use closures to perform operations lazily
/// only when logging actually takes place.
pub fn __slog_static_max_level() -> FilterLevel {
    if !cfg!(debug_assertions) {
        if cfg!(feature = "release_max_level_off") {
            return FilterLevel::Off;
        } else if cfg!(feature = "release_max_level_error") {
            return FilterLevel::Error;
        } else if cfg!(feature = "release_max_level_warn") {
            return FilterLevel::Warning;
        } else if cfg!(feature = "release_max_level_info") {
            return FilterLevel::Info;
        } else if cfg!(feature = "release_max_level_debug") {
            return FilterLevel::Debug;
        } else if cfg!(feature = "release_max_level_trace") {
            return FilterLevel::Trace;
        }
    }
    if cfg!(feature = "max_level_off") {
        FilterLevel::Off
    } else if cfg!(feature = "max_level_error") {
        FilterLevel::Error
    } else if cfg!(feature = "max_level_warn") {
        FilterLevel::Warning
    } else if cfg!(feature = "max_level_info") {
        FilterLevel::Info
    } else if cfg!(feature = "max_level_debug") {
        FilterLevel::Debug
    } else if cfg!(feature = "max_level_trace") {
        FilterLevel::Trace
    } else {
        if !cfg!(debug_assertions) {
            FilterLevel::Info
        } else {
            FilterLevel::Debug
        }
    }
}

// }}}

// {{{ Slog v1 Compat
#[deprecated(note = "Renamed to `Value`")]
/// Compatibility name to ease upgrading from `slog v1`
pub type Serialize = Value;

#[deprecated(note = "Renamed to `PushFnValue`")]
/// Compatibility name to ease upgrading from `slog v1`
pub type PushLazy<T> = PushFnValue<T>;

#[deprecated(note = "Renamed to `PushFnValueSerializer`")]
/// Compatibility name to ease upgrading from `slog v1`
pub type ValueSerializer<'a> = PushFnValueSerializer<'a>;

#[deprecated(note = "Renamed to `OwnedKVList`")]
/// Compatibility name to ease upgrading from `slog v1`
pub type OwnedKeyValueList = OwnedKVList;

#[deprecated(note = "Content of ser module moved to main namespace")]
/// Compatibility name to ease upgrading from `slog v1`
pub mod ser {
    #[allow(deprecated)]
    pub use super::{OwnedKeyValueList, PushLazy, Serialize, Serializer,
                    ValueSerializer};
}
// }}}

// {{{ Test
#[cfg(test)]
mod tests;

// }}}

// vim: foldmethod=marker foldmarker={{{,}}}
