// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::handler;
use anyhow::{anyhow, Result};
use opentelemetry_sdk::trace::SpanProcessor;
use privdrop::PrivDrop;
use slog::{debug, o, Logger};
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
        let processor: Option<Arc<dyn SpanProcessor>> = if self.dump_only {
            None
        } else {
            Some(Arc::new(tracer::create_otlp_span_processor(
                self.service_name.clone(),
                self.otlp_endpoint.clone(),
            )?))
        };

        let result = match &self.vsock {
            VsockType::Standard { port, cid } => {
                start_std_vsock(self.logger.clone(), processor.clone(), *port, *cid)
            }
            VsockType::Hybrid { socket_path } => {
                start_hybrid_vsock(self.logger.clone(), processor.clone(), socket_path)
            }
        };
        let shutdown = match processor {
            Some(processor) => processor
                .shutdown()
                .map_err(|error| anyhow!("failed to shut down trace exporter: {error}")),
            None => Ok(()),
        };
        match (result, shutdown) {
            (Err(error), Err(shutdown)) => Err(error.context(shutdown)),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), shutdown) => shutdown,
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
    processor: Option<Arc<dyn SpanProcessor>>,
    socket_path: &str,
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

        handler::handle_connection(logger.clone(), conn, processor.as_deref())?;
    }

    Ok(())
}

fn start_std_vsock(
    logger: Logger,
    processor: Option<Arc<dyn SpanProcessor>>,
    port: u32,
    cid: u32,
) -> Result<()> {
    let sock_addr = SockAddr::new_vsock(cid, port);
    let listener = VsockListener::bind(&sock_addr)?;

    debug!(logger, "Waiting for connections";
        "vsock-type" => "standard",
        "vsock-cid" => cid,
        "vsock-port" => port);

    for conn in listener.incoming() {
        let conn = conn?;

        handler::handle_connection(logger.clone(), conn, processor.as_deref())?;
    }

    Ok(())
}
