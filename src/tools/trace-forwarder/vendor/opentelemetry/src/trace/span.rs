//! # OpenTelemetry Span interface
//!
//! A `Span` represents a single operation within a trace. `Span`s can be nested to form a trace
//! tree. Each trace contains a root span, which typically describes the end-to-end latency and,
//! optionally, one or more sub-spans for its sub-operations.
//!
//! The `Span`'s start and end timestamps reflect the elapsed real time of the operation. A `Span`'s
//! start time SHOULD be set to the current time on span creation. After the `Span` is created, it
//! SHOULD be possible to change its name, set its `Attributes`, and add `Links` and `Events`.
//! These MUST NOT be changed after the `Span`'s end time has been set.
//!
//! `Spans` are not meant to be used to propagate information within a process. To prevent misuse,
//! implementations SHOULD NOT provide access to a `Span`'s attributes besides its `SpanContext`.
//!
//! Vendors may implement the `Span` interface to effect vendor-specific logic. However, alternative
//! implementations MUST NOT allow callers to create Spans directly. All `Span`s MUST be created
//! via a Tracer.
use crate::{trace::SpanContext, KeyValue};
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::time::SystemTime;

/// Interface for a single operation within a trace.
pub trait Span: fmt::Debug {
    /// An API to record events in the context of a given `Span`.
    ///
    /// Events have a time associated with the moment when they are
    /// added to the `Span`.
    ///
    /// Events SHOULD preserve the order in which they're set. This will typically match
    /// the ordering of the events' timestamps.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard event names and
    /// keys"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// which have prescribed semantic meanings.
    fn add_event(&mut self, name: String, attributes: Vec<KeyValue>) {
        self.add_event_with_timestamp(name, crate::time::now(), attributes)
    }

    /// Convenience method to record an exception/error as an `Event`
    ///
    /// An exception SHOULD be recorded as an Event on the span during which it occurred.
    /// The name of the event MUST be "exception".
    ///
    /// The semantic conventions for Errors are described in ["Semantic Conventions for Exceptions"](https://github.com/open-telemetry/opentelemetry-specification/blob/master/specification/trace/semantic_conventions/exceptions.md)
    ///
    /// For now we will not set `exception.stacktrace` attribute since the `Error::backtrace`
    /// method is still in nightly. Users can provide a stacktrace by using the
    /// `record_exception_with_stacktrace` method.
    ///
    /// Users can custom the exception message by overriding the `fmt::Display` trait's `fmt` method
    /// for the error.
    fn record_exception(&mut self, err: &dyn Error) {
        let attributes = vec![KeyValue::new("exception.message", err.to_string())];

        self.add_event("exception".to_string(), attributes);
    }

    /// Convenience method to record a exception/error as an `Event` with custom stacktrace
    ///
    /// See `Span:record_exception` method for more details.
    fn record_exception_with_stacktrace(&mut self, err: &dyn Error, stacktrace: String) {
        let attributes = vec![
            KeyValue::new("exception.message", err.to_string()),
            KeyValue::new("exception.stacktrace", stacktrace),
        ];

        self.add_event("exception".to_string(), attributes);
    }

    /// An API to record events at a specific time in the context of a given `Span`.
    ///
    /// Events SHOULD preserve the order in which they're set. This will typically match
    /// the ordering of the events' timestamps.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard event names and
    /// keys"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// which have prescribed semantic meanings.
    fn add_event_with_timestamp(
        &mut self,
        name: String,
        timestamp: SystemTime,
        attributes: Vec<KeyValue>,
    );

    /// Returns the `SpanContext` for the given `Span`. The returned value may be used even after
    /// the `Span is finished. The returned value MUST be the same for the entire `Span` lifetime.
    fn span_context(&self) -> &SpanContext;

    /// Returns true if this `Span` is recording information like events with the `add_event`
    /// operation, attributes using `set_attributes`, status with `set_status`, etc.
    ///
    /// This flag SHOULD be used to avoid expensive computations of a `Span` attributes or events in
    /// case when a `Span` is definitely not recorded. Note that any child span's recording is
    /// determined independently from the value of this flag (typically based on the sampled flag of
    /// a `TraceFlag` on `SpanContext`).
    ///
    /// This flag may be true despite the entire trace being sampled out. This allows to record and
    /// process information about the individual Span without sending it to the backend. An example
    /// of this scenario may be recording and processing of all incoming requests for the processing
    /// and building of SLA/SLO latency charts while sending only a subset - sampled spans - to the
    /// backend. See also the sampling section of SDK design.
    ///
    /// Users of the API should only access the `is_recording` property when instrumenting code and
    /// never access `SampledFlag` unless used in context propagators.
    fn is_recording(&self) -> bool;

    /// An API to set a single `Attribute` where the attribute properties are passed
    /// as arguments. To avoid extra allocations some implementations may offer a separate API for
    /// each of the possible value types.
    ///
    /// An `Attribute` is defined as a `KeyValue` pair.
    ///
    /// Attributes SHOULD preserve the order in which they're set. Setting an attribute
    /// with the same key as an existing attribute SHOULD overwrite the existing
    /// attribute's value.
    ///
    /// Note that the OpenTelemetry project documents certain ["standard
    /// attributes"](https://github.com/open-telemetry/opentelemetry-specification/tree/v0.5.0/specification/trace/semantic_conventions/README.md)
    /// that have prescribed semantic meanings.
    fn set_attribute(&mut self, attribute: KeyValue);

    /// Sets the status of the `Span`. `message` MUST be ignored when the status is `OK` or
    /// `Unset`.
    ///
    /// The order of status is `Ok` > `Error` > `Unset`. That's means set the status
    /// to `Unset` will always be ignore, set the status to `Error` only works when current
    /// status is `Unset`, set the status to `Ok` will be consider final and any further call
    /// to this function will be ignore.
    fn set_status(&mut self, code: StatusCode, message: String);

    /// Updates the `Span`'s name. After this update, any sampling behavior based on the
    /// name will depend on the implementation.
    ///
    /// It is highly discouraged to update the name of a `Span` after its creation.
    /// `Span` name is often used to group, filter and identify the logical groups of
    /// spans. Often, filtering logic will be implemented before the `Span` creation
    /// for performance reasons, and the name update may interfere with this logic.
    ///
    /// The method name is called `update_name` to differentiate this method from the
    /// regular property. It emphasizes that this operation signifies a
    /// major change for a `Span` and may lead to re-calculation of sampling or
    /// filtering decisions made previously depending on the implementation.
    fn update_name(&mut self, new_name: String);

    /// Finishes the `Span`.
    ///
    /// Implementations MUST ignore all subsequent calls to `end` (there might be
    /// exceptions when the tracer is streaming events and has no mutable state
    /// associated with the Span).
    ///
    /// Calls to `end` a Span MUST not have any effects on child `Span`s as they may
    /// still be running and can be ended later.
    ///
    /// This API MUST be non-blocking.
    fn end(&mut self) {
        self.end_with_timestamp(crate::time::now());
    }

    /// Finishes the `Span` with given timestamp
    ///
    /// For more details, refer to [`Span::end`]
    ///
    /// [`Span::end`]: Span::end()
    fn end_with_timestamp(&mut self, timestamp: SystemTime);
}

/// `SpanKind` describes the relationship between the Span, its parents,
/// and its children in a `Trace`. `SpanKind` describes two independent
/// properties that benefit tracing systems during analysis.
///
/// The first property described by `SpanKind` reflects whether the `Span`
/// is a remote child or parent. `Span`s with a remote parent are
/// interesting because they are sources of external load. `Span`s with a
/// remote child are interesting because they reflect a non-local system
/// dependency.
///
/// The second property described by `SpanKind` reflects whether a child
/// `Span` represents a synchronous call.  When a child span is synchronous,
/// the parent is expected to wait for it to complete under ordinary
/// circumstances.  It can be useful for tracing systems to know this
/// property, since synchronous `Span`s may contribute to the overall trace
/// latency. Asynchronous scenarios can be remote or local.
///
/// In order for `SpanKind` to be meaningful, callers should arrange that
/// a single `Span` does not serve more than one purpose.  For example, a
/// server-side span should not be used directly as the parent of another
/// remote span.  As a simple guideline, instrumentation should create a
/// new `Span` prior to extracting and serializing the span context for a
/// remote call.
///
/// To summarize the interpretation of these kinds:
///
/// | `SpanKind` | Synchronous | Asynchronous | Remote Incoming | Remote Outgoing |
/// |------------|-----|-----|-----|-----|
/// | `Client`   | yes |     |     | yes |
/// | `Server`   | yes |     | yes |     |
/// | `Producer` |     | yes |     | yes |
/// | `Consumer` |     | yes | yes |     |
/// | `Internal` |     |     |     |     |
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq)]
pub enum SpanKind {
    /// Indicates that the span describes a synchronous request to
    /// some remote service.  This span is the parent of a remote `Server`
    /// span and waits for its response.
    Client,
    /// Indicates that the span covers server-side handling of a
    /// synchronous RPC or other remote request.  This span is the child of
    /// a remote `Client` span that was expected to wait for a response.
    Server,
    /// Indicates that the span describes the parent of an
    /// asynchronous request.  This parent span is expected to end before
    /// the corresponding child `Consumer` span, possibly even before the
    /// child span starts. In messaging scenarios with batching, tracing
    /// individual messages requires a new `Producer` span per message to
    /// be created.
    Producer,
    /// Indicates that the span describes the child of an
    /// asynchronous `Producer` request.
    Consumer,
    /// Default value. Indicates that the span represents an
    /// internal operation within an application, as opposed to an
    /// operations with remote parents or children.
    Internal,
}

impl fmt::Display for SpanKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpanKind::Client => write!(f, "client"),
            SpanKind::Server => write!(f, "server"),
            SpanKind::Producer => write!(f, "producer"),
            SpanKind::Consumer => write!(f, "consumer"),
            SpanKind::Internal => write!(f, "internal"),
        }
    }
}

/// The `StatusCode` interface represents the status of a finished `Span`.
/// It's composed of a canonical code in conjunction with an optional
/// descriptive message.
#[cfg_attr(feature = "serialize", derive(Deserialize, Serialize))]
#[derive(Clone, Debug, PartialEq, Copy)]
pub enum StatusCode {
    /// The default status.
    Unset,
    /// OK is returned on success.
    Ok,
    /// The operation contains an error.
    Error,
}

impl StatusCode {
    /// Return a static str that represent the status code
    pub fn as_str(&self) -> &'static str {
        match self {
            StatusCode::Unset => "",
            StatusCode::Ok => "OK",
            StatusCode::Error => "ERROR",
        }
    }

    /// Return the priority of the status code.
    /// Status code with higher priority can override the lower priority one.
    pub(crate) fn priority(&self) -> i32 {
        match self {
            StatusCode::Unset => 0,
            StatusCode::Error => 1,
            StatusCode::Ok => 2,
        }
    }
}
