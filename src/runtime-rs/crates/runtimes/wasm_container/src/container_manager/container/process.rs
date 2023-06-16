// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::io::{ContainerIo, ShimIo};
use crate::container_manager::logger_with_process;
use common::types::{ContainerProcess, ProcessExitStatus, ProcessStateInfo, ProcessStatus, PID};
use rustjail::process::Process as InnerProcess;

use anyhow::{Context, Result};
use awaitgroup::{WaitGroup, Worker as WaitGroupWorker};
use libc::pid_t;
use std::sync::Arc;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{watch, RwLock},
};

pub type ProcessWatcher = (
    Option<watch::Receiver<bool>>,
    Arc<RwLock<ProcessExitStatus>>,
);

#[derive(Debug)]
pub struct WasmProcess {
    pub process: ContainerProcess,
    pub inner_process: Option<InnerProcess>,
    pub pid: pid_t,
    pub bundle: String,

    pub stdin: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub terminal: bool,

    pub status: Arc<RwLock<ProcessStatus>>,
    pub exit_status: Arc<RwLock<ProcessExitStatus>>,
    pub exit_watcher_rx: Option<watch::Receiver<bool>>,
    pub exit_watcher_tx: Option<watch::Sender<bool>>,
    pub wg_input: WaitGroup,
    pub wg_output: WaitGroup,

    logger: slog::Logger,
}

impl WasmProcess {
    pub fn new(
        process: ContainerProcess,
        inner_process: Option<InnerProcess>,
        pid: pid_t,
        bundle: &str,
        stdin: Option<String>,
        stdout: Option<String>,
        stderr: Option<String>,
        terminal: bool,
    ) -> Self {
        let (sender, receiver) = watch::channel(false);
        let logger = logger_with_process(&process);

        Self {
            process: process,
            inner_process,
            pid,
            bundle: bundle.to_string(),
            stdin,
            stdout,
            stderr,
            terminal,
            status: Arc::new(RwLock::new(ProcessStatus::Created)),
            exit_status: Arc::new(RwLock::new(ProcessExitStatus::new())),
            exit_watcher_rx: Some(receiver),
            exit_watcher_tx: Some(sender),
            wg_input: WaitGroup::new(),
            wg_output: WaitGroup::new(),
            logger,
        }
    }

    pub async fn start_io_and_wait(&mut self, container_io: ContainerIo) -> Result<()> {
        info!(self.logger, "start io and wait");

        // new shim io
        let shim_io = ShimIo::new(&self.stdin, &self.stdout, &self.stderr)
            .await
            .context("new shim io")?;

        // start io copy for stdin
        let wgw_stdin = self.wg_input.worker();
        if let Some(stdin) = shim_io.stdin {
            self.run_io_copy("stdin", wgw_stdin, stdin, container_io.stdin)
                .await?;
        }

        // prepare for wait group for stdout, stderr
        let wgw = self.wg_output.worker();

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

    pub async fn state(&self) -> Result<ProcessStateInfo> {
        let exit_status = self.exit_status.read().await;

        Ok(ProcessStateInfo {
            container_id: self.process.container_id.container_id.clone(),
            exec_id: self.process.exec_id.clone(),
            pid: PID {
                pid: self.pid as u32,
            },
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

    pub async fn update_exited_status(&mut self, exit_code: i32) -> Result<()> {
        self.exit_status.write().await.update_exit_code(exit_code);
        self.set_status(ProcessStatus::Exited).await;

        // wait on all of the container's io stream terminated
        self.wg_output.wait().await;

        let _ = self.exit_watcher_tx.take();

        Ok(())
    }

    pub fn fetch_exit_watcher(&self) -> Result<ProcessWatcher> {
        Ok((self.exit_watcher_rx.clone(), self.exit_status.clone()))
    }

    pub async fn get_status(&self) -> ProcessStatus {
        let status = self.status.read().await;
        *status
    }

    pub async fn set_status(&mut self, new_status: ProcessStatus) {
        let mut status = self.status.write().await;
        *status = new_status;
    }

    pub async fn stop(&mut self) {
        let mut status = self.status.write().await;
        *status = ProcessStatus::Stopped;
    }
}
