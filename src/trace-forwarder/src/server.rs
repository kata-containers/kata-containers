// Copyright (c) 2020-2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::handler;
use anyhow::Result;
use futures::executor::block_on;
use slog::{debug, error, info, o, Logger};
use vsock::{SockAddr, VsockListener};

use crate::tracer;

#[derive(Debug)]
pub struct VsockTraceServer {
    pub vsock_port: u32,
    pub vsock_cid: u32,

    pub jaeger_host: String,
    pub jaeger_port: u32,
    pub jaeger_service_name: String,

    pub logger: Logger,
}

impl VsockTraceServer {
    pub fn new(
        logger: &Logger,
        vsock_port: u32,
        vsock_cid: u32,
        jaeger_host: &str,
        jaeger_port: u32,
        jaeger_service_name: &str,
    ) -> Self {
        let logger = logger.new(o!("subsystem" => "server"));

        VsockTraceServer {
            vsock_port: vsock_port,
            vsock_cid: vsock_cid,
            jaeger_host: jaeger_host.to_string(),
            jaeger_port: jaeger_port,
            jaeger_service_name: jaeger_service_name.to_string(),
            logger: logger,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        let sock_addr = SockAddr::new_vsock(self.vsock_cid, self.vsock_port);

        let listener = VsockListener::bind(&sock_addr)?;

        info!(self.logger, "listening for client connections"; "vsock-port" => self.vsock_port, "vsock-cid" => self.vsock_cid);

        let result = tracer::create_jaeger_trace_exporter(
            self.jaeger_service_name.clone(),
            self.jaeger_host.clone(),
            self.jaeger_port,
        );

        let mut exporter = result?;

        for conn in listener.incoming() {
            debug!(self.logger, "got client connection");

            match conn {
                Err(e) => {
                    error!(self.logger, "client connection failed"; "error" => format!("{}", e))
                }
                Ok(conn) => {
                    debug!(self.logger, "client connection successful");

                    let logger = self.logger.new(o!());

                    let f = handler::handle_connection(logger, conn, &mut exporter);

                    block_on(f)?;
                }
            }

            debug!(self.logger, "handled client connection");
        }

        Ok(())
    }
}
