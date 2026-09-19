// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::cmp::min;
use std::fmt::Write;
use std::sync::Arc;

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use lazy_static::lazy_static;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::{tonic_types::metadata::MetadataMap, WithExportConfig, WithTonicConfig};
use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};
use tracing::{span, subscriber::NoSubscriber, Span, Subscriber};
use tracing_subscriber::prelude::*;
use tracing_subscriber::Registry;
use tracing_subscriber::{filter::filter_fn, layer::Context as LayerContext, Layer};

const DEFAULT_JAEGER_URL: &str = "http://localhost:4317";

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

lazy_static! {
    /// The ROOTSPAN is a phantom span that is running by calling [`trace_enter_root()`] at the background
    /// once the configuration is read and config.runtime.enable_tracing is enabled
    /// The ROOTSPAN exits by calling [`trace_exit_root()`] on shutdown request sent from containerd
    ///
    /// NOTE:
    ///     This allows other threads which are not directly running under some spans to be tracked easily
    ///     within the entire sandbox's lifetime.
    ///     To do this, you just need to add attribute #[instrment(parent=&(*ROOTSPAN))]
    pub static ref ROOTSPAN: Span = span!(tracing::Level::TRACE, "root-span");
}

/// The tracer wrapper for kata-containers
/// The fields and member methods should ALWAYS be PRIVATE and be exposed in a safe
/// way to other modules
unsafe impl Send for KataTracer {}
unsafe impl Sync for KataTracer {}
pub struct KataTracer {
    subscriber: Arc<dyn Subscriber + Send + Sync>,
    provider: Option<SdkTracerProvider>,
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
            subscriber: Arc::new(NoSubscriber::default()),
            provider: None,
        }
    }

    /// Call when the tracing is enabled (set in toml configuration file)
    /// This setup the subscriber, which maintains the span's information, to global and
    /// inside KATA_TRACER.
    ///
    /// Note that the span will be noop(not collected) if a invalid subscriber is set
    pub fn trace_setup(
        &mut self,
        sid: &str,
        jaeger_endpoint: &str,
        jaeger_username: &str,
        jaeger_password: &str,
    ) -> Result<()> {
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
            .with_filter(filter_fn(|metadata| {
                !metadata.target().starts_with("opentelemetry")
            }));
        // Filter before on_event: only OpenTelemetry WARN/ERROR events are formatted.
        let diagnostics = OtelDiagnostics.with_filter(filter_fn(|metadata| {
            metadata.target().starts_with("opentelemetry")
                && *metadata.level() <= tracing::Level::WARN
        }));
        let sub = Registry::default().with(layer).with(diagnostics);

        // we use Arc to let global subscriber and katatracer to SHARE the SAME subscriber
        // this is for record the global subscriber into a global variable KATA_TRACER for more usages
        let subscriber = Arc::new(sub);
        tracing::subscriber::set_global_default(subscriber.clone())?;
        self.subscriber = subscriber;
        self.provider = Some(provider);

        // enter the rootspan
        self.trace_enter_root();

        info!(sl!(), "Tracing enabled successfully");
        Ok(())
    }

    /// Flush completed spans to the OTLP receiver before stopping the runtime.
    pub async fn trace_end(&self) -> Result<()> {
        if let Some(provider) = self.provider.clone() {
            self.trace_exit_root();
            tokio::task::spawn_blocking(move || provider.shutdown())
                .await
                .context("join tracing shutdown")?
                .context("shut down OTLP trace provider")?;
        }
        Ok(())
    }

    /// Enter the global ROOTSPAN
    /// This function is a hack on tracing library's guard approach, letting the span
    /// to enter without using a RAII guard to exit. This function should only be called
    /// once, and also in paired with [`trace_exit_root()`].
    fn trace_enter_root(&self) {
        self.enter_span(&ROOTSPAN);
    }

    /// Exit the global ROOTSPAN
    /// This should be called in paired with [`trace_enter_root()`].
    fn trace_exit_root(&self) {
        self.exit_span(&ROOTSPAN);
    }

    /// let the subscriber enter the span, this has to be called in pair with exit(span)
    /// This function allows **cross function span** to run without span guard
    fn enter_span(&self, span: &Span) {
        let id: Option<span::Id> = span.into();
        self.subscriber.enter(&id.unwrap());
    }

    /// let the subscriber exit the span, this has to be called in pair to enter(span)
    fn exit_span(&self, span: &Span) {
        let id: Option<span::Id> = span.into();
        self.subscriber.exit(&id.unwrap());
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
