//! A `tracing` [`Subscriber`] that uses the [`log`] crate as a backend for
//! formatting `tracing` spans and events.
//!
//! When a [`TraceLogger`] is set as the current subscriber, it will record
//! traces by emitting [`log::Record`]s that can be collected by a logger.
//!
//! **Note**: This API has been deprecated since version 0.1.1. In order to emit
//! `tracing` events as `log` records, the ["log" and "log-always" feature
//! flags][flags] on the `tracing` crate should be used instead.
//!
//! [`log`]: log
//! [`Subscriber`]: https://docs.rs/tracing/0.1.7/tracing/subscriber/trait.Subscriber.html
//! [`log::Record`]:log::Record
//! [flags]: https://docs.rs/tracing/latest/tracing/#crate-feature-flags
#![deprecated(
    since = "0.1.1",
    note = "use the `tracing` crate's \"log\" feature flag instead"
)]
use crate::AsLog;
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{self, Write},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};
use tracing_core::{
    field,
    span::{self, Id},
    Event, Metadata, Subscriber,
};

/// A `tracing` [`Subscriber`] implementation that logs all recorded
/// trace events.
///
/// **Note**: This API has been deprecated since version 0.1.1. In order to emit
/// `tracing` events as `log` records, the ["log" and "log-always" feature
/// flags][flags] on the `tracing` crate should be used instead.
///
/// [`Subscriber`]: https://docs.rs/tracing/0.1.7/tracing/subscriber/trait.Subscriber.html
/// [flags]: https://docs.rs/tracing/latest/tracing/#crate-feature-flags
pub struct TraceLogger {
    settings: Builder,
    spans: Mutex<HashMap<Id, SpanLineBuilder>>,
    next_id: AtomicUsize,
}

thread_local! {
    static CURRENT: RefCell<Vec<Id>> = RefCell::new(Vec::new());
}
/// Configures and constructs a [`TraceLogger`].
///
#[derive(Debug)]
pub struct Builder {
    log_span_closes: bool,
    log_enters: bool,
    log_exits: bool,
    log_ids: bool,
    parent_fields: bool,
    log_parent: bool,
}

// ===== impl TraceLogger =====

impl TraceLogger {
    /// Returns a new `TraceLogger` with the default configuration.
    pub fn new() -> Self {
        Self::builder().finish()
    }

    /// Returns a `Builder` for configuring a `TraceLogger`.
    pub fn builder() -> Builder {
        Default::default()
    }

    fn from_builder(settings: Builder) -> Self {
        Self {
            settings,
            ..Default::default()
        }
    }

    fn next_id(&self) -> Id {
        Id::from_u64(self.next_id.fetch_add(1, Ordering::SeqCst) as u64)
    }
}

// ===== impl Builder =====

impl Builder {
    /// Configures whether or not the [`TraceLogger`] being constructed will log
    /// when a span closes.
    ///
    pub fn with_span_closes(self, log_span_closes: bool) -> Self {
        Self {
            log_span_closes,
            ..self
        }
    }

    /// Configures whether or not the [`TraceLogger`] being constructed will
    /// include the fields of parent spans when formatting events.
    ///
    pub fn with_parent_fields(self, parent_fields: bool) -> Self {
        Self {
            parent_fields,
            ..self
        }
    }

    /// Configures whether or not the [`TraceLogger`] being constructed will log
    /// when a span is entered.
    ///
    /// If this is set to false, fields from the current span will still be
    /// recorded as context, but the actual entry will not create a log record.
    ///
    pub fn with_span_entry(self, log_enters: bool) -> Self {
        Self { log_enters, ..self }
    }

    /// Configures whether or not the [`TraceLogger`] being constructed will log
    /// when a span is exited.
    ///
    pub fn with_span_exits(self, log_exits: bool) -> Self {
        Self { log_exits, ..self }
    }

    /// Configures whether or not the [`TraceLogger`] being constructed will
    /// include span IDs when formatting log output.
    ///
    pub fn with_ids(self, log_ids: bool) -> Self {
        Self { log_ids, ..self }
    }

    /// Configures whether or not the [`TraceLogger`] being constructed will
    /// include the names of parent spans as context when formatting events.
    ///
    pub fn with_parent_names(self, log_parent: bool) -> Self {
        Self { log_parent, ..self }
    }

    /// Complete the builder, returning a configured [`TraceLogger`].
    ///
    pub fn finish(self) -> TraceLogger {
        TraceLogger::from_builder(self)
    }
}

impl Default for Builder {
    fn default() -> Self {
        Builder {
            log_span_closes: false,
            parent_fields: true,
            log_exits: false,
            log_ids: false,
            log_parent: true,
            log_enters: false,
        }
    }
}

impl Default for TraceLogger {
    fn default() -> Self {
        TraceLogger {
            settings: Default::default(),
            spans: Default::default(),
            next_id: AtomicUsize::new(1),
        }
    }
}

#[derive(Debug)]
struct SpanLineBuilder {
    parent: Option<Id>,
    ref_count: usize,
    fields: String,
    file: Option<String>,
    line: Option<u32>,
    module_path: Option<String>,
    target: String,
    level: log::Level,
    name: &'static str,
}

impl SpanLineBuilder {
    fn new(parent: Option<Id>, meta: &Metadata<'_>, fields: String) -> Self {
        Self {
            parent,
            ref_count: 1,
            fields,
            file: meta.file().map(String::from),
            line: meta.line(),
            module_path: meta.module_path().map(String::from),
            target: String::from(meta.target()),
            level: meta.level().as_log(),
            name: meta.name(),
        }
    }

    fn log_meta(&self) -> log::Metadata<'_> {
        log::MetadataBuilder::new()
            .level(self.level)
            .target(self.target.as_ref())
            .build()
    }

    fn finish(self) {
        let log_meta = self.log_meta();
        let logger = log::logger();
        if logger.enabled(&log_meta) {
            logger.log(
                &log::Record::builder()
                    .metadata(log_meta)
                    .target(self.target.as_ref())
                    .module_path(self.module_path.as_ref().map(String::as_ref))
                    .file(self.file.as_ref().map(String::as_ref))
                    .line(self.line)
                    .args(format_args!("close {}; {}", self.name, self.fields))
                    .build(),
            );
        }
    }
}

impl field::Visit for SpanLineBuilder {
    fn record_debug(&mut self, field: &field::Field, value: &dyn fmt::Debug) {
        write!(self.fields, " {}={:?};", field.name(), value)
            .expect("write to string should never fail")
    }
}

impl Subscriber for TraceLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        log::logger().enabled(&metadata.as_log())
    }

    fn new_span(&self, attrs: &span::Attributes<'_>) -> Id {
        let id = self.next_id();
        let mut spans = self.spans.lock().unwrap();
        let mut fields = String::new();
        let parent = self.current_id();
        if self.settings.parent_fields {
            let mut next_parent = parent.as_ref();
            while let Some(parent) = next_parent.and_then(|p| spans.get(p)) {
                write!(&mut fields, "{}", parent.fields).expect("write to string cannot fail");
                next_parent = parent.parent.as_ref();
            }
        }
        let mut span = SpanLineBuilder::new(parent, attrs.metadata(), fields);
        attrs.record(&mut span);
        spans.insert(id.clone(), span);
        id
    }

    fn record(&self, span: &Id, values: &span::Record<'_>) {
        let mut spans = self.spans.lock().unwrap();
        if let Some(span) = spans.get_mut(span) {
            values.record(span);
        }
    }

    fn record_follows_from(&self, span: &Id, follows: &Id) {
        // TODO: this should eventually track the relationship?
        log::logger().log(
            &log::Record::builder()
                .level(log::Level::Trace)
                .args(format_args!("span {:?} follows_from={:?};", span, follows))
                .build(),
        );
    }

    fn enter(&self, id: &Id) {
        let _ = CURRENT.try_with(|current| {
            let mut current = current.borrow_mut();
            if current.contains(id) {
                // Ignore duplicate enters.
                return;
            }
            current.push(id.clone());
        });
        let spans = self.spans.lock().unwrap();
        if self.settings.log_enters {
            if let Some(span) = spans.get(id) {
                let log_meta = span.log_meta();
                let logger = log::logger();
                if logger.enabled(&log_meta) {
                    let current_id = self.current_id();
                    let current_fields = current_id
                        .as_ref()
                        .and_then(|id| spans.get(id))
                        .map(|span| span.fields.as_ref())
                        .unwrap_or("");
                    if self.settings.log_ids {
                        logger.log(
                            &log::Record::builder()
                                .metadata(log_meta)
                                .target(span.target.as_ref())
                                .module_path(span.module_path.as_ref().map(String::as_ref))
                                .file(span.file.as_ref().map(String::as_ref))
                                .line(span.line)
                                .args(format_args!(
                                    "enter {}; in={:?}; {}",
                                    span.name, current_id, current_fields
                                ))
                                .build(),
                        );
                    } else {
                        logger.log(
                            &log::Record::builder()
                                .metadata(log_meta)
                                .target(span.target.as_ref())
                                .module_path(span.module_path.as_ref().map(String::as_ref))
                                .file(span.file.as_ref().map(String::as_ref))
                                .line(span.line)
                                .args(format_args!("enter {}; {}", span.name, current_fields))
                                .build(),
                        );
                    }
                }
            }
        }
    }

    fn exit(&self, id: &Id) {
        let _ = CURRENT.try_with(|current| {
            let mut current = current.borrow_mut();
            if current.last() == Some(id) {
                current.pop()
            } else {
                None
            }
        });
        if self.settings.log_exits {
            let spans = self.spans.lock().unwrap();
            if let Some(span) = spans.get(id) {
                let log_meta = span.log_meta();
                let logger = log::logger();
                if logger.enabled(&log_meta) {
                    logger.log(
                        &log::Record::builder()
                            .metadata(log_meta)
                            .target(span.target.as_ref())
                            .module_path(span.module_path.as_ref().map(String::as_ref))
                            .file(span.file.as_ref().map(String::as_ref))
                            .line(span.line)
                            .args(format_args!("exit {}", span.name))
                            .build(),
                    );
                }
            }
        }
    }

    fn event(&self, event: &Event<'_>) {
        let meta = event.metadata();
        let log_meta = meta.as_log();
        let logger = log::logger();
        if logger.enabled(&log_meta) {
            let spans = self.spans.lock().unwrap();
            let current = self.current_id().and_then(|id| spans.get(&id));
            let (current_fields, parent) = current
                .map(|span| {
                    let fields = span.fields.as_ref();
                    let parent = if self.settings.log_parent {
                        Some(span.name)
                    } else {
                        None
                    };
                    (fields, parent)
                })
                .unwrap_or(("", None));
            logger.log(
                &log::Record::builder()
                    .metadata(log_meta)
                    .target(meta.target())
                    .module_path(meta.module_path().as_ref().cloned())
                    .file(meta.file().as_ref().cloned())
                    .line(meta.line())
                    .args(format_args!(
                        "{}{}{}{}",
                        parent.unwrap_or(""),
                        if parent.is_some() { ": " } else { "" },
                        LogEvent(event),
                        current_fields,
                    ))
                    .build(),
            );
        }
    }

    fn clone_span(&self, id: &Id) -> Id {
        let mut spans = self.spans.lock().unwrap();
        if let Some(span) = spans.get_mut(id) {
            span.ref_count += 1;
        }
        id.clone()
    }

    fn try_close(&self, id: Id) -> bool {
        let mut spans = self.spans.lock().unwrap();
        if spans.contains_key(&id) {
            if spans.get(&id).unwrap().ref_count == 1 {
                let span = spans.remove(&id).unwrap();
                if self.settings.log_span_closes {
                    span.finish();
                }
                return true;
            } else {
                spans.get_mut(&id).unwrap().ref_count -= 1;
            }
        }
        false
    }
}

impl TraceLogger {
    #[inline]
    fn current_id(&self) -> Option<Id> {
        CURRENT
            .try_with(|current| current.borrow().last().map(|span| self.clone_span(span)))
            .ok()?
    }
}

struct LogEvent<'a>(&'a Event<'a>);

impl<'a> fmt::Display for LogEvent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut has_logged = false;
        let mut format_fields = |field: &field::Field, value: &dyn fmt::Debug| {
            let name = field.name();
            let leading = if has_logged { " " } else { "" };
            // TODO: handle fmt error?
            let _ = if name == "message" {
                write!(f, "{}{:?};", leading, value)
            } else {
                write!(f, "{}{}={:?};", leading, name, value)
            };
            has_logged = true;
        };

        self.0.record(&mut format_fields);
        Ok(())
    }
}

impl fmt::Debug for TraceLogger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraceLogger")
            .field("settings", &self.settings)
            .field("spans", &self.spans)
            .field("current", &self.current_id())
            .field("next_id", &self.next_id)
            .finish()
    }
}
