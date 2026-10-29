// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::cmp::min;
use std::fmt::Write;
use std::sync::Arc;

use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{tonic_types::metadata::MetadataMap, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
use tokio::sync::Notify;
use tracing::{span, Span, Subscriber};
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Registry;
use tracing_subscriber::{
    filter::{filter_fn, LevelFilter, Targets},
    layer::Context as LayerContext,
    Layer,
};

const DEFAULT_JAEGER_URL: &str = "http://localhost:4317";

fn export_filter() -> Targets {
    Targets::new()
        .with_default(LevelFilter::TRACE)
        .with_target("opentelemetry", LevelFilter::OFF)
        .with_target("h2", LevelFilter::OFF)
}

// Keep exporter failures visible in local logs even when the collector is unreachable.
struct OtelDiagnostics;

impl<SubscriberType: Subscriber> Layer<SubscriberType> for OtelDiagnostics {
    fn on_event(&self, event: &tracing::Event<'_>, _context: LayerContext<'_, SubscriberType>) {
        let metadata = event.metadata();
        let mut message = String::new();
        event.record(
            &mut |field: &tracing::field::Field, value: &dyn std::fmt::Debug| {
                let _ = write!(message, "{}={:?} ", field.name(), value);
            },
        );
        if *metadata.level() == tracing::Level::ERROR {
            error!(sl!(), "{}", message.trim_end(); "target" => metadata.target());
        } else {
            warn!(sl!(), "{}", message.trim_end(); "target" => metadata.target());
        }
    }
}

// Guard against retained span references outliving request draining and causing
// exporter shutdown to drop the root span. The wait shares the drain deadline.
struct RootSpanCloseLayer {
    closed: Arc<Notify>,
}

impl<SubscriberType> Layer<SubscriberType> for RootSpanCloseLayer
where
    SubscriberType: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_close(&self, id: span::Id, ctx: LayerContext<'_, SubscriberType>) {
        if let Some(span) = ctx.span(&id) {
            if span.metadata().target() == module_path!() && span.name() == "root-span" {
                self.closed.notify_one();
            }
        }
    }
}

/// The tracer wrapper for kata-containers
/// The fields and member methods should ALWAYS be PRIVATE and be exposed in a safe
/// way to other modules
pub struct KataTracer {
    provider: Option<SdkTracerProvider>,
    root_span: Option<Span>,
    root_closed: Arc<Notify>,
}

impl Default for KataTracer {
    fn default() -> Self {
        Self::new()
    }
}

impl KataTracer {
    /// Constructor of KataTracer, this is a dummy implementation for static initialization
    pub fn new() -> Self {
        Self {
            provider: None,
            root_span: None,
            root_closed: Arc::new(Notify::new()),
        }
    }

    /// Call when the tracing is enabled (set in toml configuration file)
    /// Install the global subscriber and retain the provider and sandbox root.
    ///
    /// Note that the span will be noop(not collected) if a invalid subscriber is set
    pub fn trace_setup(
        &mut self,
        sid: &str,
        jaeger_endpoint: &str,
        jaeger_username: &str,
        jaeger_password: &str,
    ) -> Result<()> {
        if self.provider.is_some() {
            return Ok(());
        }

        let endpoint = verify_jaeger_config(jaeger_endpoint, jaeger_username, jaeger_password)?;
        let mut metadata = MetadataMap::new();
        if !jaeger_username.is_empty() {
            let credentials = BASE64.encode(format!("{jaeger_username}:{jaeger_password}"));
            metadata.insert("authorization", format!("Basic {credentials}").parse()?);
            if let Some(authorization) = metadata.get_mut("authorization") {
                authorization.set_sensitive(true);
            }
        }
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_metadata(metadata)
            .build()?;
        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(
                Resource::builder_empty()
                    .with_service_name(format!("kata-sb-{}", &sid[0..min(8, sid.len())]))
                    .build(),
            )
            .build();
        let tracer = provider.tracer("kata-runtime-rs");

        // Do not feed exporter diagnostics back into the exporter.
        let layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(export_filter());
        // Filter before on_event: only OpenTelemetry WARN/ERROR events are formatted.
        let diagnostics = OtelDiagnostics.with_filter(filter_fn(|metadata| {
            metadata.target().starts_with("opentelemetry")
                && *metadata.level() <= tracing::Level::WARN
        }));
        let sub = Registry::default()
            .with(layer)
            .with(diagnostics)
            // Keep this layer outermost so OpenTelemetry queues the root before we notify shutdown.
            .with(RootSpanCloseLayer {
                closed: self.root_closed.clone(),
            });

        tracing::subscriber::set_global_default(sub)?;
        self.provider = Some(provider);

        self.root_span =
            Some(span!(parent: None, tracing::Level::TRACE, "root-span", sandbox_id = %sid));

        info!(sl!(), "Tracing enabled successfully");
        Ok(())
    }

    pub fn begin_shutdown(&mut self) -> Option<(SdkTracerProvider, Arc<Notify>)> {
        let provider = self.provider.take()?;
        drop(self.root_span.take());
        // Notify retains a permit if the root closed synchronously before the waiter starts.
        Some((provider, self.root_closed.clone()))
    }

    pub fn root_span(&self) -> Option<Span> {
        self.root_span.clone()
    }
}

/// Verify Jaeger credentials and set the default OTLP/gRPC endpoint.
fn verify_jaeger_config(endpoint: &str, username: &str, passwd: &str) -> Result<String> {
    if username.is_empty() && !passwd.is_empty() {
        warn!(
            sl!(),
            "Jaeger password with empty username is not allowed, tracing is NOT enabled"
        );
        return Err(anyhow::anyhow!("Empty username with non-empty password"));
    }

    let endpt = if endpoint.is_empty() {
        DEFAULT_JAEGER_URL
    } else {
        endpoint
    }
    .to_owned();

    Ok(endpt)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use opentelemetry_sdk::trace::{SpanData, SpanExporter};
    use std::sync::Mutex;

    #[derive(Clone, Debug, Default)]
    pub struct RecordingExporter(pub Arc<Mutex<Vec<SpanData>>>);

    impl SpanExporter for RecordingExporter {
        async fn export(&self, batch: Vec<SpanData>) -> opentelemetry_sdk::error::OTelSdkResult {
            self.0.lock().unwrap().extend(batch);
            Ok(())
        }
    }

    pub fn tracer() -> (KataTracer, RecordingExporter, tracing::Dispatch) {
        let exporter = RecordingExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_batch_exporter(exporter.clone())
            .build();
        let mut tracer = KataTracer::new();
        let subscriber = Registry::default()
            .with(
                tracing_opentelemetry::layer()
                    .with_tracer(provider.tracer("lifecycle-test"))
                    .with_filter(export_filter()),
            )
            .with(RootSpanCloseLayer {
                closed: tracer.root_closed.clone(),
            });
        let dispatch = tracing::Dispatch::new(subscriber);
        tracer.root_span = Some(tracing::dispatcher::with_default(
            &dispatch,
            || span!(target: "runtimes::tracer", parent: None, tracing::Level::TRACE, "root-span"),
        ));
        tracer.provider = Some(provider);
        (tracer, exporter, dispatch)
    }
}
