// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::{fs::File, os::unix::fs::OpenOptionsExt, sync::Arc};

use agent::Agent;
use anyhow::{Context, Result};
use awaitgroup::{WaitGroup, Worker as WaitGroupWorker};
use common::types::{ContainerProcess, ProcessExitStatus, ProcessStateInfo, ProcessStatus, PID};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{watch, RwLock};
use tokio_util::sync::CancellationToken;

use super::container::Container;
use super::io::{BinaryLogger, ContainerIo, PassfdIo, ShimIo};
use super::logger_with_process;
use crate::UNKNOWN_EXIT_STATUS;

pub type ProcessWatcher = (
    Option<watch::Receiver<bool>>,
    Arc<RwLock<ProcessExitStatus>>,
);

#[derive(Debug, PartialEq)]
enum StdIoType {
    Stdin,
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub struct Process {
    pub process: ContainerProcess,
    pub pid: u32,
    logger: slog::Logger,
    pub bundle: String,

    pub stdin: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,

    // In linux, when a FIFO is opened and there are no writers, the reader
    // will continuously receive the HUP event. This can be problematic.
    // To avoid this problem, we open stdin in write mode and keep the stdin-writer
    pub stdin_w: Option<File>,
    // We need to open the stdout as the read mode and keep the open endpoint
    // until the process is delete. otherwise,
    // the process would exit before the containerd side open and read
    // the stdout fifo, thus Kata would write all of the stdout contents into
    // the stdout fifo and then closed the write endpoint. Then, containerd
    // open the stdout fifo and try to read, since the write side had closed,
    // thus containerd would block on the read forever.
    // Here we keep the stdout/stderr read endpoint File in the process struct,
    // which would be destroied when containerd send the delete rpc call,
    // at this time the containerd had waited the stdout read return, thus it
    // can make sure the contents in the stdout/stderr fifo wouldn't be lost.
    pub stdout_r: Option<File>,
    // The purpose is the same as stdout_r
    pub stderr_r: Option<File>,

    pub terminal: bool,

    pub height: u32,
    pub width: u32,
    pub status: Arc<RwLock<ProcessStatus>>,

    pub exit_status: Arc<RwLock<ProcessExitStatus>>,
    pub exit_watcher_rx: Option<watch::Receiver<bool>>,
    pub exit_watcher_tx: Option<watch::Sender<bool>>,

    // io streams using vsock fd passthrough feature
    pub passfd_io: Option<PassfdIo>,
}

fn open_fifo(path: &str, is_read: bool, is_write: bool) -> Result<File> {
    let file = std::fs::OpenOptions::new()
        .read(is_read)
        .write(is_write)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)?;

    Ok(file)
}

fn open_fifo_read(path: &str) -> Result<File> {
    open_fifo(path, true, false)
}

fn open_fifo_write(path: &str) -> Result<File> {
    open_fifo(path, false, true)
}

fn is_binary_stdio(value: &str) -> bool {
    url::Url::parse(value)
        .map(|uri| uri.scheme() == "binary")
        .unwrap_or(false)
}

// A broken WaitProcess connection says nothing about the guest process.
// Keep waiting until the agent reports an exit or the VMM exits.
async fn wait_for_exit_code(
    agent: &dyn Agent,
    process: &ContainerProcess,
    vmm_exit_confirmed: &CancellationToken,
    logger: &slog::Logger,
) -> i32 {
    let request = agent::WaitProcessRequest {
        process_id: process.clone().into(),
    };
    tokio::select! {
        biased;
        result = super::retry_on_disconnect(|| agent.wait_process(request.clone())) => match result {
            Ok(response) => {
                info!(logger, "process exited with status {}", response.status);
                response.status
            }
            Err(error) => {
                error!(logger, "failed to wait for process: {error:#}");
                UNKNOWN_EXIT_STATUS
            }
        },
        _ = vmm_exit_confirmed.cancelled() => UNKNOWN_EXIT_STATUS,
    }
}

impl Process {
    pub fn new(
        process: &ContainerProcess,
        pid: u32,
        bundle: &str,
        stdin: Option<String>,
        stdout: Option<String>,
        stderr: Option<String>,
        terminal: bool,
    ) -> Process {
        let (sender, receiver) = watch::channel(false);

        Process {
            process: process.clone(),
            pid,
            logger: logger_with_process(process),
            bundle: bundle.to_string(),
            stdin,
            stdout,
            stderr,
            stdin_w: None,
            stdout_r: None,
            stderr_r: None,
            terminal,
            height: 0,
            width: 0,
            status: Arc::new(RwLock::new(ProcessStatus::Created)),
            exit_status: Arc::new(RwLock::new(ProcessExitStatus::new())),
            exit_watcher_rx: Some(receiver),
            exit_watcher_tx: Some(sender),
            passfd_io: None,
        }
    }

    pub fn pre_fifos_open(&mut self) -> Result<()> {
        if let Some(ref stdout) = self.stdout {
            if !is_binary_stdio(stdout) {
                self.stdout_r = Some(open_fifo_read(stdout).context("open stdout")?);
            }
        }

        if !self.terminal {
            if let Some(ref stderr) = self.stderr {
                if !is_binary_stdio(stderr) {
                    self.stderr_r = Some(open_fifo_read(stderr).context("open stderr")?);
                }
            }
        }

        Ok(())
    }

    pub fn post_fifos_open(&mut self) -> Result<()> {
        if let Some(ref stdin) = self.stdin {
            self.stdin_w = Some(open_fifo_write(stdin)?);
        }
        Ok(())
    }

    /// Init the `passfd_io` struct and vsock connections for io to the agent.
    pub async fn passfd_io_init(&mut self, hvsock_uds_path: &str, passfd_port: u32) -> Result<()> {
        info!(self.logger, "passfd io init");

        // Binary loggers need the host shim to feed their pipes, so this
        // process uses the legacy agent streams instead of passfd.
        if self.stdout.as_deref().is_some_and(is_binary_stdio)
            || self.stderr.as_deref().is_some_and(is_binary_stdio)
        {
            info!(self.logger, "binary logger disables passfd io");
            return Ok(());
        }

        let mut passfd_io =
            PassfdIo::new(self.stdin.clone(), self.stdout.clone(), self.stderr.clone()).await;

        self.pre_fifos_open()?;
        passfd_io
            .open_and_passfd(hvsock_uds_path, passfd_port, self.terminal)
            .await
            .context("passfd connect")?;
        self.post_fifos_open()?;

        self.passfd_io = Some(passfd_io);

        Ok(())
    }

    /// (After process started) Send a WaitProcessRequest to agent in the
    /// seperate thread.
    /// This function is only used in passfd io mode.
    pub async fn passfd_io_wait(
        &mut self,
        containers: Arc<RwLock<HashMap<String, Container>>>,
        agent: Arc<dyn Agent>,
        vmm_exit_confirmed: CancellationToken,
    ) -> Result<()> {
        let logger = self.logger.clone();
        info!(logger, "start passfd io wait");
        let process = self.process.clone();
        let exit_status = self.exit_status.clone();
        let exit_notifier = self.exit_watcher_tx.take();
        let status = self.status.clone();

        tokio::spawn(async move {
            let exit_code =
                wait_for_exit_code(agent.as_ref(), &process, &vmm_exit_confirmed, &logger).await;

            let containers = containers.read().await;
            let container_id = &process.container_id.container_id;
            if let Some(c) = containers.get(container_id) {
                if let Err(err) = c.stop_process(&process).await {
                    error!(
                        logger,
                        "Failed to stop process, process = {:?}, err = {:?}", process, err
                    );
                }
            } else {
                error!(
                    logger,
                    "Failed to stop process, since container {} not found", container_id
                );
            }

            let mut exit_status = exit_status.write().await;
            exit_status.update_exit_code(exit_code);
            drop(exit_status);

            let mut status = status.write().await;
            *status = ProcessStatus::Stopped;
            drop(status);

            drop(exit_notifier);
            info!(logger, "end passfd io wait thread");
        });
        Ok(())
    }

    pub async fn start_io_and_wait(
        &mut self,
        containers: Arc<RwLock<HashMap<String, Container>>>,
        agent: Arc<dyn Agent>,
        container_io: ContainerIo,
        vmm_exit_confirmed: CancellationToken,
    ) -> Result<()> {
        info!(self.logger, "start io and wait");

        self.pre_fifos_open()?;
        // new shim io
        let shim_io = ShimIo::new(
            &self.stdin,
            &self.stdout,
            &self.stderr,
            self.process.container_id(),
            &std::env::var("NAMESPACE").unwrap_or_default(),
        )
        .await
        .context("new shim io")?;
        self.post_fifos_open()?;

        // start io copy for stdin
        if let Some(stdin) = shim_io.stdin {
            self.run_io_copy(StdIoType::Stdin, None, stdin, container_io.stdin)
                .await?;
        }

        // prepare for wait group for stdout, stderr
        let wg = WaitGroup::new();
        let wgw = wg.worker();

        // start io copy for stdout
        if let Some(stdout) = shim_io.stdout {
            self.run_io_copy(
                StdIoType::Stdout,
                Some(wgw.clone()),
                container_io.stdout,
                stdout,
            )
            .await?;
        }

        // start io copy for stderr
        if !self.terminal {
            if let Some(stderr) = shim_io.stderr {
                self.run_io_copy(StdIoType::Stderr, Some(wgw), container_io.stderr, stderr)
                    .await?;
            }
        }

        self.run_io_wait(
            containers,
            agent,
            wg,
            shim_io.binary_logger,
            vmm_exit_confirmed,
        )
        .await
        .context("run io thread")?;
        Ok(())
    }

    async fn run_io_copy(
        &self,
        io_type: StdIoType,
        wgw: Option<WaitGroupWorker>,
        mut reader: Box<dyn AsyncRead + Send + Unpin>,
        mut writer: Box<dyn AsyncWrite + Send + Unpin>,
    ) -> Result<()> {
        let io_name = format!("{io_type:?}");

        info!(self.logger, "run_io_copy[{}] starts", io_name);
        let logger = self.logger.new(o!("io_name" => io_name.clone()));

        tokio::spawn(async move {
            match tokio::io::copy(&mut reader, &mut writer).await {
                Err(e) => {
                    warn!(
                        logger,
                        "run_io_copy[{}]: failed to copy stream: {}", io_name, e
                    );
                }
                Ok(length) => {
                    info!(
                        logger,
                        "run_io_copy[{}]: stop to copy stream length {}", io_name, length
                    );
                    // Send EOF to agent by calling rpc write_stdin with 0 length data
                    if io_type == StdIoType::Stdin {
                        writer
                            .shutdown()
                            .await
                            .map_err(|e| {
                                error!(
                                    logger,
                                    "run_io_copy[{}]: failed to shutdown: {:?}", io_name, e
                                );
                                e
                            })
                            .ok();
                    }
                }
            };

            // Close the destination before notifying the waiter. Binary
            // loggers must see pipe EOF before their shutdown signal.
            drop(writer);
            if let Some(w) = wgw {
                w.done()
            }
        });

        Ok(())
    }

    /// A container is considered exited once its IO ended.
    /// This function waits for IO to end. And then, do some cleanup
    /// things.
    async fn run_io_wait(
        &mut self,
        containers: Arc<RwLock<HashMap<String, Container>>>,
        agent: Arc<dyn Agent>,
        mut wg: WaitGroup,
        binary_logger: Option<BinaryLogger>,
        vmm_exit_confirmed: CancellationToken,
    ) -> Result<()> {
        let logger = self.logger.clone();
        info!(logger, "start run io wait");
        let process = self.process.clone();
        let exit_status = self.exit_status.clone();
        let exit_notifier = self.exit_watcher_tx.take();
        let status = self.status.clone();

        tokio::spawn(async move {
            // wait on all of the container's io stream terminated
            info!(logger, "begin wait group io");
            wg.wait().await;
            info!(logger, "end wait group for io");

            if let Some(binary_logger) = binary_logger {
                binary_logger.shutdown().await;
            }

            let exit_code =
                wait_for_exit_code(agent.as_ref(), &process, &vmm_exit_confirmed, &logger).await;

            let containers = containers.read().await;
            let container_id = &process.container_id.container_id;
            if let Some(c) = containers.get(container_id) {
                if let Err(err) = c.stop_process(&process).await {
                    error!(
                        logger,
                        "Failed to stop process, process = {:?}, err = {:?}", process, err
                    );
                }
            } else {
                error!(
                    logger,
                    "Failed to stop process, since container {} not found", container_id
                );
            }

            let mut exit_status = exit_status.write().await;
            exit_status.update_exit_code(exit_code);
            drop(exit_status);

            let mut status = status.write().await;
            *status = ProcessStatus::Stopped;
            drop(status);

            drop(exit_notifier);
            info!(logger, "end io wait thread");
        });
        Ok(())
    }

    pub fn fetch_exit_watcher(&self) -> Result<ProcessWatcher> {
        Ok((self.exit_watcher_rx.clone(), self.exit_status.clone()))
    }

    pub async fn state(&self) -> Result<ProcessStateInfo> {
        let exit_status = self.exit_status.read().await;
        Ok(ProcessStateInfo {
            container_id: self.process.container_id.container_id.clone(),
            exec_id: self.process.exec_id.clone(),
            pid: PID { pid: self.pid },
            bundle: self.bundle.clone(),
            stdin: self.stdin.clone(),
            stdout: self.stdout.clone(),
            stderr: self.stderr.clone(),
            terminal: self.terminal,
            status: self.get_status().await,
            exit_status: exit_status.exit_code,
            exited_at: exit_status.exit_time,
        })
    }

    pub async fn stop(&mut self) {
        let mut status = self.status.write().await;
        *status = ProcessStatus::Stopped;
    }

    /// Close the stdin of the process in container.
    pub async fn close_io(&mut self, _agent: Arc<dyn Agent>) {
        // Close the stdin writer keeper so that
        // the end signal could be received in the read side
        self.stdin_w.take();

        // The stdin will be closed when EOF is got in rpc `read_stdout` of agent
        // so we will not call agent.close_stdin anymore.
    }

    pub async fn get_status(&self) -> ProcessStatus {
        let status = self.status.read().await;
        *status
    }

    pub async fn set_status(&self, new_status: ProcessStatus) {
        let mut status = self.status.write().await;
        *status = new_status;
    }
}

#[cfg(test)]
mod tests {
    use super::{is_binary_stdio, UNKNOWN_EXIT_STATUS};

    #[test]
    fn identifies_binary_logger_uri() {
        assert!(is_binary_stdio(
            "binary:///usr/bin/logger?config=%2Frun%2Flog.json"
        ));
        assert!(!is_binary_stdio("/run/containerd/io/stdout"));
        assert!(!is_binary_stdio("file:///run/container.log"));
    }

    #[rstest::rstest]
    #[case::passfd_recovers(true, Some(ttrpc::Code::OK), 42, &[])]
    #[case::legacy_io_recovers(false, Some(ttrpc::Code::OK), 42, &[])]
    #[case::repeated_disconnects(true, Some(ttrpc::Code::OK), 42, &[1, 2])]
    #[case::already_reaped(true, Some(ttrpc::Code::NOT_FOUND), UNKNOWN_EXIT_STATUS, &[])]
    #[case::vm_exited(false, None, UNKNOWN_EXIT_STATUS, &[])]
    #[tokio::test]
    async fn wait_process_after_disconnect(
        #[case] passfd: bool,
        #[case] reply: Option<ttrpc::Code>,
        #[case] expected_status: i32,
        #[case] backoff_secs: &[u64],
    ) {
        use super::*;
        use agent::{kata::KataAgent, AgentManager};
        use std::time::Duration;
        use tokio::io::AsyncReadExt;
        use tokio::net::UnixListener;
        use ttrpc::proto::{Codec, GenMessage, MessageHeader};

        tokio::time::timeout(Duration::from_secs(15), async {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("agent.sock");
            let listener = UnixListener::bind(&path).unwrap();
            let agent = Arc::new(KataAgent::new(Default::default()));
            agent
                .start(&format!("remote://{}", path.display()))
                .await
                .unwrap();
            agent.stop().await; // Stop the log forwarder.
            let vmm_exit = CancellationToken::new();
            let id = ContainerProcess::new("container", "exec").unwrap();
            let mut process = Process::new(&id, 0, "", None, None, None, false);
            process.set_status(ProcessStatus::Running).await;
            let (watcher, exit_status) = process.fetch_exit_watcher().unwrap();
            let mut watcher = watcher.unwrap();
            let containers = Arc::new(RwLock::new(HashMap::new()));
            let mut outputs = Vec::new();
            if passfd {
                process
                    .passfd_io_wait(containers, agent, vmm_exit.clone())
                    .await
                    .unwrap();
            } else {
                let wg = WaitGroup::new();
                let io = ContainerIo::new(agent.clone(), id, vmm_exit.clone());
                for (kind, reader) in [
                    (StdIoType::Stdout, io.stdout),
                    (StdIoType::Stderr, io.stderr),
                ] {
                    let (output, sink) = tokio::io::duplex(64);
                    outputs.push(output);
                    process
                        .run_io_copy(kind, Some(wg.worker()), reader, Box::new(sink))
                        .await
                        .unwrap();
                }
                process
                    .run_io_wait(containers, agent, wg, None, vmm_exit.clone())
                    .await
                    .unwrap();
            }

            let (mut socket, _) = listener.accept().await.unwrap();
            if !passfd {
                // Interrupt both in-flight reads before either output worker reaches EOF.
                let read_requests = async |socket: &mut tokio::net::UnixStream| {
                    let mut requests = Vec::new();
                    for _ in 0..2 {
                        let msg = GenMessage::read_from(&mut *socket).await.unwrap();
                        requests.push((
                            ttrpc::Request::decode(msg.payload).unwrap().method,
                            msg.header.stream_id,
                        ));
                    }
                    requests.sort();
                    assert_eq!(requests[0].0, "ReadStderr");
                    assert_eq!(requests[1].0, "ReadStdout");
                    requests
                };
                read_requests(&mut socket).await;
                drop(socket);
                (socket, _) = listener.accept().await.unwrap();
                for eof in [false, true] {
                    for (method, stream_id) in read_requests(&mut socket).await {
                        let mut response = ttrpc::Response::default();
                        if !eof {
                            // ReadStreamResponse.data is a length-delimited field.
                            response.payload =
                                vec![0x0a, 1, if method == "ReadStdout" { b'O' } else { b'E' }];
                        }
                        let payload = response.encode().unwrap();
                        GenMessage {
                            header: MessageHeader::new_response(stream_id, payload.len() as u32),
                            payload,
                        }
                        .write_to(&mut socket)
                        .await
                        .unwrap();
                    }
                }
                for (mut output, expected) in outputs.into_iter().zip(*b"OE") {
                    let mut data = Vec::new();
                    output.read_to_end(&mut data).await.unwrap();
                    assert_eq!(data, [expected]);
                }
            }
            let first = GenMessage::read_from(&mut socket).await.unwrap();
            assert_eq!(
                ttrpc::Request::decode(first.payload.clone())
                    .unwrap()
                    .method,
                "WaitProcess"
            );
            drop(socket);

            let (mut socket, _) = listener.accept().await.unwrap();
            for &secs in backoff_secs {
                let interrupted = GenMessage::read_from(&mut socket).await.unwrap();
                assert_eq!(first.payload, interrupted.payload);
                let disconnected_at = tokio::time::Instant::now();
                drop(socket);
                (socket, _) = listener.accept().await.unwrap();
                assert!(disconnected_at.elapsed() >= Duration::from_secs(secs));
            }
            let retry = GenMessage::read_from(&mut socket).await.unwrap();
            assert_eq!(first.payload, retry.payload);
            assert_eq!(process.get_status().await, ProcessStatus::Running);
            assert!(!watcher.has_changed().unwrap());

            if let Some(code) = reply {
                let mut response = ttrpc::Response::default();
                response.set_status(ttrpc::get_status(code, ""));
                if code == ttrpc::Code::OK {
                    // Wire encoding of WaitProcessResponse { status: 42 }.
                    response.payload = vec![0x08, 42];
                }
                let payload = response.encode().unwrap();
                GenMessage {
                    header: MessageHeader::new_response(
                        retry.header.stream_id,
                        payload.len() as u32,
                    ),
                    payload,
                }
                .write_to(&mut socket)
                .await
                .unwrap();
            } else {
                vmm_exit.cancel();
            }

            watcher
                .changed()
                .await
                .expect_err("exit notifier must close");
            assert_eq!(exit_status.read().await.exit_code, expected_status);
            assert_eq!(process.get_status().await, ProcessStatus::Stopped);
        })
        .await
        .expect("WaitProcess did not finish");
    }

    #[tokio::test]
    async fn test_passfd_io_wait_agent_error() {
        use super::*;
        use std::time::Duration;

        let container_process = ContainerProcess::new("container", "").unwrap();
        let mut process = Process::new(&container_process, 0, "", None, None, None, false);
        let (watcher, exit_status) = process.fetch_exit_watcher().unwrap();
        let agent: Arc<dyn Agent> = Arc::new(agent::kata::KataAgent::new(Default::default()));

        process
            .passfd_io_wait(
                Arc::new(RwLock::new(HashMap::new())),
                agent,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let mut watcher = watcher.unwrap();
        tokio::time::timeout(Duration::from_secs(30), watcher.changed())
            .await
            .expect("the exit is reported")
            .unwrap_err();

        assert_eq!(exit_status.read().await.exit_code, UNKNOWN_EXIT_STATUS);
        assert_eq!(process.get_status().await, ProcessStatus::Stopped);
    }
}
