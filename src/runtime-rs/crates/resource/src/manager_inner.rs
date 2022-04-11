// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use agent::{Agent, Storage};
use anyhow::{Context, Result};
use hypervisor::Hypervisor;
use kata_types::config::TomlConfig;
use kata_types::mount::Mount;
use oci::LinuxResources;

use crate::{
    cgroups::CgroupsResource,
    rootfs::{RootFsResource, Rootfs},
    share_fs::{self, ShareFs},
    volume::{Volume, VolumeResource},
    ResourceConfig,
};

pub(crate) struct ResourceManagerInner {
    sid: String,
    // TODO: remove
    #[allow(dead_code)]
    agent: Arc<dyn Agent>,
    hypervisor: Arc<dyn Hypervisor>,
    share_fs: Option<Arc<dyn ShareFs>>,

    pub rootfs_resource: RootFsResource,
    pub volume_resource: VolumeResource,
    pub cgroups_resource: CgroupsResource,
}

impl ResourceManagerInner {
    pub(crate) fn new(
        sid: &str,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        toml_config: &TomlConfig,
    ) -> Result<Self> {
        Ok(Self {
            sid: sid.to_string(),
            agent,
            hypervisor,
            share_fs: None,
            rootfs_resource: RootFsResource::new(),
            volume_resource: VolumeResource::new(),
            cgroups_resource: CgroupsResource::new(sid, toml_config)?,
        })
    }

    pub async fn prepare_before_start_vm(
        &mut self,
        device_configs: Vec<ResourceConfig>,
    ) -> Result<()> {
        for dc in device_configs {
            match dc {
                ResourceConfig::ShareFs(c) => {
                    let share_fs = share_fs::new(&self.sid, &c).context("new share fs")?;
                    share_fs
                        .setup_device_before_start_vm(self.hypervisor.as_ref())
                        .await
                        .context("setup share fs device before start vm")?;
                    self.share_fs = Some(share_fs);
                }
            };
        }

        Ok(())
    }

    pub async fn setup_after_start_vm(&mut self) -> Result<()> {
        if let Some(share_fs) = self.share_fs.as_ref() {
            share_fs
                .setup_device_after_start_vm(self.hypervisor.as_ref())
                .await
                .context("setup share fs device after start vm")?;
        }

        Ok(())
    }

    pub async fn get_storage_for_sandbox(&self) -> Result<Vec<Storage>> {
        let mut storages = vec![];
        if let Some(d) = self.share_fs.as_ref() {
            let mut s = d.get_storages().await.context("get storage")?;
            storages.append(&mut s);
        }
        Ok(storages)
    }

    pub async fn handler_rootfs(
        &self,
        cid: &str,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
    ) -> Result<Arc<dyn Rootfs>> {
        self.rootfs_resource
            .handler_rootfs(&self.share_fs, cid, bundle_path, rootfs_mounts)
            .await
    }

    pub async fn handler_volumes(
        &self,
        cid: &str,
        oci_mounts: &[oci::Mount],
    ) -> Result<Vec<Arc<dyn Volume>>> {
        self.volume_resource
            .handler_volumes(&self.share_fs, cid, oci_mounts)
            .await
    }

    pub async fn update_cgroups(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
    ) -> Result<()> {
        self.cgroups_resource
            .update_cgroups(cid, linux_resources, self.hypervisor.as_ref())
            .await
    }

    pub async fn delete_cgroups(&self) -> Result<()> {
        self.cgroups_resource.delete().await
    }

    pub async fn dump(&self) {
        self.rootfs_resource.dump().await;
        self.volume_resource.dump().await;
    }
}
