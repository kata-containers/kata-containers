// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::resource_persist::ResourceState;
use crate::{manager_inner::ResourceManagerInner, rootfs::Rootfs, volume::Volume, ResourceConfig};
use agent::{Agent, Storage};
use anyhow::Result;
use async_trait::async_trait;
use hypervisor::Hypervisor;
use kata_types::config::TomlConfig;
use kata_types::mount::Mount;
use oci::LinuxResources;
use persist::sandbox_persist::Persist;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ManagerArgs {
    pub sid: String,
    pub agent: Arc<dyn Agent>,
    pub hypervisor: Arc<dyn Hypervisor>,
    pub config: TomlConfig,
}

pub struct ResourceManager {
    inner: Arc<RwLock<ResourceManagerInner>>,
}

impl ResourceManager {
    pub fn new(
        sid: &str,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        toml_config: Arc<TomlConfig>,
    ) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(ResourceManagerInner::new(
                sid,
                agent,
                hypervisor,
                toml_config,
            )?)),
        })
    }

    pub async fn config(&self) -> Arc<TomlConfig> {
        let inner = self.inner.read().await;
        inner.config()
    }

    pub async fn prepare_before_start_vm(&self, device_configs: Vec<ResourceConfig>) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.prepare_before_start_vm(device_configs).await
    }

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
        bundle_path: &str,
        rootfs_mounts: &[Mount],
    ) -> Result<Arc<dyn Rootfs>> {
        let inner = self.inner.read().await;
        inner.handler_rootfs(cid, bundle_path, rootfs_mounts).await
    }

    pub async fn handler_volumes(
        &self,
        cid: &str,
        oci_mounts: &[oci::Mount],
    ) -> Result<Vec<Arc<dyn Volume>>> {
        let inner = self.inner.read().await;
        inner.handler_volumes(cid, oci_mounts).await
    }

    pub async fn dump(&self) {
        let inner = self.inner.read().await;
        inner.dump().await
    }

    pub async fn update_cgroups(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
    ) -> Result<()> {
        let inner = self.inner.read().await;
        inner.update_cgroups(cid, linux_resources).await
    }

    pub async fn delete_cgroups(&self) -> Result<()> {
        let inner = self.inner.read().await;
        inner.delete_cgroups().await
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
