// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use agent::{Agent, Storage};
use anyhow::Result;
use hypervisor::Hypervisor;
use kata_types::config::TomlConfig;
use kata_types::mount::Mount;
use oci::LinuxResources;
use tokio::sync::RwLock;

use crate::{manager_inner::ResourceManagerInner, rootfs::Rootfs, volume::Volume, ResourceConfig};

pub struct ResourceManager {
    inner: Arc<RwLock<ResourceManagerInner>>,
}

impl ResourceManager {
    pub fn new(
        sid: &str,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        toml_config: &TomlConfig,
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
