// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use agent::kata::KataAgent;
use agent::types::KernelModule;
use agent::{
    self, Agent, GetGuestDetailsRequest, GetIPTablesRequest, SetIPTablesRequest, VolumeStatsRequest,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use common::message::{Action, Message};
use common::{Sandbox, SandboxNetworkEnv};
use containerd_shim_protos::events::task::TaskOOM;
use hypervisor::VsockConfig;
use hypervisor::{dragonball::Dragonball, BlockConfig, Hypervisor, HYPERVISOR_DRAGONBALL};
use hypervisor::{utils::get_hvsock_path, HybridVsockConfig, DEFAULT_GUEST_VSOCK_CID};
use kata_sys_util::hooks::HookStates;
use kata_types::capabilities::CapabilityBits;
use kata_types::config::TomlConfig;
use persist::{self, sandbox_persist::Persist};
use resource::manager::ManagerArgs;
use resource::network::{dan_config_path, DanNetworkConfig, NetworkConfig, NetworkWithNetNsConfig};
use resource::{ResourceConfig, ResourceManager};
use tokio::sync::{mpsc::Sender, Mutex, RwLock};
use tracing::instrument;

use crate::health_check::HealthCheck;

pub(crate) const VIRTCONTAINER: &str = "virt_container";

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

impl std::fmt::Debug for VirtSandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtSandbox")
            .field("sid", &self.sid)
            .field("msg_sender", &self.msg_sender)
            .finish()
    }
}

impl VirtSandbox {
    pub async fn new(
        sid: &str,
        msg_sender: Sender<Message>,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        resource_manager: Arc<ResourceManager>,
    ) -> Result<Self> {
        let config = resource_manager.config().await;
        let keep_abnormal = config.runtime.keep_abnormal;
        Ok(Self {
            sid: sid.to_string(),
            msg_sender: Arc::new(Mutex::new(msg_sender)),
            inner: Arc::new(RwLock::new(SandboxInner::new())),
            agent,
            hypervisor,
            resource_manager,
            monitor: Arc::new(HealthCheck::new(true, keep_abnormal)),
        })
    }

    #[instrument]
    async fn prepare_for_start_sandbox(
        &self,
        id: &str,
        network_env: SandboxNetworkEnv,
    ) -> Result<Vec<ResourceConfig>> {
        let mut resource_configs = vec![];

        info!(sl!(), "prepare vm socket config for sandbox.");
        let vm_socket_config = self
            .prepare_vm_socket_config()
            .await
            .context("failed to prepare vm socket config")?;
        resource_configs.push(vm_socket_config);

        // prepare network config
        if !network_env.network_created {
            if let Some(network_resource) = self.prepare_network_resource(&network_env).await {
                resource_configs.push(network_resource);
            }
        }

        // prepare sharefs device config
        let virtio_fs_config =
            ResourceConfig::ShareFs(self.hypervisor.hypervisor_config().await.shared_fs);
        resource_configs.push(virtio_fs_config);

        // prepare VM rootfs device config
        let vm_rootfs = ResourceConfig::VmRootfs(
            self.prepare_rootfs_config()
                .await
                .context("failed to prepare rootfs device config")?,
        );
        resource_configs.push(vm_rootfs);

        Ok(resource_configs)
    }

    async fn prepare_network_resource(
        &self,
        network_env: &SandboxNetworkEnv,
    ) -> Option<ResourceConfig> {
        let config = self.resource_manager.config().await;
        let dan_path = dan_config_path(&config, &self.sid);

        // Network priority: DAN > NetNS
        if dan_path.exists() {
            Some(ResourceConfig::Network(NetworkConfig::Dan(
                DanNetworkConfig {
                    dan_conf_path: dan_path,
                },
            )))
        } else if let Some(netns_path) = network_env.netns.as_ref() {
            Some(ResourceConfig::Network(NetworkConfig::NetNs(
                NetworkWithNetNsConfig {
                    network_model: config.runtime.internetworking_model.clone(),
                    netns_path: netns_path.to_owned(),
                    queues: self
                        .hypervisor
                        .hypervisor_config()
                        .await
                        .network_info
                        .network_queues as usize,
                    network_created: network_env.network_created,
                },
            )))
        } else {
            None
        }
    }

    async fn execute_oci_hook_functions(
        &self,
        prestart_hooks: &[oci::Hook],
        create_runtime_hooks: &[oci::Hook],
        state: &oci::State,
    ) -> Result<()> {
        let mut st = state.clone();
        // for dragonball, we use vmm_master_tid
        let vmm_pid = self
            .hypervisor
            .get_vmm_master_tid()
            .await
            .context("get vmm master tid")?;
        st.pid = vmm_pid as i32;

        // Prestart Hooks [DEPRECATED in newest oci spec]:
        // * should be run in runtime namespace
        // * should be run after vm is started, but before container is created
        //      if Prestart Hook and CreateRuntime Hook are both supported
        // * spec details: https://github.com/opencontainers/runtime-spec/blob/c1662686cff159595277b79322d0272f5182941b/config.md#prestart
        let mut prestart_hook_states = HookStates::new();
        prestart_hook_states.execute_hooks(prestart_hooks, Some(st.clone()))?;

        // CreateRuntime Hooks:
        // * should be run in runtime namespace
        // * should be run when creating the runtime
        // * spec details: https://github.com/opencontainers/runtime-spec/blob/c1662686cff159595277b79322d0272f5182941b/config.md#createruntime-hooks
        let mut create_runtime_hook_states = HookStates::new();
        create_runtime_hook_states.execute_hooks(create_runtime_hooks, Some(st.clone()))?;
        Ok(())
    }

    // store_guest_details will get the information from the guest OS, like memory block size, agent details and is memory hotplug probe support
    async fn store_guest_details(&self) -> Result<()> {
        // get the information from agent
        let guest_details = self
            .agent
            .get_guest_details(GetGuestDetailsRequest {
                mem_block_size: true,
                mem_hotplug_probe: true,
            })
            .await
            .context("failed to store guest details")?;

        // set memory block size
        self.hypervisor
            .set_guest_memory_block_size(guest_details.mem_block_size_bytes as u32)
            .await;

        // set memory hotplug probe
        if guest_details.support_mem_hotplug_probe {
            self.hypervisor
                .set_capabilities(CapabilityBits::GuestMemoryProbe)
                .await;
        }
        info!(
            sl!(),
            "memory block size is {}, memory probe support {}",
            self.hypervisor.guest_memory_block_size().await,
            self.hypervisor
                .capabilities()
                .await?
                .is_mem_hotplug_probe_supported()
        );
        Ok(())
    }

    async fn prepare_rootfs_config(&self) -> Result<BlockConfig> {
        let boot_info = self.hypervisor.hypervisor_config().await.boot_info;

        let image = {
            let initrd_path = boot_info.initrd.clone();
            let image_path = boot_info.image;
            if !initrd_path.is_empty() {
                Ok(initrd_path)
            } else if !image_path.is_empty() {
                Ok(image_path)
            } else {
                Err(anyhow!("failed to get image"))
            }
        }
        .context("get image")?;

        Ok(BlockConfig {
            path_on_host: image,
            is_readonly: true,
            driver_option: boot_info.vm_rootfs_driver,
            ..Default::default()
        })
    }

    async fn prepare_vm_socket_config(&self) -> Result<ResourceConfig> {
        // It will check the hypervisor's capabilities to see if it supports hybrid-vsock.
        // If it does not, it'll assume that it only supports legacy vsock.
        let vm_socket = if self
            .hypervisor
            .capabilities()
            .await?
            .is_hybrid_vsock_supported()
        {
            // Firecracker/Dragonball/CLH use the hybrid-vsock device model.
            ResourceConfig::HybridVsock(HybridVsockConfig {
                guest_cid: DEFAULT_GUEST_VSOCK_CID,
                uds_path: get_hvsock_path(&self.sid),
            })
        } else {
            // Qemu uses the vsock device model.
            ResourceConfig::Vsock(VsockConfig {
                guest_cid: libc::VMADDR_CID_ANY,
            })
        };

        Ok(vm_socket)
    }

    fn has_prestart_hooks(
        &self,
        prestart_hooks: Vec<oci::Hook>,
        create_runtime_hooks: Vec<oci::Hook>,
    ) -> bool {
        !prestart_hooks.is_empty() || !create_runtime_hooks.is_empty()
    }
}

#[async_trait]
impl Sandbox for VirtSandbox {
    #[instrument(name = "sb: start")]
    async fn start(
        &self,
        dns: Vec<String>,
        spec: &oci::Spec,
        state: &oci::State,
        network_env: SandboxNetworkEnv,
    ) -> Result<()> {
        let id = &self.sid;

        // if sandbox running, return
        // if sandbox not running try to start sandbox
        let mut inner = self.inner.write().await;
        if inner.state == SandboxState::Running {
            warn!(sl!(), "sandbox is running, no need to start");
            return Ok(());
        }

        self.hypervisor
            .prepare_vm(id, network_env.netns.clone())
            .await
            .context("prepare vm")?;

        // generate device and setup before start vm
        // should after hypervisor.prepare_vm
        let resources = self
            .prepare_for_start_sandbox(id, network_env.clone())
            .await?;

        self.resource_manager
            .prepare_before_start_vm(resources)
            .await
            .context("set up device before start vm")?;

        // start vm
        self.hypervisor.start_vm(10_000).await.context("start vm")?;
        info!(sl!(), "start vm");

        // execute pre-start hook functions, including Prestart Hooks and CreateRuntime Hooks
        let (prestart_hooks, create_runtime_hooks) = match spec.hooks.as_ref() {
            Some(hooks) => (hooks.prestart.clone(), hooks.create_runtime.clone()),
            None => (Vec::new(), Vec::new()),
        };
        self.execute_oci_hook_functions(&prestart_hooks, &create_runtime_hooks, state)
            .await?;

        // 1. if there are pre-start hook functions, network config might have been changed.
        //    We need to rescan the netns to handle the change.
        // 2. Do not scan the netns if we want no network for the VM.
        // TODO In case of vm factory, scan the netns to hotplug interfaces after the VM is started.
        let config = self.resource_manager.config().await;
        if self.has_prestart_hooks(prestart_hooks, create_runtime_hooks)
            && !config.runtime.disable_new_netns
            && !dan_config_path(&config, &self.sid).exists()
        {
            if let Some(netns_path) = network_env.netns {
                let network_resource = NetworkConfig::NetNs(NetworkWithNetNsConfig {
                    network_model: config.runtime.internetworking_model.clone(),
                    netns_path: netns_path.to_owned(),
                    queues: self
                        .hypervisor
                        .hypervisor_config()
                        .await
                        .network_info
                        .network_queues as usize,
                    network_created: network_env.network_created,
                });
                self.resource_manager
                    .handle_network(network_resource)
                    .await
                    .context("set up device after start vm")?;
            }
        }

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
            hostname: spec.hostname.clone(),
            dns,
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

        // get and store guest details
        self.store_guest_details()
            .await
            .context("failed to store guest details")?;

        let agent = self.agent.clone();
        let sender = self.msg_sender.clone();
        info!(sl!(), "oom watcher start");
        tokio::spawn(async move {
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

        self.cleanup().await.context("do the clean up")?;

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

    async fn cleanup(&self) -> Result<()> {
        info!(sl!(), "delete hypervisor");
        self.hypervisor
            .cleanup()
            .await
            .context("delete hypervisor")?;

        info!(sl!(), "resource clean up");
        self.resource_manager
            .cleanup()
            .await
            .context("resource clean up")?;

        // TODO: cleanup other sandbox resource
        Ok(())
    }

    async fn agent_sock(&self) -> Result<String> {
        self.agent.agent_sock().await
    }

    async fn direct_volume_stats(&self, volume_guest_path: &str) -> Result<String> {
        let req: agent::VolumeStatsRequest = VolumeStatsRequest {
            volume_guest_path: volume_guest_path.to_string(),
        };
        let result = self
            .agent
            .get_volume_stats(req)
            .await
            .context("sandbox: failed to process direct volume stats query")?;
        Ok(result.data)
    }

    async fn direct_volume_resize(&self, resize_req: agent::ResizeVolumeRequest) -> Result<()> {
        self.agent
            .resize_volume(resize_req)
            .await
            .context("sandbox: failed to resize direct-volume")?;
        Ok(())
    }

    async fn set_iptables(&self, is_ipv6: bool, data: Vec<u8>) -> Result<Vec<u8>> {
        info!(sl!(), "sb: set_iptables invoked");
        let req = SetIPTablesRequest { is_ipv6, data };
        let resp = self
            .agent
            .set_ip_tables(req)
            .await
            .context("sandbox: failed to set iptables")?;
        Ok(resp.data)
    }

    async fn get_iptables(&self, is_ipv6: bool) -> Result<Vec<u8>> {
        info!(sl!(), "sb: get_iptables invoked");
        let req = GetIPTablesRequest { is_ipv6 };
        let resp = self
            .agent
            .get_ip_tables(req)
            .await
            .context("sandbox: failed to get iptables")?;
        Ok(resp.data)
    }

    async fn agent_metrics(&self) -> Result<String> {
        self.agent
            .get_metrics(agent::Empty::new())
            .await
            .map_err(|err| anyhow!("failed to get agent metrics {:?}", err))
            .map(|resp| resp.metrics)
    }

    async fn hypervisor_metrics(&self) -> Result<String> {
        self.hypervisor.get_hypervisor_metrics().await
    }
}

#[async_trait]
impl Persist for VirtSandbox {
    type State = crate::sandbox_persist::SandboxState;
    type ConstructorArgs = SandboxRestoreArgs;

    /// Save a state of Sandbox
    async fn save(&self) -> Result<Self::State> {
        let sandbox_state = crate::sandbox_persist::SandboxState {
            sandbox_type: VIRTCONTAINER.to_string(),
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
        let agent = Arc::new(KataAgent::new(kata_types::config::Agent::default()));
        let sid = sandbox_args.sid;
        let keep_abnormal = config.runtime.keep_abnormal;
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
            monitor: Arc::new(HealthCheck::new(true, keep_abnormal)),
        })
    }
}
