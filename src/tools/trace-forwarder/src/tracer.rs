// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp_api::KeyValue;
use opentelemetry_sdk::trace::{BatchSpanProcessor, SpanExporter};
use opentelemetry_sdk::Resource;

pub fn create_otlp_span_processor(
    service_name: String,
    otlp_endpoint: String,
) -> Result<BatchSpanProcessor, std::io::Error> {
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
