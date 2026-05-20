// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use byteorder::{ByteOrder, NetworkEndian};
use futures::executor::block_on;
use slog::{debug, info, o, warn, Logger};
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::os::unix::io::{FromRawFd, RawFd};

// The VSOCK "packet" protocol used comprises two elements:
//
// 1) The header (the number of bytes in the payload).
// 2) The payload bytes.
//
// This constant defines the number of bytes used to encode the header on the
// wire. In other words, the first 64-bits of the packet contain a number
// specifying how many bytes are in the remainder of the packet.
//
// Must match the value of the variable of the same name in the agents
// vsock-exporter.
const HEADER_SIZE_BYTES: u64 = std::mem::size_of::<u64>() as u64;

fn mk_io_err(msg: &str) -> std::io::Error {
    std::io::Error::other(msg.to_string())
}

async fn handle_async_connection(
    logger: Logger,
    mut conn: &mut dyn Read,
    dump_only: bool,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "handler"));

    debug!(logger, "handling connection");

    handle_trace_data(logger.clone(), &mut conn, dump_only)
        .await
        .map_err(|e| mk_io_err(&format!("failed to handle data: {e:}")))?;

    debug!(&logger, "handled connection");

    Ok(())
}

async fn handle_trace_data(logger: Logger, reader: &mut dyn Read, dump_only: bool) -> Result<()> {
    loop {
        let mut header: [u8; HEADER_SIZE_BYTES as usize] = [0; HEADER_SIZE_BYTES as usize];

        info!(logger, "waiting for traces");

        match reader.read_exact(&mut header) {
            Ok(_) => debug!(logger, "read header"),
            Err(e) => {
                if e.kind() == ErrorKind::UnexpectedEof {
                    info!(logger, "agent shut down");
                    break;
                }

                return Err(anyhow!("failed to read header: {:}", e));
            }
        };

        let payload_len: u64 = NetworkEndian::read_u64(&header);

        let mut encoded_payload = vec![0; payload_len as usize];

        reader
            .read_exact(&mut encoded_payload)
            .with_context(|| "failed to read payload")?;

        debug!(logger, "read payload");

        // Note: In OpenTelemetry 0.27, SpanData no longer implements Deserialize.
        // The agent sends serialized SpanData, but we can't deserialize it directly.
        // For now, we log the raw data in dump mode and warn about the incompatibility.

        if dump_only {
            debug!(
                logger,
                "dump-only: received {} bytes",
                encoded_payload.len()
            );
        } else {
            warn!(
                logger,
                "Received span data but cannot process: OpenTelemetry 0.27 SpanData is not deserializable. \
                 Agent and forwarder OpenTelemetry versions may be incompatible."
            );
        }
    }

    Ok(())
}

pub fn handle_connection(
    logger: Logger,
    fd: RawFd,
    _tracer: &opentelemetry_sdk::trace::Tracer,
    dump_only: bool,
) -> Result<()> {
    let mut file = unsafe { File::from_raw_fd(fd) };

    let conn = handle_async_connection(logger, &mut file, dump_only);

    block_on(conn)?;

    Ok(())
}
