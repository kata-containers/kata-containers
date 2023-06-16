// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::container::container::Container;
use crate::sandbox::sandbox::WasmSandbox;

use common::{
    error::Error,
    types::{
        ContainerConfig, ContainerID, ContainerProcess, ExecProcessRequest, KillRequest,
        ProcessExitStatus, ProcessStateInfo, ProcessType, ResizePTYRequest, ShutdownRequest,
        StatsInfo, UpdateRequest, PID,
    },
    ContainerManager,
};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct WasmContainerManager {
    sandbox: Arc<WasmSandbox>,
    pid: u32,
    containers: Arc<RwLock<HashMap<String, Container>>>,
}

impl WasmContainerManager {
    pub async fn new(
        sandbox: Arc<WasmSandbox>,
        pid: u32,
        containers: Arc<RwLock<HashMap<String, Container>>>,
    ) -> Self {
        Self {
            sandbox,
            pid,
            containers,
        }
    }
}

#[async_trait]
impl ContainerManager for WasmContainerManager {
    // container lifecycle
    async fn create_container(&self, config: ContainerConfig, spec: oci::Spec) -> Result<PID> {
        let mut spec = spec;
        self.sandbox.update_container_namespaces(&mut spec).await?;

        let container = Container::new(self.pid, config, spec)?;

        let mut containers = self.containers.write().await;
        container.create().await.context("create wasm container")?;
        containers.insert(container.container_id.to_string(), container);

        Ok(PID { pid: self.pid })
    }

    async fn pause_container(&self, container_id: &ContainerID) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&container_id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.container_id.clone()))?;

        c.pause().await.context("pause container")?;

        Ok(())
    }

    async fn resume_container(&self, container_id: &ContainerID) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&container_id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.container_id.clone()))?;

        c.resume().await.context("resume container")?;

        Ok(())
    }

    async fn stats_container(&self, container_id: &ContainerID) -> Result<StatsInfo> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&container_id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.container_id.clone()))?;

        let stats = c.stats().await.context("stats container")?;

        Ok(stats)
    }

    async fn update_container(&self, req: UpdateRequest) -> Result<()> {
        let resource = serde_json::from_slice::<oci::LinuxResources>(&req.value)
            .context("deserialize LinuxResource")?;

        let containers = self.containers.read().await;
        let container_id = &req.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

        c.update(resource).await.context("update container")?;

        Ok(())
    }

    async fn connect_container(&self, _container_id: &ContainerID) -> Result<PID> {
        Ok(PID { pid: self.pid })
    }

    // process lifecycle
    async fn close_process_io(&self, process_id: &ContainerProcess) -> Result<()> {
        let containers = self.containers.read().await;
        let container_id = &process_id.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

        c.close_io(process_id).await.context("close process io")?;

        Ok(())
    }

    async fn delete_process(&self, process_id: &ContainerProcess) -> Result<ProcessStateInfo> {
        let container_id = &process_id.container_id.container_id;
        let state = match process_id.process_type {
            ProcessType::Container => {
                let mut containers = self.containers.write().await;
                let c = containers
                    .remove(container_id)
                    .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

                c.state_process(process_id).await?
            }
            ProcessType::Exec => {
                let containers = self.containers.read().await;
                let c = containers
                    .get(container_id)
                    .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

                let state = c.state_process(process_id).await?;

                c.delete_exec_process(process_id)
                    .await
                    .context("delete process")?;

                state
            }
        };

        Ok(state)
    }

    async fn exec_process(&self, req: ExecProcessRequest) -> Result<()> {
        if req.spec_type_url.is_empty() {
            return Err(anyhow!("invalid type url"));
        }

        let containers = self.containers.read().await;
        let container_id = &req.process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

        c.create_exec_process(req)
            .await
            .context("create exec process")?;

        Ok(())
    }

    async fn kill_process(&self, req: &KillRequest) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&req.process.container_id.container_id)
            .ok_or_else(|| {
                Error::ContainerNotFound(req.process.container_id.container_id.clone())
            })?;

        c.signal_process(&req.process, req.signal, req.all)
            .await
            .context("kill process")?;

        Ok(())
    }

    async fn resize_process_pty(&self, req: &ResizePTYRequest) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&req.process.container_id.container_id)
            .ok_or_else(|| {
                Error::ContainerNotFound(req.process.container_id.container_id.clone())
            })?;

        c.resize_pty(&req.process, req.width, req.height)
            .await
            .context("resize pty")?;

        Ok(())
    }

    async fn start_process(&self, process_id: &ContainerProcess) -> Result<PID> {
        let containers = self.containers.read().await;
        let container_id = &process_id.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

        c.start(process_id).await.context("start")?;

        Ok(PID { pid: self.pid })
    }

    async fn state_process(&self, process_id: &ContainerProcess) -> Result<ProcessStateInfo> {
        let containers = self.containers.read().await;
        let container_id = &process_id.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

        let state = c.state_process(process_id).await?;

        Ok(state)
    }

    async fn wait_process(&self, process_id: &ContainerProcess) -> Result<ProcessExitStatus> {
        let (watcher, status) = {
            let containers = self.containers.read().await;
            let container_id = &process_id.container_id.container_id;
            let c = containers
                .get(container_id)
                .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

            c.fetch_exit_watcher(process_id).await?
        };

        let mut watcher = watcher
            .ok_or("no watcher to wait")
            .map_err(|e| anyhow!(e))?;

        while watcher.changed().await.is_ok() {}

        let status = status.read().await;

        let containers = self.containers.read().await;
        let container_id = &process_id.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        c.stop_process(process_id).await.context("stop container")?;

        Ok(status.clone())
    }

    // utility
    async fn pid(&self) -> Result<PID> {
        Ok(PID { pid: self.pid })
    }

    async fn need_shutdown_sandbox(&self, req: &ShutdownRequest) -> bool {
        req.is_now
            || self.containers.read().await.is_empty()
            || self.sandbox.sid == req.container_id
    }

    async fn is_sandbox_container(&self, process_id: &ContainerProcess) -> bool {
        process_id.process_type == ProcessType::Container
            && process_id.container_id.container_id == self.sandbox.sid
    }
}
