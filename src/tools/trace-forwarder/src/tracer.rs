// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;

pub fn create_otlp_trace_exporter(
    service_name: String,
    otlp_endpoint: String,
) -> Result<SdkTracerProvider, std::io::Error> {
    let exporter_type = "otlp";

    // Create OTLP exporter
    let exporter = match opentelemetry_otlp::SpanExporter::builder()
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

    // Create tracer provider with resource attributes
    let resource = Resource::builder_empty()
        .with_attributes([
            KeyValue::new("service.name", service_name),
            KeyValue::new("exporter", exporter_type),
        ])
        .build();
    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource)
        .build();

    Ok(provider)
}
