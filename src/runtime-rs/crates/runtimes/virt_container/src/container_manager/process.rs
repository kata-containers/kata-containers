// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::sync::Arc;

use agent::Agent;
use anyhow::{Context, Result};
use awaitgroup::{WaitGroup, Worker as WaitGroupWorker};
use common::types::{ContainerProcess, ProcessExitStatus, ProcessStateInfo, ProcessStatus, PID};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{watch, RwLock};

use super::container::Container;
use super::io::{ContainerIo, PassfdIo, ShimIo};
use super::logger_with_process;

pub type ProcessWatcher = (
    Option<watch::Receiver<bool>>,
    Arc<RwLock<ProcessExitStatus>>,
);

#[derive(Debug)]
pub struct Process {
    pub process: ContainerProcess,
    pub pid: u32,
    logger: slog::Logger,
    pub bundle: String,

    pub stdin: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub terminal: bool,

    pub height: u32,
    pub width: u32,
    pub status: Arc<RwLock<ProcessStatus>>,

    pub exit_status: Arc<RwLock<ProcessExitStatus>>,
    pub exit_watcher_rx: Option<watch::Receiver<bool>>,
    pub exit_watcher_tx: Option<watch::Sender<bool>>,
    // used to sync between stdin io copy thread(tokio) and the close it call.
    // close io call should wait until the stdin io copy finished to
    // prevent stdin data lost.
    pub wg_stdin: WaitGroup,

    // io streams using vsock fd passthrough feature
    pub passfd_io: Option<PassfdIo>,
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
            terminal,
            height: 0,
            width: 0,
            status: Arc::new(RwLock::new(ProcessStatus::Created)),
            exit_status: Arc::new(RwLock::new(ProcessExitStatus::new())),
            exit_watcher_rx: Some(receiver),
            exit_watcher_tx: Some(sender),
            wg_stdin: WaitGroup::new(),
            passfd_io: None,
        }
    }

    /// Init the `passfd_io` struct and vsock connections for io to the agent.
    pub async fn passfd_io_init(&mut self, hvsock_uds_path: &str, passfd_port: u32) -> Result<()> {
        info!(self.logger, "passfd io init");

        let mut passfd_io =
            PassfdIo::new(self.stdin.clone(), self.stdout.clone(), self.stderr.clone()).await;

        passfd_io
            .open_and_passfd(hvsock_uds_path, passfd_port, self.terminal)
            .await
            .context("passfd connect")?;

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
    ) -> Result<()> {
        let logger = self.logger.clone();
        info!(logger, "start passfd io wait");
        let process = self.process.clone();
        let exit_status = self.exit_status.clone();
        let exit_notifier = self.exit_watcher_tx.take();
        let status = self.status.clone();

        tokio::spawn(async move {
            let req = agent::WaitProcessRequest {
                process_id: process.clone().into(),
            };

            info!(logger, "begin passfd io wait process");
            let resp = match agent.wait_process(req).await {
                Ok(ret) => ret,
                Err(e) => {
                    error!(logger, "failed to passfd io wait process {:?}", e);
                    return;
                }
            };

            info!(
                logger,
                "end passfd io wait process exit code {}", resp.status
            );

            let containers = containers.read().await;
            let container_id = &process.container_id.container_id;
            let c = match containers.get(container_id) {
                Some(c) => c,
                None => {
                    error!(
                        logger,
                        "Failed to stop process, since container {} not found", container_id
                    );
                    return;
                }
            };

            if let Err(err) = c.stop_process(&process).await {
                error!(
                    logger,
                    "Failed to stop process, process = {:?}, err = {:?}", process, err
                );
            }

            let mut exit_status = exit_status.write().await;
            exit_status.update_exit_code(resp.status);
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
    ) -> Result<()> {
        info!(self.logger, "start io and wait");

        // new shim io
        let shim_io = ShimIo::new(&self.stdin, &self.stdout, &self.stderr)
            .await
            .context("new shim io")?;

        // start io copy for stdin
        let wgw_stdin = self.wg_stdin.worker();
        if let Some(stdin) = shim_io.stdin {
            self.run_io_copy("stdin", wgw_stdin, stdin, container_io.stdin)
                .await?;
        }

        // prepare for wait group for stdout, stderr
        let wg = WaitGroup::new();
        let wgw = wg.worker();

        // start io copy for stdout
        if let Some(stdout) = shim_io.stdout {
            self.run_io_copy("stdout", wgw.clone(), container_io.stdout, stdout)
                .await?;
        }

        // start io copy for stderr
        if !self.terminal {
            if let Some(stderr) = shim_io.stderr {
                self.run_io_copy("stderr", wgw, container_io.stderr, stderr)
                    .await?;
            }
        }

        self.run_io_wait(containers, agent, wg)
            .await
            .context("run io thread")?;
        Ok(())
    }

    async fn run_io_copy<'a>(
        &'a self,
        io_name: &'a str,
        wgw: WaitGroupWorker,
        mut reader: Box<dyn AsyncRead + Send + Unpin>,
        mut writer: Box<dyn AsyncWrite + Send + Unpin>,
    ) -> Result<()> {
        info!(self.logger, "run io copy for {}", io_name);
        let io_name = io_name.to_string();
        let logger = self.logger.new(o!("io_name" => io_name));
        tokio::spawn(async move {
            match tokio::io::copy(&mut reader, &mut writer).await {
                Err(e) => {
                    warn!(logger, "run_io_copy: failed to copy stream: {}", e);
                }
                Ok(length) => {
                    info!(logger, "run_io_copy: stop to copy stream length {}", length)
                }
            };

            wgw.done();
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

            let req = agent::WaitProcessRequest {
                process_id: process.clone().into(),
            };

            info!(logger, "begin wait process");
            let resp = match agent.wait_process(req).await {
                Ok(ret) => ret,
                Err(e) => {
                    error!(logger, "failed to wait process {:?}", e);
                    return;
                }
            };

            info!(logger, "end wait process exit code {}", resp.status);

            let containers = containers.read().await;
            let container_id = &process.container_id.container_id;
            let c = match containers.get(container_id) {
                Some(c) => c,
                None => {
                    error!(
                        logger,
                        "Failed to stop process, since container {} not found", container_id
                    );
                    return;
                }
            };

            if let Err(err) = c.stop_process(&process).await {
                error!(
                    logger,
                    "Failed to stop process, process = {:?}, err = {:?}", process, err
                );
            }

            let mut exit_status = exit_status.write().await;
            exit_status.update_exit_code(resp.status);
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
    pub async fn close_io(&mut self, agent: Arc<dyn Agent>) {
        // In passfd io mode, the stdin close and sync logic is handled
        // in the agent side.
        if self.passfd_io.is_none() {
            self.wg_stdin.wait().await;
        }

        let req = agent::CloseStdinRequest {
            process_id: self.process.clone().into(),
        };

        if let Err(e) = agent.close_stdin(req).await {
            warn!(self.logger, "failed close process io: {:?}", e);
        }
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
