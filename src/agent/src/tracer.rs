// Copyright (c) 2020-2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::config::AgentConfig;
use anyhow::Result;
use opentelemetry::{global, sdk::trace::Config, trace::TracerProvider};
use slog::{info, o, Logger};
use std::error::Error;
use std::fmt;
use std::str::FromStr;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

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

    info!(logger, "tracing setup");

    Ok(())
}

pub fn end_tracing() {
    global::shutdown_tracer_provider();
}
