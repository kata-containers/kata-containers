// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};

use std::{collections::HashMap, sync::Arc};

use agent::Agent;
use async_trait::async_trait;
use common::{
    error::Error,
    types::{
        ContainerConfig, ContainerID, ContainerProcess, ExecProcessRequest, KillRequest,
        ProcessExitStatus, ProcessStateInfo, ProcessType, ResizePTYRequest, ShutdownRequest,
        StatsInfo, UpdateRequest, PID,
    },
    ContainerManager,
};
use oci::Process as OCIProcess;
use resource::ResourceManager;
use tokio::sync::RwLock;

use super::{logger_with_process, Container};

unsafe impl Send for VirtContainerManager {}
unsafe impl Sync for VirtContainerManager {}
pub struct VirtContainerManager {
    sid: String,
    pid: u32,
    containers: Arc<RwLock<HashMap<String, Container>>>,
    resource_manager: Arc<ResourceManager>,
    agent: Arc<dyn Agent>,
}

impl VirtContainerManager {
    pub fn new(
        sid: &str,
        pid: u32,
        agent: Arc<dyn Agent>,
        resource_manager: Arc<ResourceManager>,
    ) -> Self {
        Self {
            sid: sid.to_string(),
            pid,
            containers: Default::default(),
            resource_manager,
            agent,
        }
    }
}

#[async_trait]
impl ContainerManager for VirtContainerManager {
    async fn create_container(&self, config: ContainerConfig, spec: oci::Spec) -> Result<PID> {
        let container = Container::new(
            self.pid,
            config,
            self.agent.clone(),
            self.resource_manager.clone(),
        )
        .context("new container")?;

        let mut containers = self.containers.write().await;
        container.create(spec).await.context("create")?;
        containers.insert(container.container_id.to_string(), container);

        Ok(PID { pid: self.pid })
    }

    async fn close_process_io(&self, process: &ContainerProcess) -> Result<()> {
        let containers = self.containers.read().await;
        let container_id = &process.container_id.to_string();
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

        c.close_io(process).await.context("close io")?;
        Ok(())
    }

    async fn delete_process(&self, process: &ContainerProcess) -> Result<ProcessStateInfo> {
        let container_id = &process.container_id.container_id;
        match process.process_type {
            ProcessType::Container => {
                let mut containers = self.containers.write().await;
                let c = containers
                    .remove(container_id)
                    .ok_or_else(|| Error::ContainerNotFound(container_id.to_string()))?;
                c.state_process(process).await.context("state process")
            }
            ProcessType::Exec => {
                let containers = self.containers.read().await;
                let c = containers
                    .get(container_id)
                    .ok_or_else(|| Error::ContainerNotFound(container_id.to_string()))?;
                let state = c.state_process(process).await.context("state process");
                c.delete_exec_process(process)
                    .await
                    .context("delete process")?;
                return state;
            }
        }
    }

    async fn exec_process(&self, req: ExecProcessRequest) -> Result<()> {
        if req.spec_type_url.is_empty() {
            return Err(anyhow!("invalid type url"));
        }
        let oci_process: OCIProcess =
            serde_json::from_slice(&req.spec_value).context("serde from slice")?;

        let containers = self.containers.read().await;
        let container_id = &req.process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        c.exec_process(
            &req.process,
            req.stdin,
            req.stdout,
            req.stderr,
            req.terminal,
            oci_process,
        )
        .await
        .context("exec")?;
        Ok(())
    }

    async fn kill_process(&self, req: &KillRequest) -> Result<()> {
        let containers = self.containers.read().await;
        let container_id = &req.process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        c.kill_process(&req.process, req.signal, req.all)
            .await
            .map_err(|err| {
                warn!(
                    sl!(),
                    "failed to signal process {:?} {:?}", &req.process, err
                );
                err
            })
            .ok();
        Ok(())
    }

    async fn wait_process(&self, process: &ContainerProcess) -> Result<ProcessExitStatus> {
        let logger = logger_with_process(process);

        let containers = self.containers.read().await;
        let container_id = &process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        let (watcher, status) = c.wait_process(process).await.context("wait")?;
        drop(containers);

        match watcher {
            Some(mut watcher) => {
                info!(logger, "begin wait exit");
                while watcher.changed().await.is_ok() {}
                info!(logger, "end wait exited");
            }
            None => {
                warn!(logger, "failed to find watcher for wait process");
            }
        }

        let status = status.read().await;

        info!(logger, "wait process exit status {:?}", status);

        // stop process
        let containers = self.containers.read().await;
        let container_id = &process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        c.stop_process(process).await.context("stop container")?;
        Ok(status.clone())
    }

    async fn start_process(&self, process: &ContainerProcess) -> Result<PID> {
        let containers = self.containers.read().await;
        let container_id = &process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        c.start(process).await.context("start")?;
        Ok(PID { pid: self.pid })
    }

    async fn state_process(&self, process: &ContainerProcess) -> Result<ProcessStateInfo> {
        let containers = self.containers.read().await;
        let container_id = &process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        let state = c.state_process(process).await.context("state process")?;
        Ok(state)
    }

    async fn pause_container(&self, id: &ContainerID) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(id.container_id.clone()))?;
        c.pause().await.context("pause")?;
        Ok(())
    }

    async fn resume_container(&self, id: &ContainerID) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(id.container_id.clone()))?;
        c.resume().await.context("resume")?;
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

    async fn stats_container(&self, id: &ContainerID) -> Result<StatsInfo> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(id.container_id.clone()))?;
        let stats = c.stats().await.context("stats")?;
        Ok(StatsInfo::from(stats))
    }

    async fn update_container(&self, req: UpdateRequest) -> Result<()> {
        let resource = serde_json::from_slice::<oci::LinuxResources>(&req.value)
            .context("deserialize LinuxResource")?;
        let containers = self.containers.read().await;
        let container_id = &req.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.to_string()))?;
        c.update(&resource).await.context("stats")
    }

    async fn pid(&self) -> Result<PID> {
        Ok(PID { pid: self.pid })
    }

    async fn connect_container(&self, _id: &ContainerID) -> Result<PID> {
        Ok(PID { pid: self.pid })
    }

    async fn need_shutdown_sandbox(&self, req: &ShutdownRequest) -> bool {
        req.is_now || self.containers.read().await.is_empty() || self.sid == req.container_id
    }

    async fn is_sandbox_container(&self, process: &ContainerProcess) -> bool {
        process.process_type == ProcessType::Container
            && process.container_id.container_id == self.sid
    }
}
