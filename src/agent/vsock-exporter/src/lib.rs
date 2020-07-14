// Copyright (c) 2020 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use byteorder::{ByteOrder, NetworkEndian};
use nix::sys::socket::{SockAddr, VsockAddr};
use opentelemetry::exporter::trace::{ExportResult, SpanData, SpanExporter};
use slog::{error, o, Logger};
use std::io::{Error, ErrorKind, Write};
use std::net::Shutdown;
use std::sync::{Arc, Mutex};
use vsock::VsockStream;

const ANY_CID: &'static str = "any";

// reserved for "host"
const DEFAULT_CID: u32 = 2;
const DEFAULT_PORT: u32 = 10240;

const HEADER_SIZE_BYTES: u64 = std::mem::size_of::<u64>() as u64;

#[derive(Debug)]
pub struct Exporter {
    port: u32,
    cid: u32,
    conn: Arc<Mutex<VsockStream>>,
    logger: Logger,
}

impl Exporter {
    /// Create a new exporter builder.
    pub fn builder() -> Builder {
        Builder::default()
    }
}

fn make_error(desc: String) -> std::io::Error {
    Error::new(ErrorKind::Other, desc.to_string())
}

fn write_span(writer: &mut dyn Write, span: &SpanData) -> Result<(), std::io::Error> {
    let encoded_payload: Vec<u8> =
        bincode::serialize(&span).map_err(|e| make_error(e.to_string()))?;

    let payload_len: u64 = encoded_payload.len() as u64;

    let mut payload_len_as_bytes: [u8; HEADER_SIZE_BYTES as usize] =
        [0; HEADER_SIZE_BYTES as usize];

    NetworkEndian::write_u64(&mut payload_len_as_bytes, payload_len);

    writer
        .write_all(&payload_len_as_bytes)
        .map_err(|e| make_error(format!("failed to write trace header: {:?}", e)))?;

    let result = writer
        .write_all(&encoded_payload)
        .map_err(|e| make_error(format!("failed to write trace payload: {:?}", e)));

    result
}

fn handle_batch(writer: &mut dyn Write, batch: Vec<Arc<SpanData>>) -> Result<(), std::io::Error> {
    for entry in batch {
        let span_data = &*entry;

        write_span(writer, span_data).map_err(|e| e)?;
    }

    Ok(())
}

impl SpanExporter for Exporter {
    fn export(&self, batch: Vec<Arc<SpanData>>) -> ExportResult {
        let conn_ref = self.conn.clone();

        let conn = conn_ref.lock();

        if conn.is_err() {
            error!(self.logger, "failed to obtain connection"; "error" => format!("{}", conn.unwrap_err()));

            return ExportResult::FailedNotRetryable;
        }

        let mut conn = conn.unwrap();

        let result = handle_batch(&mut *conn, batch);

        if result.is_err() {
            error!(self.logger, "failed to handle batch"; "error" => format!("{}", result.unwrap_err()));

            return ExportResult::FailedNotRetryable;
        }

        ExportResult::Success
    }

    fn shutdown(&self) {
        let conn = self.conn.lock().map_err(|e| {
            error!(self.logger, "failed to obtain connection"; "error" => format!("{}", e));
            return;
        });

        let conn = conn.unwrap();

        let result = conn.shutdown(Shutdown::Write);
        if result.is_err() {
            error!(self.logger, "failed to shutdown VSOCK connection"; "error" => format!("{}", result.unwrap_err()));
        }
    }
}

#[derive(Debug)]
pub struct Builder {
    port: u32,
    cid: u32,
    logger: Logger,
}

impl Default for Builder {
    fn default() -> Self {
        let logger = Logger::root(slog::Discard, o!());

        Builder {
            cid: DEFAULT_CID,
            port: DEFAULT_PORT,
            logger: logger,
        }
    }
}

impl Builder {
    pub fn with_cid(self, cid: u32) -> Self {
        Builder { cid, ..self }
    }

    pub fn with_port(self, port: u32) -> Self {
        Builder { port, ..self }
    }

    pub fn with_logger(self, logger: &Logger) -> Self {
        Builder {
            logger: logger.new(o!()),
            ..self
        }
    }

    pub fn init(self) -> Exporter {
        let Builder { port, cid, logger } = self;

        let vsock_addr = VsockAddr::new(self.cid, self.port);
        let sock_addr = SockAddr::Vsock(vsock_addr);

        let cid_str: String;

        if self.cid == libc::VMADDR_CID_ANY {
            cid_str = ANY_CID.to_string();
        } else {
            cid_str = format!("{}", self.cid);
        }

        // Handle unrecoverable error. The trait doesn't allow an error
        // return, so force a hard error.
        let conn = VsockStream::connect(&sock_addr).expect(&format!(
            "failed to connect to VSOCK server (port: {}, cid: {}) - {}",
            self.port, cid_str, "tracing enabled so ensure trace forwarder is running on host"
        ));

        let logger = logger
            .clone()
            .new(o!("subsystem" => "vsock", "cid" => cid_str, "port" => self.port));

        Exporter {
            port: port,
            cid: cid,
            conn: Arc::new(Mutex::new(conn)),
            logger: logger,
        }
    }
}
