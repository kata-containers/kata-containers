// {{{ Crate docs
//! JSON `Drain` for `slog-rs`
//!
//! ```
//! #[macro_use]
//! extern crate slog;
//!
//! use slog::Drain;
//! use std::sync::Mutex;
//!
//! fn main() {
//!     let root = slog::Logger::root(
//!         Mutex::new(slog_json::Json::default(std::io::stderr())).map(slog::Fuse),
//!         o!("version" => env!("CARGO_PKG_VERSION"))
//!     );
//! }
//! ```
// }}}

// {{{ Imports & meta
#![warn(missing_docs)]
#[macro_use]
extern crate slog;

use serde::ser::SerializeMap;
use serde::serde_if_integer128;
use slog::Key;
use slog::Record;
use slog::{FnValue, PushFnValue};
use slog::{OwnedKVList, SendSyncRefUnwindSafeKV, KV};
use std::{fmt, io, result};

use std::cell::RefCell;
use std::fmt::Write;

// }}}

// {{{ Serialize
thread_local! {
    static TL_BUF: RefCell<String> = RefCell::new(String::with_capacity(128))
}

/// `slog::Serializer` adapter for `serde::Serializer`
///
/// Newtype to wrap serde Serializer, so that `Serialize` can be implemented
/// for it
struct SerdeSerializer<S: serde::Serializer> {
    /// Current state of map serializing: `serde::Serializer::MapState`
    ser_map: S::SerializeMap,
}

impl<S: serde::Serializer> SerdeSerializer<S> {
    /// Start serializing map of values
    fn start(ser: S, len: Option<usize>) -> result::Result<Self, slog::Error> {
        let ser_map = ser.serialize_map(len).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("serde serialization error: {}", e),
            )
        })?;
        Ok(SerdeSerializer { ser_map })
    }

    /// Finish serialization, and return the serializer
    fn end(self) -> result::Result<S::Ok, S::Error> {
        self.ser_map.end()
    }
}

macro_rules! impl_m(
    ($s:expr, $key:expr, $val:expr) => ({
        let k_s:  &str = $key.as_ref();
        $s.ser_map.serialize_entry(k_s, $val)
             .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("serde serialization error: {}", e)))?;
        Ok(())
    });
);

impl<S> slog::Serializer for SerdeSerializer<S>
where
    S: serde::Serializer,
{
    fn emit_bool(&mut self, key: Key, val: bool) -> slog::Result {
        impl_m!(self, key, &val)
    }

    fn emit_unit(&mut self, key: Key) -> slog::Result {
        impl_m!(self, key, &())
    }

    fn emit_char(&mut self, key: Key, val: char) -> slog::Result {
        impl_m!(self, key, &val)
    }

    fn emit_none(&mut self, key: Key) -> slog::Result {
        let val: Option<()> = None;
        impl_m!(self, key, &val)
    }
    fn emit_u8(&mut self, key: Key, val: u8) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_i8(&mut self, key: Key, val: i8) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_u16(&mut self, key: Key, val: u16) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_i16(&mut self, key: Key, val: i16) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_usize(&mut self, key: Key, val: usize) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_isize(&mut self, key: Key, val: isize) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_u32(&mut self, key: Key, val: u32) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_i32(&mut self, key: Key, val: i32) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_f32(&mut self, key: Key, val: f32) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_u64(&mut self, key: Key, val: u64) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_i64(&mut self, key: Key, val: i64) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_f64(&mut self, key: Key, val: f64) -> slog::Result {
        impl_m!(self, key, &val)
    }
    serde_if_integer128! {
        fn emit_u128(&mut self, key: Key, val: u128) -> slog::Result {
            impl_m!(self, key, &val)
        }
        fn emit_i128(&mut self, key: Key, val: i128) -> slog::Result {
            impl_m!(self, key, &val)
        }
    }
    fn emit_str(&mut self, key: Key, val: &str) -> slog::Result {
        impl_m!(self, key, &val)
    }
    fn emit_arguments(
        &mut self,
        key: Key,
        val: &fmt::Arguments,
    ) -> slog::Result {
        TL_BUF.with(|buf| {
            let mut buf = buf.borrow_mut();

            buf.write_fmt(*val).unwrap();

            let res = { || impl_m!(self, key, &*buf) }();
            buf.clear();
            res
        })
    }

    #[cfg(feature = "nested-values")]
    fn emit_serde(
        &mut self,
        key: Key,
        value: &dyn slog::SerdeValue,
    ) -> slog::Result {
        impl_m!(self, key, value.as_serde())
    }
}
// }}}

// {{{ Json
/// Json `Drain`
///
/// Each record will be printed as a Json map
/// to a given `io`
pub struct Json<W: io::Write> {
    newlines: bool,
    flush: bool,
    values: Vec<OwnedKVList>,
    io: RefCell<W>,
    pretty: bool,
}

impl<W> Json<W>
where
    W: io::Write,
{
    /// New `Json` `Drain` with default key-value pairs added
    pub fn default(io: W) -> Json<W> {
        JsonBuilder::new(io).add_default_keys().build()
    }

    /// Build custom `Json` `Drain`
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::new_ret_no_self))]
    pub fn new(io: W) -> JsonBuilder<W> {
        JsonBuilder::new(io)
    }

    fn log_impl<F>(
        &self,
        serializer: &mut serde_json::ser::Serializer<&mut W, F>,
        rinfo: &Record,
        logger_values: &OwnedKVList,
    ) -> io::Result<()>
    where
        F: serde_json::ser::Formatter,
    {
        let mut serializer = SerdeSerializer::start(&mut *serializer, None)?;

        for kv in &self.values {
            kv.serialize(rinfo, &mut serializer)?;
        }

        logger_values.serialize(rinfo, &mut serializer)?;

        rinfo.kv().serialize(rinfo, &mut serializer)?;

        let res = serializer.end();

        res.map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(())
    }
}

impl<W> slog::Drain for Json<W>
where
    W: io::Write,
{
    type Ok = ();
    type Err = io::Error;
    fn log(
        &self,
        rinfo: &Record,
        logger_values: &OwnedKVList,
    ) -> io::Result<()> {
        let mut io = self.io.borrow_mut();
        let io = if self.pretty {
            let mut serializer = serde_json::Serializer::pretty(&mut *io);
            self.log_impl(&mut serializer, rinfo, logger_values)?;
            serializer.into_inner()
        } else {
            let mut serializer = serde_json::Serializer::new(&mut *io);
            self.log_impl(&mut serializer, rinfo, logger_values)?;
            serializer.into_inner()
        };
        if self.newlines {
            io.write_all("\n".as_bytes())?;
        }
        if self.flush {
            io.flush()?;
        }
        Ok(())
    }
}

// }}}

// {{{ JsonBuilder
/// Json `Drain` builder
///
/// Create with `Json::new`.
pub struct JsonBuilder<W: io::Write> {
    newlines: bool,
    flush: bool,
    values: Vec<OwnedKVList>,
    io: W,
    pretty: bool,
}

impl<W> JsonBuilder<W>
where
    W: io::Write,
{
    fn new(io: W) -> Self {
        JsonBuilder {
            newlines: true,
            flush: false,
            values: vec![],
            io,
            pretty: false,
        }
    }

    /// Build `Json` `Drain`
    ///
    /// This consumes the builder.
    pub fn build(self) -> Json<W> {
        Json {
            values: self.values,
            newlines: self.newlines,
            flush: self.flush,
            io: RefCell::new(self.io),
            pretty: self.pretty,
        }
    }

    /// Set writing a newline after every log record
    pub fn set_newlines(mut self, enabled: bool) -> Self {
        self.newlines = enabled;
        self
    }

    /// Enable flushing of the `io::Write` after every log record
    pub fn set_flush(mut self, enabled: bool) -> Self {
        self.flush = enabled;
        self
    }

    /// Set whether or not pretty formatted logging should be used
    pub fn set_pretty(mut self, enabled: bool) -> Self {
        self.pretty = enabled;
        self
    }

    /// Add custom values to be printed with this formatter
    pub fn add_key_value<T>(mut self, value: slog::OwnedKV<T>) -> Self
    where
        T: SendSyncRefUnwindSafeKV + 'static,
    {
        self.values.push(value.into());
        self
    }

    /// Add default key-values:
    ///
    /// * `ts` - timestamp
    /// * `level` - record logging level name
    /// * `msg` - msg - formatted logging message
    pub fn add_default_keys(self) -> Self {
        self.add_key_value(o!(
            "ts" => FnValue(move |_ : &Record| {
                    time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .ok()
            }),
            "level" => FnValue(move |rinfo : &Record| {
                rinfo.level().as_short_str()
            }),
            "msg" => PushFnValue(move |record : &Record, ser| {
                ser.emit(record.msg())
            }),
        ))
    }
}
// }}}
// vim: foldmethod=marker foldmarker={{{,}}}
