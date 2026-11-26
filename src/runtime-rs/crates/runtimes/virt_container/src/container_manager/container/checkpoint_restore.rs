// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use super::*;

impl Container {
    pub async fn new_restored(
        pid: u32,
        config: ContainerConfig,
        spec: oci::Spec,
        agent: Arc<dyn Agent>,
        resource_manager: Arc<ResourceManager>,
        restored_rootfs: Arc<dyn Rootfs>,
        use_guest_resolv_conf: bool,
    ) -> Result<Self> {
        let container = Self::new(
            pid,
            config,
            spec,
            agent,
            resource_manager,
            use_guest_resolv_conf,
            None,
        )
        .await?;

        let mut inner = container.inner.write().await;
        inner.restored_paused = true;
        inner.rootfs.push(restored_rootfs);

        drop(inner);

        Ok(container)
    }

    pub async fn discard_unstarted_restored(&self) -> Result<bool> {
        let mut inner = self.inner.write().await;
        if !inner.restored_paused {
            return Ok(false);
        }

        let device_manager = self.resource_manager.get_device_manager().await;
        inner
            .discard_unstarted_container(self.container_id.container_id.as_str(), &device_manager)
            .await
            .context("cleanup unstarted restored container")?;
        inner.restored_paused = false;

        Ok(true)
    }

    pub async fn checkpoint_rootfs_ids(&self) -> Result<Vec<String>> {
        let inner = self.inner.read().await;
        if !inner.exec_processes.is_empty() {
            return Err(anyhow!(
                "container {} has active exec processes",
                self.container_id
            ));
        }

        if inner.rootfs.len() != 1 {
            return Err(anyhow!(
                "container {} has {} rootfs objects, expected one",
                self.container_id,
                inner.rootfs.len()
            ));
        }

        let ids = inner.rootfs[0].get_device_ids().await?;
        if ids.is_empty() || ids.iter().any(String::is_empty) {
            return Err(anyhow!(
                "container {} rootfs is not block-backed",
                self.container_id
            ));
        }

        Ok(ids)
    }
}
