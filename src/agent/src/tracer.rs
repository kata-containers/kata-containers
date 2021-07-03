// Copyright (c) 2020-2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::config::AgentConfig;
use anyhow::Result;
use opentelemetry::sdk::propagation::TraceContextPropagator;
use opentelemetry::{global, sdk::trace::Config, trace::TracerProvider};
use slog::{info, o, Logger};
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;
use ttrpc::r#async::TtrpcContext;

#[derive(Debug, PartialEq)]
pub enum TraceType {
    Disabled,
    Isolated,
}

#[derive(Debug)]
pub struct TraceTypeError {
    details: String,
}

impl TraceTypeError {
    fn new(msg: &str) -> TraceTypeError {
        TraceTypeError {
            details: msg.into(),
        }
    }
}

impl Error for TraceTypeError {}

impl fmt::Display for TraceTypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl FromStr for TraceType {
    type Err = TraceTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "isolated" => Ok(TraceType::Isolated),
            "disabled" => Ok(TraceType::Disabled),
            _ => Err(TraceTypeError::new("invalid trace type")),
        }
    }
}

pub fn setup_tracing(name: &'static str, logger: &Logger, _agent_cfg: &AgentConfig) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "vsock-tracer"));

    let exporter = vsock_exporter::Exporter::builder()
        .with_logger(&logger)
        .init();

    let config = Config::default();

    let builder = opentelemetry::sdk::trace::TracerProvider::builder()
        .with_simple_exporter(exporter)
        .with_config(config);

    let provider = builder.build();

    // We don't need a versioned tracer.
    let version = None;

    let tracer = provider.get_tracer(name, version);

    let _global_provider = global::set_tracer_provider(provider);

    let layer = OpenTelemetryLayer::new(tracer);

    let subscriber = Registry::default().with(layer);

    tracing::subscriber::set_global_default(subscriber)?;

    global::set_text_map_propagator(TraceContextPropagator::new());

    info!(logger, "tracing setup");

    Ok(())
}

pub fn end_tracing() {
    global::shutdown_tracer_provider();
}

pub fn extract_carrier_from_ttrpc(ttrpc_context: &TtrpcContext) -> HashMap<String, String> {
    let mut carrier = HashMap::new();
    for (k, v) in &ttrpc_context.metadata {
        carrier.insert(k.clone(), v.join(","));
    }

    carrier
}

#[macro_export]
macro_rules! trace_rpc_call {
    ($ctx: ident, $name:literal, $req: ident) => {
        // extract context from request context
        let parent_context = global::get_text_map_propagator(|propagator| {
            propagator.extract(&extract_carrier_from_ttrpc($ctx))
        });

        // generate tracing span
        let rpc_span = span!(tracing::Level::INFO, $name, "mod"="rpc.rs", req=?$req);

        // assign parent span from external context
        rpc_span.set_parent(parent_context);
        let _enter = rpc_span.enter();
    };
}
