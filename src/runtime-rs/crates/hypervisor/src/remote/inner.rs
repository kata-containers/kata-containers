// Copyright 2024 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{
    device::DeviceType, hypervisor_persist::HypervisorState, HypervisorConfig, HYPERVISOR_REMOTE,
};
use crate::{MemoryConfig, VcpuThreadIds};
use anyhow::{Context, Result};
use async_trait::async_trait;
use kata_types::{
    annotations::{
        cri_containerd::{SANDBOX_NAMESPACE_LABEL_KEY, SANDBOX_NAME_LABEL_KEY},
        KATA_ANNO_CFG_HYPERVISOR_DEFAULT_MEMORY, KATA_ANNO_CFG_HYPERVISOR_DEFAULT_VCPUS,
        KATA_ANNO_CFG_HYPERVISOR_IMAGE_PATH, KATA_ANNO_CFG_HYPERVISOR_MACHINE_TYPE,
    },
    capabilities::{Capabilities, CapabilityBits},
};
use persist::sandbox_persist::Persist;
use protocols::{
    remote::{CreateVMRequest, StartVMRequest, StopVMRequest},
    remote_ttrpc_async::HypervisorClient,
};
use std::{collections::HashMap, time};
use tokio::sync::{mpsc, Mutex};
use ttrpc::context::{self};
use ttrpc::r#async::Client;

const REMOTE_SCHEME: &str = "remote";
const DEFAULT_MIN_TIMEOUT: i32 = time::Duration::from_secs(60).as_millis() as i32;

pub struct RemoteInner {
    /// sandbox id
    pub(crate) id: String,
    /// hypervisor config
    pub(crate) config: HypervisorConfig,
    /// agent socket path
    pub(crate) agent_socket_path: String,
    /// netns path
    pub(crate) netns: Option<String>,
    /// hypervisor unix client
    pub(crate) client: Option<Client>,

    exit_notify: Option<mpsc::Sender<i32>>,
    exit_waiter: Mutex<(mpsc::Receiver<i32>, i32)>,
}

impl std::fmt::Debug for RemoteInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteInner")
            .field("id", &self.id)
            .field("config", &self.config)
            .field("agent_socket_path", &self.agent_socket_path)
            .field("netns", &self.netns)
            .finish()
    }
}

impl RemoteInner {
    pub fn new() -> Self {
        let (exit_notify, exit_waiter) = mpsc::channel(1);

        Self {
            id: "".to_string(),
            config: HypervisorConfig::default(),
            agent_socket_path: "".to_string(),
            netns: None,
            client: None,

            exit_notify: Some(exit_notify),
            exit_waiter: Mutex::new((exit_waiter, 0)),
        }
    }

    fn get_ttrpc_client(&mut self) -> Result<HypervisorClient> {
        match self.client {
            Some(ref c) => Ok(HypervisorClient::new(c.clone())),
            None => {
                let c = Client::connect(&format!(
                    "unix://{}",
                    &self.config.remote_info.hypervisor_socket
                ))
                .context("connect to ")?;
                self.client = Some(c.clone());
                Ok(HypervisorClient::new(c))
            }
        }
    }

    fn prepare_annotations(
        &self,
        oci_annotations: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        let mut annotations: HashMap<String, String> = HashMap::new();
        let config = &self.config;
        annotations.insert(
            SANDBOX_NAME_LABEL_KEY.to_string(),
            oci_annotations
                .get(SANDBOX_NAME_LABEL_KEY)
                .cloned()
                .unwrap_or_default(),
        );
        annotations.insert(
            SANDBOX_NAMESPACE_LABEL_KEY.to_string(),
            oci_annotations
                .get(SANDBOX_NAMESPACE_LABEL_KEY)
                .cloned()
                .unwrap_or_default(),
        );
        annotations.insert(
            KATA_ANNO_CFG_HYPERVISOR_MACHINE_TYPE.to_string(),
            config.machine_info.machine_type.to_string(),
        );
        annotations.insert(
            KATA_ANNO_CFG_HYPERVISOR_DEFAULT_VCPUS.to_string(),
            config.cpu_info.default_vcpus.to_string(),
        );
        annotations.insert(
            KATA_ANNO_CFG_HYPERVISOR_DEFAULT_MEMORY.to_string(),
            config.memory_info.default_memory.to_string(),
        );
        annotations.insert(
            KATA_ANNO_CFG_HYPERVISOR_IMAGE_PATH.to_string(),
            config.boot_info.image.to_string(),
        );
        annotations
    }

    pub(crate) async fn prepare_vm(
        &mut self,
        id: &str,
        netns: Option<String>,
        annotations: &HashMap<String, String>,
    ) -> Result<()> {
        info!(sl!(), "Preparing REMOTE VM");
        self.id = id.to_string();

        if let Some(netns_path) = &netns {
            debug!(sl!(), "set netns for vmm master {:?}", &netns_path);
            std::fs::metadata(netns_path).context("check netns path")?;
        }

        let client = self.get_ttrpc_client()?;

        let ctx = context::Context::default();
        let req = CreateVMRequest {
            id: id.to_string(),
            annotations: self.prepare_annotations(annotations),
            networkNamespacePath: netns.clone().unwrap_or_default(),
            ..Default::default()
        };
        info!(sl!(), "Preparing REMOTE VM req: {:?}", req.clone());
        let resp = client.create_vm(ctx, &req).await?;
        info!(sl!(), "Preparing REMOTE VM resp: {:?}", resp.clone());
        self.agent_socket_path = resp.agentSocketPath;
        self.netns = netns;
        Ok(())
    }

    pub(crate) async fn start_vm(&mut self, timeout: i32) -> Result<()> {
        info!(sl!(), "Starting REMOTE VM");

        let mut min_timeout = DEFAULT_MIN_TIMEOUT;
        if self.config.remote_info.hypervisor_timeout > 0 {
            min_timeout = self.config.remote_info.hypervisor_timeout.min(timeout);
        }
        let timeout = min_timeout;

        let client = self.get_ttrpc_client()?;

        let req = StartVMRequest {
            id: self.id.clone(),
            ..Default::default()
        };
        let ctx =
            context::with_timeout(time::Duration::from_secs(timeout as u64).as_nanos() as i64);
        let _resp = client.start_vm(ctx, &req).await?;

        Ok(())
    }

    pub(crate) async fn stop_vm(&mut self) -> Result<()> {
        info!(sl!(), "Stopping REMOTE VM");

        let client = self.get_ttrpc_client()?;

        let ctx = context::with_timeout(time::Duration::from_secs(1).as_nanos() as i64);
        let req = StopVMRequest {
            id: self.id.clone(),
            ..Default::default()
        };
        let _resp = client.stop_vm(ctx, &req).await?;

        self.exit_notify.take().unwrap().send(1).await?;
        Ok(())
    }

    pub(crate) async fn pause_vm(&self) -> Result<()> {
        warn!(sl!(), "RemoteInner::pause_vm(): NOT YET IMPLEMENTED");
        todo!()
    }

    pub(crate) async fn wait_vm(&self) -> Result<i32> {
        info!(sl!(), "Wait Remote VM");
        let mut waiter = self.exit_waiter.lock().await;
        if let Some(exitcode) = waiter.0.recv().await {
            waiter.1 = exitcode;
        }

        Ok(waiter.1)
    }

    pub(crate) async fn resume_vm(&self) -> Result<()> {
        warn!(sl!(), "RemoteInner::resume_vm(): NOT YET IMPLEMENTED");
        todo!()
    }

    pub(crate) async fn save_vm(&self) -> Result<()> {
        warn!(sl!(), "RemoteInner::save_vm(): NOT YET IMPLEMENTED");
        todo!()
    }

    pub(crate) async fn add_device(&self, device: DeviceType) -> Result<DeviceType> {
        warn!(sl!(), "RemoteInner::add_device(): NOT YET IMPLEMENTED");
        Ok(device)
    }

    pub(crate) async fn remove_device(&self, _device: DeviceType) -> Result<()> {
        warn!(sl!(), "RemoteInner::remove_device(): NOT YET IMPLEMENTED");
        Ok(())
    }

    pub(crate) async fn update_device(&self, _device: DeviceType) -> Result<()> {
        warn!(sl!(), "RemoteInner::update_device(): NOT YET IMPLEMENTED");
        Ok(())
    }

    pub(crate) async fn get_agent_socket(&self) -> Result<String> {
        Ok(format!("{}://{}", REMOTE_SCHEME, &self.agent_socket_path))
    }

    pub(crate) async fn disconnect(&mut self) {
        warn!(sl!(), "RemoteInner::disconnect(): NOT YET IMPLEMENTED");
        todo!()
    }

    pub fn hypervisor_config(&self) -> HypervisorConfig {
        info!(
            sl!(),
            "RemoteInner::hypervisor_config(): {:?}",
            self.config.clone()
        );
        self.config.clone()
    }

    pub(crate) async fn get_thread_ids(&self) -> Result<VcpuThreadIds> {
        warn!(sl!(), "RemoteInner::get_thread_ids(): NOT YET IMPLEMENTED");
        let vcpu_thread_ids: VcpuThreadIds = VcpuThreadIds {
            vcpus: HashMap::new(),
        };
        Ok(vcpu_thread_ids)
    }

    pub(crate) async fn get_vmm_master_tid(&self) -> Result<u32> {
        warn!(sl!(), "RemoteInner::get_vmm_master_tid()");
        let tid = nix::unistd::gettid().as_raw();
        Ok(tid as u32)
    }

    pub(crate) async fn get_ns_path(&self) -> Result<String> {
        info!(sl!(), "RemoteInner::get_ns_path()");
        Ok(self.netns.clone().unwrap_or_default())
    }

    pub(crate) async fn cleanup(&self) -> Result<()> {
        info!(sl!(), "RemoteInner::cleanup(): NOT YET IMPLEMENTED");
        Ok(())
    }

    pub(crate) async fn resize_vcpu(
        &mut self,
        _old_vcpus: u32,
        _new_vcpus: u32,
    ) -> Result<(u32, u32)> {
        info!(sl!(), "RemoteInner::resize_vcpu(): NOT YET IMPLEMENTED");
        Ok((_old_vcpus, _new_vcpus))
    }

    pub(crate) async fn get_pids(&self) -> Result<Vec<u32>> {
        warn!(sl!(), "RemoteInner::get_pids(): NOT YET IMPLEMENTED");
        todo!()
    }

    pub(crate) async fn check(&self) -> Result<()> {
        warn!(sl!(), "RemoteInner::check(): NOT YET IMPLEMENTED");
        todo!()
    }

    pub(crate) async fn get_jailer_root(&self) -> Result<String> {
        warn!(sl!(), "RemoteInner::get_jailer_root(): NOT YET IMPLEMENTED");
        Ok("".into())
    }

    pub(crate) async fn capabilities(&self) -> Result<Capabilities> {
        Ok(Capabilities::default())
    }

    pub fn set_hypervisor_config(&mut self, config: HypervisorConfig) {
        self.config = config;
    }

    pub(crate) async fn get_hypervisor_metrics(&self) -> Result<String> {
        warn!(
            sl!(),
            "RemoteInner::get_hypervisor_metrics(): NOT YET IMPLEMENTED"
        );
        todo!()
    }

    pub(crate) fn set_capabilities(&mut self, _flag: CapabilityBits) {
        warn!(
            sl!(),
            "RemoteInner::set_capabilities(): NOT YET IMPLEMENTED"
        );
        todo!()
    }

    pub(crate) fn set_guest_memory_block_size(&mut self, _size: u32) {
        info!(
            sl!(),
            "RemoteInner::set_guest_memory_block_size(): NOT YET IMPLEMENTED"
        )
    }

    pub(crate) fn guest_memory_block_size_mb(&self) -> u32 {
        warn!(
            sl!(),
            "RemoteInner::guest_memory_block_size_mb(): NOT YET IMPLEMENTED"
        );
        0
    }

    pub(crate) fn resize_memory(&self, _new_mem_mb: u32) -> Result<(u32, MemoryConfig)> {
        Ok((
            _new_mem_mb,
            MemoryConfig {
                ..Default::default()
            },
        ))
    }
}

#[async_trait]
impl Persist for RemoteInner {
    type State = HypervisorState;
    type ConstructorArgs = ();

    /// Save a state of hypervisor
    async fn save(&self) -> Result<Self::State> {
        Ok(HypervisorState {
            hypervisor_type: HYPERVISOR_REMOTE.to_string(),
            id: self.id.clone(),
            config: self.config.clone(),
            netns: self.netns.clone(),
            ..Default::default()
        })
    }

    /// Restore hypervisor
    async fn restore(
        _hypervisor_args: Self::ConstructorArgs,
        hypervisor_state: Self::State,
    ) -> Result<Self> {
        let (exit_notify, exit_waiter) = mpsc::channel(1);

        Ok(RemoteInner {
            id: hypervisor_state.id,
            config: hypervisor_state.config,
            agent_socket_path: "".to_string(),
            netns: hypervisor_state.netns,
            client: None,
            exit_notify: Some(exit_notify),
            exit_waiter: Mutex::new((exit_waiter, 0)),
        })
    }
}
