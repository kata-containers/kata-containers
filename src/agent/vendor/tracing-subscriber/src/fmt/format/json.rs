use super::{Format, FormatEvent, FormatFields, FormatTime};
use crate::{
    field::{RecordFields, VisitOutput},
    fmt::fmt_layer::{FmtContext, FormattedFields},
    registry::LookupSpan,
};
use serde::ser::{SerializeMap, Serializer as _};
use serde_json::Serializer;
use std::{
    collections::BTreeMap,
    fmt::{self, Write},
    io,
};
use tracing_core::{
    field::{self, Field},
    span::Record,
    Event, Subscriber,
};
use tracing_serde::AsSerde;

#[cfg(feature = "tracing-log")]
use tracing_log::NormalizeEvent;

/// Marker for `Format` that indicates that the verbose JSON log format should be used.
///
/// The full format includes fields from all entered spans.
///
/// # Example Output
///
/// ```json
/// {
///     "timestamp":"Feb 20 11:28:15.096",
///     "level":"INFO",
///     "fields":{"message":"some message","key":"value"}
///     "target":"mycrate",
///     "span":{name":"leaf"},
///     "spans":[{"name":"root"},{"name":"leaf"}],
/// }
/// ```
///
/// # Options
///
/// - [`Json::flatten_event`] can be used to enable flattening event fields into
/// the root
/// - [`Json::with_current_span`] can be used to control logging of the current
/// span
/// - [`Json::with_span_list`] can be used to control logging of the span list
/// object.
///
/// By default, event fields are not flattened, and both current span and span
/// list are logged.
///
/// [`Json::flatten_event`]: #method.flatten_event
/// [`Json::with_current_span`]: #method.with_current_span
/// [`Json::with_span_list`]: #method.with_span_list
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Json {
    pub(crate) flatten_event: bool,
    pub(crate) display_current_span: bool,
    pub(crate) display_span_list: bool,
}

impl Json {
    /// If set to `true` event metadata will be flattened into the root object.
    pub fn flatten_event(&mut self, flatten_event: bool) {
        self.flatten_event = flatten_event;
    }

    /// If set to `false`, formatted events won't contain a field for the current span.
    pub fn with_current_span(&mut self, display_current_span: bool) {
        self.display_current_span = display_current_span;
    }

    /// If set to `false`, formatted events won't contain a list of all currently
    /// entered spans. Spans are logged in a list from root to leaf.
    pub fn with_span_list(&mut self, display_span_list: bool) {
        self.display_span_list = display_span_list;
    }
}

struct SerializableContext<'a, 'b, Span, N>(
    &'b crate::layer::Context<'a, Span>,
    std::marker::PhantomData<N>,
)
where
    Span: Subscriber + for<'lookup> crate::registry::LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static;

impl<'a, 'b, Span, N> serde::ser::Serialize for SerializableContext<'a, 'b, Span, N>
where
    Span: Subscriber + for<'lookup> crate::registry::LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn serialize<Ser>(&self, serializer_o: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::ser::Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut serializer = serializer_o.serialize_seq(None)?;

        if let Some(leaf_span) = self.0.lookup_current() {
            for span in leaf_span.scope().from_root() {
                serializer.serialize_element(&SerializableSpan(&span, self.1))?;
            }
        }

        serializer.end()
    }
}

struct SerializableSpan<'a, 'b, Span, N>(
    &'b crate::registry::SpanRef<'a, Span>,
    std::marker::PhantomData<N>,
)
where
    Span: for<'lookup> crate::registry::LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static;

impl<'a, 'b, Span, N> serde::ser::Serialize for SerializableSpan<'a, 'b, Span, N>
where
    Span: for<'lookup> crate::registry::LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn serialize<Ser>(&self, serializer: Ser) -> Result<Ser::Ok, Ser::Error>
    where
        Ser: serde::ser::Serializer,
    {
        let mut serializer = serializer.serialize_map(None)?;

        let ext = self.0.extensions();
        let data = ext
            .get::<FormattedFields<N>>()
            .expect("Unable to find FormattedFields in extensions; this is a bug");

        // TODO: let's _not_ do this, but this resolves
        // https://github.com/tokio-rs/tracing/issues/391.
        // We should probably rework this to use a `serde_json::Value` or something
        // similar in a JSON-specific layer, but I'd (david)
        // rather have a uglier fix now rather than shipping broken JSON.
        match serde_json::from_str::<serde_json::Value>(data) {
            Ok(serde_json::Value::Object(fields)) => {
                for field in fields {
                    serializer.serialize_entry(&field.0, &field.1)?;
                }
            }
            // We have fields for this span which are valid JSON but not an object.
            // This is probably a bug, so panic if we're in debug mode
            Ok(_) if cfg!(debug_assertions) => panic!(
                "span '{}' had malformed fields! this is a bug.\n  error: invalid JSON object\n  fields: {:?}",
                self.0.metadata().name(),
                data
            ),
            // If we *aren't* in debug mode, it's probably best not to
            // crash the program, let's log the field found but also an
            // message saying it's type  is invalid
            Ok(value) => {
                serializer.serialize_entry("field", &value)?;
                serializer.serialize_entry("field_error", "field was no a valid object")?
            }
            // We have previously recorded fields for this span
            // should be valid JSON. However, they appear to *not*
            // be valid JSON. This is almost certainly a bug, so
            // panic if we're in debug mode
            Err(e) if cfg!(debug_assertions) => panic!(
                "span '{}' had malformed fields! this is a bug.\n  error: {}\n  fields: {:?}",
                self.0.metadata().name(),
                e,
                data
            ),
            // If we *aren't* in debug mode, it's probably best not
            // crash the program, but let's at least make sure it's clear
            // that the fields are not supposed to be missing.
            Err(e) => serializer.serialize_entry("field_error", &format!("{}", e))?,
        };
        serializer.serialize_entry("name", self.0.metadata().name())?;
        serializer.end()
    }
}

impl<S, N, T> FormatEvent<S, N> for Format<Json, T>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
    N: for<'writer> FormatFields<'writer> + 'static,
    T: FormatTime,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: &mut dyn fmt::Write,
        event: &Event<'_>,
    ) -> fmt::Result
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let mut timestamp = String::new();
        self.timer.format_time(&mut timestamp)?;

        #[cfg(feature = "tracing-log")]
        let normalized_meta = event.normalized_metadata();
        #[cfg(feature = "tracing-log")]
        let meta = normalized_meta.as_ref().unwrap_or_else(|| event.metadata());
        #[cfg(not(feature = "tracing-log"))]
        let meta = event.metadata();

        let mut visit = || {
            let mut serializer = Serializer::new(WriteAdaptor::new(writer));

            let mut serializer = serializer.serialize_map(None)?;

            if self.display_timestamp {
                serializer.serialize_entry("timestamp", &timestamp)?;
            }

            if self.display_level {
                serializer.serialize_entry("level", &meta.level().as_serde())?;
            }

            let format_field_marker: std::marker::PhantomData<N> = std::marker::PhantomData;

            let current_span = if self.format.display_current_span || self.format.display_span_list
            {
                event
                    .parent()
                    .and_then(|id| ctx.span(id))
                    .or_else(|| ctx.lookup_current())
            } else {
                None
            };

            if self.format.flatten_event {
                let mut visitor = tracing_serde::SerdeMapVisitor::new(serializer);
                event.record(&mut visitor);

                serializer = visitor.take_serializer()?;
            } else {
                use tracing_serde::fields::AsMap;
                serializer.serialize_entry("fields", &event.field_map())?;
            };

            if self.display_target {
                serializer.serialize_entry("target", meta.target())?;
            }

            if self.format.display_current_span {
                if let Some(ref span) = current_span {
                    serializer
                        .serialize_entry("span", &SerializableSpan(span, format_field_marker))
                        .unwrap_or(());
                }
            }

            if self.format.display_span_list && current_span.is_some() {
                serializer.serialize_entry(
                    "spans",
                    &SerializableContext(&ctx.ctx, format_field_marker),
                )?;
            }

            if self.display_thread_name {
                let current_thread = std::thread::current();
                match current_thread.name() {
                    Some(name) => {
                        serializer.serialize_entry("threadName", name)?;
                    }
                    // fall-back to thread id when name is absent and ids are not enabled
                    None if !self.display_thread_id => {
                        serializer
                            .serialize_entry("threadName", &format!("{:?}", current_thread.id()))?;
                    }
                    _ => {}
                }
            }

            if self.display_thread_id {
                serializer
                    .serialize_entry("threadId", &format!("{:?}", std::thread::current().id()))?;
            }

            serializer.end()
        };

        visit().map_err(|_| fmt::Error)?;
        writeln!(writer)
    }
}

impl Default for Json {
    fn default() -> Json {
        Json {
            flatten_event: false,
            display_current_span: true,
            display_span_list: true,
        }
    }
}

/// The JSON [`FormatFields`] implementation.
///
/// [`FormatFields`]: trait.FormatFields.html
#[derive(Debug)]
pub struct JsonFields {
    // reserve the ability to add fields to this without causing a breaking
    // change in the future.
    _private: (),
}

impl JsonFields {
    /// Returns a new JSON [`FormatFields`] implementation.
    ///
    /// [`FormatFields`]: trait.FormatFields.html
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl Default for JsonFields {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> FormatFields<'a> for JsonFields {
    /// Format the provided `fields` to the provided `writer`, returning a result.
    fn format_fields<R: RecordFields>(
        &self,
        writer: &'a mut dyn fmt::Write,
        fields: R,
    ) -> fmt::Result {
        let mut v = JsonVisitor::new(writer);
        fields.record(&mut v);
        v.finish()
    }

    /// Record additional field(s) on an existing span.
    ///
    /// By default, this appends a space to the current set of fields if it is
    /// non-empty, and then calls `self.format_fields`. If different behavior is
    /// required, the default implementation of this method can be overridden.
    fn add_fields(&self, current: &'a mut String, fields: &Record<'_>) -> fmt::Result {
        if !current.is_empty() {
            // If fields were previously recorded on this span, we need to parse
            // the current set of fields as JSON, add the new fields, and
            // re-serialize them. Otherwise, if we just appended the new fields
            // to a previously serialized JSON object, we would end up with
            // malformed JSON.
            //
            // XXX(eliza): this is far from efficient, but unfortunately, it is
            // necessary as long as the JSON formatter is implemented on top of
            // an interface that stores all formatted fields as strings.
            //
            // We should consider reimplementing the JSON formatter as a
            // separate layer, rather than a formatter for the `fmt` layer —
            // then, we could store fields as JSON values, and add to them
            // without having to parse and re-serialize.
            let mut new = String::new();
            let map: BTreeMap<&'_ str, serde_json::Value> =
                serde_json::from_str(current).map_err(|_| fmt::Error)?;
            let mut v = JsonVisitor::new(&mut new);
            v.values = map;
            fields.record(&mut v);
            v.finish()?;
            *current = new;
        } else {
            // If there are no previously recorded fields, we can just reuse the
            // existing string.
            let mut v = JsonVisitor::new(current);
            fields.record(&mut v);
            v.finish()?;
        }

        Ok(())
    }
}

/// The [visitor] produced by [`JsonFields`]'s [`MakeVisitor`] implementation.
///
/// [visitor]: ../../field/trait.Visit.html
/// [`JsonFields`]: struct.JsonFields.html
/// [`MakeVisitor`]: ../../field/trait.MakeVisitor.html
pub struct JsonVisitor<'a> {
    values: BTreeMap<&'a str, serde_json::Value>,
    writer: &'a mut dyn Write,
}

impl<'a> fmt::Debug for JsonVisitor<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("JsonVisitor {{ values: {:?} }}", self.values))
    }
}

impl<'a> JsonVisitor<'a> {
    /// Returns a new default visitor that formats to the provided `writer`.
    ///
    /// # Arguments
    /// - `writer`: the writer to format to.
    /// - `is_empty`: whether or not any fields have been previously written to
    ///   that writer.
    pub fn new(writer: &'a mut dyn Write) -> Self {
        Self {
            values: BTreeMap::new(),
            writer,
        }
    }
}

impl<'a> crate::field::VisitFmt for JsonVisitor<'a> {
    fn writer(&mut self) -> &mut dyn fmt::Write {
        self.writer
    }
}

impl<'a> crate::field::VisitOutput<fmt::Result> for JsonVisitor<'a> {
    fn finish(self) -> fmt::Result {
        let inner = || {
            let mut serializer = Serializer::new(WriteAdaptor::new(self.writer));
            let mut ser_map = serializer.serialize_map(None)?;

            for (k, v) in self.values {
                ser_map.serialize_entry(k, &v)?;
            }

            ser_map.end()
        };

        if inner().is_err() {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

impl<'a> field::Visit for JsonVisitor<'a> {
    /// Visit a double precision floating point value.
    fn record_f64(&mut self, field: &Field, value: f64) {
        self.values
            .insert(field.name(), serde_json::Value::from(value));
    }

    /// Visit a signed 64-bit integer value.
    fn record_i64(&mut self, field: &Field, value: i64) {
        self.values
            .insert(field.name(), serde_json::Value::from(value));
    }

    /// Visit an unsigned 64-bit integer value.
    fn record_u64(&mut self, field: &Field, value: u64) {
        self.values
            .insert(field.name(), serde_json::Value::from(value));
    }

    /// Visit a boolean value.
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.values
            .insert(field.name(), serde_json::Value::from(value));
    }

    /// Visit a string value.
    fn record_str(&mut self, field: &Field, value: &str) {
        self.values
            .insert(field.name(), serde_json::Value::from(value));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        match field.name() {
            // Skip fields that are actually log metadata that have already been handled
            #[cfg(feature = "tracing-log")]
            name if name.starts_with("log.") => (),
            name if name.starts_with("r#") => {
                self.values
                    .insert(&name[2..], serde_json::Value::from(format!("{:?}", value)));
            }
            name => {
                self.values
                    .insert(name, serde_json::Value::from(format!("{:?}", value)));
            }
        };
    }
}

/// A bridge between `fmt::Write` and `io::Write`.
///
/// This is needed because tracing-subscriber's FormatEvent expects a fmt::Write
/// while serde_json's Serializer expects an io::Write.
struct WriteAdaptor<'a> {
    fmt_write: &'a mut dyn fmt::Write,
}

impl<'a> WriteAdaptor<'a> {
    fn new(fmt_write: &'a mut dyn fmt::Write) -> Self {
        Self { fmt_write }
    }
}

impl<'a> io::Write for WriteAdaptor<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s =
            std::str::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        self.fmt_write
            .write_str(s)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(s.as_bytes().len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> fmt::Debug for WriteAdaptor<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("WriteAdaptor { .. }")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fmt::{format::FmtSpan, test::MockMakeWriter, time::FormatTime, SubscriberBuilder};
    use lazy_static::lazy_static;
    use std::{fmt, sync::Mutex};
    use tracing::{self, subscriber::with_default};

    struct MockTime;
    impl FormatTime for MockTime {
        fn format_time(&self, w: &mut dyn fmt::Write) -> fmt::Result {
            write!(w, "fake time")
        }
    }

    fn subscriber() -> SubscriberBuilder<JsonFields, Format<Json>> {
        SubscriberBuilder::default().json()
    }

    #[test]
    fn json() {
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
        }

        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"tracing_subscriber::fmt::format::json::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let subscriber = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_span_list(true);
        test_json(expected, subscriber, &BUF, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_flattened_event() {
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
        }

        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"tracing_subscriber::fmt::format::json::test\",\"message\":\"some json test\"}\n";
        let subscriber = subscriber()
            .flatten_event(true)
            .with_current_span(true)
            .with_span_list(true);
        test_json(expected, subscriber, &BUF, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_disabled_current_span_event() {
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
        }

        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3}],\"target\":\"tracing_subscriber::fmt::format::json::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let subscriber = subscriber()
            .flatten_event(false)
            .with_current_span(false)
            .with_span_list(true);
        test_json(expected, subscriber, &BUF, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_disabled_span_list_event() {
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
        }

        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":42,\"name\":\"json_span\",\"number\":3},\"target\":\"tracing_subscriber::fmt::format::json::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let subscriber = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_span_list(false);
        test_json(expected, subscriber, &BUF, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_nested_span() {
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
        }

        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"span\":{\"answer\":43,\"name\":\"nested_json_span\",\"number\":4},\"spans\":[{\"answer\":42,\"name\":\"json_span\",\"number\":3},{\"answer\":43,\"name\":\"nested_json_span\",\"number\":4}],\"target\":\"tracing_subscriber::fmt::format::json::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let subscriber = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_span_list(true);
        test_json(expected, subscriber, &BUF, || {
            let span = tracing::span!(tracing::Level::INFO, "json_span", answer = 42, number = 3);
            let _guard = span.enter();
            let span = tracing::span!(
                tracing::Level::INFO,
                "nested_json_span",
                answer = 43,
                number = 4
            );
            let _guard = span.enter();
            tracing::info!("some json test");
        });
    }

    #[test]
    fn json_no_span() {
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
        }

        let expected =
        "{\"timestamp\":\"fake time\",\"level\":\"INFO\",\"target\":\"tracing_subscriber::fmt::format::json::test\",\"fields\":{\"message\":\"some json test\"}}\n";
        let subscriber = subscriber()
            .flatten_event(false)
            .with_current_span(true)
            .with_span_list(true);
        test_json(expected, subscriber, &BUF, || {
            tracing::info!("some json test");
        });
    }

    #[test]
    fn record_works() {
        // This test reproduces issue #707, where using `Span::record` causes
        // any events inside the span to be ignored.
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
        }

        let buffer = MockMakeWriter::new(&BUF);
        let subscriber = crate::fmt().json().with_writer(buffer.clone()).finish();

        with_default(subscriber, || {
            tracing::info!("an event outside the root span");
            assert_eq!(
                parse_as_json(&buffer)["fields"]["message"],
                "an event outside the root span"
            );

            let span = tracing::info_span!("the span", na = tracing::field::Empty);
            span.record("na", &"value");
            let _enter = span.enter();

            tracing::info!("an event inside the root span");
            assert_eq!(
                parse_as_json(&buffer)["fields"]["message"],
                "an event inside the root span"
            );
        });
    }

    #[test]
    fn json_span_event_show_correct_context() {
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
        }

        let buffer = MockMakeWriter::new(&BUF);
        let subscriber = subscriber()
            .with_writer(buffer.clone())
            .flatten_event(false)
            .with_current_span(true)
            .with_span_list(false)
            .with_span_events(FmtSpan::FULL)
            .finish();

        with_default(subscriber, || {
            let context = "parent";
            let parent_span = tracing::info_span!("parent_span", context);

            let event = parse_as_json(&buffer);
            assert_eq!(event["fields"]["message"], "new");
            assert_eq!(event["span"]["context"], "parent");

            let _parent_enter = parent_span.enter();
            let event = parse_as_json(&buffer);
            assert_eq!(event["fields"]["message"], "enter");
            assert_eq!(event["span"]["context"], "parent");

            let context = "child";
            let child_span = tracing::info_span!("child_span", context);
            let event = parse_as_json(&buffer);
            assert_eq!(event["fields"]["message"], "new");
            assert_eq!(event["span"]["context"], "child");

            let _child_enter = child_span.enter();
            let event = parse_as_json(&buffer);
            assert_eq!(event["fields"]["message"], "enter");
            assert_eq!(event["span"]["context"], "child");

            drop(_child_enter);
            let event = parse_as_json(&buffer);
            assert_eq!(event["fields"]["message"], "exit");
            assert_eq!(event["span"]["context"], "child");

            drop(child_span);
            let event = parse_as_json(&buffer);
            assert_eq!(event["fields"]["message"], "close");
            assert_eq!(event["span"]["context"], "child");

            drop(_parent_enter);
            let event = parse_as_json(&buffer);
            assert_eq!(event["fields"]["message"], "exit");
            assert_eq!(event["span"]["context"], "parent");

            drop(parent_span);
            let event = parse_as_json(&buffer);
            assert_eq!(event["fields"]["message"], "close");
            assert_eq!(event["span"]["context"], "parent");
        });
    }

    #[test]
    fn json_span_event_with_no_fields() {
        // Check span events serialize correctly.
        // Discussion: https://github.com/tokio-rs/tracing/issues/829#issuecomment-661984255
        lazy_static! {
            static ref BUF: Mutex<Vec<u8>> = Mutex::new(Vec::new());
        }

        let buffer = MockMakeWriter::new(&BUF);
        let subscriber = subscriber()
            .with_writer(buffer.clone())
            .flatten_event(false)
            .with_current_span(false)
            .with_span_list(false)
            .with_span_events(FmtSpan::FULL)
            .finish();

        with_default(subscriber, || {
            let span = tracing::info_span!("valid_json");
            assert_eq!(parse_as_json(&buffer)["fields"]["message"], "new");

            let _enter = span.enter();
            assert_eq!(parse_as_json(&buffer)["fields"]["message"], "enter");

            drop(_enter);
            assert_eq!(parse_as_json(&buffer)["fields"]["message"], "exit");

            drop(span);
            assert_eq!(parse_as_json(&buffer)["fields"]["message"], "close");
        });
    }

    fn parse_as_json(buffer: &MockMakeWriter<'_>) -> serde_json::Value {
        let buf = String::from_utf8(buffer.buf().to_vec()).unwrap();
        let json = buf
            .lines()
            .last()
            .expect("expected at least one line to be written!");
        match serde_json::from_str(json) {
            Ok(v) => v,
            Err(e) => panic!(
                "assertion failed: JSON shouldn't be malformed\n  error: {}\n  json: {}",
                e, json
            ),
        }
    }

    fn test_json<T>(
        expected: &str,
        builder: crate::fmt::SubscriberBuilder<JsonFields, Format<Json>>,
        buf: &'static Mutex<Vec<u8>>,
        producer: impl FnOnce() -> T,
    ) {
        let make_writer = MockMakeWriter::new(buf);
        let subscriber = builder
            .with_writer(make_writer.clone())
            .with_timer(MockTime)
            .finish();

        with_default(subscriber, producer);

        let buf = make_writer.buf();
        let actual = std::str::from_utf8(&buf[..]).unwrap();
        assert_eq!(
            serde_json::from_str::<std::collections::HashMap<&str, serde_json::Value>>(expected)
                .unwrap(),
            serde_json::from_str(actual).unwrap()
        );
    }
}
