// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use agent::{self, kata::KataAgent, types::KernelModule, Agent};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use common::{
    message::{Action, Message},
    Sandbox,
};
use containerd_shim_protos::events::task::TaskOOM;
use hypervisor::{dragonball::Dragonball, Hypervisor, HYPERVISOR_DRAGONBALL};
use kata_types::config::TomlConfig;
use resource::{
    manager::ManagerArgs,
    network::{NetworkConfig, NetworkWithNetNsConfig},
    ResourceConfig, ResourceManager,
};
use tokio::sync::{mpsc::Sender, Mutex, RwLock};

use crate::{health_check::HealthCheck, sandbox_persist::SandboxTYPE};
use persist::{self, sandbox_persist::Persist};
pub struct SandboxRestoreArgs {
    pub sid: String,
    pub toml_config: TomlConfig,
    pub sender: Sender<Message>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SandboxState {
    Init,
    Running,
    Stopped,
}

struct SandboxInner {
    state: SandboxState,
}

impl SandboxInner {
    pub fn new() -> Self {
        Self {
            state: SandboxState::Init,
        }
    }
}

unsafe impl Send for VirtSandbox {}
unsafe impl Sync for VirtSandbox {}
#[derive(Clone)]
pub struct VirtSandbox {
    sid: String,
    msg_sender: Arc<Mutex<Sender<Message>>>,
    inner: Arc<RwLock<SandboxInner>>,
    resource_manager: Arc<ResourceManager>,
    agent: Arc<dyn Agent>,
    hypervisor: Arc<dyn Hypervisor>,
    monitor: Arc<HealthCheck>,
}

impl VirtSandbox {
    pub async fn new(
        sid: &str,
        msg_sender: Sender<Message>,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        resource_manager: Arc<ResourceManager>,
    ) -> Result<Self> {
        Ok(Self {
            sid: sid.to_string(),
            msg_sender: Arc::new(Mutex::new(msg_sender)),
            inner: Arc::new(RwLock::new(SandboxInner::new())),
            agent,
            hypervisor,
            resource_manager,
            monitor: Arc::new(HealthCheck::new(true, false)),
        })
    }

    async fn prepare_for_start_sandbox(
        &self,
        _id: &str,
        netns: Option<String>,
    ) -> Result<Vec<ResourceConfig>> {
        let mut resource_configs = vec![];

        let config = self.resource_manager.config().await;
        if let Some(netns_path) = netns {
            let network_config = ResourceConfig::Network(NetworkConfig::NetworkResourceWithNetNs(
                NetworkWithNetNsConfig {
                    network_model: config.runtime.internetworking_model.clone(),
                    netns_path,
                    queues: self
                        .hypervisor
                        .hypervisor_config()
                        .await
                        .network_info
                        .network_queues as usize,
                },
            ));
            resource_configs.push(network_config);
        }

        let hypervisor_config = self.hypervisor.hypervisor_config().await;
        let virtio_fs_config = ResourceConfig::ShareFs(hypervisor_config.shared_fs);
        resource_configs.push(virtio_fs_config);

        Ok(resource_configs)
    }
}

#[async_trait]
impl Sandbox for VirtSandbox {
    async fn start(&self, netns: Option<String>) -> Result<()> {
        let id = &self.sid;

        // if sandbox running, return
        // if sandbox not running try to start sandbox
        let mut inner = self.inner.write().await;
        if inner.state == SandboxState::Running {
            warn!(sl!(), "sandbox is running, no need to start");
            return Ok(());
        }

        self.hypervisor
            .prepare_vm(id, netns.clone())
            .await
            .context("prepare vm")?;

        // generate device and setup before start vm
        // should after hypervisor.prepare_vm
        let resources = self.prepare_for_start_sandbox(id, netns).await?;
        self.resource_manager
            .prepare_before_start_vm(resources)
            .await
            .context("set up device before start vm")?;

        // start vm
        self.hypervisor.start_vm(10_000).await.context("start vm")?;
        info!(sl!(), "start vm");

        // connect agent
        // set agent socket
        let address = self
            .hypervisor
            .get_agent_socket()
            .await
            .context("get agent socket")?;
        self.agent.start(&address).await.context("connect")?;

        self.resource_manager
            .setup_after_start_vm()
            .await
            .context("setup device after start vm")?;

        // create sandbox in vm
        let agent_config = self.agent.agent_config().await;
        let kernel_modules = KernelModule::set_kernel_modules(agent_config.kernel_modules)?;
        let req = agent::CreateSandboxRequest {
            hostname: "".to_string(),
            dns: vec![],
            storages: self
                .resource_manager
                .get_storage_for_sandbox()
                .await
                .context("get storages for sandbox")?,
            sandbox_pidns: false,
            sandbox_id: id.to_string(),
            guest_hook_path: self
                .hypervisor
                .hypervisor_config()
                .await
                .security_info
                .guest_hook_path,
            kernel_modules,
        };

        self.agent
            .create_sandbox(req)
            .await
            .context("create sandbox")?;

        inner.state = SandboxState::Running;
        let agent = self.agent.clone();
        let sender = self.msg_sender.clone();
        info!(sl!(), "oom watcher start");
        let _ = tokio::spawn(async move {
            loop {
                match agent
                    .get_oom_event(agent::Empty::new())
                    .await
                    .context("get oom event")
                {
                    Ok(resp) => {
                        let cid = &resp.container_id;
                        warn!(sl!(), "send oom event for container {}", &cid);
                        let event = TaskOOM {
                            container_id: cid.to_string(),
                            ..Default::default()
                        };
                        let msg = Message::new(Action::Event(Arc::new(event)));
                        let lock_sender = sender.lock().await;
                        if let Err(err) = lock_sender.send(msg).await.context("send event") {
                            error!(
                                sl!(),
                                "failed to send oom event for {} error {:?}", cid, err
                            );
                        }
                    }
                    Err(err) => {
                        warn!(sl!(), "failed to get oom event error {:?}", err);
                        break;
                    }
                }
            }
        });
        self.monitor.start(id, self.agent.clone());
        self.save().await.context("save state")?;
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!(sl!(), "begin stop sandbox");
        self.hypervisor.stop_vm().await.context("stop vm")?;
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        info!(sl!(), "shutdown");

        self.stop().await.context("stop")?;

        info!(sl!(), "delete cgroup");
        self.resource_manager
            .delete_cgroups()
            .await
            .context("delete cgroups")?;

        info!(sl!(), "stop monitor");
        self.monitor.stop().await;

        info!(sl!(), "stop agent");
        self.agent.stop().await;

        // stop server
        info!(sl!(), "send shutdown message");
        let msg = Message::new(Action::Shutdown);
        let sender = self.msg_sender.clone();
        let sender = sender.lock().await;
        sender.send(msg).await.context("send shutdown msg")?;
        Ok(())
    }

    async fn cleanup(&self, _id: &str) -> Result<()> {
        self.resource_manager.delete_cgroups().await?;
        self.hypervisor.cleanup().await?;
        // TODO: cleanup other snadbox resource
        Ok(())
    }
}

#[async_trait]
impl Persist for VirtSandbox {
    type State = crate::sandbox_persist::SandboxState;
    type ConstructorArgs = SandboxRestoreArgs;

    /// Save a state of Sandbox
    async fn save(&self) -> Result<Self::State> {
        let sandbox_state = crate::sandbox_persist::SandboxState {
            sandbox_type: SandboxTYPE::VIRTCONTAINER,
            resource: Some(self.resource_manager.save().await?),
            hypervisor: Some(self.hypervisor.save_state().await?),
        };
        persist::to_disk(&sandbox_state, &self.sid)?;
        Ok(sandbox_state)
    }
    /// Restore Sandbox
    async fn restore(
        sandbox_args: Self::ConstructorArgs,
        sandbox_state: Self::State,
    ) -> Result<Self> {
        let config = sandbox_args.toml_config;
        let r = sandbox_state.resource.unwrap_or_default();
        let h = sandbox_state.hypervisor.unwrap_or_default();
        let hypervisor = match h.hypervisor_type.as_str() {
            // TODO support other hypervisors
            HYPERVISOR_DRAGONBALL => Ok(Arc::new(Dragonball::restore((), h).await?)),
            _ => Err(anyhow!("Unsupported hypervisor {}", &h.hypervisor_type)),
        }?;
        let agent = Arc::new(KataAgent::new(kata_types::config::Agent {
            debug: true,
            enable_tracing: false,
            server_port: 1024,
            log_port: 1025,
            dial_timeout_ms: 10,
            reconnect_timeout_ms: 3_000,
            request_timeout_ms: 30_000,
            health_check_request_timeout_ms: 90_000,
            kernel_modules: Default::default(),
            container_pipe_size: 0,
            debug_console_enabled: false,
        }));
        let sid = sandbox_args.sid;
        let args = ManagerArgs {
            sid: sid.clone(),
            agent: agent.clone(),
            hypervisor: hypervisor.clone(),
            config,
        };
        let resource_manager = Arc::new(ResourceManager::restore(args, r).await?);
        Ok(Self {
            sid: sid.to_string(),
            msg_sender: Arc::new(Mutex::new(sandbox_args.sender)),
            inner: Arc::new(RwLock::new(SandboxInner::new())),
            agent,
            hypervisor,
            resource_manager,
            monitor: Arc::new(HealthCheck::new(true, false)),
        })
    }
}
