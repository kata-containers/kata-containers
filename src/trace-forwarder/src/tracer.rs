// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use opentelemetry::api::Key;
use std::net::SocketAddr;

pub fn create_jaeger_trace_exporter(
    jaeger_service_name: String,
    jaeger_host: String,
    jaeger_port: u32,
) -> Result<opentelemetry_jaeger::Exporter, std::io::Error> {
    let exporter_type = "jaeger";

    let jaeger_addr = format!("{}:{}", jaeger_host, jaeger_port);

    let process = opentelemetry_jaeger::Process {
        service_name: jaeger_service_name,
        tags: vec![Key::new("exporter").string(exporter_type)],
    };

    let socket_addr: SocketAddr = match jaeger_addr.parse() {
        Ok(a) => a,
        Err(e) => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to parse Jaeger address: {:?}", e.to_string()),
            ))
        }
    };

    let exporter = match opentelemetry_jaeger::Exporter::builder()
        .with_agent_endpoint(socket_addr.to_string())
        .with_process(process)
        .init()
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
