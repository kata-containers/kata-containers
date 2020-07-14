// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use opentelemetry::api::trace::provider::Provider;
use opentelemetry::global::BoxedTracer;
use slog::{o, Logger};

use opentelemetry::{global, sdk};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::layer::Layered;
use tracing_subscriber::{Layer, Registry};
use vsock_exporter;

pub fn setup_tracing(logger: &Logger) {
    let logger = logger.new(o!("subsystem" => "vsock-tracer"));

    let exporter = vsock_exporter::Exporter::builder()
        .with_logger(&logger)
        .init();

    let provider = sdk::Provider::builder()
        .with_simple_exporter(exporter)
        .with_config(sdk::Config {
            default_sampler: Box::new(sdk::Sampler::Always),
            ..Default::default()
        })
        .build();

    global::set_provider(provider);
}

pub fn shutdown_tracing() {
    global::set_provider(sdk::Provider::default());
}

pub fn get_subscriber(
    name: &'static str,
) -> Layered<OpenTelemetryLayer<Registry, BoxedTracer>, Registry> {
    // XXX: Get the tracer (which will be a NOP tracer if no explicit tracer
    // previously registered).
    let tracer: opentelemetry::global::BoxedTracer = global::trace_provider().get_tracer(name);

    let layer = tracing_opentelemetry::layer().with_tracer(tracer);

    layer.with_subscriber(Registry::default())
}
