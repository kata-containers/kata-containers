// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use agent::{Agent, Storage};
use anyhow::Result;
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use hypervisor::Hypervisor;
use kata_types::config::TomlConfig;
use kata_types::mount::Mount;
use oci::{Linux, LinuxResources};
use oci_spec::runtime as oci;
use persist::sandbox_persist::Persist;
use tokio::sync::RwLock;
use tracing::instrument;

use crate::cdi_devices::ContainerDevice;
use crate::cpu_mem::initial_size::InitialSizeManager;
use crate::network::NetworkConfig;
use crate::resource_persist::ResourceState;
use crate::ResourceUpdateOp;
use crate::{manager_inner::ResourceManagerInner, rootfs::Rootfs, volume::Volume, ResourceConfig};

pub struct ManagerArgs {
    pub sid: String,
    pub agent: Arc<dyn Agent>,
    pub hypervisor: Arc<dyn Hypervisor>,
    pub config: TomlConfig,
}

pub struct ResourceManager {
    inner: Arc<RwLock<ResourceManagerInner>>,
}

impl std::fmt::Debug for ResourceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceManager").finish()
    }
}

impl ResourceManager {
    pub async fn new(
        sid: &str,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        toml_config: Arc<TomlConfig>,
        init_size_manager: InitialSizeManager,
    ) -> Result<Self> {
        // Regist resource logger for later use.
        logging::register_subsystem_logger("runtimes", "resource");

        Ok(Self {
            inner: Arc::new(RwLock::new(
                ResourceManagerInner::new(sid, agent, hypervisor, toml_config, init_size_manager)
                    .await?,
            )),
        })
    }

    pub async fn config(&self) -> Arc<TomlConfig> {
        let inner = self.inner.read().await;
        inner.config()
    }

    pub async fn get_device_manager(&self) -> Arc<RwLock<DeviceManager>> {
        let inner = self.inner.read().await;
        inner.get_device_manager()
    }

    #[instrument]
    pub async fn prepare_before_start_vm(&self, device_configs: Vec<ResourceConfig>) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.prepare_before_start_vm(device_configs).await
    }

    pub async fn handle_network(&self, network_config: NetworkConfig) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.handle_network(network_config).await
    }

    #[instrument]
    pub async fn setup_after_start_vm(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.setup_after_start_vm().await
    }

    pub async fn get_storage_for_sandbox(&self) -> Result<Vec<Storage>> {
        let inner = self.inner.read().await;
        inner.get_storage_for_sandbox().await
    }

    pub async fn handler_rootfs(
        &self,
        cid: &str,
        root: &oci::Root,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
    ) -> Result<Arc<dyn Rootfs>> {
        let inner = self.inner.read().await;
        inner
            .handler_rootfs(cid, root, bundle_path, rootfs_mounts)
            .await
    }

    pub async fn handler_volumes(
        &self,
        cid: &str,
        spec: &oci::Spec,
    ) -> Result<Vec<Arc<dyn Volume>>> {
        let inner = self.inner.read().await;
        inner.handler_volumes(cid, spec).await
    }

    pub async fn handler_devices(&self, cid: &str, linux: &Linux) -> Result<Vec<ContainerDevice>> {
        let inner = self.inner.read().await;
        inner.handler_devices(cid, linux).await
    }

    pub async fn dump(&self) {
        let inner = self.inner.read().await;
        inner.dump().await
    }

    pub async fn update_linux_resource(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
        op: ResourceUpdateOp,
    ) -> Result<Option<LinuxResources>> {
        let inner = self.inner.read().await;
        inner.update_linux_resource(cid, linux_resources, op).await
    }

    pub async fn cleanup(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.cleanup().await
    }
}

#[async_trait]
impl Persist for ResourceManager {
    type State = ResourceState;
    type ConstructorArgs = ManagerArgs;

    /// Save a state of ResourceManager
    async fn save(&self) -> Result<Self::State> {
        let inner = self.inner.read().await;
        inner.save().await
    }

    /// Restore ResourceManager
    async fn restore(
        resource_args: Self::ConstructorArgs,
        resource_state: Self::State,
    ) -> Result<Self> {
        let inner = ResourceManagerInner::restore(resource_args, resource_state).await?;
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
        })
    }
}
