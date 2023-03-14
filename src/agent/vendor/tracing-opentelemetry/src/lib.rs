//! # Tracing OpenTelemetry
//!
//! [`tracing`] is a framework for instrumenting Rust programs to collect
//! structured, event-based diagnostic information. This crate provides a layer
//! that connects spans from multiple systems into a trace and emits them to
//! [OpenTelemetry]-compatible distributed tracing systems for processing and
//! visualization.
//!
//! [OpenTelemetry]: https://opentelemetry.io
//! [`tracing`]: https://github.com/tokio-rs/tracing
//!
//! *Compiler support: [requires `rustc` 1.42+][msrv]*
//!
//! [msrv]: #supported-rust-versions
//!
//! ### Special Fields
//!
//! Fields with an `otel.` prefix are reserved for this crate and have specific
//! meaning. They are treated as ordinary fields by other layers. The current
//! special fields are:
//!
//! * `otel.name`: Override the span name sent to OpenTelemetry exporters.
//! Setting this field is useful if you want to display non-static information
//! in your span name.
//! * `otel.kind`: Set the span kind to one of the supported OpenTelemetry [span kinds].
//! * `otel.status_code`: Set the span status code to one of the supported OpenTelemetry [span status codes].
//! * `otel.status_message`: Set the span status message.
//!
//! [span kinds]: https://docs.rs/opentelemetry/latest/opentelemetry/trace/enum.SpanKind.html
//! [span status codes]: https://docs.rs/opentelemetry/latest/opentelemetry/trace/enum.StatusCode.html
//!
//! ### Semantic Conventions
//!
//! OpenTelemetry defines conventional names for attributes of common
//! operations. These names can be assigned directly as fields, e.g.
//! `trace_span!("request", "otel.kind" = %SpanKind::Client, "http.url" = ..)`, and they
//! will be passed through to your configured OpenTelemetry exporter. You can
//! find the full list of the operations and their expected field names in the
//! [semantic conventions] spec.
//!
//! [semantic conventions]: https://github.com/open-telemetry/opentelemetry-specification/tree/master/specification/trace/semantic_conventions
//!
//! ### Stability Status
//!
//! The OpenTelemetry specification is currently in beta so some breaking
//! changes may still occur on the path to 1.0. You can follow the changes via
//! the [spec repository] to track progress toward stabilization.
//!
//! [spec repository]: https://github.com/open-telemetry/opentelemetry-specification
//!
//! ## Examples
//!
//! ```
//! use opentelemetry::sdk::export::trace::stdout;
//! use tracing::{error, span};
//! use tracing_subscriber::layer::SubscriberExt;
//! use tracing_subscriber::Registry;
//!
//! // Create a new OpenTelemetry pipeline
//! let tracer = stdout::new_pipeline().install_simple();
//!
//! // Create a tracing layer with the configured tracer
//! let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
//!
//! // Use the tracing subscriber `Registry`, or any other subscriber
//! // that impls `LookupSpan`
//! let subscriber = Registry::default().with(telemetry);
//!
//! // Trace executed code
//! tracing::subscriber::with_default(subscriber, || {
//!     // Spans will be sent to the configured OpenTelemetry exporter
//!     let root = span!(tracing::Level::TRACE, "app_start", work_units = 2);
//!     let _enter = root.enter();
//!
//!     error!("This event will be logged in the root span.");
//! });
//! ```
//!
//! ## Supported Rust Versions
//!
//! Tracing is built against the latest stable release. The minimum supported
//! version is 1.42. The current Tracing version is not guaranteed to build on
//! Rust versions earlier than the minimum supported version.
//!
//! Tracing follows the same compiler support policies as the rest of the Tokio
//! project. The current stable Rust compiler and the three most recent minor
//! versions before it will always be supported. For example, if the current
//! stable compiler version is 1.45, the minimum supported version will not be
//! increased past 1.42, three minor versions prior. Increasing the minimum
//! supported compiler version is not considered a semver breaking change as
//! long as doing so complies with this policy.
//!
#![deny(unreachable_pub)]
#![cfg_attr(test, deny(warnings))]
#![doc(html_root_url = "https://docs.rs/tracing-opentelemetry/0.13.0")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/logo-type.png",
    issue_tracker_base_url = "https://github.com/tokio-rs/tracing/issues/"
)]
#![cfg_attr(docsrs, deny(broken_intra_doc_links))]

/// Implementation of the trace::Layer as a source of OpenTelemetry data.
mod layer;
/// Span extension which enables OpenTelemetry context management.
mod span_ext;
/// Protocols for OpenTelemetry Tracers that are compatible with Tracing
mod tracer;

pub use layer::{layer, OpenTelemetryLayer};
pub use span_ext::OpenTelemetrySpanExt;
pub use tracer::PreSampledTracer;
