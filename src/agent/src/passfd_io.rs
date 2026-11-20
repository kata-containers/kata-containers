// Copyright 2024 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//
use anyhow::Result;
use lazy_static::lazy_static;
use rustjail::process::ProcessIo;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tokio_vsock::SockAddr::Vsock;
use tokio_vsock::{VsockListener, VsockStream};

lazy_static! {
    static ref HVSOCK_STREAMS: Arc<Mutex<HashMap<u32, VsockStream>>> =
        Arc::new(Mutex::new(HashMap::new()));
}

// Convenience function to obtain the scope logger.
fn sl() -> slog::Logger {
    slog_scope::logger().new(o!("subsystem" => "passfd_io"))
}

pub(crate) async fn start_listen(port: u32) -> Result<()> {
    info!(sl(), "start listening on port {}", port);
    let mut listener = VsockListener::bind(libc::VMADDR_CID_ANY, port)?;
    tokio::spawn(async move {
        loop {
            if let Ok((stream, Vsock(addr))) = listener.accept().await {
                // We should insert the stream into the mapping as soon
                // to minimize the risk of encountering race conditions.
                let port = addr.port();

                // Write the host port back to the peer so the host side
                // (runtime-rs) can correlate this connection with the process
                // stdio when it connects over a plain hybrid-vsock UDS (CLH,
                // which does not support recv-fd).
                //
                // For the Dragonball passfd path this echoed line is simply
                // ignored by runtime-rs, which keeps waiting for the
                // "OK <port>" response produced by the hybrid-vsock recv-fd
                // module instead.
                let mut stream = stream;
                // The guest vsock send path may not be writable immediately after
                // accept (observed on CLH: the host never receives the echoed
                // hostport line). Run the echo in its own task with a timeout and
                // retries so that (a) the accept loop never blocks and (b) we can
                // observe whether the write ever makes it into the guest vsock
                // stack. Log the outcome on every attempt.
                tokio::spawn(async move {
                    let mut hostport_written = false;
                    for attempt in 0..5 {
                        let data = format!("{port}\n");
                        let write_fut = stream.write_all(data.as_bytes());
                        match tokio::time::timeout(
                            tokio::time::Duration::from_millis(500),
                            write_fut,
                        )
                        .await
                        {
                            Ok(Ok(_)) => {
                                hostport_written = true;
                                info!(sl(), "wrote hostport to peer {}", port);
                                break;
                            }
                            Ok(Err(e)) => warn!(
                                sl(),
                                "write hostport to peer {} attempt {} failed: {}", port, attempt, e
                            ),
                            Err(_) => warn!(
                                sl(),
                                "write hostport to peer {} attempt {} timed out", port, attempt
                            ),
                        }
                    }
                    if !hostport_written {
                        warn!(sl(), "give up writing hostport to peer {}", port);
                    }
                    HVSOCK_STREAMS.lock().await.insert(port, stream);
                    info!(sl(), "accept connection from peer port {}", port);
                });
            }
        }
    });
    Ok(())
}

async fn take_stream(port: u32) -> Option<VsockStream> {
    // There may be a race condition where the stream is accepted but
    // not yet inserted into the mapping. We will retry several times.
    // If it still fails, we just give up.
    let mut count = 0;
    while count < 3 {
        let stream = HVSOCK_STREAMS.lock().await.remove(&port);
        if stream.is_some() {
            return stream;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        count += 1;
    }

    warn!(sl(), "failed to take stream for port {}", port);
    None
}

macro_rules! take_io_stream {
    ($port: ident) => {
        if $port == 0 {
            None
        } else {
            take_stream($port).await
        }
    };
}

pub(crate) async fn take_io_streams(
    stdin_port: u32,
    stdout_port: u32,
    stderr_port: u32,
) -> ProcessIo {
    let stdin = take_io_stream!(stdin_port);
    let stdout = take_io_stream!(stdout_port);
    let stderr = take_io_stream!(stderr_port);
    debug!(
        sl(),
        "take passfd io streams {} {} {}", stdin_port, stdout_port, stderr_port
    );
    ProcessIo::new(stdin, stdout, stderr)
}
