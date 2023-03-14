//! Formatters for logging `tracing` events.
use super::time::{FormatTime, SystemTime};
use crate::{
    field::{MakeOutput, MakeVisitor, RecordFields, VisitFmt, VisitOutput},
    fmt::fmt_layer::FmtContext,
    fmt::fmt_layer::FormattedFields,
    registry::LookupSpan,
};

use std::fmt::{self, Debug, Display, Write};
use tracing_core::{
    field::{self, Field, Visit},
    span, Event, Level, Subscriber,
};

#[cfg(feature = "tracing-log")]
use tracing_log::NormalizeEvent;

#[cfg(feature = "ansi")]
use ansi_term::{Colour, Style};

#[cfg(feature = "json")]
mod json;
#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub use json::*;

#[cfg(feature = "ansi")]
mod pretty;
#[cfg(feature = "ansi")]
#[cfg_attr(docsrs, doc(cfg(feature = "ansi")))]
pub use pretty::*;

/// A type that can format a tracing [`Event`] to a [`Writer`].
///
/// `FormatEvent` is primarily used in the context of [`fmt::Subscriber`] or
/// [`fmt::Layer`]. Each time an event is dispatched to [`fmt::Subscriber`] or
/// [`fmt::Layer`], the subscriber or layer
/// forwards it to its associated `FormatEvent` to emit a log message.
///
/// This trait is already implemented for function pointers with the same
/// signature as `format_event`.
///
/// # Arguments
///
/// The following arguments are passed to `FormatEvent::format_event`:
///
/// * A [`FmtContext`]. This is an extension of the [`layer::Context`] type,
///   which can be used for accessing stored information such as the current
///   span context an event occurred in.
///
///   In addition, [`FmtContext`] exposes access to the [`FormatFields`]
///   implementation that the subscriber was configured to use via the
///   [`FmtContext::field_format`] method. This can be used when the
///   [`FormatEvent`] implementation needs to format the event's fields.
///
///   For convenience, [`FmtContext`] also [implements `FormatFields`],
///   forwarding to the configured [`FormatFields`] type.
///
/// * A [`Writer`] to which the formatted representation of the event is
///   written. This type implements the [`std::fmt::Write`] trait, and therefore
///   can be used with the [`std::write!`] and [`std::writeln!`] macros, as well
///   as calling [`std::fmt::Write`] methods directly.
///
///   The [`Writer`] type also implements additional methods that provide
///   information about how the event should be formatted. The
///   [`Writer::has_ansi_escapes`] method indicates whether [ANSI terminal
///   escape codes] are supported by the underlying I/O writer that the event
///   will be written to. If this returns `true`, the formatter is permitted to
///   use ANSI escape codes to add colors and other text formatting to its
///   output. If it returns `false`, the event will be written to an output that
///   does not support ANSI escape codes (such as a log file), and they should
///   not be emitted.
///
///   Crates like [`ansi_term`] and [`owo-colors`] can be used to add ANSI
///   escape codes to formatted output.
///
/// * The actual [`Event`] to be formatted.
///
/// # Examples
///
/// This example re-implements a simiplified version of this crate's [default
/// formatter]:
///
/// ```rust
/// use std::fmt;
/// use tracing_core::{Subscriber, Event};
/// use tracing_subscriber::fmt::{
///     format::{self, FormatEvent, FormatFields},
///     FmtContext,
///     FormattedFields,
/// };
/// use tracing_subscriber::registry::LookupSpan;
///
/// struct MyFormatter;
///
/// impl<S, N> FormatEvent<S, N> for MyFormatter
/// where
///     S: Subscriber + for<'a> LookupSpan<'a>,
///     N: for<'a> FormatFields<'a> + 'static,
/// {
///     fn format_event(
///         &self,
///         ctx: &FmtContext<'_, S, N>,
///         mut writer: format::Writer<'_>,
///         event: &Event<'_>,
///     ) -> fmt::Result {
///         // Format values from the event's's metadata:
///         let metadata = event.metadata();
///         write!(&mut writer, "{} {}: ", metadata.level(), metadata.target())?;
///
///         // Format all the spans in the event's span context.
///         if let Some(scope) = ctx.event_scope() {
///             for span in scope.from_root() {
///                 write!(writer, "{}", span.name())?;
///
///                 // `FormattedFields` is a formatted representation of the span's
///                 // fields, which is stored in its extensions by the `fmt` layer's
///                 // `new_span` method. The fields will have been formatted
///                 // by the same field formatter that's provided to the event
///                 // formatter in the `FmtContext`.
///                 let ext = span.extensions();
///                 let fields = &ext
///                     .get::<FormattedFields<N>>()
///                     .expect("will never be `None`");
///
///                 // Skip formatting the fields if the span had no fields.
///                 if !fields.is_empty() {
///                     write!(writer, "{{{}}}", fields)?;
///                 }
///                 write!(writer, ": ")?;
///             }
///         }
///
///         // Write fields on the event
///         ctx.field_format().format_fields(writer.by_ref(), event)?;
///
///         writeln!(writer)
///     }
/// }
///
/// let _subscriber = tracing_subscriber::fmt()
///     .event_format(MyFormatter)
///     .init();
///
/// let _span = tracing::info_span!("my_span", answer = 42).entered();
/// tracing::info!(question = "life, the universe, and everything", "hello world");
/// ```
///
/// This formatter will print events like this:
///
/// ```text
/// DEBUG yak_shaving::shaver: some-span{field-on-span=foo}: started shaving yak
/// ```
///
/// [`fmt::Layer`]: super::Layer
/// [`fmt::Subscriber`]: super::Subscriber
/// [`Event`]: tracing::Event
/// [implements `FormatFields`]: super::FmtContext#impl-FormatFields<'writer>
/// [ANSI terminal escape codes]: https://en.wikipedia.org/wiki/ANSI_escape_code
/// [`Writer::has_ansi_escapes`]: Writer::has_ansi_escapes
/// [`ansi_term`]: https://crates.io/crates/ansi_term
/// [`owo-colors`]: https://crates.io/crates/owo-colors
/// [default formatter]: Full
pub trait FormatEvent<S, N>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    /// Write a log message for `Event` in `Context` to the given [`Writer`].
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result;
}

impl<S, N> FormatEvent<S, N>
    for fn(ctx: &FmtContext<'_, S, N>, Writer<'_>, &Event<'_>) -> fmt::Result
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        (*self)(ctx, writer, event)
    }
}
/// A type that can format a [set of fields] to a [`Writer`].
///
/// `FormatFields` is primarily used in the context of [`FmtSubscriber`]. Each
/// time a span or event with fields is recorded, the subscriber will format
/// those fields with its associated `FormatFields` implementation.
///
/// [set of fields]: ../field/trait.RecordFields.html
/// [`FmtSubscriber`]: ../fmt/struct.Subscriber.html
pub trait FormatFields<'writer> {
    /// Format the provided `fields` to the provided [`Writer`], returning a result.
    fn format_fields<R: RecordFields>(&self, writer: Writer<'writer>, fields: R) -> fmt::Result;

    /// Record additional field(s) on an existing span.
    ///
    /// By default, this appends a space to the current set of fields if it is
    /// non-empty, and then calls `self.format_fields`. If different behavior is
    /// required, the default implementation of this method can be overridden.
    fn add_fields(
        &self,
        current: &'writer mut FormattedFields<Self>,
        fields: &span::Record<'_>,
    ) -> fmt::Result {
        if !current.fields.is_empty() {
            current.fields.push(' ');
        }
        self.format_fields(current.as_writer(), fields)
    }
}

/// Returns the default configuration for an [event formatter].
///
/// Methods on the returned event formatter can be used for further
/// configuration. For example:
///
/// ```rust
/// let format = tracing_subscriber::fmt::format()
///     .without_time()         // Don't include timestamps
///     .with_target(false)     // Don't include event targets.
///     .with_level(false)      // Don't include event levels.
///     .compact();             // Use a more compact, abbreviated format.
///
/// // Use the configured formatter when building a new subscriber.
/// tracing_subscriber::fmt()
///     .event_format(format)
///     .init();
/// ```
pub fn format() -> Format {
    Format::default()
}

/// Returns the default configuration for a JSON [event formatter].
#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
pub fn json() -> Format<Json> {
    format().json()
}

/// Returns a [`FormatFields`] implementation that formats fields using the
/// provided function or closure.
///
/// [`FormatFields`]: trait.FormatFields.html
pub fn debug_fn<F>(f: F) -> FieldFn<F>
where
    F: Fn(&mut Writer<'_>, &Field, &dyn fmt::Debug) -> fmt::Result + Clone,
{
    FieldFn(f)
}

/// A writer to which formatted representations of spans and events are written.
///
/// This type is provided as input to the [`FormatEvent::format_event`] and
/// [`FormatFields::format_fields`] methods, which will write formatted
/// representations of [`Event`]s and [fields] to the `Writer`.
///
/// This type implements the [`std::fmt::Write`] trait, allowing it to be used
/// with any function that takes an instance of [`std::fmt::Write`].
/// Additionally, it can be used with the standard library's [`std::write!`] and
/// [`std::writeln!`] macros.
///
/// Additionally, a `Writer` may expose additional `tracing`-specific
/// information to the formatter implementation.
pub struct Writer<'writer> {
    writer: &'writer mut dyn fmt::Write,
    // TODO(eliza): add ANSI support
    is_ansi: bool,
}

/// A [`FormatFields`] implementation that formats fields by calling a function
/// or closure.
///
/// [`FormatFields`]: trait.FormatFields.html
#[derive(Debug, Clone)]
pub struct FieldFn<F>(F);
/// The [visitor] produced by [`FieldFn`]'s [`MakeVisitor`] implementation.
///
/// [visitor]: ../../field/trait.Visit.html
/// [`FieldFn`]: struct.FieldFn.html
/// [`MakeVisitor`]: ../../field/trait.MakeVisitor.html
pub struct FieldFnVisitor<'a, F> {
    f: F,
    writer: Writer<'a>,
    result: fmt::Result,
}
/// Marker for `Format` that indicates that the compact log format should be used.
///
/// The compact format only includes the fields from the most recently entered span.
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub struct Compact;

/// Marker for `Format` that indicates that the verbose log format should be used.
///
/// The full format includes fields from all entered spans.
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub struct Full;

/// A pre-configured event formatter.
///
/// You will usually want to use this as the `FormatEvent` for a `FmtSubscriber`.
///
/// The default logging format, [`Full`] includes all fields in each event and its containing
/// spans. The [`Compact`] logging format includes only the fields from the most-recently-entered
/// span.
#[derive(Debug, Clone)]
pub struct Format<F = Full, T = SystemTime> {
    format: F,
    pub(crate) timer: T,
    pub(crate) ansi: Option<bool>,
    pub(crate) display_timestamp: bool,
    pub(crate) display_target: bool,
    pub(crate) display_level: bool,
    pub(crate) display_thread_id: bool,
    pub(crate) display_thread_name: bool,
}

// === impl Writer ===

impl<'writer> Writer<'writer> {
    // TODO(eliza): consider making this a public API?
    // We may not want to do that if we choose to expose specialized
    // constructors instead (e.g. `from_string` that stores whether the string
    // is empty...?)
    pub(crate) fn new(writer: &'writer mut impl fmt::Write) -> Self {
        Self {
            writer: writer as &mut dyn fmt::Write,
            is_ansi: false,
        }
    }

    // TODO(eliza): consider making this a public API?
    pub(crate) fn with_ansi(self, is_ansi: bool) -> Self {
        Self { is_ansi, ..self }
    }

    /// Return a new `Writer` that mutably borrows `self`.
    ///
    /// This can be used to temporarily borrow a `Writer` to pass a new `Writer`
    /// to a function that takes a `Writer` by value, allowing the original writer
    /// to still be used once that function returns.
    pub fn by_ref(&mut self) -> Writer<'_> {
        let is_ansi = self.is_ansi;
        Writer {
            writer: self as &mut dyn fmt::Write,
            is_ansi,
        }
    }

    /// Writes a string slice into this `Writer`, returning whether the write succeeded.
    ///
    /// This method can only succeed if the entire string slice was successfully
    /// written, and this method will not return until all data has been written
    /// or an error occurs.
    ///
    /// This is identical to calling the [`write_str` method] from the `Writer`'s
    /// [`std::fmt::Write`] implementation. However, it is also provided as an
    /// inherent method, so that `Writer`s can be used without needing to import the
    /// [`std::fmt::Write`] trait.
    ///
    /// # Errors
    ///
    /// This function will return an instance of [`std::fmt::Error`] on error.
    ///
    /// [`write_str` method]: std::fmt::Write::write_str
    #[inline]
    pub fn write_str(&mut self, s: &str) -> fmt::Result {
        self.writer.write_str(s)
    }

    /// Writes a [`char`] into this writer, returning whether the write succeeded.
    ///
    /// A single [`char`] may be encoded as more than one byte.
    /// This method can only succeed if the entire byte sequence was successfully
    /// written, and this method will not return until all data has been
    /// written or an error occurs.
    ///
    /// This is identical to calling the [`write_char` method] from the `Writer`'s
    /// [`std::fmt::Write`] implementation. However, it is also provided as an
    /// inherent method, so that `Writer`s can be used without needing to import the
    /// [`std::fmt::Write`] trait.
    ///
    /// # Errors
    ///
    /// This function will return an instance of [`std::fmt::Error`] on error.
    ///
    /// [`write_char` method]: std::fmt::Write::write_char
    #[inline]
    pub fn write_char(&mut self, c: char) -> fmt::Result {
        self.writer.write_char(c)
    }

    /// Glue for usage of the [`write!`] macro with `Writer`s.
    ///
    /// This method should generally not be invoked manually, but rather through
    /// the [`write!`] macro itself.
    ///
    /// This is identical to calling the [`write_fmt` method] from the `Writer`'s
    /// [`std::fmt::Write`] implementation. However, it is also provided as an
    /// inherent method, so that `Writer`s can be used with the [`write!` macro]
    /// without needing to import the
    /// [`std::fmt::Write`] trait.
    ///
    /// [`write_fmt` method]: std::fmt::Write::write_fmt
    pub fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        self.writer.write_fmt(args)
    }

    /// Returns `true` if [ANSI escape codes] may be used to add colors
    /// and other formatting when writing to this `Writer`.
    ///
    /// If this returns `false`, formatters should not emit ANSI escape codes.
    ///
    /// [ANSI escape codes]: https://en.wikipedia.org/wiki/ANSI_escape_code
    pub fn has_ansi_escapes(&self) -> bool {
        self.is_ansi
    }

    pub(in crate::fmt::format) fn bold(&self) -> Style {
        #[cfg(feature = "ansi")]
        {
            if self.is_ansi {
                return Style::new().bold();
            }
        }

        Style::new()
    }

    pub(in crate::fmt::format) fn dimmed(&self) -> Style {
        #[cfg(feature = "ansi")]
        {
            if self.is_ansi {
                return Style::new().dimmed();
            }
        }

        Style::new()
    }

    pub(in crate::fmt::format) fn italic(&self) -> Style {
        #[cfg(feature = "ansi")]
        {
            if self.is_ansi {
                return Style::new().italic();
            }
        }

        Style::new()
    }
}

impl fmt::Write for Writer<'_> {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Writer::write_str(self, s)
    }

    #[inline]
    fn write_char(&mut self, c: char) -> fmt::Result {
        Writer::write_char(self, c)
    }

    #[inline]
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        Writer::write_fmt(self, args)
    }
}

impl fmt::Debug for Writer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Writer")
            .field("writer", &format_args!("<&mut dyn fmt::Write>"))
            .field("is_ansi", &self.is_ansi)
            .finish()
    }
}

// === impl Format ===

impl Default for Format<Full, SystemTime> {
    fn default() -> Self {
        Format {
            format: Full,
            timer: SystemTime,
            ansi: None,
            display_timestamp: true,
            display_target: true,
            display_level: true,
            display_thread_id: false,
            display_thread_name: false,
        }
    }
}

impl<F, T> Format<F, T> {
    /// Use a less verbose output format.
    ///
    /// See [`Compact`].
    pub fn compact(self) -> Format<Compact, T> {
        Format {
            format: Compact,
            timer: self.timer,
            ansi: self.ansi,
            display_target: self.display_target,
            display_timestamp: self.display_timestamp,
            display_level: self.display_level,
            display_thread_id: self.display_thread_id,
            display_thread_name: self.display_thread_name,
        }
    }

    /// Use an excessively pretty, human-readable output format.
    ///
    /// See [`Pretty`].
    ///
    /// Note that this requires the "ansi" feature to be enabled.
    ///
    /// # Options
    ///
    /// [`Format::with_ansi`] can be used to disable ANSI terminal escape codes (which enable
    /// formatting such as colors, bold, italic, etc) in event formatting. However, a field
    /// formatter must be manually provided to avoid ANSI in the formatting of parent spans, like
    /// so:
    ///
    /// ```
    /// # use tracing_subscriber::fmt::format;
    /// tracing_subscriber::fmt()
    ///    .pretty()
    ///    .with_ansi(false)
    ///    .fmt_fields(format::PrettyFields::new().with_ansi(false))
    ///    // ... other settings ...
    ///    .init();
    /// ```
    #[cfg(feature = "ansi")]
    #[cfg_attr(docsrs, doc(cfg(feature = "ansi")))]
    pub fn pretty(self) -> Format<Pretty, T> {
        Format {
            format: Pretty::default(),
            timer: self.timer,
            ansi: self.ansi,
            display_target: self.display_target,
            display_timestamp: self.display_timestamp,
            display_level: self.display_level,
            display_thread_id: self.display_thread_id,
            display_thread_name: self.display_thread_name,
        }
    }

    /// Use the full JSON format.
    ///
    /// The full format includes fields from all entered spans.
    ///
    /// # Example Output
    ///
    /// ```ignore,json
    /// {"timestamp":"Feb 20 11:28:15.096","level":"INFO","target":"mycrate","fields":{"message":"some message", "key": "value"}}
    /// ```
    ///
    /// # Options
    ///
    /// - [`Format::flatten_event`] can be used to enable flattening event fields into the root
    /// object.
    ///
    /// [`Format::flatten_event`]: #method.flatten_event
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn json(self) -> Format<Json, T> {
        Format {
            format: Json::default(),
            timer: self.timer,
            ansi: self.ansi,
            display_target: self.display_target,
            display_timestamp: self.display_timestamp,
            display_level: self.display_level,
            display_thread_id: self.display_thread_id,
            display_thread_name: self.display_thread_name,
        }
    }

    /// Use the given [`timer`] for log message timestamps.
    ///
    /// See [`time` module] for the provided timer implementations.
    ///
    /// Note that using the `"time"` feature flag enables the
    /// additional time formatters [`UtcTime`] and [`LocalTime`], which use the
    /// [`time` crate] to provide more sophisticated timestamp formatting
    /// options.
    ///
    /// [`timer`]: super::time::FormatTime
    /// [`time` module]: mod@super::time
    /// [`UtcTime`]: super::time::UtcTime
    /// [`LocalTime`]: super::time::LocalTime
    /// [`time` crate]: https://docs.rs/time/0.3
    pub fn with_timer<T2>(self, timer: T2) -> Format<F, T2> {
        Format {
            format: self.format,
            timer,
            ansi: self.ansi,
            display_target: self.display_target,
            display_timestamp: self.display_timestamp,
            display_level: self.display_level,
            display_thread_id: self.display_thread_id,
            display_thread_name: self.display_thread_name,
        }
    }

    /// Do not emit timestamps with log messages.
    pub fn without_time(self) -> Format<F, ()> {
        Format {
            format: self.format,
            timer: (),
            ansi: self.ansi,
            display_timestamp: false,
            display_target: self.display_target,
            display_level: self.display_level,
            display_thread_id: self.display_thread_id,
            display_thread_name: self.display_thread_name,
        }
    }

    /// Enable ANSI terminal colors for formatted output.
    pub fn with_ansi(self, ansi: bool) -> Format<F, T> {
        Format {
            ansi: Some(ansi),
            ..self
        }
    }

    /// Sets whether or not an event's target is displayed.
    pub fn with_target(self, display_target: bool) -> Format<F, T> {
        Format {
            display_target,
            ..self
        }
    }

    /// Sets whether or not an event's level is displayed.
    pub fn with_level(self, display_level: bool) -> Format<F, T> {
        Format {
            display_level,
            ..self
        }
    }

    /// Sets whether or not the [thread ID] of the current thread is displayed
    /// when formatting events
    ///
    /// [thread ID]: https://doc.rust-lang.org/stable/std/thread/struct.ThreadId.html
    pub fn with_thread_ids(self, display_thread_id: bool) -> Format<F, T> {
        Format {
            display_thread_id,
            ..self
        }
    }

    /// Sets whether or not the [name] of the current thread is displayed
    /// when formatting events
    ///
    /// [name]: https://doc.rust-lang.org/stable/std/thread/index.html#naming-threads
    pub fn with_thread_names(self, display_thread_name: bool) -> Format<F, T> {
        Format {
            display_thread_name,
            ..self
        }
    }

    #[inline]
    fn format_timestamp(&self, writer: &mut Writer<'_>) -> fmt::Result
    where
        T: FormatTime,
    {
        // If timestamps are disabled, do nothing.
        if !self.display_timestamp {
            return Ok(());
        }

        // If ANSI color codes are enabled, format the timestamp with ANSI
        // colors.
        #[cfg(feature = "ansi")]
        {
            if writer.has_ansi_escapes() {
                let style = Style::new().dimmed();
                write!(writer, "{}", style.prefix())?;
                self.timer.format_time(writer)?;
                write!(writer, "{} ", style.suffix())?;
                return Ok(());
            }
        }

        // Otherwise, just format the timestamp without ANSI formatting.
        self.timer.format_time(writer)?;
        writer.write_char(' ')
    }
}

#[cfg(feature = "json")]
#[cfg_attr(docsrs, doc(cfg(feature = "json")))]
impl<T> Format<Json, T> {
    /// Use the full JSON format with the event's event fields flattened.
    ///
    /// # Example Output
    ///
    /// ```ignore,json
    /// {"timestamp":"Feb 20 11:28:15.096","level":"INFO","target":"mycrate", "message":"some message", "key": "value"}
    /// ```
    /// See [`Json`](../format/struct.Json.html).
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn flatten_event(mut self, flatten_event: bool) -> Format<Json, T> {
        self.format.flatten_event(flatten_event);
        self
    }

    /// Sets whether or not the formatter will include the current span in
    /// formatted events.
    ///
    /// See [`format::Json`](../fmt/format/struct.Json.html)
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn with_current_span(mut self, display_current_span: bool) -> Format<Json, T> {
        self.format.with_current_span(display_current_span);
        self
    }

    /// Sets whether or not the formatter will include a list (from root to
    /// leaf) of all currently entered spans in formatted events.
    ///
    /// See [`format::Json`](../fmt/format/struct.Json.html)
    #[cfg(feature = "json")]
    #[cfg_attr(docsrs, doc(cfg(feature = "json")))]
    pub fn with_span_list(mut self, display_span_list: bool) -> Format<Json, T> {
        self.format.with_span_list(display_span_list);
        self
    }
}

impl<S, N, T> FormatEvent<S, N> for Format<Full, T>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    T: FormatTime,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        #[cfg(feature = "tracing-log")]
        let normalized_meta = event.normalized_metadata();
        #[cfg(feature = "tracing-log")]
        let meta = normalized_meta.as_ref().unwrap_or_else(|| event.metadata());
        #[cfg(not(feature = "tracing-log"))]
        let meta = event.metadata();

        // if the `Format` struct *also* has an ANSI color configuration,
        // override the writer...the API for configuring ANSI color codes on the
        // `Format` struct is deprecated, but we still need to honor those
        // configurations.
        if let Some(ansi) = self.ansi {
            writer = writer.with_ansi(ansi);
        }

        self.format_timestamp(&mut writer)?;

        if self.display_level {
            let fmt_level = {
                #[cfg(feature = "ansi")]
                {
                    FmtLevel::new(meta.level(), writer.has_ansi_escapes())
                }
                #[cfg(not(feature = "ansi"))]
                {
                    FmtLevel::new(meta.level())
                }
            };
            write!(writer, "{} ", fmt_level)?;
        }

        if self.display_thread_name {
            let current_thread = std::thread::current();
            match current_thread.name() {
                Some(name) => {
                    write!(writer, "{} ", FmtThreadName::new(name))?;
                }
                // fall-back to thread id when name is absent and ids are not enabled
                None if !self.display_thread_id => {
                    write!(writer, "{:0>2?} ", current_thread.id())?;
                }
                _ => {}
            }
        }

        if self.display_thread_id {
            write!(writer, "{:0>2?} ", std::thread::current().id())?;
        }

        let dimmed = writer.dimmed();

        if let Some(scope) = ctx.event_scope() {
            let bold = writer.bold();

            let mut seen = false;

            for span in scope.from_root() {
                write!(writer, "{}", bold.paint(span.metadata().name()))?;
                seen = true;

                let ext = span.extensions();
                if let Some(fields) = &ext.get::<FormattedFields<N>>() {
                    if !fields.is_empty() {
                        write!(writer, "{}{}{}", bold.paint("{"), fields, bold.paint("}"))?;
                    }
                }
                write!(writer, "{}", dimmed.paint(":"))?;
            }

            if seen {
                writer.write_char(' ')?;
            }
        };

        if self.display_target {
            write!(
                writer,
                "{}{} ",
                dimmed.paint(meta.target()),
                dimmed.paint(":")
            )?;
        }

        ctx.format_fields(writer.by_ref(), event)?;
        writeln!(writer)
    }
}

impl<S, N, T> FormatEvent<S, N> for Format<Compact, T>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
    T: FormatTime,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        #[cfg(feature = "tracing-log")]
        let normalized_meta = event.normalized_metadata();
        #[cfg(feature = "tracing-log")]
        let meta = normalized_meta.as_ref().unwrap_or_else(|| event.metadata());
        #[cfg(not(feature = "tracing-log"))]
        let meta = event.metadata();

        // if the `Format` struct *also* has an ANSI color configuration,
        // override the writer...the API for configuring ANSI color codes on the
        // `Format` struct is deprecated, but we still need to honor those
        // configurations.
        if let Some(ansi) = self.ansi {
            writer = writer.with_ansi(ansi);
        }

        self.format_timestamp(&mut writer)?;

        if self.display_level {
            let fmt_level = {
                #[cfg(feature = "ansi")]
                {
                    FmtLevel::new(meta.level(), writer.has_ansi_escapes())
                }
                #[cfg(not(feature = "ansi"))]
                {
                    FmtLevel::new(meta.level())
                }
            };
            write!(writer, "{} ", fmt_level)?;
        }

        if self.display_thread_name {
            let current_thread = std::thread::current();
            match current_thread.name() {
                Some(name) => {
                    write!(writer, "{} ", FmtThreadName::new(name))?;
                }
                // fall-back to thread id when name is absent and ids are not enabled
                None if !self.display_thread_id => {
                    write!(writer, "{:0>2?} ", current_thread.id())?;
                }
                _ => {}
            }
        }

        if self.display_thread_id {
            write!(writer, "{:0>2?} ", std::thread::current().id())?;
        }

        let fmt_ctx = {
            #[cfg(feature = "ansi")]
            {
                FmtCtx::new(ctx, event.parent(), writer.has_ansi_escapes())
            }
            #[cfg(not(feature = "ansi"))]
            {
                FmtCtx::new(&ctx, event.parent())
            }
        };
        write!(writer, "{}", fmt_ctx)?;

        if self.display_target {
            write!(
                writer,
                "{}{} ",
                writer.bold().paint(meta.target()),
                writer.dimmed().paint(":")
            )?;
        }

        ctx.format_fields(writer.by_ref(), event)?;

        let dimmed = writer.dimmed();
        for span in ctx
            .event_scope()
            .into_iter()
            .map(crate::registry::Scope::from_root)
            .flatten()
        {
            let exts = span.extensions();
            if let Some(fields) = exts.get::<FormattedFields<N>>() {
                if !fields.is_empty() {
                    write!(writer, " {}", dimmed.paint(&fields.fields))?;
                }
            }
        }
        writeln!(writer)
    }
}

// === impl FormatFields ===
impl<'writer, M> FormatFields<'writer> for M
where
    M: MakeOutput<Writer<'writer>, fmt::Result>,
    M::Visitor: VisitFmt + VisitOutput<fmt::Result>,
{
    fn format_fields<R: RecordFields>(&self, writer: Writer<'writer>, fields: R) -> fmt::Result {
        let mut v = self.make_visitor(writer);
        fields.record(&mut v);
        v.finish()
    }
}

/// The default [`FormatFields`] implementation.
///
/// [`FormatFields`]: trait.FormatFields.html
#[derive(Debug)]
pub struct DefaultFields {
    // reserve the ability to add fields to this without causing a breaking
    // change in the future.
    _private: (),
}

/// The [visitor] produced by [`DefaultFields`]'s [`MakeVisitor`] implementation.
///
/// [visitor]: super::super::field::Visit
/// [`MakeVisitor`]: super::super::field::MakeVisitor
#[derive(Debug)]
pub struct DefaultVisitor<'a> {
    writer: Writer<'a>,
    is_empty: bool,
    result: fmt::Result,
}

impl DefaultFields {
    /// Returns a new default [`FormatFields`] implementation.
    ///
    /// [`FormatFields`]: trait.FormatFields.html
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for DefaultFields {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> MakeVisitor<Writer<'a>> for DefaultFields {
    type Visitor = DefaultVisitor<'a>;

    #[inline]
    fn make_visitor(&self, target: Writer<'a>) -> Self::Visitor {
        DefaultVisitor::new(target, true)
    }
}

// === impl DefaultVisitor ===

impl<'a> DefaultVisitor<'a> {
    /// Returns a new default visitor that formats to the provided `writer`.
    ///
    /// # Arguments
    /// - `writer`: the writer to format to.
    /// - `is_empty`: whether or not any fields have been previously written to
    ///   that writer.
    pub fn new(writer: Writer<'a>, is_empty: bool) -> Self {
        Self {
            writer,
            is_empty,
            result: Ok(()),
        }
    }

    fn maybe_pad(&mut self) {
        if self.is_empty {
            self.is_empty = false;
        } else {
            self.result = write!(self.writer, " ");
        }
    }
}

impl<'a> field::Visit for DefaultVisitor<'a> {
    fn record_str(&mut self, field: &Field, value: &str) {
        if self.result.is_err() {
            return;
        }

        if field.name() == "message" {
            self.record_debug(field, &format_args!("{}", value))
        } else {
            self.record_debug(field, &value)
        }
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        if let Some(source) = value.source() {
            let italic = self.writer.italic();
            self.record_debug(
                field,
                &format_args!(
                    "{} {}{}{}{}",
                    value,
                    italic.paint(field.name()),
                    italic.paint(".sources"),
                    self.writer.dimmed().paint("="),
                    ErrorSourceList(source)
                ),
            )
        } else {
            self.record_debug(field, &format_args!("{}", value))
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if self.result.is_err() {
            return;
        }

        self.maybe_pad();
        self.result = match field.name() {
            "message" => write!(self.writer, "{:?}", value),
            // Skip fields that are actually log metadata that have already been handled
            #[cfg(feature = "tracing-log")]
            name if name.starts_with("log.") => Ok(()),
            name if name.starts_with("r#") => write!(
                self.writer,
                "{}{}{:?}",
                self.writer.italic().paint(&name[2..]),
                self.writer.dimmed().paint("="),
                value
            ),
            name => write!(
                self.writer,
                "{}{}{:?}",
                self.writer.italic().paint(name),
                self.writer.dimmed().paint("="),
                value
            ),
        };
    }
}

impl<'a> crate::field::VisitOutput<fmt::Result> for DefaultVisitor<'a> {
    fn finish(self) -> fmt::Result {
        self.result
    }
}

impl<'a> crate::field::VisitFmt for DefaultVisitor<'a> {
    fn writer(&mut self) -> &mut dyn fmt::Write {
        &mut self.writer
    }
}

/// Renders an error into a list of sources, *including* the error
struct ErrorSourceList<'a>(&'a (dyn std::error::Error + 'static));

impl<'a> Display for ErrorSourceList<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut list = f.debug_list();
        let mut curr = Some(self.0);
        while let Some(curr_err) = curr {
            list.entry(&format_args!("{}", curr_err));
            curr = curr_err.source();
        }
        list.finish()
    }
}

struct FmtCtx<'a, S, N> {
    ctx: &'a FmtContext<'a, S, N>,
    span: Option<&'a span::Id>,
    #[cfg(feature = "ansi")]
    ansi: bool,
}

impl<'a, S, N: 'a> FmtCtx<'a, S, N>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    #[cfg(feature = "ansi")]
    pub(crate) fn new(
        ctx: &'a FmtContext<'_, S, N>,
        span: Option<&'a span::Id>,
        ansi: bool,
    ) -> Self {
        Self { ctx, span, ansi }
    }

    #[cfg(not(feature = "ansi"))]
    pub(crate) fn new(ctx: &'a FmtContext<'_, S, N>, span: Option<&'a span::Id>) -> Self {
        Self { ctx, span }
    }

    fn bold(&self) -> Style {
        #[cfg(feature = "ansi")]
        {
            if self.ansi {
                return Style::new().bold();
            }
        }

        Style::new()
    }
}

impl<'a, S, N: 'a> fmt::Display for FmtCtx<'a, S, N>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bold = self.bold();
        let mut seen = false;

        let span = self
            .span
            .and_then(|id| self.ctx.ctx.span(id))
            .or_else(|| self.ctx.ctx.lookup_current());

        let scope = span.into_iter().flat_map(|span| span.scope().from_root());

        for span in scope {
            seen = true;
            write!(f, "{}:", bold.paint(span.metadata().name()))?;
        }

        if seen {
            f.write_char(' ')?;
        }
        Ok(())
    }
}

#[cfg(not(feature = "ansi"))]
struct Style;

#[cfg(not(feature = "ansi"))]
impl Style {
    fn new() -> Self {
        Style
    }

    fn bold(self) -> Self {
        self
    }

    fn paint(&self, d: impl fmt::Display) -> impl fmt::Display {
        d
    }
}

struct FmtThreadName<'a> {
    name: &'a str,
}

impl<'a> FmtThreadName<'a> {
    pub(crate) fn new(name: &'a str) -> Self {
        Self { name }
    }
}

impl<'a> fmt::Display for FmtThreadName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::sync::atomic::{
            AtomicUsize,
            Ordering::{AcqRel, Acquire, Relaxed},
        };

        // Track the longest thread name length we've seen so far in an atomic,
        // so that it can be updated by any thread.
        static MAX_LEN: AtomicUsize = AtomicUsize::new(0);
        let len = self.name.len();
        // Snapshot the current max thread name length.
        let mut max_len = MAX_LEN.load(Relaxed);

        while len > max_len {
            // Try to set a new max length, if it is still the value we took a
            // snapshot of.
            match MAX_LEN.compare_exchange(max_len, len, AcqRel, Acquire) {
                // We successfully set the new max value
                Ok(_) => break,
                // Another thread set a new max value since we last observed
                // it! It's possible that the new length is actually longer than
                // ours, so we'll loop again and check whether our length is
                // still the longest. If not, we'll just use the newer value.
                Err(actual) => max_len = actual,
            }
        }

        // pad thread name using `max_len`
        write!(f, "{:>width$}", self.name, width = max_len)
    }
}

struct FmtLevel<'a> {
    level: &'a Level,
    #[cfg(feature = "ansi")]
    ansi: bool,
}

impl<'a> FmtLevel<'a> {
    #[cfg(feature = "ansi")]
    pub(crate) fn new(level: &'a Level, ansi: bool) -> Self {
        Self { level, ansi }
    }

    #[cfg(not(feature = "ansi"))]
    pub(crate) fn new(level: &'a Level) -> Self {
        Self { level }
    }
}

const TRACE_STR: &str = "TRACE";
const DEBUG_STR: &str = "DEBUG";
const INFO_STR: &str = " INFO";
const WARN_STR: &str = " WARN";
const ERROR_STR: &str = "ERROR";

#[cfg(not(feature = "ansi"))]
impl<'a> fmt::Display for FmtLevel<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self.level {
            Level::TRACE => f.pad(TRACE_STR),
            Level::DEBUG => f.pad(DEBUG_STR),
            Level::INFO => f.pad(INFO_STR),
            Level::WARN => f.pad(WARN_STR),
            Level::ERROR => f.pad(ERROR_STR),
        }
    }
}

#[cfg(feature = "ansi")]
impl<'a> fmt::Display for FmtLevel<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ansi {
            match *self.level {
                Level::TRACE => write!(f, "{}", Colour::Purple.paint(TRACE_STR)),
                Level::DEBUG => write!(f, "{}", Colour::Blue.paint(DEBUG_STR)),
                Level::INFO => write!(f, "{}", Colour::Green.paint(INFO_STR)),
                Level::WARN => write!(f, "{}", Colour::Yellow.paint(WARN_STR)),
                Level::ERROR => write!(f, "{}", Colour::Red.paint(ERROR_STR)),
            }
        } else {
            match *self.level {
                Level::TRACE => f.pad(TRACE_STR),
                Level::DEBUG => f.pad(DEBUG_STR),
                Level::INFO => f.pad(INFO_STR),
                Level::WARN => f.pad(WARN_STR),
                Level::ERROR => f.pad(ERROR_STR),
            }
        }
    }
}

// === impl FieldFn ===

impl<'a, F> MakeVisitor<Writer<'a>> for FieldFn<F>
where
    F: Fn(&mut Writer<'a>, &Field, &dyn fmt::Debug) -> fmt::Result + Clone,
{
    type Visitor = FieldFnVisitor<'a, F>;

    fn make_visitor(&self, writer: Writer<'a>) -> Self::Visitor {
        FieldFnVisitor {
            writer,
            f: self.0.clone(),
            result: Ok(()),
        }
    }
}

impl<'a, F> Visit for FieldFnVisitor<'a, F>
where
    F: Fn(&mut Writer<'a>, &Field, &dyn fmt::Debug) -> fmt::Result,
{
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        if self.result.is_ok() {
            self.result = (self.f)(&mut self.writer, field, value)
        }
    }
}

impl<'a, F> VisitOutput<fmt::Result> for FieldFnVisitor<'a, F>
where
    F: Fn(&mut Writer<'a>, &Field, &dyn fmt::Debug) -> fmt::Result,
{
    fn finish(self) -> fmt::Result {
        self.result
    }
}

impl<'a, F> VisitFmt for FieldFnVisitor<'a, F>
where
    F: Fn(&mut Writer<'a>, &Field, &dyn fmt::Debug) -> fmt::Result,
{
    fn writer(&mut self) -> &mut dyn fmt::Write {
        &mut self.writer
    }
}

impl<'a, F> fmt::Debug for FieldFnVisitor<'a, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldFnVisitor")
            .field("f", &format_args!("{}", std::any::type_name::<F>()))
            .field("writer", &self.writer)
            .field("result", &self.result)
            .finish()
    }
}

// === printing synthetic Span events ===

/// Configures what points in the span lifecycle are logged as events.
///
/// See also [`with_span_events`](../struct.SubscriberBuilder.html#method.with_span_events).
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct FmtSpan(u8);

impl FmtSpan {
    /// one event when span is created
    pub const NEW: FmtSpan = FmtSpan(1 << 0);
    /// one event per enter of a span
    pub const ENTER: FmtSpan = FmtSpan(1 << 1);
    /// one event per exit of a span
    pub const EXIT: FmtSpan = FmtSpan(1 << 2);
    /// one event when the span is dropped
    pub const CLOSE: FmtSpan = FmtSpan(1 << 3);

    /// spans are ignored (this is the default)
    pub const NONE: FmtSpan = FmtSpan(0);
    /// one event per enter/exit of a span
    pub const ACTIVE: FmtSpan = FmtSpan(FmtSpan::ENTER.0 | FmtSpan::EXIT.0);
    /// events at all points (new, enter, exit, drop)
    pub const FULL: FmtSpan =
        FmtSpan(FmtSpan::NEW.0 | FmtSpan::ENTER.0 | FmtSpan::EXIT.0 | FmtSpan::CLOSE.0);

    /// Check whether or not a certain flag is set for this [`FmtSpan`]
    fn contains(&self, other: FmtSpan) -> bool {
        self.clone() & other.clone() == other
    }
}

macro_rules! impl_fmt_span_bit_op {
    ($trait:ident, $func:ident, $op:tt) => {
        impl std::ops::$trait for FmtSpan {
            type Output = FmtSpan;

            fn $func(self, rhs: Self) -> Self::Output {
                FmtSpan(self.0 $op rhs.0)
            }
        }
    };
}

macro_rules! impl_fmt_span_bit_assign_op {
    ($trait:ident, $func:ident, $op:tt) => {
        impl std::ops::$trait for FmtSpan {
            fn $func(&mut self, rhs: Self) {
                *self = FmtSpan(self.0 $op rhs.0)
            }
        }
    };
}

impl_fmt_span_bit_op!(BitAnd, bitand, &);
impl_fmt_span_bit_op!(BitOr, bitor, |);
impl_fmt_span_bit_op!(BitXor, bitxor, ^);

impl_fmt_span_bit_assign_op!(BitAndAssign, bitand_assign, &);
impl_fmt_span_bit_assign_op!(BitOrAssign, bitor_assign, |);
impl_fmt_span_bit_assign_op!(BitXorAssign, bitxor_assign, ^);

impl Debug for FmtSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut wrote_flag = false;
        let mut write_flags = |flag, flag_str| -> fmt::Result {
            if self.contains(flag) {
                if wrote_flag {
                    f.write_str(" | ")?;
                }

                f.write_str(flag_str)?;
                wrote_flag = true;
            }

            Ok(())
        };

        if FmtSpan::NONE | self.clone() == FmtSpan::NONE {
            f.write_str("FmtSpan::NONE")?;
        } else {
            write_flags(FmtSpan::NEW, "FmtSpan::NEW")?;
            write_flags(FmtSpan::ENTER, "FmtSpan::ENTER")?;
            write_flags(FmtSpan::EXIT, "FmtSpan::EXIT")?;
            write_flags(FmtSpan::CLOSE, "FmtSpan::CLOSE")?;
        }

        Ok(())
    }
}

pub(super) struct FmtSpanConfig {
    pub(super) kind: FmtSpan,
    pub(super) fmt_timing: bool,
}

impl FmtSpanConfig {
    pub(super) fn without_time(self) -> Self {
        Self {
            kind: self.kind,
            fmt_timing: false,
        }
    }
    pub(super) fn with_kind(self, kind: FmtSpan) -> Self {
        Self {
            kind,
            fmt_timing: self.fmt_timing,
        }
    }
    pub(super) fn trace_new(&self) -> bool {
        self.kind.contains(FmtSpan::NEW)
    }
    pub(super) fn trace_enter(&self) -> bool {
        self.kind.contains(FmtSpan::ENTER)
    }
    pub(super) fn trace_exit(&self) -> bool {
        self.kind.contains(FmtSpan::EXIT)
    }
    pub(super) fn trace_close(&self) -> bool {
        self.kind.contains(FmtSpan::CLOSE)
    }
}

impl Debug for FmtSpanConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind.fmt(f)
    }
}

impl Default for FmtSpanConfig {
    fn default() -> Self {
        Self {
            kind: FmtSpan::NONE,
            fmt_timing: true,
        }
    }
}

pub(super) struct TimingDisplay(pub(super) u64);
impl Display for TimingDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut t = self.0 as f64;
        for unit in ["ns", "µs", "ms", "s"].iter() {
            if t < 10.0 {
                return write!(f, "{:.2}{}", t, unit);
            } else if t < 100.0 {
                return write!(f, "{:.1}{}", t, unit);
            } else if t < 1000.0 {
                return write!(f, "{:.0}{}", t, unit);
            }
            t /= 1000.0;
        }
        write!(f, "{:.0}s", t * 1000.0)
    }
}

#[cfg(test)]
pub(super) mod test {
    use crate::fmt::{test::MockMakeWriter, time::FormatTime};
    use tracing::{
        self,
        dispatcher::{set_default, Dispatch},
        subscriber::with_default,
    };

    use super::*;
    use std::fmt;

    pub(crate) struct MockTime;
    impl FormatTime for MockTime {
        fn format_time(&self, w: &mut Writer<'_>) -> fmt::Result {
            write!(w, "fake time")
        }
    }

    #[test]
    fn disable_everything() {
        // This test reproduces https://github.com/tokio-rs/tracing/issues/1354
        let make_writer = MockMakeWriter::default();
        let subscriber = crate::fmt::Subscriber::builder()
            .with_writer(make_writer.clone())
            .without_time()
            .with_level(false)
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(false);
        #[cfg(feature = "ansi")]
        let subscriber = subscriber.with_ansi(false);
        run_test(subscriber, make_writer, "hello\n")
    }

    fn test_ansi<T>(
        is_ansi: bool,
        expected: &str,
        builder: crate::fmt::SubscriberBuilder<DefaultFields, Format<T>>,
    ) where
        Format<T, MockTime>: FormatEvent<crate::Registry, DefaultFields>,
        T: Send + Sync + 'static,
    {
        let make_writer = MockMakeWriter::default();
        let subscriber = builder
            .with_writer(make_writer.clone())
            .with_ansi(is_ansi)
            .with_timer(MockTime);
        run_test(subscriber, make_writer, expected)
    }

    #[cfg(not(feature = "ansi"))]
    fn test_without_ansi<T>(
        expected: &str,
        builder: crate::fmt::SubscriberBuilder<DefaultFields, Format<T>>,
    ) where
        Format<T, MockTime>: FormatEvent<crate::Registry, DefaultFields>,
        T: Send + Sync,
    {
        let make_writer = MockMakeWriter::default();
        let subscriber = builder.with_writer(make_writer).with_timer(MockTime);
        run_test(subscriber, make_writer, expected)
    }

    fn test_without_level<T>(
        expected: &str,
        builder: crate::fmt::SubscriberBuilder<DefaultFields, Format<T>>,
    ) where
        Format<T, MockTime>: FormatEvent<crate::Registry, DefaultFields>,
        T: Send + Sync + 'static,
    {
        let make_writer = MockMakeWriter::default();
        let subscriber = builder
            .with_writer(make_writer.clone())
            .with_level(false)
            .with_ansi(false)
            .with_timer(MockTime);
        run_test(subscriber, make_writer, expected);
    }

    fn test_overridden_parents<T>(
        expected: &str,
        builder: crate::fmt::SubscriberBuilder<DefaultFields, Format<T>>,
    ) where
        Format<T, MockTime>: FormatEvent<crate::Registry, DefaultFields>,
        T: Send + Sync + 'static,
    {
        let make_writer = MockMakeWriter::default();
        let collector = builder
            .with_writer(make_writer.clone())
            .with_level(false)
            .with_ansi(false)
            .with_timer(MockTime)
            .finish();

        with_default(collector, || {
            let span1 = tracing::info_span!("span1", span = 1);
            let span2 = tracing::info_span!(parent: &span1, "span2", span = 2);
            tracing::info!(parent: &span2, "hello");
        });
        assert_eq!(expected, make_writer.get_string());
    }

    fn test_overridden_parents_in_scope<T>(
        expected1: &str,
        expected2: &str,
        builder: crate::fmt::SubscriberBuilder<DefaultFields, Format<T>>,
    ) where
        Format<T, MockTime>: FormatEvent<crate::Registry, DefaultFields>,
        T: Send + Sync + 'static,
    {
        let make_writer = MockMakeWriter::default();
        let subscriber = builder
            .with_writer(make_writer.clone())
            .with_level(false)
            .with_ansi(false)
            .with_timer(MockTime)
            .finish();

        with_default(subscriber, || {
            let span1 = tracing::info_span!("span1", span = 1);
            let span2 = tracing::info_span!(parent: &span1, "span2", span = 2);
            let span3 = tracing::info_span!("span3", span = 3);
            let _e3 = span3.enter();

            tracing::info!("hello");
            assert_eq!(expected1, make_writer.get_string().as_str());

            tracing::info!(parent: &span2, "hello");
            assert_eq!(expected2, make_writer.get_string().as_str());
        });
    }

    fn run_test(subscriber: impl Into<Dispatch>, buf: MockMakeWriter, expected: &str) {
        let _default = set_default(&subscriber.into());
        tracing::info!("hello");
        assert_eq!(expected, buf.get_string())
    }

    mod default {
        use super::*;
        #[cfg(feature = "ansi")]
        #[test]
        fn with_ansi_true() {
            let expected = "\u{1b}[2mfake time\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m \u{1b}[2mtracing_subscriber::fmt::format::test\u{1b}[0m\u{1b}[2m:\u{1b}[0m hello\n";
            test_ansi(true, expected, crate::fmt::Subscriber::builder());
        }

        #[cfg(feature = "ansi")]
        #[test]
        fn with_ansi_false() {
            let expected = "fake time  INFO tracing_subscriber::fmt::format::test: hello\n";
            test_ansi(false, expected, crate::fmt::Subscriber::builder());
        }

        #[cfg(not(feature = "ansi"))]
        #[test]
        fn without_ansi() {
            let expected = "fake time  INFO tracing_subscriber::fmt::format::test: hello\n";
            test_without_ansi(expected, crate::fmt::Subscriber::builder())
        }

        #[test]
        fn without_level() {
            let expected = "fake time tracing_subscriber::fmt::format::test: hello\n";
            test_without_level(expected, crate::fmt::Subscriber::builder())
        }

        #[test]
        fn overridden_parents() {
            let expected = "fake time span1{span=1}:span2{span=2}: tracing_subscriber::fmt::format::test: hello\n";
            test_overridden_parents(expected, crate::fmt::Subscriber::builder())
        }

        #[test]
        fn overridden_parents_in_scope() {
            test_overridden_parents_in_scope(
                "fake time span3{span=3}: tracing_subscriber::fmt::format::test: hello\n",
                "fake time span1{span=1}:span2{span=2}: tracing_subscriber::fmt::format::test: hello\n",
                crate::fmt::Subscriber::builder(),
            )
        }
    }

    mod compact {
        use super::*;

        #[cfg(feature = "ansi")]
        #[test]
        fn with_ansi_true() {
            let expected = "\u{1b}[2mfake time\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m \u{1b}[1mtracing_subscriber::fmt::format::test\u{1b}[0m\u{1b}[2m:\u{1b}[0m hello\n";
            test_ansi(true, expected, crate::fmt::Subscriber::builder().compact())
        }

        #[cfg(feature = "ansi")]
        #[test]
        fn with_ansi_false() {
            let expected = "fake time  INFO tracing_subscriber::fmt::format::test: hello\n";
            test_ansi(false, expected, crate::fmt::Subscriber::builder().compact());
        }

        #[cfg(not(feature = "ansi"))]
        #[test]
        fn without_ansi() {
            let expected = "fake time  INFO tracing_subscriber::fmt::format::test: hello\n";
            test_without_ansi(expected, crate::fmt::Subscriber::builder().compact())
        }

        #[test]
        fn without_level() {
            let expected = "fake time tracing_subscriber::fmt::format::test: hello\n";
            test_without_level(expected, crate::fmt::Subscriber::builder().compact());
        }

        #[test]
        fn overridden_parents() {
            let expected = "fake time span1:span2: tracing_subscriber::fmt::format::test: hello span=1 span=2\n";
            test_overridden_parents(expected, crate::fmt::Subscriber::builder().compact())
        }

        #[test]
        fn overridden_parents_in_scope() {
            test_overridden_parents_in_scope(
                "fake time span3: tracing_subscriber::fmt::format::test: hello span=3\n",
                "fake time span1:span2: tracing_subscriber::fmt::format::test: hello span=1 span=2\n",
                crate::fmt::Subscriber::builder().compact(),
            )
        }
    }

    #[test]
    fn format_nanos() {
        fn fmt(t: u64) -> String {
            TimingDisplay(t).to_string()
        }

        assert_eq!(fmt(1), "1.00ns");
        assert_eq!(fmt(12), "12.0ns");
        assert_eq!(fmt(123), "123ns");
        assert_eq!(fmt(1234), "1.23µs");
        assert_eq!(fmt(12345), "12.3µs");
        assert_eq!(fmt(123456), "123µs");
        assert_eq!(fmt(1234567), "1.23ms");
        assert_eq!(fmt(12345678), "12.3ms");
        assert_eq!(fmt(123456789), "123ms");
        assert_eq!(fmt(1234567890), "1.23s");
        assert_eq!(fmt(12345678901), "12.3s");
        assert_eq!(fmt(123456789012), "123s");
        assert_eq!(fmt(1234567890123), "1235s");
    }

    #[test]
    fn fmt_span_combinations() {
        let f = FmtSpan::NONE;
        assert!(!f.contains(FmtSpan::NEW));
        assert!(!f.contains(FmtSpan::ENTER));
        assert!(!f.contains(FmtSpan::EXIT));
        assert!(!f.contains(FmtSpan::CLOSE));

        let f = FmtSpan::ACTIVE;
        assert!(!f.contains(FmtSpan::NEW));
        assert!(f.contains(FmtSpan::ENTER));
        assert!(f.contains(FmtSpan::EXIT));
        assert!(!f.contains(FmtSpan::CLOSE));

        let f = FmtSpan::FULL;
        assert!(f.contains(FmtSpan::NEW));
        assert!(f.contains(FmtSpan::ENTER));
        assert!(f.contains(FmtSpan::EXIT));
        assert!(f.contains(FmtSpan::CLOSE));

        let f = FmtSpan::NEW | FmtSpan::CLOSE;
        assert!(f.contains(FmtSpan::NEW));
        assert!(!f.contains(FmtSpan::ENTER));
        assert!(!f.contains(FmtSpan::EXIT));
        assert!(f.contains(FmtSpan::CLOSE));
    }
}
