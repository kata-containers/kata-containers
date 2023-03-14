//! Context extensions for tracing
use crate::{global, trace::SpanContext, Context, ContextGuard, KeyValue};
use std::error::Error;
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref NOOP_SPAN: SynchronizedSpan = SynchronizedSpan {
        span_context: SpanContext::empty_context(),
        inner: None,
    };
}

/// A reference to the currently active span in this context.
#[derive(Debug)]
pub struct SpanRef<'a>(&'a SynchronizedSpan);

#[derive(Debug)]
struct SynchronizedSpan {
    /// Immutable span context
    span_context: SpanContext,
    /// Mutable span inner that requires synchronization
    inner: Option<Box<Mutex<dyn crate::trace::Span + Send + Sync>>>,
}

impl SpanRef<'_> {
    fn with_inner_mut<F: FnOnce(&mut dyn crate::trace::Span)>(&self, f: F) {
        if let Some(ref inner) = self.0.inner {
            match inner.lock() {
                Ok(mut locked) => f(&mut *locked),
                Err(err) => global::handle_error(err),
            }
        }
    }
}

impl SpanRef<'_> {
    /// An API to record events in the context of a given `Span`.
    pub fn add_event(&self, name: String, attributes: Vec<KeyValue>) {
        self.with_inner_mut(|inner| inner.add_event(name, attributes))
    }

    /// Convenience method to record an exception/error as an `Event`
    pub fn record_exception(&self, err: &dyn Error) {
        self.with_inner_mut(|inner| inner.record_exception(err))
    }

    /// Convenience method to record a exception/error as an `Event` with custom stacktrace
    pub fn record_exception_with_stacktrace(&self, err: &dyn Error, stacktrace: String) {
        self.with_inner_mut(|inner| inner.record_exception_with_stacktrace(err, stacktrace))
    }

    /// An API to record events at a specific time in the context of a given `Span`.
    pub fn add_event_with_timestamp(
        &self,
        name: String,
        timestamp: std::time::SystemTime,
        attributes: Vec<crate::KeyValue>,
    ) {
        self.with_inner_mut(move |inner| {
            inner.add_event_with_timestamp(name, timestamp, attributes)
        })
    }

    /// Returns the `SpanContext` for the given `Span`.
    pub fn span_context(&self) -> &SpanContext {
        &self.0.span_context
    }

    /// Returns true if this `Span` is recording information like events with the `add_event`
    /// operation, attributes using `set_attributes`, status with `set_status`, etc.
    pub fn is_recording(&self) -> bool {
        self.0
            .inner
            .as_ref()
            .and_then(|inner| inner.lock().ok().map(|active| active.is_recording()))
            .unwrap_or(false)
    }

    /// An API to set a single `Attribute` where the attribute properties are passed
    /// as arguments. To avoid extra allocations some implementations may offer a separate API for
    /// each of the possible value types.
    pub fn set_attribute(&self, attribute: crate::KeyValue) {
        self.with_inner_mut(move |inner| inner.set_attribute(attribute))
    }

    /// Sets the status of the `Span`. If used, this will override the default `Span`
    /// status, which is `Unset`. `message` MUST be ignored when the status is `OK` or `Unset`
    pub fn set_status(&self, code: super::StatusCode, message: String) {
        self.with_inner_mut(move |inner| inner.set_status(code, message))
    }

    /// Updates the `Span`'s name. After this update, any sampling behavior based on the
    /// name will depend on the implementation.
    pub fn update_name(&self, new_name: String) {
        self.with_inner_mut(move |inner| inner.update_name(new_name))
    }

    /// Finishes the `Span`.
    pub fn end(&self) {
        self.end_with_timestamp(crate::time::now());
    }

    /// Finishes the `Span` with given timestamp
    pub fn end_with_timestamp(&self, timestamp: std::time::SystemTime) {
        self.with_inner_mut(move |inner| inner.end_with_timestamp(timestamp))
    }
}

/// Methods for storing and retrieving trace data in a context.
pub trait TraceContextExt {
    /// Returns a clone of the current context with the included span.
    ///
    /// This is useful for building tracers.
    fn current_with_span<T: crate::trace::Span + Send + Sync + 'static>(span: T) -> Self;

    /// Returns a clone of this context with the included span.
    ///
    /// This is useful for building tracers.
    fn with_span<T: crate::trace::Span + Send + Sync + 'static>(&self, span: T) -> Self;

    /// Returns a reference to this context's span, or the default no-op span if
    /// none has been set.
    ///
    /// # Examples
    ///
    /// ```
    /// use opentelemetry::{
    ///     sdk::trace as sdktrace,
    ///     trace::{SpanContext, TraceContextExt, Tracer, TracerProvider},
    ///     Context,
    /// };
    ///
    /// // returns a reference to an empty span by default
    /// assert_eq!(Context::current().span().span_context(), &SpanContext::empty_context());
    ///
    /// sdktrace::TracerProvider::default().get_tracer("my-component", None).in_span("my-span", |cx| {
    ///     // Returns a reference to the current span if set
    ///     assert_ne!(cx.span().span_context(), &SpanContext::empty_context());
    /// });
    /// ```
    fn span(&self) -> SpanRef<'_>;

    /// Used to see if a span has been marked as active
    ///
    /// This is useful for building tracers.
    fn has_active_span(&self) -> bool;

    /// Returns a copy of this context with the span context included.
    ///
    /// This is useful for building propagators.
    fn with_remote_span_context(&self, span_context: crate::trace::SpanContext) -> Self;
}

impl TraceContextExt for Context {
    fn current_with_span<T: crate::trace::Span + Send + Sync + 'static>(span: T) -> Self {
        Context::current_with_value(SynchronizedSpan {
            span_context: span.span_context().clone(),
            inner: Some(Box::new(Mutex::new(span))),
        })
    }

    fn with_span<T: crate::trace::Span + Send + Sync + 'static>(&self, span: T) -> Self {
        self.with_value(SynchronizedSpan {
            span_context: span.span_context().clone(),
            inner: Some(Box::new(Mutex::new(span))),
        })
    }

    fn span(&self) -> SpanRef<'_> {
        if let Some(span) = self.get::<SynchronizedSpan>() {
            SpanRef(span)
        } else {
            SpanRef(&*NOOP_SPAN)
        }
    }

    fn has_active_span(&self) -> bool {
        self.get::<SynchronizedSpan>().is_some()
    }

    fn with_remote_span_context(&self, span_context: crate::trace::SpanContext) -> Self {
        self.with_value(SynchronizedSpan {
            span_context,
            inner: None,
        })
    }
}

/// Mark a given `Span` as active.
///
/// The `Tracer` MUST provide a way to update its active `Span`, and MAY provide convenience
/// methods to manage a `Span`'s lifetime and the scope in which a `Span` is active. When an
/// active `Span` is made inactive, the previously-active `Span` SHOULD be made active. A `Span`
/// maybe finished (i.e. have a non-null end time) but still be active. A `Span` may be active
/// on one thread after it has been made inactive on another.
///
/// # Examples
///
/// ```
/// use opentelemetry::{global, trace::{Span, Tracer}, KeyValue};
/// use opentelemetry::trace::{get_active_span, mark_span_as_active};
///
/// fn my_function() {
///     let tracer = global::tracer("my-component-a");
///     // start an active span in one function
///     let span = tracer.start("span-name");
///     let _guard = mark_span_as_active(span);
///     // anything happening in functions we call can still access the active span...
///     my_other_function();
/// }
///
/// fn my_other_function() {
///     // call methods on the current span from
///     get_active_span(|span| {
///         span.add_event("An event!".to_string(), vec![KeyValue::new("happened", true)]);
///     });
/// }
/// ```
#[must_use = "Dropping the guard detaches the context."]
pub fn mark_span_as_active<T: crate::trace::Span + Send + Sync + 'static>(span: T) -> ContextGuard {
    let cx = Context::current_with_span(span);
    cx.attach()
}

/// Executes a closure with a reference to this thread's current span.
///
/// # Examples
///
/// ```
/// use opentelemetry::{global, trace::{Span, Tracer}, KeyValue};
/// use opentelemetry::trace::get_active_span;
///
/// fn my_function() {
///     // start an active span in one function
///     global::tracer("my-component").in_span("span-name", |_cx| {
///         // anything happening in functions we call can still access the active span...
///         my_other_function();
///     })
/// }
///
/// fn my_other_function() {
///     // call methods on the current span from
///     get_active_span(|span| {
///         span.add_event("An event!".to_string(), vec![KeyValue::new("happened", true)]);
///     })
/// }
/// ```
pub fn get_active_span<F, T>(f: F) -> T
where
    F: FnOnce(SpanRef<'_>) -> T,
{
    f(Context::current().span())
}
