//! Utilities for working with global telemetry primitives
//!
//! ## Global Trace API
//!
//! The global trace API **provides applications access to their configured
//! [`TracerProvider`] instance from anywhere in the codebase**. This allows
//! applications to be less coupled to the specific Open Telemetry SDK while not
//! manually passing references to each part of the code that needs to create
//! [`Span`]s. Additionally, **3rd party middleware** or **library code** can be
//! written against this generic API and not constrain users to a specific
//! implementation choice.
//!
//! ### Usage in Applications
//!
//! Applications configure their tracer either by [installing a trace pipeline],
//! or calling [`set_tracer_provider`].
//!
//! ```
//! # #[cfg(feature="trace")]
//! # {
//! use opentelemetry::trace::{Tracer, NoopTracerProvider};
//! use opentelemetry::global;
//!
//! fn init_tracer() {
//!     let provider = NoopTracerProvider::new();
//!
//!     // Configure the global `TracerProvider` singleton when your app starts
//!     // (there is a no-op default if this is not set by your application)
//!     let _ = global::set_tracer_provider(provider);
//! }
//!
//! fn do_something_tracked() {
//!     // Then you can get a named tracer instance anywhere in your codebase.
//!     let tracer = global::tracer("my-component");
//!
//!     tracer.in_span("doing_work", |cx| {
//!         // Traced app logic here...
//!     });
//! }
//!
//! // in main or other app start
//! let _ = init_tracer();
//! do_something_tracked();
//! # }
//! ```
//!
//! ### Usage in Libraries
//!
//! ```
//! # #[cfg(feature="trace")]
//! # {
//! use opentelemetry::trace::Tracer;
//! use opentelemetry::global;
//!
//! pub fn my_traced_library_function() {
//!     // End users of your library will configure their global tracer provider
//!     // so you can use the global tracer without any setup
//!     let tracer = global::tracer_with_version("my-library-name", env!("CARGO_PKG_VERSION"));
//!
//!     tracer.in_span("doing_library_work", |cx| {
//!         // Traced library logic here...
//!     });
//! }
//! # }
//! ```
//!
//! [installing a trace pipeline]: crate::sdk::export::trace::stdout::PipelineBuilder::install
//! [`TracerProvider`]: crate::trace::TracerProvider
//! [`Span`]: crate::trace::Span
//!
//! ## Global Metrics API
//!
//! The global metrics API **provides applications access to their configured
//! [`MeterProvider`] instance from anywhere in the codebase**. This allows
//! applications to be less coupled to the specific Open Telemetry SDK while not
//! manually passing references to each part of the code that needs to create
//! metric instruments. Additionally, **3rd party middleware** or **library code** can be
//! written against this generic API and not constrain users to a specific
//! implementation choice.
//!
//! ### Usage in Applications
//!
//! Applications configure their tracer either by [installing a metrics pipeline],
//! or calling [`set_meter_provider`].
//!
//! ```
//! # #[cfg(feature="metrics")]
//! # {
//! use opentelemetry::metrics::{Meter, noop::NoopMeterProvider};
//! use opentelemetry::{global, KeyValue};
//!
//! fn init_meter() {
//!     let provider = NoopMeterProvider::new();
//!
//!     // Configure the global `MeterProvider` singleton when your app starts
//!     // (there is a no-op default if this is not set by your application)
//!     global::set_meter_provider(provider)
//! }
//!
//! fn do_something_instrumented() {
//!     // Then you can get a named tracer instance anywhere in your codebase.
//!     let tracer = global::meter("my-component");
//!     let counter = tracer.u64_counter("my_counter").init();
//!
//!     // record metrics
//!     counter.add(1, &[KeyValue::new("mykey", "myvalue")]);
//! }
//!
//! // in main or other app start
//! init_meter();
//! do_something_instrumented();
//! # }
//! ```
//!
//! ### Usage in Libraries
//!
//! ```
//! # #[cfg(feature="metrics")]
//! # {
//! use opentelemetry::trace::Tracer;
//! use opentelemetry::{global, KeyValue};
//!
//! pub fn my_traced_library_function() {
//!     // End users of your library will configure their global meter provider
//!     // so you can use the global meter without any setup
//!     let tracer = global::meter("my-library-name");
//!     let counter = tracer.u64_counter("my_counter").init();
//!
//!     // record metrics
//!     counter.add(1, &[KeyValue::new("mykey", "myvalue")]);
//! }
//! # }
//! ```
//!
//! [installing a metrics pipeline]: crate::sdk::export::metrics::stdout::StdoutExporterBuilder::try_init
//! [`MeterProvider`]: crate::metrics::MeterProvider
//! [`set_meter_provider`]: crate::global::set_meter_provider

mod error_handler;
#[cfg(feature = "metrics")]
mod metrics;
#[cfg(feature = "trace")]
mod propagation;
#[cfg(feature = "trace")]
mod trace;

pub use error_handler::{handle_error, set_error_handler};
#[cfg(feature = "metrics")]
#[cfg_attr(docsrs, doc(cfg(feature = "metrics")))]
pub use metrics::{
    meter, meter_provider, meter_with_version, set_meter_provider, GlobalMeterProvider,
};
#[cfg(feature = "trace")]
#[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
pub use propagation::{get_text_map_propagator, set_text_map_propagator};
#[cfg(feature = "trace")]
#[cfg_attr(docsrs, doc(cfg(feature = "trace")))]
pub use trace::{
    force_flush_tracer_provider, set_tracer_provider, shutdown_tracer_provider, tracer,
    tracer_provider, tracer_with_version, BoxedSpan, BoxedTracer, GenericTracer,
    GenericTracerProvider, GlobalTracerProvider,
};
