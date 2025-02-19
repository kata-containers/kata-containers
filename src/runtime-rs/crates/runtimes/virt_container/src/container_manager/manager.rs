// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;

use std::{collections::HashMap, sync::Arc};

use agent::Agent;
use common::{
    error::Error,
    types::{
        ContainerConfig, ContainerID, ContainerProcess, ExecProcessRequest, KillRequest,
        ProcessExitStatus, ProcessStateInfo, ProcessType, ResizePTYRequest, ShutdownRequest,
        StatsInfo, UpdateRequest, PID,
    },
    ContainerManager,
};
use hypervisor::Hypervisor;
use oci::Process as OCIProcess;
use oci_spec::runtime as oci;
use resource::ResourceManager;
use runtime_spec as spec;
use tokio::sync::RwLock;
use tracing::instrument;

use kata_sys_util::{hooks::HookStates, netns::NetnsGuard};

use super::{logger_with_process, Container};

pub struct VirtContainerManager {
    sid: String,
    pid: u32,
    containers: Arc<RwLock<HashMap<String, Container>>>,
    resource_manager: Arc<ResourceManager>,
    agent: Arc<dyn Agent>,
    hypervisor: Arc<dyn Hypervisor>,
}

impl std::fmt::Debug for VirtContainerManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtContainerManager")
            .field("sid", &self.sid)
            .field("pid", &self.pid)
            .finish()
    }
}

fn from_hooks(hooks: &Option<Vec<oci::Hook>>) -> &[oci::Hook] {
    match hooks {
        Some(hooks_vec) => hooks_vec.as_slice(),
        None => &[],
    }
}

impl VirtContainerManager {
    pub fn new(
        sid: &str,
        pid: u32,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        resource_manager: Arc<ResourceManager>,
    ) -> Self {
        Self {
            sid: sid.to_string(),
            pid,
            containers: Default::default(),
            resource_manager,
            agent,
            hypervisor,
        }
    }
}

#[async_trait]
impl ContainerManager for VirtContainerManager {
    #[instrument]
    async fn create_container(&self, config: ContainerConfig, spec: oci::Spec) -> Result<PID> {
        let mut container = Container::new(
            self.pid,
            config.clone(),
            spec.clone(),
            self.agent.clone(),
            self.resource_manager.clone(),
            self.hypervisor.get_passfd_listener_addr().await.ok(),
        )
        .await
        .context("new container")?;

        // CreateContainer Hooks:
        // * should be run in vmm namespace (hook path in runtime namespace)
        // * should be run after the vm is started, before container is created, and after CreateRuntime Hooks
        // * spec details: https://github.com/opencontainers/runtime-spec/blob/c1662686cff159595277b79322d0272f5182941b/config.md#createcontainer-hooks
        let vmm_master_tid = self.hypervisor.get_vmm_master_tid().await?;
        let vmm_ns_path = self.hypervisor.get_ns_path().await?;
        let vmm_netns_path = format!("{}/{}", vmm_ns_path, "net");
        let state = spec::State {
            version: spec.version().clone(),
            id: config.container_id.clone(),
            status: spec::ContainerState::Creating,
            pid: vmm_master_tid as i32,
            bundle: config.bundle.clone(),
            annotations: spec.annotations().clone().unwrap_or_default(),
        };

        // new scope, CreateContainer hooks in which will execute in a new network namespace
        {
            let _netns_guard = NetnsGuard::new(&vmm_netns_path).context("vmm netns guard")?;
            if let Some(hooks) = spec.hooks().as_ref() {
                let mut create_container_hook_states = HookStates::new();
                create_container_hook_states
                    .execute_hooks(from_hooks(hooks.create_container()), Some(state))?;
            }
        }

        let mut containers = self.containers.write().await;
        if let Err(e) = container.create(spec).await {
            if let Err(inner_e) = container.cleanup().await {
                warn!(sl!(), "failed to cleanup container {:?}", inner_e);
            }

            return Err(e);
        }

        containers.insert(container.container_id.to_string(), container);
        Ok(PID { pid: self.pid })
    }

    #[instrument]
    async fn close_process_io(&self, process: &ContainerProcess) -> Result<()> {
        let containers = self.containers.read().await;
        let container_id = &process.container_id.to_string();
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;

        c.close_io(process).await.context("close io")?;
        Ok(())
    }

    #[instrument]
    async fn delete_process(&self, process: &ContainerProcess) -> Result<ProcessStateInfo> {
        let container_id = &process.container_id.container_id;
        match process.process_type {
            ProcessType::Container => {
                let mut containers = self.containers.write().await;
                let c = containers
                    .remove(container_id)
                    .ok_or_else(|| Error::ContainerNotFound(container_id.to_string()))?;

                // Poststop Hooks:
                // * should be run in runtime namespace
                // * should be run after the container is deleted but before delete operation returns
                // * spec details: https://github.com/opencontainers/runtime-spec/blob/c1662686cff159595277b79322d0272f5182941b/config.md#poststop
                let c_spec = c.spec().await;

                let state = spec::State {
                    version: c_spec.version().clone(),
                    id: c.container_id.to_string(),
                    status: spec::ContainerState::Stopped,
                    pid: self.pid as i32,
                    bundle: c.config().await.bundle,
                    annotations: c_spec.annotations().clone().unwrap_or_default(),
                };
                if let Some(hooks) = c_spec.hooks().as_ref() {
                    let mut poststop_hook_states = HookStates::new();
                    poststop_hook_states
                        .execute_hooks(from_hooks(hooks.poststop()), Some(state))?;
                }

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

    #[instrument]
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

    #[instrument]
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

    #[instrument]
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

        Ok(status.clone())
    }

    #[instrument]
    async fn start_process(&self, process: &ContainerProcess) -> Result<PID> {
        let containers = self.containers.read().await;
        let container_id = &process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        c.start(self.containers.clone(), process)
            .await
            .context("start")?;

        // Poststart Hooks:
        // * should be run in runtime namespace
        // * should be run after user-specific command is executed but before start operation returns
        // * spec details: https://github.com/opencontainers/runtime-spec/blob/c1662686cff159595277b79322d0272f5182941b/config.md#poststart
        let c_spec = c.spec().await;
        let vmm_master_tid = self.hypervisor.get_vmm_master_tid().await?;
        let state = spec::State {
            version: c_spec.version().clone(),
            id: c.container_id.to_string(),
            status: spec::ContainerState::Running,
            pid: vmm_master_tid as i32,
            bundle: c.config().await.bundle,
            annotations: c_spec.annotations().clone().unwrap_or_default(),
        };
        if let Some(hooks) = c_spec.hooks().as_ref() {
            let mut poststart_hook_states = HookStates::new();
            poststart_hook_states.execute_hooks(from_hooks(hooks.poststart()), Some(state))?;
        }

        Ok(PID { pid: self.pid })
    }

    #[instrument]
    async fn state_process(&self, process: &ContainerProcess) -> Result<ProcessStateInfo> {
        let containers = self.containers.read().await;
        let container_id = &process.container_id.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.clone()))?;
        let state = c.state_process(process).await.context("state process")?;
        Ok(state)
    }

    #[instrument]
    async fn pause_container(&self, id: &ContainerID) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(id.container_id.clone()))?;
        c.pause().await.context("pause")?;
        Ok(())
    }

    #[instrument]
    async fn resume_container(&self, id: &ContainerID) -> Result<()> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(id.container_id.clone()))?;
        c.resume().await.context("resume")?;
        Ok(())
    }

    #[instrument]
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

    #[instrument]
    async fn stats_container(&self, id: &ContainerID) -> Result<StatsInfo> {
        let containers = self.containers.read().await;
        let c = containers
            .get(&id.container_id)
            .ok_or_else(|| Error::ContainerNotFound(id.container_id.clone()))?;
        let stats = c.stats().await.context("stats")?;
        Ok(StatsInfo::from(stats))
    }

    #[instrument]
    async fn update_container(&self, req: UpdateRequest) -> Result<()> {
        let resource = serde_json::from_slice::<oci::LinuxResources>(&req.value)
            .context("deserialize LinuxResource")?;
        let containers = self.containers.read().await;
        let container_id = &req.container_id;
        let c = containers
            .get(container_id)
            .ok_or_else(|| Error::ContainerNotFound(container_id.to_string()))?;
        c.update(&resource).await.context("update_container")
    }

    #[instrument]
    async fn pid(&self) -> Result<PID> {
        Ok(PID { pid: self.pid })
    }

    #[instrument]
    async fn connect_container(&self, _id: &ContainerID) -> Result<PID> {
        Ok(PID { pid: self.pid })
    }

    #[instrument]
    async fn need_shutdown_sandbox(&self, req: &ShutdownRequest) -> bool {
        req.is_now || self.containers.read().await.is_empty() || self.sid == req.container_id
    }

    #[instrument]
    async fn is_sandbox_container(&self, process: &ContainerProcess) -> bool {
        process.process_type == ProcessType::Container
            && process.container_id.container_id == self.sid
    }
}
