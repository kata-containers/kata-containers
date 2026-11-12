// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2026 IBM Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::handler;
use anyhow::{anyhow, Context, Result};
use opentelemetry_sdk::trace::SpanProcessor;
use privdrop::PrivDrop;
use slog::{debug, error, o, Logger};
use std::future::Future;
use std::sync::Arc;
use tokio::io::AsyncRead;
use tokio::net::UnixListener;
use tokio::signal::unix::{signal, Signal, SignalKind};
use tokio_vsock::VsockListener;

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

    pub async fn start(&self) -> Result<()> {
        let signals = if self.dump_only {
            None
        } else {
            Some(termination_signals()?)
        };
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
                start_std_vsock(
                    self.logger.clone(),
                    processor.clone(),
                    *port,
                    *cid,
                    wait_for_termination(signals),
                )
                .await
            }
            VsockType::Hybrid { socket_path } => {
                start_hybrid_vsock(
                    self.logger.clone(),
                    processor.clone(),
                    socket_path,
                    wait_for_termination(signals),
                )
                .await
            }
        };
        finish_processing(processor, result).await
    }
}

fn termination_signals() -> Result<(Signal, Signal)> {
    Ok((
        signal(SignalKind::terminate()).context("register SIGTERM handler")?,
        signal(SignalKind::interrupt()).context("register SIGINT handler")?,
    ))
}

async fn wait_for_termination(signals: Option<(Signal, Signal)>) -> Result<()> {
    match signals {
        Some((mut terminate, mut interrupt)) => {
            tokio::select! {
                event = terminate.recv() => event.context("SIGTERM signal stream closed"),
                event = interrupt.recv() => event.context("SIGINT signal stream closed"),
            }
        }
        None => std::future::pending().await,
    }
}

async fn finish_processing(
    processor: Option<Arc<dyn SpanProcessor>>,
    result: Result<()>,
) -> Result<()> {
    let shutdown = match processor {
        Some(processor) => tokio::task::spawn_blocking(move || processor.shutdown())
            .await
            .context("trace shutdown worker failed")
            .and_then(|result| result.context("failed to shut down trace exporter")),
        None => Ok(()),
    };
    match (result, shutdown) {
        (Err(error), Err(shutdown)) => Err(error.context(shutdown)),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), shutdown) => shutdown,
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

async fn start_hybrid_vsock(
    logger: Logger,
    processor: Option<Arc<dyn SpanProcessor>>,
    socket_path: &str,
    shutdown: impl Future<Output = Result<()>>,
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

    serve_connections(logger, processor, listener, shutdown).await
}

async fn start_std_vsock(
    logger: Logger,
    processor: Option<Arc<dyn SpanProcessor>>,
    port: u32,
    cid: u32,
    shutdown: impl Future<Output = Result<()>>,
) -> Result<()> {
    let listener = VsockListener::bind(cid, port)?;

    debug!(logger, "Waiting for connections";
        "vsock-type" => "standard",
        "vsock-cid" => cid,
        "vsock-port" => port);

    serve_connections(logger, processor, listener, shutdown).await
}

trait TraceListener {
    type Connection: AsyncRead + Unpin;

    async fn accept_connection(&mut self) -> std::io::Result<Self::Connection>;
}

impl TraceListener for UnixListener {
    type Connection = tokio::net::UnixStream;

    async fn accept_connection(&mut self) -> std::io::Result<Self::Connection> {
        self.accept().await.map(|(connection, _)| connection)
    }
}

impl TraceListener for VsockListener {
    type Connection = tokio_vsock::VsockStream;

    async fn accept_connection(&mut self) -> std::io::Result<Self::Connection> {
        self.accept().await.map(|(connection, _)| connection)
    }
}

async fn serve_connections<L: TraceListener>(
    logger: Logger,
    processor: Option<Arc<dyn SpanProcessor>>,
    mut listener: L,
    shutdown: impl Future<Output = Result<()>>,
) -> Result<()> {
    tokio::pin!(shutdown);
    loop {
        let connection = tokio::select! {
            biased;
            result = &mut shutdown => return result,
            connection = listener.accept_connection() => connection.context("accept trace connection")?,
        };
        tokio::select! {
            biased;
            result = &mut shutdown => return result,
            result = handler::handle_connection(logger.clone(), connection, processor.clone()) => {
                if let Err(error) = result {
                    error!(logger, "failed to handle trace connection"; "error" => format!("{error:#}"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::tests::{agent_span, packet, RecordingExporter};
    use nix::sys::signal::{kill, Signal as PosixSignal};
    use nix::unistd::Pid;
    use opentelemetry_sdk::error::OTelSdkResult;
    use opentelemetry_sdk::trace::{BatchConfigBuilder, BatchSpanProcessor, SpanData};
    use std::convert::TryFrom;
    use std::io::Cursor;
    use std::process::Stdio;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;
    use tokio::sync::{oneshot, Notify};

    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    fn logger() -> Logger {
        Logger::root(slog::Discard, o!())
    }

    #[tokio::test]
    async fn malformed_connection_does_not_stop_the_next_guest() {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("traces.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let exporter = RecordingExporter::default();
        let processor: Arc<dyn SpanProcessor> =
            Arc::new(BatchSpanProcessor::builder(exporter.clone()).build());
        let server_processor = processor.clone();
        let (stop_tx, stop_rx) = oneshot::channel();
        let server = async {
            let result =
                serve_connections(logger(), Some(server_processor.clone()), listener, async {
                    stop_rx.await.context("test shutdown sender closed")
                })
                .await;
            finish_processing(Some(server_processor), result).await
        };
        let client = async {
            let mut malformed = UnixStream::connect(&socket).await.unwrap();
            malformed.write_all(&packet(b"not JSON")).await.unwrap();
            malformed.shutdown().await.unwrap();
            drop(malformed);

            let mut valid = UnixStream::connect(&socket).await.unwrap();
            valid
                .write_all(&packet(&serde_json::to_vec(&agent_span()).unwrap()))
                .await
                .unwrap();
            valid.shutdown().await.unwrap();
            drop(valid);

            loop {
                if exporter.spans.lock().unwrap().len() == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            stop_tx.send(()).unwrap();
        };
        let (result, ()) =
            tokio::time::timeout(TEST_TIMEOUT, async { tokio::join!(server, client) })
                .await
                .expect("server did not recover from the malformed connection");
        result.unwrap();
        assert_eq!(exporter.spans.lock().unwrap().len(), 1);
    }

    #[derive(Debug)]
    struct NotifyingProcessor {
        processor: BatchSpanProcessor,
        queued: Arc<Notify>,
        records: Arc<AtomicUsize>,
    }

    impl SpanProcessor for NotifyingProcessor {
        fn on_start(
            &self,
            span: &mut opentelemetry_sdk::trace::Span,
            context: &opentelemetry_otlp_api::Context,
        ) {
            self.processor.on_start(span, context);
        }

        fn on_end(&self, span: SpanData) {
            self.processor.on_end(span);
            self.records.fetch_add(1, Ordering::Relaxed);
            self.queued.notify_one();
        }

        fn force_flush(&self) -> OTelSdkResult {
            self.processor.force_flush()
        }

        fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
            self.processor.shutdown_with_timeout(timeout)
        }
    }

    #[tokio::test]
    async fn shutdown_flushes_queued_spans_during_a_partial_record() {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("traces.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let exporter = RecordingExporter::default();
        let queued = Arc::new(Notify::new());
        let config = BatchConfigBuilder::default()
            .with_scheduled_delay(Duration::from_secs(60))
            .build();
        let processor: Arc<dyn SpanProcessor> = Arc::new(NotifyingProcessor {
            processor: BatchSpanProcessor::builder(exporter.clone())
                .with_batch_config(config)
                .build(),
            queued: queued.clone(),
            records: Arc::new(AtomicUsize::new(0)),
        });
        let (stop_tx, stop_rx) = oneshot::channel();
        let (finished_tx, finished_rx) = oneshot::channel();
        let server = async {
            let result = serve_connections(logger(), Some(processor.clone()), listener, async {
                stop_rx.await.context("test shutdown sender closed")
            })
            .await;
            let result = finish_processing(Some(processor), result).await;
            finished_tx.send(()).unwrap();
            result
        };
        let client = async {
            let mut connection = UnixStream::connect(&socket).await.unwrap();
            let mut data = packet(&serde_json::to_vec(&agent_span()).unwrap());
            data.push(0);
            connection.write_all(&data).await.unwrap();
            queued.notified().await;
            assert!(exporter.spans.lock().unwrap().is_empty());
            stop_tx.send(()).unwrap();
            finished_rx.await.unwrap();
            drop(connection);
        };
        let (result, ()) =
            tokio::time::timeout(TEST_TIMEOUT, async { tokio::join!(server, client) })
                .await
                .expect("shutdown did not interrupt the partial record");
        result.unwrap();
        assert_eq!(exporter.spans.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn buffered_records_do_not_starve_shutdown() {
        let exporter = RecordingExporter::default();
        let queued = Arc::new(Notify::new());
        let records = Arc::new(AtomicUsize::new(0));
        let processor: Arc<dyn SpanProcessor> = Arc::new(NotifyingProcessor {
            processor: BatchSpanProcessor::builder(exporter.clone()).build(),
            queued: queued.clone(),
            records: records.clone(),
        });
        let frame = packet(&serde_json::to_vec(&agent_span()).unwrap());
        let mut input = Vec::new();
        for _ in 0..100 {
            input.extend_from_slice(&frame);
        }
        tokio::select! {
            biased;
            _ = queued.notified() => {}
            result = handler::handle_connection(logger(), Cursor::new(input), Some(processor.clone())) => {
                panic!("connection did not yield to shutdown: {:?}", result);
            }
        }
        assert_eq!(records.load(Ordering::Relaxed), 1);
        finish_processing(Some(processor), Ok(())).await.unwrap();
        assert_eq!(exporter.spans.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn shutdown_interrupts_an_idle_listener() {
        let directory = tempfile::tempdir().unwrap();
        let socket = directory.path().join("traces.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        let result = tokio::time::timeout(
            TEST_TIMEOUT,
            serve_connections(logger(), None, listener, async {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok(())
            }),
        )
        .await
        .expect("shutdown did not interrupt accept");
        result.unwrap();
    }

    #[tokio::test]
    async fn termination_signals_handle_sigint_and_sigterm() {
        const CHILD_MODE: &str = "KATA_TRACE_FORWARDER_TEST_SIGNAL_CHILD";
        const READY: &str = "KATA_TRACE_SIGNAL_READY";
        if std::env::var_os(CHILD_MODE).as_deref() == Some(std::ffi::OsStr::new("1")) {
            let signals = termination_signals().unwrap();
            println!("{}", READY);
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
            wait_for_termination(Some(signals)).await.unwrap();
            return;
        }

        for signal in [PosixSignal::SIGINT, PosixSignal::SIGTERM] {
            let mut child = tokio::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "server::tests::termination_signals_handle_sigint_and_sigterm",
                    "--exact",
                    "--nocapture",
                ])
                .env(CHILD_MODE, "1")
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
            tokio::time::timeout(TEST_TIMEOUT, async {
                while let Some(line) = lines.next_line().await.unwrap() {
                    if line.contains(READY) {
                        return;
                    }
                }
                panic!("child exited before registering signal handlers");
            })
            .await
            .expect("child did not register signal handlers");

            let pid = Pid::from_raw(i32::try_from(child.id().unwrap()).unwrap());
            kill(pid, signal).unwrap();
            let status = tokio::time::timeout(TEST_TIMEOUT, child.wait())
                .await
                .expect("child did not handle the termination signal")
                .unwrap();
            assert!(status.success(), "signal {:?}: {}", signal, status);
        }
    }
}
