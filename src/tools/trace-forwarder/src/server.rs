// Copyright (c) 2020-2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::handler;
use anyhow::{anyhow, Result};
use opentelemetry::sdk::export::trace::SpanExporter;
use privdrop::PrivDrop;
use slog::{debug, o, Logger};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;
use vsock::{SockAddr, VsockListener};

use crate::tracer;

// Username that is assumed to exist, used when dropping root privileges
// when running with Hybrid VSOCK.
pub const NON_PRIV_USER: &str = "nobody";

const ROOT_DIR: &str = "/";

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

fn drop_privs(logger: &Logger) -> Result<()> {
    debug!(logger, "Dropping privileges"; "new-user" => NON_PRIV_USER);

    nix::unistd::chdir(ROOT_DIR)
        .map_err(|e| anyhow!("Unable to chdir to {:?}: {:?}", ROOT_DIR, e))?;

    PrivDrop::default()
        .user(NON_PRIV_USER)
        .apply()
        .map_err(|e| anyhow!("Failed to drop privileges to user {}: {}", NON_PRIV_USER, e))?;

    Ok(())
}

fn start_hybrid_vsock(
    logger: Logger,
    exporter: &mut dyn SpanExporter,
    socket_path: &str,
    dump_only: bool,
) -> Result<()> {
    let logger =
        logger.new(o!("vsock-type" => "hybrid", "vsock-socket-path" => socket_path.to_string()));

    let effective = nix::unistd::Uid::effective();

    if !effective.is_root() {
        return Err(anyhow!("You need to be root"));
    }

    // Remove the socket if it already exists
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)?;

    // Having bound to the socket, drop privileges
    drop_privs(&logger)?;

    debug!(logger, "Waiting for connections");

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
