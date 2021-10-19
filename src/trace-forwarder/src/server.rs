// Copyright (c) 2020-2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::handler;
use anyhow::{anyhow, Result};
use opentelemetry::sdk::export::trace::SpanExporter;
use slog::{debug, o, Logger};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;
use vsock::{SockAddr, VsockListener};

use crate::tracer;

#[derive(Debug, Clone, PartialEq)]
pub enum VsockType {
    Standard { port: u32, cid: u32 },
    Hybrid { socket_path: String },
}

#[derive(Debug)]
pub struct VsockTraceServer {
    pub vsock: VsockType,

    pub jaeger_host: String,
    pub jaeger_port: u32,
    pub jaeger_service_name: String,

    pub logger: Logger,
    pub dump_only: bool,
}

impl VsockTraceServer {
    pub fn new(
        logger: &Logger,
        vsock: VsockType,
        jaeger_host: &str,
        jaeger_port: u32,
        jaeger_service_name: &str,
        dump_only: bool,
    ) -> Self {
        let logger = logger.new(o!("subsystem" => "server"));

        VsockTraceServer {
            vsock,
            jaeger_host: jaeger_host.to_string(),
            jaeger_port,
            jaeger_service_name: jaeger_service_name.to_string(),
            logger,
            dump_only,
        }
    }

    pub fn start(&self) -> Result<()> {
        let result = tracer::create_jaeger_trace_exporter(
            self.jaeger_service_name.clone(),
            self.jaeger_host.clone(),
            self.jaeger_port,
        );

        let mut exporter = result?;

        match &self.vsock {
            VsockType::Standard { port, cid } => start_std_vsock(
                self.logger.clone(),
                &mut exporter,
                *port,
                *cid,
                self.dump_only,
            ),
            VsockType::Hybrid { socket_path } => start_hybrid_vsock(
                self.logger.clone(),
                &mut exporter,
                socket_path,
                self.dump_only,
            ),
        }
    }
}

fn start_hybrid_vsock(
    logger: Logger,
    exporter: &mut dyn SpanExporter,
    socket_path: &str,
    dump_only: bool,
) -> Result<()> {
    // Remove the socket if it already exists
    let _ = std::fs::remove_file(socket_path);

    let listener =
        UnixListener::bind(socket_path).map_err(|e| anyhow!("You need to be root: {:?}", e))?;

    debug!(logger, "Waiting for connections";
        "vsock-type" => "hybrid",
        "vsock-socket-path" => socket_path);

    for conn in listener.incoming() {
        let conn = conn?;

        let fd = conn.as_raw_fd();

        handler::handle_connection(logger.clone(), fd, exporter, dump_only)?;
    }

    Ok(())
}

fn start_std_vsock(
    logger: Logger,
    exporter: &mut dyn SpanExporter,
    port: u32,
    cid: u32,
    dump_only: bool,
) -> Result<()> {
    let sock_addr = SockAddr::new_vsock(cid, port);
    let listener = VsockListener::bind(&sock_addr)?;

    debug!(logger, "Waiting for connections";
        "vsock-type" => "standard",
        "vsock-cid" => cid,
        "vsock-port" => port);

    for conn in listener.incoming() {
        let conn = conn?;

        let fd = conn.as_raw_fd();

        handler::handle_connection(logger.clone(), fd, exporter, dump_only)?;
    }

    Ok(())
}
