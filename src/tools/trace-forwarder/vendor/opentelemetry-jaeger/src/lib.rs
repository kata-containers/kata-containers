//! Collects OpenTelemetry spans and reports them to a given Jaeger
//! `agent` or `collector` endpoint. See the [Jaeger Docs] for details
//! about Jaeger and deployment information.
//!
//! *Compiler support: [requires `rustc` 1.46+][msrv]*
//!
//! [Jaeger Docs]: https://www.jaegertracing.io/docs/
//! [msrv]: #supported-rust-versions
//!
//! ### Quickstart
//!
//! First make sure you have a running version of the Jaeger instance
//! you want to send data to:
//!
//! ```shell
//! $ docker run -d -p6831:6831/udp -p6832:6832/udp -p16686:16686 -p14268:14268 jaegertracing/all-in-one:latest
//! ```
//!
//! Then install a new jaeger pipeline with the recommended defaults to start
//! exporting telemetry:
//!
//! ```no_run
//! use opentelemetry::trace::Tracer;
//! use opentelemetry::global;
//!
//! fn main() -> Result<(), opentelemetry::trace::TraceError> {
//!     global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());
//!     let tracer = opentelemetry_jaeger::new_pipeline().install_simple()?;
//!
//!     tracer.in_span("doing_work", |cx| {
//!         // Traced app logic here...
//!     });
//!
//!     global::shutdown_tracer_provider(); // export remaining spans
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Performance
//!
//! For optimal performance, a batch exporter is recommended as the simple exporter
//! will export each span synchronously on drop. You can enable the [`rt-tokio`],
//! [`rt-tokio-current-thread`] or [`rt-async-std`] features and specify a runtime
//! on the pipeline builder to have a batch exporter configured for you
//! automatically.
//!
//! ```toml
//! [dependencies]
//! opentelemetry = { version = "*", features = ["rt-tokio"] }
//! opentelemetry-jaeger = { version = "*", features = ["tokio"] }
//! ```
//!
//! ```no_run
//! # fn main() -> Result<(), opentelemetry::trace::TraceError> {
//! let tracer = opentelemetry_jaeger::new_pipeline()
//!     .install_batch(opentelemetry::runtime::Tokio)?;
//! # Ok(())
//! # }
//! ```
//!
//! [`tokio`]: https://tokio.rs
//! [`async-std`]: https://async.rs
//!
//! ### Jaeger Exporter From Environment Variables
//!
//! The jaeger pipeline builder can be configured dynamically via environment
//! variables. All variables are optional, a full list of accepted options can
//! be found in the [jaeger variables spec].
//!
//! [jaeger variables spec]: https://github.com/open-telemetry/opentelemetry-specification/blob/master/specification/sdk-environment-variables.md#jaeger-exporter
//!
//! ### Jaeger Collector Example
//!
//! If you want to skip the agent and submit spans directly to a Jaeger collector,
//! you can enable the optional `collector_client` feature for this crate. This
//! example expects a Jaeger collector running on `http://localhost:14268`.
//!
//! ```toml
//! [dependencies]
//! opentelemetry-jaeger = { version = "..", features = ["collector_client", "isahc"] }
//! ```
//!
//! Then you can use the [`with_collector_endpoint`] method to specify the endpoint:
//!
//! [`with_collector_endpoint`]: PipelineBuilder::with_collector_endpoint()
//!
//! ```ignore
//! // Note that this requires the `collector_client` feature.
//! // We enabled the `isahc` feature for a default isahc http client.
//! // You can also provide your own implementation via new_pipeline().with_http_client() method.
//! use opentelemetry::trace::{Tracer, TraceError};
//!
//! fn main() -> Result<(), TraceError> {
//!     let tracer = opentelemetry_jaeger::new_pipeline()
//!         .with_collector_endpoint("http://localhost:14268/api/traces")
//!         // optionally set username and password as well.
//!         .with_collector_username("username")
//!         .with_collector_password("s3cr3t")
//!         .install_simple()?;
//!
//!     tracer.in_span("doing_work", |cx| {
//!         // Traced app logic here...
//!     });
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Kitchen Sink Full Configuration
//!
//! Example showing how to override all configuration options. See the
//! [`PipelineBuilder`] docs for details of each option.
//!
//!
//! ```no_run
//! use opentelemetry::{KeyValue, trace::{Tracer, TraceError}};
//! use opentelemetry::sdk::{trace::{self, IdGenerator, Sampler}, Resource};
//! use opentelemetry::global;
//!
//! fn main() -> Result<(), TraceError> {
//!     global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());
//!     let tracer = opentelemetry_jaeger::new_pipeline()
//!         .with_agent_endpoint("localhost:6831")
//!         .with_service_name("my_app")
//!         .with_tags(vec![KeyValue::new("process_key", "process_value")])
//!         .with_max_packet_size(9_216)
//!         .with_trace_config(
//!             trace::config()
//!                 .with_sampler(Sampler::AlwaysOn)
//!                 .with_id_generator(IdGenerator::default())
//!                 .with_max_events_per_span(64)
//!                 .with_max_attributes_per_span(16)
//!                 .with_max_events_per_span(16)
//!                 .with_resource(Resource::new(vec![KeyValue::new("key", "value")])),
//!         )
//!         .install_batch(opentelemetry::runtime::Tokio)?;
//!
//!     tracer.in_span("doing_work", |cx| {
//!         // Traced app logic here...
//!     });
//!
//!     global::shutdown_tracer_provider(); // export remaining spans
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Crate Feature Flags
//!
//! The following crate feature flags are available:
//!
//! * `collector_client`: Export span data directly to a Jaeger collector. User MUST provide the http client.
//!
//! * `surf_collector_client`: Export span data with Jaeger collector backed by a surf default http client.
//!
//! * `reqwest_collector_client`: Export span data with Jaeger collector backed by a reqwest http client.
//!
//! * `reqwest_blocking_collector_client`: Export span data with Jaeger collector backed by a reqwest blocking http client.
//!
//! * `isahc_collector_client`: Export span data with Jaeger collector backed by a isahc http client.
//!
//! * `wasm_collector_client`: Enable collector in wasm.
//!
//! ## Supported Rust Versions
//!
//! OpenTelemetry is built against the latest stable release. The minimum
//! supported version is 1.46. The current OpenTelemetry version is not
//! guaranteed to build on Rust versions earlier than the minimum supported
//! version.
//!
//! The current stable Rust compiler and the three most recent minor versions
//! before it will always be supported. For example, if the current stable
//! compiler version is 1.49, the minimum supported version will not be
//! increased past 1.46, three minor versions prior. Increasing the minimum
//! supported compiler version is not considered a semver breaking change as
//! long as doing so complies with this policy.
#![warn(
    future_incompatible,
    missing_debug_implementations,
    missing_docs,
    nonstandard_style,
    rust_2018_idioms,
    unreachable_pub,
    unused
)]
#![cfg_attr(docsrs, feature(doc_cfg), deny(broken_intra_doc_links))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/open-telemetry/opentelemetry-rust/main/assets/logo.svg"
)]
#![cfg_attr(test, deny(warnings))]

mod exporter;
mod propagator {
    //! # Jaeger Propagator
    //!
    //! Extract and inject values from Jaeger's `uber-trace-id` header.
    //!
    //! See [`Jaeger documentation`] for detail of Jaeger propagation format.
    //!
    //! [`Jaeger documentation`]: https://www.jaegertracing.io/docs/1.18/client-libraries/#propagation-format
    use opentelemetry::{
        propagation::{text_map_propagator::FieldIter, Extractor, Injector, TextMapPropagator},
        trace::{
            SpanContext, SpanId, TraceContextExt, TraceId, TraceState, TRACE_FLAG_DEBUG,
            TRACE_FLAG_NOT_SAMPLED, TRACE_FLAG_SAMPLED,
        },
        Context,
    };
    use std::borrow::Cow;
    use std::str::FromStr;

    const JAEGER_HEADER: &str = "uber-trace-id";
    const JAEGER_BAGGAGE_PREFIX: &str = "uberctx-";
    const DEPRECATED_PARENT_SPAN: &str = "0";

    lazy_static::lazy_static! {
        static ref JAEGER_HEADER_FIELD: [String; 1] = [JAEGER_HEADER.to_string()];
    }

    /// The Jaeger propagator propagates span contexts in jaeger's propagation format.
    ///
    /// See [`Jaeger documentation`] for format details.
    ///
    /// Note that jaeger header can be set in http header or encoded as url
    ///
    ///  [`Jaeger documentation`]: https://www.jaegertracing.io/docs/1.18/client-libraries/#propagation-format
    #[derive(Clone, Debug)]
    pub struct Propagator {
        _private: (),
    }

    impl Default for Propagator {
        fn default() -> Self {
            Propagator { _private: () }
        }
    }

    impl Propagator {
        /// Create a Jaeger propagator
        pub fn new() -> Self {
            Propagator::default()
        }

        /// Extract span context from header value
        fn extract_span_context(&self, extractor: &dyn Extractor) -> Result<SpanContext, ()> {
            let mut header_value = Cow::from(extractor.get(JAEGER_HEADER).unwrap_or(""));
            // if there is no :, it means header_value could be encoded as url, try decode first
            if !header_value.contains(':') {
                header_value = Cow::from(header_value.replace("%3A", ":"));
            }

            let parts = header_value.split_terminator(':').collect::<Vec<&str>>();
            if parts.len() != 4 {
                return Err(());
            }

            // extract trace id
            let trace_id = self.extract_trace_id(parts[0])?;
            let span_id = self.extract_span_id(parts[1])?;
            // Ignore parent span id since it's deprecated.
            let flag = self.extract_flag(parts[3])?;
            let trace_state = self.extract_trace_state(extractor)?;

            Ok(SpanContext::new(trace_id, span_id, flag, true, trace_state))
        }

        /// Extract trace id from the header.
        fn extract_trace_id(&self, trace_id: &str) -> Result<TraceId, ()> {
            if trace_id.len() > 32 {
                return Err(());
            }

            // allow variable length, padding 0 when length is less than 32
            let padded_trace_id = format!("{:0>32}", trace_id);

            u128::from_str_radix(padded_trace_id.as_str(), 16)
                .map(TraceId::from_u128)
                .map_err(|_| ())
        }

        /// Extract span id from the header.
        fn extract_span_id(&self, span_id: &str) -> Result<SpanId, ()> {
            if span_id.len() != 16 {
                return Err(());
            }

            u64::from_str_radix(span_id, 16)
                .map(SpanId::from_u64)
                .map_err(|_| ())
        }

        /// Extract flag from the header
        ///
        /// First bit control whether to sample
        /// Second bit control whether it's a debug trace
        /// Third bit is not used.
        /// Forth bit is firehose flag, which is not supported in OT now.
        fn extract_flag(&self, flag: &str) -> Result<u8, ()> {
            if flag.len() > 2 {
                return Err(());
            }
            let flag = u8::from_str(flag).map_err(|_| ())?;
            if flag & 0x01 == 0x01 {
                if flag & 0x02 == 0x02 {
                    Ok(TRACE_FLAG_SAMPLED | TRACE_FLAG_DEBUG)
                } else {
                    Ok(TRACE_FLAG_SAMPLED)
                }
            } else {
                // Debug flag should only be set when sampled flag is set.
                // So if debug flag is set alone. We will just use not sampled flag
                Ok(TRACE_FLAG_NOT_SAMPLED)
            }
        }

        fn extract_trace_state(&self, extractor: &dyn Extractor) -> Result<TraceState, ()> {
            let uber_context_keys = extractor
                .keys()
                .into_iter()
                .filter(|key| key.starts_with(JAEGER_BAGGAGE_PREFIX))
                .filter_map(|key| {
                    extractor
                        .get(key)
                        .map(|value| (key.to_string(), value.to_string()))
                });

            TraceState::from_key_value(uber_context_keys)
        }
    }

    impl TextMapPropagator for Propagator {
        fn inject_context(&self, cx: &Context, injector: &mut dyn Injector) {
            let span = cx.span();
            let span_context = span.span_context();
            if span_context.is_valid() {
                let flag: u8 = if span_context.is_sampled() {
                    if span_context.is_debug() {
                        0x03
                    } else {
                        0x01
                    }
                } else {
                    0x00
                };
                let header_value = format!(
                    "{:032x}:{:016x}:{:01}:{:01}",
                    span_context.trace_id().to_u128(),
                    span_context.span_id().to_u64(),
                    DEPRECATED_PARENT_SPAN,
                    flag,
                );
                injector.set(JAEGER_HEADER, header_value);
            }
        }

        fn extract_with_context(&self, cx: &Context, extractor: &dyn Extractor) -> Context {
            cx.with_remote_span_context(
                self.extract_span_context(extractor)
                    .unwrap_or_else(|_| SpanContext::empty_context()),
            )
        }

        fn fields(&self) -> FieldIter<'_> {
            FieldIter::new(JAEGER_HEADER_FIELD.as_ref())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use opentelemetry::{
            propagation::{Injector, TextMapPropagator},
            testing::trace::TestSpan,
            trace::{
                SpanContext, SpanId, TraceContextExt, TraceId, TraceState, TRACE_FLAG_DEBUG,
                TRACE_FLAG_NOT_SAMPLED, TRACE_FLAG_SAMPLED,
            },
            Context,
        };
        use std::collections::HashMap;

        const LONG_TRACE_ID_STR: &str = "000000000000004d0000000000000016";
        const SHORT_TRACE_ID_STR: &str = "4d0000000000000016";
        const TRACE_ID: u128 = 0x0000_0000_0000_004d_0000_0000_0000_0016;
        const SPAN_ID_STR: &str = "0000000000017c29";
        const SPAN_ID: u64 = 0x0000_0000_0001_7c29;

        fn get_extract_data() -> Vec<(&'static str, &'static str, u8, SpanContext)> {
            vec![
                (
                    LONG_TRACE_ID_STR,
                    SPAN_ID_STR,
                    1,
                    SpanContext::new(
                        TraceId::from_u128(TRACE_ID),
                        SpanId::from_u64(SPAN_ID),
                        TRACE_FLAG_SAMPLED,
                        true,
                        TraceState::default(),
                    ),
                ),
                (
                    SHORT_TRACE_ID_STR,
                    SPAN_ID_STR,
                    1,
                    SpanContext::new(
                        TraceId::from_u128(TRACE_ID),
                        SpanId::from_u64(SPAN_ID),
                        TRACE_FLAG_SAMPLED,
                        true,
                        TraceState::default(),
                    ),
                ),
                (
                    LONG_TRACE_ID_STR,
                    SPAN_ID_STR,
                    3,
                    SpanContext::new(
                        TraceId::from_u128(TRACE_ID),
                        SpanId::from_u64(SPAN_ID),
                        TRACE_FLAG_DEBUG | TRACE_FLAG_SAMPLED,
                        true,
                        TraceState::default(),
                    ),
                ),
                (
                    LONG_TRACE_ID_STR,
                    SPAN_ID_STR,
                    0,
                    SpanContext::new(
                        TraceId::from_u128(TRACE_ID),
                        SpanId::from_u64(SPAN_ID),
                        TRACE_FLAG_NOT_SAMPLED,
                        true,
                        TraceState::default(),
                    ),
                ),
                (
                    "invalidtractid",
                    SPAN_ID_STR,
                    0,
                    SpanContext::empty_context(),
                ),
                (
                    LONG_TRACE_ID_STR,
                    "invalidspanID",
                    0,
                    SpanContext::empty_context(),
                ),
                (
                    LONG_TRACE_ID_STR,
                    SPAN_ID_STR,
                    120,
                    SpanContext::empty_context(),
                ),
            ]
        }

        fn get_inject_data() -> Vec<(SpanContext, String)> {
            vec![
                (
                    SpanContext::new(
                        TraceId::from_u128(TRACE_ID),
                        SpanId::from_u64(SPAN_ID),
                        TRACE_FLAG_SAMPLED,
                        true,
                        TraceState::default(),
                    ),
                    format!("{}:{}:0:1", LONG_TRACE_ID_STR, SPAN_ID_STR),
                ),
                (
                    SpanContext::new(
                        TraceId::from_u128(TRACE_ID),
                        SpanId::from_u64(SPAN_ID),
                        TRACE_FLAG_NOT_SAMPLED,
                        true,
                        TraceState::default(),
                    ),
                    format!("{}:{}:0:0", LONG_TRACE_ID_STR, SPAN_ID_STR),
                ),
                (
                    SpanContext::new(
                        TraceId::from_u128(TRACE_ID),
                        SpanId::from_u64(SPAN_ID),
                        TRACE_FLAG_DEBUG | TRACE_FLAG_SAMPLED,
                        true,
                        TraceState::default(),
                    ),
                    format!("{}:{}:0:3", LONG_TRACE_ID_STR, SPAN_ID_STR),
                ),
            ]
        }

        #[test]
        fn test_extract_empty() {
            let map: HashMap<String, String> = HashMap::new();
            let propagator = Propagator::new();
            let context = propagator.extract(&map);
            assert_eq!(context.span().span_context(), &SpanContext::empty_context())
        }

        #[test]
        fn test_extract() {
            for (trace_id, span_id, flag, expected) in get_extract_data() {
                let mut map: HashMap<String, String> = HashMap::new();
                map.set(
                    JAEGER_HEADER,
                    format!("{}:{}:0:{}", trace_id, span_id, flag),
                );
                let propagator = Propagator::new();
                let context = propagator.extract(&map);
                assert_eq!(context.span().span_context(), &expected);
            }
        }

        #[test]
        fn test_extract_too_many_parts() {
            let mut map: HashMap<String, String> = HashMap::new();
            map.set(
                JAEGER_HEADER,
                format!("{}:{}:0:1:aa", LONG_TRACE_ID_STR, SPAN_ID_STR),
            );
            let propagator = Propagator::new();
            let context = propagator.extract(&map);
            assert_eq!(context.span().span_context(), &SpanContext::empty_context());
        }

        #[test]
        fn test_extract_invalid_flag() {
            let mut map: HashMap<String, String> = HashMap::new();
            map.set(
                JAEGER_HEADER,
                format!("{}:{}:0:aa", LONG_TRACE_ID_STR, SPAN_ID_STR),
            );
            let propagator = Propagator::new();
            let context = propagator.extract(&map);
            assert_eq!(context.span().span_context(), &SpanContext::empty_context());
        }

        #[test]
        fn test_extract_from_url() {
            let mut map: HashMap<String, String> = HashMap::new();
            map.set(
                JAEGER_HEADER,
                format!("{}%3A{}%3A0%3A1", LONG_TRACE_ID_STR, SPAN_ID_STR),
            );
            let propagator = Propagator::new();
            let context = propagator.extract(&map);
            assert_eq!(
                context.span().span_context(),
                &SpanContext::new(
                    TraceId::from_u128(TRACE_ID),
                    SpanId::from_u64(SPAN_ID),
                    1,
                    true,
                    TraceState::default(),
                )
            );
        }

        #[test]
        fn test_inject() {
            let propagator = Propagator::new();
            for (span_context, header_value) in get_inject_data() {
                let mut injector = HashMap::new();
                propagator.inject_context(
                    &Context::current_with_span(TestSpan(span_context)),
                    &mut injector,
                );
                assert_eq!(injector.get(JAEGER_HEADER), Some(&header_value));
            }
        }
    }
}

pub use exporter::{new_pipeline, Error, Exporter, PipelineBuilder, Process};
pub use propagator::Propagator;
