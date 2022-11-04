// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;

use agent::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::Hypervisor;
use kata_types::config::hypervisor::SharedFsInfo;
use tokio::sync::RwLock;

use super::{
    share_virtio_fs::{
        prepare_virtiofs, setup_inline_virtiofs, FS_TYPE_VIRTIO_FS, KATA_VIRTIO_FS_DEV_TYPE,
        MOUNT_GUEST_TAG,
    },
    ShareFs, *,
};

lazy_static! {
    pub(crate) static ref SHARED_DIR_VIRTIO_FS_OPTIONS: Vec::<String> = vec![String::from("nodev")];
}

#[derive(Debug, Clone)]
pub struct ShareVirtioFsInlineConfig {
    pub id: String,
}

#[derive(Default)]
pub struct ShareVirtioFsInlineInner {
    mounted_info_set: HashMap<String, MountedInfo>,
}

pub struct ShareVirtioFsInline {
    config: ShareVirtioFsInlineConfig,
    share_fs_mount: Arc<dyn ShareFsMount>,
    inner: Arc<RwLock<ShareVirtioFsInlineInner>>,
}

impl ShareVirtioFsInline {
    pub(crate) fn new(id: &str, _config: &SharedFsInfo) -> Result<Self> {
        Ok(Self {
            config: ShareVirtioFsInlineConfig { id: id.to_string() },
            share_fs_mount: Arc::new(VirtiofsShareMount::new(id)),
            inner: Arc::new(RwLock::new(ShareVirtioFsInlineInner::default())),
        })
    }
}

#[async_trait]
impl ShareFs for ShareVirtioFsInline {
    fn get_share_fs_mount(&self) -> Arc<dyn ShareFsMount> {
        self.share_fs_mount.clone()
    }

    async fn setup_device_before_start_vm(&self, h: &dyn Hypervisor) -> Result<()> {
        prepare_virtiofs(h, INLINE_VIRTIO_FS, &self.config.id, "")
            .await
            .context("prepare virtiofs")?;
        Ok(())
    }

    async fn setup_device_after_start_vm(&self, h: &dyn Hypervisor) -> Result<()> {
        setup_inline_virtiofs(&self.config.id, h)
            .await
            .context("setup inline virtiofs")?;
        Ok(())
    }
    async fn get_storages(&self) -> Result<Vec<Storage>> {
        // setup storage
        let mut storages: Vec<Storage> = Vec::new();

        let shared_volume: Storage = Storage {
            driver: String::from(KATA_VIRTIO_FS_DEV_TYPE),
            driver_options: Vec::new(),
            source: String::from(MOUNT_GUEST_TAG),
            fs_type: String::from(FS_TYPE_VIRTIO_FS),
            fs_group: None,
            options: SHARED_DIR_VIRTIO_FS_OPTIONS.clone(),
            mount_point: String::from(KATA_GUEST_SHARE_DIR),
        };

        storages.push(shared_volume);
        Ok(storages)
    }

    async fn get_mounted_info(&self, source: &str) -> Option<MountedInfo> {
        let inner = self.inner.read().await;
        inner.mounted_info_set.get(source).map(|m| m.clone())
    }

    async fn set_mounted_info(&self, source: &str, mounted_info: MountedInfo) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner
            .mounted_info_set
            .insert(source.to_owned(), mounted_info.clone());
        Ok(())
    }
}
