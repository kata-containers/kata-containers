// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::handler;
use anyhow::{anyhow, Result};
use opentelemetry::trace::TracerProvider as _;
use privdrop::PrivDrop;
use slog::{debug, o, Logger};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;
use std::sync::Arc;
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

    pub otlp_endpoint: String,
    pub service_name: String,

    pub logger: Logger,
    pub dump_only: bool,
}

impl VsockTraceServer {
    pub fn new(
        logger: &Logger,
        vsock: VsockType,
        otlp_endpoint: &str,
        service_name: &str,
        dump_only: bool,
    ) -> Self {
        let logger = logger.new(o!("subsystem" => "server"));

        VsockTraceServer {
            vsock,
            otlp_endpoint: otlp_endpoint.to_string(),
            service_name: service_name.to_string(),
            logger,
            dump_only,
        }
    }

    pub fn start(&self) -> Result<()> {
        let provider = tracer::create_otlp_trace_exporter(
            self.service_name.clone(),
            self.otlp_endpoint.clone(),
        )?;

        // Get a tracer from the provider and wrap in Arc for sharing across connections
        let tracer = provider.tracer("kata-trace-forwarder");
        let shared_tracer = Arc::new(tracer);

        match &self.vsock {
            VsockType::Standard { port, cid } => start_std_vsock(
                self.logger.clone(),
                shared_tracer.clone(),
                *port,
                *cid,
                self.dump_only,
            ),
            VsockType::Hybrid { socket_path } => start_hybrid_vsock(
                self.logger.clone(),
                shared_tracer,
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
    tracer: Arc<opentelemetry_sdk::trace::Tracer>,
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

        handler::handle_connection(logger.clone(), fd, &tracer, dump_only)?;
    }

    Ok(())
}

fn start_std_vsock(
    logger: Logger,
    tracer: Arc<opentelemetry_sdk::trace::Tracer>,
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

        handler::handle_connection(logger.clone(), fd, &tracer, dump_only)?;
    }

    Ok(())
}
