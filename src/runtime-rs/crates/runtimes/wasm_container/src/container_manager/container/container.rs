// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::container_inner::ContainerInner;
use super::io::ContainerIo;
use super::process::ProcessWatcher;
use anyhow::{anyhow, Result};
use common::types::{
    ContainerConfig, ContainerID, ContainerProcess, ExecProcessRequest, ProcessStateInfo,
    ProcessStatus, ProcessType, StatsInfo,
};
use oci::LinuxResources;

use libc::pid_t;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Container {
    _pid: u32,
    pub container_id: ContainerID,
    inner: Arc<RwLock<ContainerInner>>,
    logger: slog::Logger,
}

impl Container {
    pub fn new(pid: u32, config: ContainerConfig, spec: oci::Spec) -> Result<Self> {
        let container_id = config.container_id.clone();

        let logger = sl!().new(o!("container_id" => container_id.clone()));

        let container_inner = ContainerInner::new(config, spec, logger.clone())?;

        Ok(Self {
            _pid: pid,
            container_id: ContainerID::new(&container_id)?,
            inner: Arc::new(RwLock::new(container_inner)),
            logger,
        })
    }

    pub async fn create(&self) -> Result<()> {
        info!(self.logger, "wasm container creating");

        let mut inner = self.inner.write().await;

        inner.create().await?;

        info!(self.logger, "wasm container created");

        Ok(())
    }

    pub async fn start(&self, process: &ContainerProcess) -> Result<()> {
        info!(self.logger, "wasm container starting");

        let mut inner = self.inner.write().await;

        match process.process_type {
            ProcessType::Container => {
                inner.start().await?;

                let container_io = ContainerIo::new(self.inner.clone(), process.clone());
                inner.init_process.start_io_and_wait(container_io).await?;
            }
            ProcessType::Exec => {
                inner.start_exec_process(process).await?;

                let container_io = ContainerIo::new(self.inner.clone(), process.clone());

                let exec_process = inner
                    .exec_processes
                    .get_mut(&process.exec_id)
                    .ok_or(anyhow!("no exec process"))?;
                exec_process.start_io_and_wait(container_io).await?;
            }
        };

        info!(self.logger, "wasm container started");

        Ok(())
    }

    pub async fn stats(&self) -> Result<StatsInfo> {
        info!(self.logger, "wasm container stating");

        let inner = self.inner.read().await;
        let stats = inner.stats().await?;

        info!(self.logger, "wasm container stated");

        Ok(stats)
    }

    pub async fn pause(&self) -> Result<()> {
        info!(self.logger, "wasm container pausing");

        let mut inner = self.inner.write().await;
        inner.pause().await?;

        info!(self.logger, "wasm container paused");

        Ok(())
    }

    pub async fn resume(&self) -> Result<()> {
        info!(self.logger, "wasm container resuming");

        let mut inner = self.inner.write().await;
        inner.resume().await?;

        info!(self.logger, "wasm container resumed");

        Ok(())
    }

    pub async fn update(&self, resources: LinuxResources) -> Result<()> {
        info!(self.logger, "wasm container updating");

        let mut inner = self.inner.write().await;
        inner.update(resources).await?;

        info!(self.logger, "wasm container updated");

        Ok(())
    }

    pub async fn create_exec_process(&self, req: ExecProcessRequest) -> Result<()> {
        info!(self.logger, "wasm container creating exec process");

        let mut inner = self.inner.write().await;
        inner.create_exec_process(req).await?;

        info!(self.logger, "wasm container created exec process");

        Ok(())
    }

    pub async fn delete_exec_process(&self, process: &ContainerProcess) -> Result<()> {
        info!(self.logger, "wasm process deleting");

        let mut inner = self.inner.write().await;
        inner.delete_exec_process(&process).await?;

        info!(self.logger, "wasm process deleted");

        Ok(())
    }

    pub async fn stop_process(&self, process: &ContainerProcess) -> Result<()> {
        info!(self.logger, "wasm process stopping");

        let mut inner = self.inner.write().await;
        inner.stop_process(process).await?;

        info!(self.logger, "wasm process stopped");

        Ok(())
    }

    pub async fn close_io(&self, process: &ContainerProcess) -> Result<()> {
        info!(self.logger, "wasm container closing input");

        let mut inner = self.inner.write().await;
        inner.close_io(process).await?;

        info!(self.logger, "wasm container closed input");

        Ok(())
    }

    pub async fn signal_process(
        &self,
        process: &ContainerProcess,
        signal: u32,
        all: bool,
    ) -> Result<()> {
        info!(self.logger, "wasm container sending signal");

        let mut inner = self.inner.write().await;
        inner.signal_process(process, signal, all).await?;

        info!(self.logger, "wasm container sent signal");

        Ok(())
    }

    pub async fn resize_pty(
        &self,
        process: &ContainerProcess,
        width: u32,
        height: u32,
    ) -> Result<()> {
        info!(self.logger, "wasm resizing pty");

        let mut inner = self.inner.write().await;

        if inner.init_process.get_status().await != ProcessStatus::Running {
            warn!(self.logger, "wasm container is not running");
            return Ok(());
        }
        inner.win_resize_process(process, height, width).await?;

        info!(self.logger, "wasm resized pty");

        Ok(())
    }

    pub async fn state_process(&self, process: &ContainerProcess) -> Result<ProcessStateInfo> {
        let inner = self.inner.read().await;
        inner.state_process(process).await
    }

    pub async fn fetch_exit_watcher(&self, process: &ContainerProcess) -> Result<ProcessWatcher> {
        let inner = self.inner.read().await;
        inner.fetch_exit_watcher(process)
    }

    pub async fn find_process_by_pid(&self, pid: pid_t) -> Option<ContainerProcess> {
        let inner = self.inner.read().await;
        inner.find_process_by_pid(&self.container_id, pid)
    }

    pub async fn update_exited_process(
        &self,
        process: &ContainerProcess,
        exit_code: i32,
    ) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.update_exited_process(process, exit_code).await
    }
}
