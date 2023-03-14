//! # OpenTelemetry Tracing API.
//!
//! The tracing API consists of a few main traits:
//!
//! * The `Tracer` trait which describes all tracing operations.
//! * The `Span` trait with is a mutable object storing information about the
//! current operation execution.
//! * The `SpanContext` struct is the portion of a `Span` which must be
//! serialized and propagated along side of a distributed context
//!
//! ## Tracer
//!
//! The OpenTelemetry library achieves in-process context propagation of `Span`s by
//! way of the `Tracer`.
//!
//! The `Tracer` is responsible for tracking the currently active `Span`, and
//! exposes methods for creating and activating new `Span`s. The `Tracer` is
//! configured with `Propagator`s which support transferring span context across
//! process boundaries.
//!
//! `Tracer`s are generally expected to be used as singletons. Implementations
//! SHOULD provide a single global default `Tracer`.
//!
//! Some applications may require multiple `Tracer` instances, e.g. to create
//! `Span`s on behalf of other applications. Implementations MAY provide a global
//! registry of `Tracer`s for such applications.
//!
//! ## Span
//!
//! A `Span` represents a single operation within a trace. Spans can be nested to
//! form a trace tree. Each trace contains a root span, which typically describes
//! the end-to-end latency and, optionally, one or more sub-spans for its
//! sub-operations.
//!
//! `Span`s encapsulate:
//!
//! - The span name
//! - An immutable `SpanContext` that uniquely identifies the `Span`
//! - A parent span in the form of a `SpanContext`, or None
//! - A start timestamp
//! - An end timestamp
//! - An ordered mapping of `Attribute`s
//! - A list of `Link`s to other `Span`s
//! - A list of timestamped `Event`s
//! - A `Status`.
//!
//! The _span name_ is a human-readable string which concisely identifies the work
//! represented by the `Span`, for example, an RPC method name, a function name,
//! or the name of a subtask or stage within a larger computation. The span name
//! should be the most general string that identifies a (statistically) interesting
//! _class of Spans_, rather than individual Span instances. That is, "get_user" is
//! a reasonable name, while "get_user/314159", where "314159" is a user ID, is not
//! a good name due to its high cardinality.
//!
//! For example, here are potential span names for an endpoint that gets a
//! hypothetical account information:
//!
//! | Span Name                 | Guidance     |
//! | ------------------------- | ------------ |
//! | `get`                     | Too general  |
//! | `get_account/42`          | Too specific |
//! | `get_account`             | Good, and account_id=42 would make a nice Span attribute |
//! | `get_account/{accountId}` | Also good (using the "HTTP route") |
//!
//! The `Span`'s start and end timestamps reflect the elapsed real time of the
//! operation. A `Span`'s start time SHOULD be set to the current time on span
//! creation. After the `Span` is created, it SHOULD be possible to
//! change the its name, set its `Attribute`s, and add `Link`s and `Event`s. These
//! MUST NOT be changed after the `Span`'s end time has been set.
//!
//! `Span`s are not meant to be used to propagate information within a process. To
//! prevent misuse, implementations SHOULD NOT provide access to a `Span`'s
//! attributes besides its `SpanContext`.
//!
//! Vendors may implement the `Span` interface to effect vendor-specific logic.
//! However, alternative implementations MUST NOT allow callers to create `Span`s
//! directly. All `Span`s MUST be created via a `Tracer`.
//!
//! ## SpanContext
//!
//! A `SpanContext` represents the portion of a `Span` which must be serialized and
//! propagated along side of a distributed context. `SpanContext`s are immutable.
//! `SpanContext`.
//!
//! The OpenTelemetry `SpanContext` representation conforms to the [w3c TraceContext
//! specification](https://www.w3.org/TR/trace-context/). It contains two
//! identifiers - a `TraceId` and a `SpanId` - along with a set of common
//! `TraceFlags` and system-specific `TraceState` values.
//!
//! `TraceId` A valid trace identifier is a non-zero `u128`
//!
//! `SpanId` A valid span identifier is a non-zero `u64` byte.
//!
//! `TraceFlags` contain details about the trace. Unlike Tracestate values,
//! TraceFlags are present in all traces. Currently, the only `TraceFlags` is a
//! boolean `sampled`
//! [flag](https://www.w3.org/TR/trace-context/#trace-flags).
//!
//! `Tracestate` carries system-specific configuration data, represented as a list
//! of key-value pairs. TraceState allows multiple tracing systems to participate in
//! the same trace.
//!
//! `IsValid` is a boolean flag which returns true if the SpanContext has a non-zero
//! TraceID and a non-zero SpanID.
//!
//! `IsRemote` is a boolean flag which returns true if the SpanContext was propagated
//! from a remote parent. When creating children from remote spans, their IsRemote
//! flag MUST be set to false.
//!
//! Please review the W3C specification for details on the [Tracestate
//! field](https://www.w3.org/TR/trace-context/#tracestate-field).
//!
use ::futures::channel::{mpsc::TrySendError, oneshot::Canceled};
use thiserror::Error;

mod context;
mod event;
mod futures;
mod id_generator;
mod link;
mod noop;
mod provider;
mod span;
mod span_context;
mod tracer;

pub use self::{
    context::{get_active_span, mark_span_as_active, TraceContextExt},
    event::Event,
    futures::FutureExt,
    id_generator::IdGenerator,
    link::Link,
    noop::{NoopSpan, NoopSpanExporter, NoopTracer, NoopTracerProvider},
    provider::TracerProvider,
    span::{Span, SpanKind, StatusCode},
    span_context::{
        SpanContext, SpanId, TraceId, TraceState, TRACE_FLAG_DEBUG, TRACE_FLAG_DEFERRED,
        TRACE_FLAG_NOT_SAMPLED, TRACE_FLAG_SAMPLED,
    },
    tracer::{SpanBuilder, Tracer},
};
use crate::sdk::export::ExportError;
use std::time;

/// Describe the result of operations in tracing API.
pub type TraceResult<T> = Result<T, TraceError>;

/// Errors returned by the trace API.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum TraceError {
    /// Export failed with the error returned by the exporter
    #[error("Exporter {} failed with {0}", .0.exporter_name())]
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
