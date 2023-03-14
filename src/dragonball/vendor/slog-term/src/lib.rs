// {{{ Module docs
//! `slog-rs`'s `Drain` for terminal output
//!
//! This crate implements output formatting targeting logging to
//! terminal/console/shell or similar text-based IO.
//!
//! **Warning**: `slog-term` (like `slog-rs` itself) is fast, modular and
//! extensible.  It comes with a price: a lot of details (*that you don't care
//! about
//! right now and think they are stupid, until you actually do and then you are
//! happy that someone thought of them for you*) are being taken into
//! consideration. Anyway, **if you just want to get a logging to terminal
//! working with `slog`**, consider using a wrapper crate like
//! [sloggers](https://docs.rs/sloggers/) instead.
//!
//! **Note**: A lot of users gets bitten by the fact that
//! `slog::Logger::root(...)` requires a drain that is
//! safe to send and share across threads (`Send+Sync`). With shared resource
//! like terminal or a file to which you log, a synchronization needs to be
//! taken care of. If you get compilation errors around `Sync` or `Send` you
//! are doing something wrong around it.
//!
//! Using `Decorator` open trait, user can implement outputting
//! using different colors, terminal types and so on.
//!
//! # Synchronization via `PlainSyncDecorator`
//!
//! This logger works by synchronizing on the IO directly in
//! `PlainSyncDecorator`.  The formatting itself is thread-safe.
//!
//! ```
//! use slog::*;
//!
//! let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
//! let logger = Logger::root(
//!     slog_term::FullFormat::new(plain)
//!     .build().fuse(), o!()
//! );
//!
//! info!(logger, "Logging ready!");
//! ```
//!
//! # Synchronization via `slog_async`
//!
//! This drain puts logging into a separate thread via `slog_async::Async`:
//! formatting and writing to terminal is happening in a one dedicated thread,
//! so no further synchronization is required.
//!
//! ```
//! use slog::{Drain, o, info};
//!
//! let decorator = slog_term::TermDecorator::new().build();
//! let drain = slog_term::CompactFormat::new(decorator).build().fuse();
//! let drain = slog_async::Async::new(drain).build().fuse();
//!
//! let log = slog::Logger::root(drain, o!());
//!
//! info!(log, "Logging ready!");
//! ```
//!
//! # Synchronization via `Mutex`
//!
//! This drain synchronizes by wrapping everything in a big mutex (yes,
//! `Mutex<Drain>` implements a `Drain` trait). This is kind of slow, but in
//! scripting languages like Ruby or Python pretty much the whole code is
//! running in a one
//! huge mutex and noone seems to mind, so I'm sure you're going to get away
//! with this. Personally, I am a bit sad, that I've spent so much effort to
//! give you tools to make your code as efficient as possible, and you choose
//! this. ಠ_ಠ . But I'm here to serve, not to tell you what to do.
//!
//! ```
//! use slog::{Drain, o, info};
//!
//! let decorator = slog_term::TermDecorator::new().build();
//! let drain = slog_term::CompactFormat::new(decorator).build();
//! let drain = std::sync::Mutex::new(drain).fuse();
//!
//! let log = slog::Logger::root(drain, o!());
//!
//! info!(log, "Logging ready!");
//! ```
// }}}

// {{{ Imports & meta
#![warn(missing_docs)]

use slog::Drain;
use slog::Key;
use slog::*;
use std::cell::RefCell;
use std::io::Write as IoWrite;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::result;
use std::{fmt, io, mem, sync};
// }}}

// {{{ Decorator
/// Output decorator
///
/// Trait implementing strategy of output formating in terms of IO,
/// colors, etc.
pub trait Decorator {
    /// Get a `RecordDecorator` for a given `record`
    ///
    /// This allows `Decorator` to have on-stack data per processed `Record`s
    ///
    fn with_record<F>(
        &self,
        _record: &Record,
        _logger_values: &OwnedKVList,
        f: F,
    ) -> io::Result<()>
    where
        F: FnOnce(&mut dyn RecordDecorator) -> io::Result<()>;
}

impl<T: ?Sized> Decorator for Box<T>
where
    T: Decorator,
{
    fn with_record<F>(
        &self,
        record: &Record,
        logger_kv: &OwnedKVList,
        f: F,
    ) -> io::Result<()>
    where
        F: FnOnce(&mut dyn RecordDecorator) -> io::Result<()>,
    {
        (**self).with_record(record, logger_kv, f)
    }
}

/// Per-record decorator
pub trait RecordDecorator: io::Write {
    /// Reset formatting to defaults
    fn reset(&mut self) -> io::Result<()>;

    /// Format normal text
    fn start_whitespace(&mut self) -> io::Result<()> {
        self.reset()
    }

    /// Format `Record` message
    fn start_msg(&mut self) -> io::Result<()> {
        self.reset()
    }

    /// Format timestamp
    fn start_timestamp(&mut self) -> io::Result<()> {
        self.reset()
    }

    /// Format `Record` level
    fn start_level(&mut self) -> io::Result<()> {
        self.reset()
    }

    /// Format a comma between key-value pairs
    fn start_comma(&mut self) -> io::Result<()> {
        self.reset()
    }

    /// Format key
    fn start_key(&mut self) -> io::Result<()> {
        self.reset()
    }

    /// Format a value
    fn start_value(&mut self) -> io::Result<()> {
        self.reset()
    }

    /// Format a file location
    fn start_location(&mut self) -> io::Result<()> {
        self.reset()
    }

    /// Format value
    fn start_separator(&mut self) -> io::Result<()> {
        self.reset()
    }
}

impl RecordDecorator for Box<dyn RecordDecorator> {
    fn reset(&mut self) -> io::Result<()> {
        (**self).reset()
    }
    fn start_whitespace(&mut self) -> io::Result<()> {
        (**self).start_whitespace()
    }

    /// Format `Record` message
    fn start_msg(&mut self) -> io::Result<()> {
        (**self).start_msg()
    }

    /// Format timestamp
    fn start_timestamp(&mut self) -> io::Result<()> {
        (**self).start_timestamp()
    }

    /// Format `Record` level
    fn start_level(&mut self) -> io::Result<()> {
        (**self).start_level()
    }

    /// Format `Record` message
    fn start_comma(&mut self) -> io::Result<()> {
        (**self).start_comma()
    }

    /// Format key
    fn start_key(&mut self) -> io::Result<()> {
        (**self).start_key()
    }

    /// Format value
    fn start_value(&mut self) -> io::Result<()> {
        (**self).start_value()
    }

    /// Format file location
    fn start_location(&mut self) -> io::Result<()> {
        (**self).start_location()
    }

    /// Format value
    fn start_separator(&mut self) -> io::Result<()> {
        (**self).start_separator()
    }
}
// }}}

// {{{ Misc
/// Returns `true` if message was not empty
pub fn print_msg_header(
    fn_timestamp: &dyn ThreadSafeTimestampFn<Output = io::Result<()>>,
    mut rd: &mut dyn RecordDecorator,
    record: &Record,
    use_file_location: bool,
) -> io::Result<bool> {
    rd.start_timestamp()?;
    fn_timestamp(&mut rd)?;

    rd.start_whitespace()?;
    write!(rd, " ")?;

    rd.start_level()?;
    write!(rd, "{}", record.level().as_short_str())?;

    if use_file_location {
        rd.start_location()?;
        write!(
            rd,
            "[{}:{}:{}]",
            record.location().file,
            record.location().line,
            record.location().column
        )?;
    }

    rd.start_whitespace()?;
    write!(rd, " ")?;

    rd.start_msg()?;
    let mut count_rd = CountingWriter::new(&mut rd);
    write!(count_rd, "{}", record.msg())?;
    Ok(count_rd.count() != 0)
}

// }}}

// {{{ Header Printer
/// Threadsafe header formatting function type
///
/// To satify `slog-rs` thread and unwind safety requirements, the
/// bounds expressed by this trait need to satisfied for a function
/// to be used in timestamp formatting.
pub trait ThreadSafeHeaderFn:
    Fn(
        &dyn ThreadSafeTimestampFn<Output = io::Result<()>>,
        &mut dyn RecordDecorator,
        &Record,
        bool,
    ) -> io::Result<bool>
    + Send
    + Sync
    + UnwindSafe
    + RefUnwindSafe
    + 'static
{
}

impl<F> ThreadSafeHeaderFn for F
where
    F: Fn(
            &dyn ThreadSafeTimestampFn<Output = io::Result<()>>,
            &mut dyn RecordDecorator,
            &Record,
            bool,
        ) -> io::Result<bool>
        + Send
        + Sync,
    F: UnwindSafe + RefUnwindSafe + 'static,
    F: ?Sized,
{
}

// }}}

// {{{ Term
/// Terminal-output formatting `Drain`
///
/// **Note**: logging to `FullFormat` drain is thread-safe, since every
/// line of output is formatted independently. However, the underlying
/// IO, needs to be synchronized.
pub struct FullFormat<D>
where
    D: Decorator,
{
    decorator: D,
    fn_timestamp: Box<dyn ThreadSafeTimestampFn<Output = io::Result<()>>>,
    use_original_order: bool,
    use_file_location: bool,
    header_printer: Box<dyn ThreadSafeHeaderFn>,
}

/// Streamer builder
pub struct FullFormatBuilder<D>
where
    D: Decorator,
{
    decorator: D,
    fn_timestamp: Box<dyn ThreadSafeTimestampFn<Output = io::Result<()>>>,
    original_order: bool,
    file_location: bool,
    header_printer: Box<dyn ThreadSafeHeaderFn>,
}

impl<D> FullFormatBuilder<D>
where
    D: Decorator,
{
    /// Use the UTC time zone for the timestamp
    pub fn use_utc_timestamp(mut self) -> Self {
        self.fn_timestamp = Box::new(timestamp_utc);
        self
    }

    /// Use the local time zone for the timestamp (default)
    pub fn use_local_timestamp(mut self) -> Self {
        self.fn_timestamp = Box::new(timestamp_local);
        self
    }

    /// Provide a custom function to generate the timestamp
    pub fn use_custom_timestamp<F>(mut self, f: F) -> Self
    where
        F: ThreadSafeTimestampFn,
    {
        self.fn_timestamp = Box::new(f);
        self
    }

    /// Enable the file location in log in this format [file:line:column]
    pub fn use_file_location(mut self) -> Self {
        self.file_location = true;
        self
    }

    /// Use the original ordering of key-value pairs
    ///
    /// By default, key-values are printed in a reversed order. This option will
    /// change it to the order in which key-values were added.
    pub fn use_original_order(mut self) -> Self {
        self.original_order = true;
        self
    }

    /// Provide a function that print the header
    ///
    /// If not used, `slog_term::print_msg_header` will be used.
    ///
    /// The header is the part before the log message and key-values. It usually contains the time,
    /// the log level.
    ///
    /// The default function:
    /// ```compile_fail
    /// pub fn print_msg_header(
    ///     fn_timestamp: &dyn ThreadSafeTimestampFn<Output = io::Result<()>>,
    ///     mut rd: &mut dyn RecordDecorator,
    ///     record: &Record,
    ///     use_file_location: bool,
    /// ) -> io::Result<bool> {
    ///     rd.start_timestamp()?;
    ///     fn_timestamp(&mut rd)?;
    ///
    ///     rd.start_whitespace()?;
    ///     write!(rd, " ")?;
    ///
    ///     rd.start_level()?;
    ///     write!(rd, "{}", record.level().as_short_str())?;
    ///
    ///     if use_file_location {
    ///         rd.start_location()?;
    ///         write!(
    ///             rd,
    ///             "[{}:{}:{}]",
    ///             record.location().file,
    ///             record.location().line,
    ///             record.location().column
    ///         )?;
    ///     }
    ///
    ///     rd.start_whitespace()?;
    ///     write!(rd, " ")?;
    ///
    ///     rd.start_msg()?;
    ///     let mut count_rd = CountingWriter::new(&mut rd);
    ///     write!(count_rd, "{}", record.msg())?;
    ///     Ok(count_rd.count() != 0)
    /// }
    /// ```
    ///
    /// produces this output:
    /// ```text
    /// Oct 19 09:20:37.962 INFO an event log, my_key: my_value
    /// ```
    ///
    /// the `Oct 19 09:20:37.962 INFO` part is the header.
    pub fn use_custom_header_print<F>(mut self, f: F) -> Self
    where
        F: ThreadSafeHeaderFn,
    {
        self.header_printer = Box::new(f);
        self
    }

    /// Build `FullFormat`
    pub fn build(self) -> FullFormat<D> {
        FullFormat {
            decorator: self.decorator,
            fn_timestamp: self.fn_timestamp,
            use_original_order: self.original_order,
            use_file_location: self.file_location,
            header_printer: self.header_printer,
        }
    }
}

impl<D> Drain for FullFormat<D>
where
    D: Decorator,
{
    type Ok = ();
    type Err = io::Error;

    fn log(
        &self,
        record: &Record,
        values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        self.format_full(record, values)
    }
}

impl<D> FullFormat<D>
where
    D: Decorator,
{
    /// New `TermBuilder`
    #[allow(clippy::new_ret_no_self)]
    pub fn new(d: D) -> FullFormatBuilder<D> {
        FullFormatBuilder {
            fn_timestamp: Box::new(timestamp_local),
            decorator: d,
            original_order: false,
            file_location: false,
            header_printer: Box::new(print_msg_header),
        }
    }

    fn format_full(
        &self,
        record: &Record,
        values: &OwnedKVList,
    ) -> io::Result<()> {
        self.decorator.with_record(record, values, |decorator| {
            let header_printer = &self.header_printer;
            let comma_needed = header_printer(
                &*self.fn_timestamp,
                decorator,
                record,
                self.use_file_location,
            )?;

            {
                let mut serializer = Serializer::new(
                    decorator,
                    comma_needed,
                    self.use_original_order,
                );

                record.kv().serialize(record, &mut serializer)?;

                values.serialize(record, &mut serializer)?;

                serializer.finish()?;
            }

            decorator.start_whitespace()?;
            writeln!(decorator)?;

            decorator.flush()?;

            Ok(())
        })
    }
}
// }}}

// {{{ CompactFormat
/// Compact terminal-output formatting `Drain`
///
/// **Note**: Compact logging format is not `Sync` (thread-safe) and needs to be
/// synchronized externally, as current output depends on the previous one.
///
/// Put it into a `std::sync::Mutex` or `slog_async::Async` worker-thread to
/// serialize accesses to it.
pub struct CompactFormat<D>
where
    D: Decorator,
{
    decorator: D,
    history: RefCell<Vec<(Vec<u8>, Vec<u8>)>>,
    fn_timestamp: Box<dyn ThreadSafeTimestampFn<Output = io::Result<()>>>,
    header_printer: Box<dyn ThreadSafeHeaderFn>,
}

/// Streamer builder
pub struct CompactFormatBuilder<D>
where
    D: Decorator,
{
    decorator: D,
    fn_timestamp: Box<dyn ThreadSafeTimestampFn<Output = io::Result<()>>>,
    header_printer: Box<dyn ThreadSafeHeaderFn>,
}

impl<D> CompactFormatBuilder<D>
where
    D: Decorator,
{
    /// Use the UTC time zone for the timestamp
    pub fn use_utc_timestamp(mut self) -> Self {
        self.fn_timestamp = Box::new(timestamp_utc);
        self
    }

    /// Use the local time zone for the timestamp (default)
    pub fn use_local_timestamp(mut self) -> Self {
        self.fn_timestamp = Box::new(timestamp_local);
        self
    }

    /// Provide a custom function to generate the timestamp
    pub fn use_custom_timestamp<F>(mut self, f: F) -> Self
    where
        F: ThreadSafeTimestampFn,
    {
        self.fn_timestamp = Box::new(f);
        self
    }

    /// Provide a function that print the header
    ///
    /// If not used, `slog_term::print_msg_header` will be used
    pub fn use_custom_header_print<F>(mut self, f: F) -> Self
    where
        F: ThreadSafeHeaderFn,
    {
        self.header_printer = Box::new(f);
        self
    }

    /// Build the streamer
    pub fn build(self) -> CompactFormat<D> {
        CompactFormat {
            decorator: self.decorator,
            fn_timestamp: self.fn_timestamp,
            history: RefCell::new(vec![]),
            header_printer: self.header_printer,
        }
    }
}

impl<D> Drain for CompactFormat<D>
where
    D: Decorator,
{
    type Ok = ();
    type Err = io::Error;

    fn log(
        &self,
        record: &Record,
        values: &OwnedKVList,
    ) -> result::Result<Self::Ok, Self::Err> {
        self.format_compact(record, values)
    }
}

impl<D> CompactFormat<D>
where
    D: Decorator,
{
    /// New `CompactFormatBuilder`
    #[allow(clippy::new_ret_no_self)]
    pub fn new(d: D) -> CompactFormatBuilder<D> {
        CompactFormatBuilder {
            fn_timestamp: Box::new(timestamp_local),
            decorator: d,
            header_printer: Box::new(print_msg_header),
        }
    }

    fn format_compact(
        &self,
        record: &Record,
        values: &OwnedKVList,
    ) -> io::Result<()> {
        self.decorator.with_record(record, values, |decorator| {
            let indent = {
                let mut history_ref = self.history.borrow_mut();
                let mut serializer =
                    CompactFormatSerializer::new(decorator, &mut *history_ref);

                values.serialize(record, &mut serializer)?;

                serializer.finish()?
            };

            decorator.start_whitespace()?;

            for _ in 0..indent {
                write!(decorator, " ")?;
            }

            let header_printer = &self.header_printer;
            let comma_needed =
                header_printer(&*self.fn_timestamp, decorator, record, false)?;

            {
                let mut serializer =
                    Serializer::new(decorator, comma_needed, false);

                record.kv().serialize(record, &mut serializer)?;

                serializer.finish()?;
            }

            decorator.start_whitespace()?;
            writeln!(decorator)?;

            decorator.flush()?;

            Ok(())
        })
    }
}
// }}}

// {{{ Serializer
/// Serializer for the lines
pub struct Serializer<'a> {
    comma_needed: bool,
    decorator: &'a mut dyn RecordDecorator,
    reverse: bool,
    stack: Vec<(String, String)>,
}

impl<'a> Serializer<'a> {
    /// Create `Serializer` instance
    pub fn new(
        d: &'a mut dyn RecordDecorator,
        comma_needed: bool,
        reverse: bool,
    ) -> Self {
        Serializer {
            comma_needed,
            decorator: d,
            reverse,
            stack: vec![],
        }
    }

    fn maybe_print_comma(&mut self) -> io::Result<()> {
        if self.comma_needed {
            self.decorator.start_comma()?;
            write!(self.decorator, ", ")?;
        }
        self.comma_needed |= true;
        Ok(())
    }

    /// Write out all the whole stack
    pub fn finish(mut self) -> io::Result<()> {
        loop {
            if let Some((k, v)) = self.stack.pop() {
                self.maybe_print_comma()?;
                self.decorator.start_key()?;
                write!(self.decorator, "{}", k)?;
                write!(self.decorator, ":")?;
                self.decorator.start_whitespace()?;
                write!(self.decorator, " ")?;
                self.decorator.start_value()?;
                write!(self.decorator, "{}", v)?;
            } else {
                return Ok(());
            }
        }
    }
}

impl<'a> Drop for Serializer<'a> {
    fn drop(&mut self) {
        if !self.stack.is_empty() {
            panic!("stack not empty");
        }
    }
}

macro_rules! s(
    ($s:expr, $k:expr, $v:expr) => {

        if $s.reverse {
            $s.stack.push(($k.into(), format!("{}", $v)));
        } else {
        $s.maybe_print_comma()?;
        $s.decorator.start_key()?;
        write!($s.decorator, "{}", $k)?;
        $s.decorator.start_separator()?;
        write!($s.decorator, ":")?;
        $s.decorator.start_whitespace()?;
        write!($s.decorator, " ")?;
        $s.decorator.start_value()?;
        write!($s.decorator, "{}", $v)?;
        }
    };
);

impl<'a> slog::ser::Serializer for Serializer<'a> {
    fn emit_none(&mut self, key: Key) -> slog::Result {
        s!(self, key, "None");
        Ok(())
    }
    fn emit_unit(&mut self, key: Key) -> slog::Result {
        s!(self, key, "()");
        Ok(())
    }

    fn emit_bool(&mut self, key: Key, val: bool) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }

    fn emit_char(&mut self, key: Key, val: char) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }

    fn emit_usize(&mut self, key: Key, val: usize) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_isize(&mut self, key: Key, val: isize) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }

    fn emit_u8(&mut self, key: Key, val: u8) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_i8(&mut self, key: Key, val: i8) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_u16(&mut self, key: Key, val: u16) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_i16(&mut self, key: Key, val: i16) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_u32(&mut self, key: Key, val: u32) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_i32(&mut self, key: Key, val: i32) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_f32(&mut self, key: Key, val: f32) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_u64(&mut self, key: Key, val: u64) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_i64(&mut self, key: Key, val: i64) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_f64(&mut self, key: Key, val: f64) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_str(&mut self, key: Key, val: &str) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    fn emit_arguments(
        &mut self,
        key: Key,
        val: &fmt::Arguments,
    ) -> slog::Result {
        s!(self, key, val);
        Ok(())
    }
    #[cfg(feature = "nested-values")]
    fn emit_serde(
        &mut self,
        key: Key,
        val: &dyn slog::SerdeValue,
    ) -> slog::Result {
        let mut writer = Vec::new();
        serde::ser::Serialize::serialize(
            val.as_serde(),
            &mut serde_json::Serializer::new(&mut writer),
        )
        .map_err(std::io::Error::from)?;
        let val =
            std::str::from_utf8(&writer).expect("serde JSON is always UTF-8");
        s!(self, key, val);
        Ok(())
    }
}
// }}}

// {{{ CompactFormatSerializer
/// The Compact format serializer
pub struct CompactFormatSerializer<'a> {
    decorator: &'a mut dyn RecordDecorator,
    history: &'a mut Vec<(Vec<u8>, Vec<u8>)>,
    buf: Vec<(Vec<u8>, Vec<u8>)>,
}

impl<'a> CompactFormatSerializer<'a> {
    /// Create `CompactFormatSerializer` instance
    pub fn new(
        d: &'a mut dyn RecordDecorator,
        history: &'a mut Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Self {
        CompactFormatSerializer {
            decorator: d,
            history,
            buf: vec![],
        }
    }

    /// Write out all the whole stack
    pub fn finish(&mut self) -> io::Result<usize> {
        let mut indent = 0;

        for mut buf in self.buf.drain(..).rev() {
            let (print, trunc, push) =
                if let Some(prev) = self.history.get_mut(indent) {
                    if *prev != buf {
                        *prev = mem::take(&mut buf);
                        (true, true, false)
                    } else {
                        (false, false, false)
                    }
                } else {
                    (true, false, true)
                };

            if push {
                self.history.push(mem::take(&mut buf));
            }

            if trunc {
                self.history.truncate(indent + 1);
            }

            if print {
                let &(ref k, ref v) =
                    self.history.get(indent).expect("assertion failed");
                self.decorator.start_whitespace()?;
                for _ in 0..indent {
                    write!(self.decorator, " ")?;
                }
                self.decorator.start_key()?;
                self.decorator.write_all(k)?;
                self.decorator.start_separator()?;
                write!(self.decorator, ":")?;
                self.decorator.start_whitespace()?;
                write!(self.decorator, " ")?;
                self.decorator.start_value()?;
                self.decorator.write_all(v)?;

                self.decorator.start_whitespace()?;
                writeln!(self.decorator)?;
            }

            indent += 1;
        }

        Ok(indent)
    }
}

macro_rules! cs(
    ($s:expr, $k:expr, $v:expr) => {

        let mut k = vec!();
        let mut v = vec!();
        write!(&mut k, "{}", $k)?;
        write!(&mut v, "{}", $v)?;
        $s.buf.push((k, v));
    };
);

impl<'a> slog::ser::Serializer for CompactFormatSerializer<'a> {
    fn emit_none(&mut self, key: Key) -> slog::Result {
        cs!(self, key, "None");
        Ok(())
    }
    fn emit_unit(&mut self, key: Key) -> slog::Result {
        cs!(self, key, "()");
        Ok(())
    }

    fn emit_bool(&mut self, key: Key, val: bool) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }

    fn emit_char(&mut self, key: Key, val: char) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }

    fn emit_usize(&mut self, key: Key, val: usize) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_isize(&mut self, key: Key, val: isize) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }

    fn emit_u8(&mut self, key: Key, val: u8) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_i8(&mut self, key: Key, val: i8) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_u16(&mut self, key: Key, val: u16) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_i16(&mut self, key: Key, val: i16) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_u32(&mut self, key: Key, val: u32) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_i32(&mut self, key: Key, val: i32) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_f32(&mut self, key: Key, val: f32) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_u64(&mut self, key: Key, val: u64) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_i64(&mut self, key: Key, val: i64) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_f64(&mut self, key: Key, val: f64) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_str(&mut self, key: Key, val: &str) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
    fn emit_arguments(
        &mut self,
        key: Key,
        val: &fmt::Arguments,
    ) -> slog::Result {
        cs!(self, key, val);
        Ok(())
    }
}
// }}}

// {{{ CountingWriter
/// Wrapper for `Write` types that counts total bytes written.
pub struct CountingWriter<'a> {
    wrapped: &'a mut dyn io::Write,
    count: usize,
}

impl<'a> CountingWriter<'a> {
    /// Create `CountingWriter` instance
    pub fn new(wrapped: &'a mut dyn io::Write) -> CountingWriter {
        CountingWriter { wrapped, count: 0 }
    }

    /// Returns the count of the total bytes written.
    pub fn count(&self) -> usize {
        self.count
    }
}

impl<'a> io::Write for CountingWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.wrapped.write(buf).map(|n| {
            self.count += n;
            n
        })
    }

    fn flush(&mut self) -> io::Result<()> {
        self.wrapped.flush()
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.wrapped.write_all(buf).map(|_| {
            self.count += buf.len();
        })
    }
}
// }}}

// {{{ Timestamp
/// Threadsafe timestamp formatting function type
///
/// To satify `slog-rs` thread and unwind safety requirements, the
/// bounds expressed by this trait need to satisfied for a function
/// to be used in timestamp formatting.
pub trait ThreadSafeTimestampFn:
    Fn(&mut dyn io::Write) -> io::Result<()>
    + Send
    + Sync
    + UnwindSafe
    + RefUnwindSafe
    + 'static
{
}

impl<F> ThreadSafeTimestampFn for F
where
    F: Fn(&mut dyn io::Write) -> io::Result<()> + Send + Sync,
    F: UnwindSafe + RefUnwindSafe + 'static,
    F: ?Sized,
{
}

const TIMESTAMP_FORMAT: &[time::format_description::FormatItem] = time::macros::format_description!("[month repr:short] [day] [hour repr:24]:[minute]:[second].[subsecond digits:3]");

/// Default local timezone timestamp function
///
/// The exact format used, is still subject to change.
pub fn timestamp_local(io: &mut dyn io::Write) -> io::Result<()> {
    let now: time::OffsetDateTime = std::time::SystemTime::now().into();
    write!(
        io,
        "{}",
        now.format(TIMESTAMP_FORMAT)
            .map_err(convert_time_fmt_error)?
    )
}

/// Default UTC timestamp function
///
/// The exact format used, is still subject to change.
pub fn timestamp_utc(io: &mut dyn io::Write) -> io::Result<()> {
    let now = time::OffsetDateTime::now_utc();
    write!(
        io,
        "{}",
        now.format(TIMESTAMP_FORMAT)
            .map_err(convert_time_fmt_error)?
    )
}
fn convert_time_fmt_error(cause: time::error::Format) -> io::Error {
    io::Error::new(io::ErrorKind::Other, cause)
}

// }}}

// {{{ Plain

/// Plain (no-op) `Decorator` implementation
///
/// This decorator doesn't do any coloring, and doesn't do any synchronization
/// between threads, so is not `Sync`. It is however useful combined with
/// `slog_async::Async` drain, as `slog_async::Async` uses only one thread,
/// and thus requires only `Send` from `Drain`s it wraps.
///
/// ```
/// use slog::*;
/// use slog_async::Async;
///
/// let decorator = slog_term::PlainDecorator::new(std::io::stdout());
/// let drain = Async::new(
///        slog_term::FullFormat::new(decorator).build().fuse()
/// )
/// .build()
/// .fuse();
/// ```

pub struct PlainDecorator<W>(RefCell<W>)
where
    W: io::Write;

impl<W> PlainDecorator<W>
where
    W: io::Write,
{
    /// Create `PlainDecorator` instance
    pub fn new(io: W) -> Self {
        PlainDecorator(RefCell::new(io))
    }
}

impl<W> Decorator for PlainDecorator<W>
where
    W: io::Write,
{
    fn with_record<F>(
        &self,
        _record: &Record,
        _logger_values: &OwnedKVList,
        f: F,
    ) -> io::Result<()>
    where
        F: FnOnce(&mut dyn RecordDecorator) -> io::Result<()>,
    {
        f(&mut PlainRecordDecorator(&self.0))
    }
}

/// Record decorator used by `PlainDecorator`
pub struct PlainRecordDecorator<'a, W: 'a>(&'a RefCell<W>)
where
    W: io::Write;

impl<'a, W> io::Write for PlainRecordDecorator<'a, W>
where
    W: io::Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.borrow_mut().flush()
    }
}

impl<'a, W> Drop for PlainRecordDecorator<'a, W>
where
    W: io::Write,
{
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl<'a, W> RecordDecorator for PlainRecordDecorator<'a, W>
where
    W: io::Write,
{
    fn reset(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// }}}

// {{{ PlainSync
/// PlainSync `Decorator` implementation
///
/// This implementation is exactly like `PlainDecorator` but it takes care
/// of synchronizing writes to `io`.
///
/// ```
/// use slog::*;
///
/// let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
/// let root = Logger::root(
///     slog_term::FullFormat::new(plain).build().fuse(), o!()
/// );
/// ```
pub struct PlainSyncDecorator<W>(sync::Arc<sync::Mutex<W>>)
where
    W: io::Write;

impl<W> PlainSyncDecorator<W>
where
    W: io::Write,
{
    /// Create `PlainSyncDecorator` instance
    pub fn new(io: W) -> Self {
        PlainSyncDecorator(sync::Arc::new(sync::Mutex::new(io)))
    }
}

impl<W> Decorator for PlainSyncDecorator<W>
where
    W: io::Write,
{
    fn with_record<F>(
        &self,
        _record: &Record,
        _logger_values: &OwnedKVList,
        f: F,
    ) -> io::Result<()>
    where
        F: FnOnce(&mut dyn RecordDecorator) -> io::Result<()>,
    {
        f(&mut PlainSyncRecordDecorator {
            io: self.0.clone(),
            buf: vec![],
        })
    }
}

/// `RecordDecorator` used by `PlainSyncDecorator`
pub struct PlainSyncRecordDecorator<W>
where
    W: io::Write,
{
    io: sync::Arc<sync::Mutex<W>>,
    buf: Vec<u8>,
}

impl<W> io::Write for PlainSyncRecordDecorator<W>
where
    W: io::Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }

        let mut io = self.io.lock().map_err(|_| {
            io::Error::new(io::ErrorKind::Other, "mutex locking error")
        })?;

        io.write_all(&self.buf)?;
        self.buf.clear();
        io.flush()
    }
}

impl<W> Drop for PlainSyncRecordDecorator<W>
where
    W: io::Write,
{
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl<W> RecordDecorator for PlainSyncRecordDecorator<W>
where
    W: io::Write,
{
    fn reset(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// }}}

// {{{ TermDecorator

/// Any type of a terminal supported by `term` crate
// TODO: https://github.com/Stebalien/term/issues/70
enum AnyTerminal {
    /// Stdout terminal
    Stdout {
        term: Box<term::StdoutTerminal>,
        supports_reset: bool,
        supports_color: bool,
        supports_bold: bool,
    },
    /// Stderr terminal
    Stderr {
        term: Box<term::StderrTerminal>,
        supports_reset: bool,
        supports_color: bool,
        supports_bold: bool,
    },
    FallbackStdout,
    FallbackStderr,
}

impl AnyTerminal {
    fn should_use_color(&self) -> bool {
        match *self {
            AnyTerminal::Stdout { .. } => atty::is(atty::Stream::Stdout),
            AnyTerminal::Stderr { .. } => atty::is(atty::Stream::Stderr),
            AnyTerminal::FallbackStdout => false,
            AnyTerminal::FallbackStderr => false,
        }
    }
}

/// `TermDecorator` builder
pub struct TermDecoratorBuilder {
    use_stderr: bool,
    color: Option<bool>,
}

impl TermDecoratorBuilder {
    fn new() -> Self {
        TermDecoratorBuilder {
            use_stderr: true,
            color: None,
        }
    }

    /// Output to `stderr`
    pub fn stderr(mut self) -> Self {
        self.use_stderr = true;
        self
    }

    /// Output to `stdout`
    pub fn stdout(mut self) -> Self {
        self.use_stderr = false;
        self
    }

    /// Force colored output
    pub fn force_color(mut self) -> Self {
        self.color = Some(true);
        self
    }

    /// Force plain output
    pub fn force_plain(mut self) -> Self {
        self.color = Some(false);
        self
    }

    /// Try to build `TermDecorator`
    ///
    /// Unlike `build` this will not fall-back to raw `stdout`/`stderr`
    /// if it wasn't able to use terminal and its features directly
    /// (eg. if `TERM` env. was not set).
    pub fn try_build(self) -> Option<TermDecorator> {
        let io = if self.use_stderr {
            term::stderr().map(|t| {
                let supports_reset = t.supports_reset();
                let supports_color = t.supports_color();
                let supports_bold = t.supports_attr(term::Attr::Bold);
                AnyTerminal::Stderr {
                    term: t,
                    supports_reset,
                    supports_color,
                    supports_bold,
                }
            })
        } else {
            term::stdout().map(|t| {
                let supports_reset = t.supports_reset();
                let supports_color = t.supports_color();
                let supports_bold = t.supports_attr(term::Attr::Bold);
                AnyTerminal::Stdout {
                    term: t,
                    supports_reset,
                    supports_color,
                    supports_bold,
                }
            })
        };

        io.map(|io| {
            let use_color = self.color.unwrap_or_else(|| io.should_use_color());
            TermDecorator {
                use_color,
                term: RefCell::new(io),
            }
        })
    }

    /// Build `TermDecorator`
    ///
    /// Unlike `try_build` this it will fall-back to using plain `stdout`/`stderr`
    /// if it wasn't able to use terminal directly.
    pub fn build(self) -> TermDecorator {
        let io = if self.use_stderr {
            term::stderr()
                .map(|t| {
                    let supports_reset = t.supports_reset();
                    let supports_color = t.supports_color();
                    let supports_bold = t.supports_attr(term::Attr::Bold);
                    AnyTerminal::Stderr {
                        term: t,
                        supports_reset,
                        supports_color,
                        supports_bold,
                    }
                })
                .unwrap_or(AnyTerminal::FallbackStderr)
        } else {
            term::stdout()
                .map(|t| {
                    let supports_reset = t.supports_reset();
                    let supports_color = t.supports_color();
                    let supports_bold = t.supports_attr(term::Attr::Bold);
                    AnyTerminal::Stdout {
                        term: t,
                        supports_reset,
                        supports_color,
                        supports_bold,
                    }
                })
                .unwrap_or(AnyTerminal::FallbackStdout)
        };

        let use_color = self.color.unwrap_or_else(|| io.should_use_color());
        TermDecorator {
            term: RefCell::new(io),
            use_color,
        }
    }
}

/// `Decorator` implemented using `term` crate
///
/// This decorator will add nice formatting to the logs it's outputting. It's
/// based on `term` crate.
///
/// It does not deal with serialization so is `!Sync`. Run in a separate thread
/// with `slog_async::Async`.
pub struct TermDecorator {
    term: RefCell<AnyTerminal>,
    use_color: bool,
}

impl TermDecorator {
    /// Start building `TermDecorator`
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> TermDecoratorBuilder {
        TermDecoratorBuilder::new()
    }

    /// `Level` color
    ///
    /// Standard level to Unix color conversion used by `TermDecorator`
    pub fn level_to_color(level: slog::Level) -> u16 {
        match level {
            Level::Critical => 5,
            Level::Error => 1,
            Level::Warning => 3,
            Level::Info => 2,
            Level::Debug => 6,
            Level::Trace => 4,
        }
    }
}

impl Decorator for TermDecorator {
    fn with_record<F>(
        &self,
        record: &Record,
        _logger_values: &OwnedKVList,
        f: F,
    ) -> io::Result<()>
    where
        F: FnOnce(&mut dyn RecordDecorator) -> io::Result<()>,
    {
        let mut term = self.term.borrow_mut();
        let mut deco = TermRecordDecorator {
            term: &mut *term,
            level: record.level(),
            use_color: self.use_color,
        };
        {
            f(&mut deco)
        }
    }
}

/// Record decorator used by `TermDecorator`
pub struct TermRecordDecorator<'a> {
    term: &'a mut AnyTerminal,
    level: slog::Level,
    use_color: bool,
}

impl<'a> io::Write for TermRecordDecorator<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match *self.term {
            AnyTerminal::Stdout { ref mut term, .. } => term.write(buf),
            AnyTerminal::Stderr { ref mut term, .. } => term.write(buf),
            AnyTerminal::FallbackStdout => std::io::stdout().write(buf),
            AnyTerminal::FallbackStderr => std::io::stderr().write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match *self.term {
            AnyTerminal::Stdout { ref mut term, .. } => term.flush(),
            AnyTerminal::Stderr { ref mut term, .. } => term.flush(),
            AnyTerminal::FallbackStdout => std::io::stdout().flush(),
            AnyTerminal::FallbackStderr => std::io::stderr().flush(),
        }
    }
}

impl<'a> Drop for TermRecordDecorator<'a> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

fn term_error_to_io_error(e: term::Error) -> io::Error {
    match e {
        term::Error::Io(e) => e,
        e => io::Error::new(io::ErrorKind::Other, format!("term error: {}", e)),
    }
}

impl<'a> RecordDecorator for TermRecordDecorator<'a> {
    fn reset(&mut self) -> io::Result<()> {
        if !self.use_color {
            return Ok(());
        }
        match *self.term {
            AnyTerminal::Stdout {
                ref mut term,
                supports_reset,
                ..
            } if supports_reset => term.reset(),
            AnyTerminal::Stderr {
                ref mut term,
                supports_reset,
                ..
            } if supports_reset => term.reset(),
            _ => Ok(()),
        }
        .map_err(term_error_to_io_error)
    }

    fn start_level(&mut self) -> io::Result<()> {
        if !self.use_color {
            return Ok(());
        }
        let color = TermDecorator::level_to_color(self.level);
        match *self.term {
            AnyTerminal::Stdout {
                ref mut term,
                supports_color,
                ..
            } if supports_color => term.fg(color as term::color::Color),
            AnyTerminal::Stderr {
                ref mut term,
                supports_color,
                ..
            } if supports_color => term.fg(color as term::color::Color),
            _ => Ok(()),
        }
        .map_err(term_error_to_io_error)
    }

    fn start_key(&mut self) -> io::Result<()> {
        if !self.use_color {
            return Ok(());
        }
        match self.term {
            &mut AnyTerminal::Stdout {
                ref mut term,
                supports_color,
                supports_bold,
                ..
            } => {
                if supports_bold {
                    term.attr(term::Attr::Bold)
                } else if supports_color {
                    term.fg(term::color::BRIGHT_WHITE)
                } else {
                    Ok(())
                }
            }
            &mut AnyTerminal::Stderr {
                ref mut term,
                supports_color,
                supports_bold,
                ..
            } => {
                if supports_bold {
                    term.attr(term::Attr::Bold)
                } else if supports_color {
                    term.fg(term::color::BRIGHT_WHITE)
                } else {
                    Ok(())
                }
            }
            &mut AnyTerminal::FallbackStdout
            | &mut AnyTerminal::FallbackStderr => Ok(()),
        }
        .map_err(term_error_to_io_error)
    }

    fn start_msg(&mut self) -> io::Result<()> {
        // msg is just like key
        self.start_key()
    }
}

// }}}

// {{{ TestStdoutWriter
/// Replacement for `std::io::stdout()` for when output capturing by rust's test
/// harness is required.
///
/// # Note
///
/// Due to the way that output capturing works in Rust, using this class has no effect
/// if the logger is later passed to another thread that is not controlled by Rust's
/// testing framework.
/// See [rust-lang/rust#42474](https://github.com/rust-lang/rust/issues/42474) for reference.
///
/// For this reason, combining this drain with [Async](https://github.com/slog-rs/async), for example, has no effect.
///
/// # Example
///
/// ```
/// # use slog::{Drain, info, o, Logger};
/// #[test]
/// fn test_logger() {
///     let logger = {
///         let decorator = slog_term::PlainSyncDecorator::new(slog_term::TestStdoutWriter);
///         let drain = slog_term::FullFormat::new(decorator).build().fuse();
///
///         Logger::root_typed(drain, o!())
///     };
///     info!(logger, "Hi from logger test");
/// }
/// ```
pub struct TestStdoutWriter;

impl io::Write for TestStdoutWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        print!(
            "{}",
            std::str::from_utf8(data)
                .map_err(|x| io::Error::new(io::ErrorKind::InvalidData, x))?
        );
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}
// }}}

// {{{ Helpers
/// Create a `CompactFormat` drain with default settings
pub fn term_compact() -> CompactFormat<TermDecorator> {
    let decorator = TermDecorator::new().build();
    CompactFormat::new(decorator).build()
}

/// Create a `FullFormat` drain with default settings
pub fn term_full() -> FullFormat<TermDecorator> {
    let decorator = TermDecorator::new().build();
    FullFormat::new(decorator).build()
}

// }}}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_logger() {
        let logger = {
            let decorator = PlainSyncDecorator::new(TestStdoutWriter);
            let drain = FullFormat::new(decorator).build().fuse();

            slog::Logger::root_typed(drain, o!())
        };
        info!(logger, "Hi from logger test");
    }
}
// vim: foldmethod=marker foldmarker={{{,}}}
