// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{Device as HypervisorDevice, ShareFsMountConfig, ShareFsMountType, ShareFsOperation},
    Hypervisor,
};
use kata_types::config::hypervisor::SharedFsInfo;

use super::{
    share_virtio_fs::{
        prepare_virtiofs, FS_TYPE_VIRTIO_FS, KATA_VIRTIO_FS_DEV_TYPE, MOUNT_GUEST_TAG,
        PASSTHROUGH_FS_DIR,
    },
    utils, ShareFs, *,
};

lazy_static! {
    pub(crate) static ref SHARED_DIR_VIRTIO_FS_OPTIONS: Vec::<String> = vec![String::from("nodev")];
}

#[derive(Debug, Clone)]
pub struct ShareVirtioFsInlineConfig {
    pub id: String,
}

pub struct ShareVirtioFsInline {
    config: ShareVirtioFsInlineConfig,
    share_fs_mount: Arc<dyn ShareFsMount>,
}

impl ShareVirtioFsInline {
    pub(crate) fn new(id: &str, _config: &SharedFsInfo) -> Result<Self> {
        Ok(Self {
            config: ShareVirtioFsInlineConfig { id: id.to_string() },
            share_fs_mount: Arc::new(VirtiofsShareMount::new(id)),
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
}

async fn setup_inline_virtiofs(id: &str, h: &dyn Hypervisor) -> Result<()> {
    // - source is the absolute path of PASSTHROUGH_FS_DIR on host, e.g.
    //   /run/kata-containers/shared/sandboxes/<sid>/passthrough
    // - mount point is the path relative to KATA_GUEST_SHARE_DIR in guest
    let mnt = format!("/{}", PASSTHROUGH_FS_DIR);

    let rw_source = utils::get_host_rw_shared_path(id).join(PASSTHROUGH_FS_DIR);
    utils::ensure_dir_exist(&rw_source)?;

    let ro_source = utils::get_host_ro_shared_path(id).join(PASSTHROUGH_FS_DIR);
    let source = String::from(ro_source.to_str().unwrap());

    let virtio_fs = HypervisorDevice::ShareFsMount(ShareFsMountConfig {
        source: source.clone(),
        fstype: ShareFsMountType::PASSTHROUGH,
        mount_point: mnt,
        config: None,
        tag: String::from(MOUNT_GUEST_TAG),
        op: ShareFsOperation::Mount,
        prefetch_list_path: None,
    });
    h.add_device(virtio_fs)
        .await
        .context(format!("fail to attach passthrough fs {:?}", source))
}
