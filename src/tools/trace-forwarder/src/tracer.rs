// Copyright (c) 2020-2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use opentelemetry::KeyValue;
use std::net::SocketAddr;

pub fn create_jaeger_trace_exporter(
    jaeger_service_name: String,
    jaeger_host: String,
    jaeger_port: u32,
) -> Result<opentelemetry_jaeger::Exporter, std::io::Error> {
    let exporter_type = "jaeger";

    let jaeger_addr = format!("{}:{}", jaeger_host, jaeger_port);

    let socket_addr: SocketAddr = match jaeger_addr.parse() {
        Ok(a) => a,
        Err(e) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to parse Jaeger address: {:?}", e.to_string()),
            ))
        }
    };

    let exporter = match opentelemetry_jaeger::new_pipeline()
        .with_service_name(jaeger_service_name)
        .with_agent_endpoint(socket_addr.to_string())
        .with_tags(vec![KeyValue::new("exporter", exporter_type)])
        .init_exporter()
    {
        Ok(x) => x,
        Err(e) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to create exporter: {:?}", e.to_string()),
            ))
        }
    };

    Ok(exporter)
}
