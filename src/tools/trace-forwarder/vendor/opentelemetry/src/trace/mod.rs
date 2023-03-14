//! API for tracing applications and libraries.
//!
//! The `trace` module includes types for tracking the progression of a single
//! request while it is handled by services that make up an application. A trace
//! is a tree of [`Span`]s which are objects that represent the work being done
//! by individual services or components involved in a request as it flows
//! through a system. This module implements the OpenTelemetry [trace
//! specification].
//!
//! [trace specification]: https://github.com/open-telemetry/opentelemetry-specification/blob/v1.3.0/specification/trace/api.md
//!
//! ## Getting Started
//!
//! In application code:
//!
//! ```no_run
//! # #[cfg(feature = "trace")]
//! # {
//! use opentelemetry::{global, sdk::export::trace::stdout, trace::Tracer};
//!
//! fn main() {
//!     // Create a new trace pipeline that prints to stdout
//!     let tracer = stdout::new_pipeline().install_simple();
//!
//!     tracer.in_span("doing_work", |cx| {
//!         // Traced app logic here...
//!     });
//!
//!     // Shutdown trace pipeline
//!     global::shutdown_tracer_provider();
//! }
//! # }
//! ```
//!
//! In library code:
//!
//! ```
//! # #[cfg(feature = "trace")]
//! # {
//! use opentelemetry::{global, trace::{Span, Tracer, TracerProvider}};
//!
//! fn my_library_function() {
//!     // Use the global tracer provider to get access to the user-specified
//!     // tracer configuration
//!     let tracer_provider = global::tracer_provider();
//!
//!     // Get a tracer for this library
//!     let tracer = tracer_provider.tracer("my_name", Some(env!("CARGO_PKG_VERSION")));
//!
//!     // Create spans
//!     let mut span = tracer.start("doing_work");
//!
//!     // Do work...
//!
//!     // End the span
//!     span.end();
//! }
//! # }
//! ```
//!
//! ## Overview
//!
//! The tracing API consists of a three main traits:
//!
//! * [`TracerProvider`]s are the entry point of the API. They provide access to
//!   `Tracer`s.
//! * [`Tracer`]s are types responsible for creating `Span`s.
//! * [`Span`]s provide the API to trace an operation.
//!
//! ## Working with Async Runtimes
//!
//! Exporting spans often involves sending data over a network or performing
//! other I/O tasks. OpenTelemetry allows you to schedule these tasks using
//! whichever runtime you area already using such as [Tokio] or [async-std].
//! When using an async runtime it's best to use the [`BatchSpanProcessor`]
//! where the spans will be sent in batches as opposed to being sent once ended,
//! which often ends up being more efficient.
//!
//! [`BatchSpanProcessor`]: crate::sdk::trace::BatchSpanProcessor
//! [Tokio]: https://tokio.rs
//! [async-std]: https://async.rs
//!
//! ## Managing Active Spans
//!
//! Spans can be marked as "active" for a given [`Context`], and all newly
//! created spans will automatically be children of the currently active span.
//!
//! The active span for a given thread can be managed via [`get_active_span`]
//! and [`mark_span_as_active`].
//!
//! [`Context`]: crate::Context
//!
//! ```
//! # #[cfg(feature = "trace")]
//! # {
//! use opentelemetry::{global, trace::{self, Span, StatusCode, Tracer, TracerProvider}};
//!
//! fn may_error(rand: f32) {
//!     if rand < 0.5 {
//!         // Get the currently active span to record additional attributes,
//!         // status, etc.
//!         trace::get_active_span(|span| {
//!             span.set_status(StatusCode::Error, "value too small".into());
//!         });
//!     }
//! }
//!
//! // Get a tracer
//! let tracer = global::tracer("my_tracer");
//!
//! // Create a span
//! let span = tracer.start("parent_span");
//!
//! // Mark the span as active
//! let active = trace::mark_span_as_active(span);
//!
//! // Any span created here will be a child of `parent_span`...
//!
//! // Drop the guard and the span will no longer be active
//! drop(active)
//! # }
//! ```
//!
//! Additionally [`Tracer::with_span`] and [`Tracer::in_span`] can be used as shorthand to
//! simplify managing the parent context.
//!
//! ```
//! # #[cfg(feature = "trace")]
//! # {
//! use opentelemetry::{global, trace::Tracer};
//!
//! // Get a tracer
//! let tracer = global::tracer("my_tracer");
//!
//! // Use `in_span` to create a new span and mark it as the parent, dropping it
//! // at the end of the block.
//! tracer.in_span("parent_span", |cx| {
//!     // spans created here will be children of `parent_span`
//! });
//!
//! // Use `with_span` to mark a span as active for a given period.
//! let span = tracer.start("parent_span");
//! tracer.with_span(span, |cx| {
//!     // spans created here will be children of `parent_span`
//! });
//! # }
//! ```
//!
//! #### Async active spans
//!
//! Async spans can be propagated with [`TraceContextExt`] and [`FutureExt`].
//!
//! ```
//! # #[cfg(feature = "trace")]
//! # {
//! use opentelemetry::{Context, global, trace::{FutureExt, TraceContextExt, Tracer}};
//!
//! async fn some_work() { }
//!
//! // Get a tracer
//! let tracer = global::tracer("my_tracer");
//!
//! // Start a span
//! let span = tracer.start("my_span");
//!
//! // Perform some async work with this span as the currently active parent.
//! some_work().with_context(Context::current_with_span(span));
//! # }
//! ```

use ::futures::channel::{mpsc::TrySendError, oneshot::Canceled};
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt;
use std::time;
use thiserror::Error;

mod context;
pub mod noop;
mod span;
mod span_context;
mod tracer;
mod tracer_provider;

pub use self::{
    context::{get_active_span, mark_span_as_active, FutureExt, SpanRef, TraceContextExt},
    span::{Span, SpanKind, StatusCode},
    span_context::{SpanContext, SpanId, TraceFlags, TraceId, TraceState, TraceStateError},
    tracer::{SpanBuilder, Tracer},
    tracer_provider::TracerProvider,
};
use crate::{sdk::export::ExportError, KeyValue};

/// Describe the result of operations in tracing API.
pub type TraceResult<T> = Result<T, TraceError>;

/// Errors returned by the trace API.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TraceError {
    /// Export failed with the error returned by the exporter
    #[error("Exporter {} encountered the following error(s): {0}", .0.exporter_name())]
    ExportFailed(Box<dyn ExportError>),

    /// Export failed to finish after certain period and processor stopped the export.
    #[error("Exporting timed out after {} seconds", .0.as_secs())]
    ExportTimedOut(time::Duration),

    /// Other errors propagated from trace SDK that weren't covered above
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}

impl<T> From<T> for TraceError
where
    T: ExportError,
{
    fn from(err: T) -> Self {
        TraceError::ExportFailed(Box::new(err))
    }
}

impl<T> From<TrySendError<T>> for TraceError {
    fn from(err: TrySendError<T>) -> Self {
        TraceError::Other(Box::new(err.into_send_error()))
    }
}

impl From<Canceled> for TraceError {
    fn from(err: Canceled) -> Self {
        TraceError::Other(Box::new(err))
    }
}

impl From<String> for TraceError {
    fn from(err_msg: String) -> Self {
        TraceError::Other(Box::new(Custom(err_msg)))
    }
}

impl From<&'static str> for TraceError {
    fn from(err_msg: &'static str) -> Self {
        TraceError::Other(Box::new(Custom(err_msg.into())))
    }
}

/// Wrap type for string
#[derive(Error, Debug)]
#[error("{0}")]
struct Custom(String);

/// Interface for generating IDs
pub trait IdGenerator: Send + Sync + fmt::Debug {
    /// Generate a new `TraceId`
    fn new_trace_id(&self) -> TraceId;

    /// Generate a new `SpanId`
    fn new_span_id(&self) -> SpanId;
}

/// A `Span` has the ability to add events. Events have a time associated
/// with the moment when they are added to the `Span`.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Event name
    pub name: Cow<'static, str>,
    /// Event timestamp
    pub timestamp: time::SystemTime,
    /// Event attributes
    pub attributes: Vec<KeyValue>,
    /// Number of dropped attributes
    pub dropped_attributes_count: u32,
}

impl Event {
    /// Create new `Event`
    pub fn new<T: Into<Cow<'static, str>>>(
        name: T,
        timestamp: time::SystemTime,
        attributes: Vec<KeyValue>,
        dropped_attributes_count: u32,
    ) -> Self {
        Event {
            name: name.into(),
            timestamp,
            attributes,
            dropped_attributes_count,
        }
    }

    /// Create new `Event` with a given name.
    pub fn with_name<T: Into<Cow<'static, str>>>(name: T) -> Self {
        Event {
            name: name.into(),
            timestamp: crate::time::now(),
            attributes: Vec::new(),
            dropped_attributes_count: 0,
        }
    }
}

/// During the `Span` creation user MUST have the ability to record links to other `Span`s. Linked
/// `Span`s can be from the same or a different trace.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    span_context: SpanContext,
    pub(crate) attributes: Vec<KeyValue>,
    pub(crate) dropped_attributes_count: u32,
}

impl Link {
    /// Create a new link
    pub fn new(span_context: SpanContext, attributes: Vec<KeyValue>) -> Self {
        Link {
            span_context,
            attributes,
            dropped_attributes_count: 0,
        }
    }

    /// The span context of the linked span
    pub fn span_context(&self) -> &SpanContext {
        &self.span_context
    }

    /// Attributes of the span link
    pub fn attributes(&self) -> &Vec<KeyValue> {
        &self.attributes
    }

    /// Dropped attributes count
    pub fn dropped_attributes_count(&self) -> u32 {
        self.dropped_attributes_count
    }
}
