// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp_api::KeyValue;
use opentelemetry_sdk::trace::{BatchSpanProcessor, SpanExporter};
use opentelemetry_sdk::Resource;
use tracing_subscriber::fmt::writer::MakeWriter;

pub fn init_sdk_diagnostics<W>(writer: W) -> Result<()>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .with_ansi(false)
        .with_writer(writer)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .context("failed to initialize OpenTelemetry diagnostics")
}

pub fn create_otlp_span_processor(
    service_name: String,
    otlp_endpoint: String,
) -> std::result::Result<BatchSpanProcessor, std::io::Error> {
    let exporter_type = "otlp";

    // Create OTLP exporter
    let mut exporter = match opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(otlp_endpoint)
        .build()
    {
        Ok(e) => e,
        Err(e) => {
            return Err(std::io::Error::other(format!(
                "failed to create OTLP exporter: {:?}",
                e
            )))
        }
    };

    let resource = Resource::builder_empty()
        .with_attributes([
            KeyValue::new("service.name", service_name),
            KeyValue::new("exporter", exporter_type),
        ])
        .build();
    exporter.set_resource(&resource);
    Ok(BatchSpanProcessor::builder(exporter).build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::tests::{agent_span, RecordingExporter};
    use opentelemetry_sdk::trace::{BatchConfigBuilder, SpanProcessor};
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    #[derive(Clone, Default)]
    struct CapturedDiagnostics(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturedDiagnostics {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn timer_export_failures_are_visible_in_sdk_diagnostics() {
        let captured = CapturedDiagnostics::default();
        let writer = captured.clone();
        init_sdk_diagnostics(move || writer.clone()).unwrap();

        let exporter = RecordingExporter {
            failure: Some("timer-export-test"),
            ..Default::default()
        };
        let config = BatchConfigBuilder::default()
            .with_scheduled_delay(Duration::from_millis(10))
            .build();
        let processor = BatchSpanProcessor::builder(exporter)
            .with_batch_config(config)
            .build();
        processor.on_end(crate::span_data::convert_span(agent_span()).unwrap());

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let data = captured.0.lock().unwrap();
            let text = String::from_utf8_lossy(&data);
            if text.contains("BatchSpanProcessor.ExportError") && text.contains("timer-export-test")
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "missing export diagnostic: {}",
                text
            );
            drop(data);
            std::thread::sleep(Duration::from_millis(10));
        }
        processor.shutdown().unwrap();
    }
}
