// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use agent::{self, Agent};
use anyhow::{Context, Result};
use async_trait::async_trait;
use common::{
    message::{Action, Message},
    Sandbox,
};
use containerd_shim_protos::events::task::TaskOOM;
use hypervisor::Hypervisor;
use kata_types::config::TomlConfig;
use resource::{
    network::{NetworkConfig, NetworkWithNetNsConfig},
    ResourceConfig, ResourceManager,
};
use tokio::sync::{mpsc::Sender, Mutex, RwLock};

use crate::health_check::HealthCheck;

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
        config: &TomlConfig,
    ) -> Result<Vec<ResourceConfig>> {
        let mut resource_configs = vec![];

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
    async fn start(&self, netns: Option<String>, config: &TomlConfig) -> Result<()> {
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
        let resources = self.prepare_for_start_sandbox(id, netns, config).await?;
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
            guest_hook_path: "".to_string(),
            kernel_modules: vec![],
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
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!(sl!(), "begin stop sandbox");
        // TODO: stop sandbox
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        info!(sl!(), "shutdown");

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
        // TODO: cleanup
        Ok(())
    }
}
