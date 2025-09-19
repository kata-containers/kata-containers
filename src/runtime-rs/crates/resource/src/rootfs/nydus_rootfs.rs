// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::{fs, sync::Arc};

use super::{Rootfs, TYPE_OVERLAY_FS};
use crate::{
    rootfs::ROOTFS,
    share_fs::{
        do_get_guest_path, get_host_rw_shared_path, ShareFs,
        ShareFsRootfsConfig, PASSTHROUGH_FS_DIR,
    },
    share_fs::{ShareFsMount, ShareFsVolumeConfig},
};
use agent::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::{device::device_manager::DeviceManager, Hypervisor};
use kata_types::mount::{Mount, NydusExtraOptions};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;
// Used for nydus rootfs
pub(crate) const NYDUS_ROOTFS_TYPE: &str = "fuse.nydus-overlayfs";

const SNAPSHOT_DIR: &str = "snapshotdir";
const KATA_OVERLAY_DEV_TYPE: &str = "overlayfs";

pub(crate) struct NydusRootfs {
    guest_path: String,
    rootfs: Storage,
    source: String,
    share_fs_mount: Arc<dyn ShareFsMount>,
}

impl NydusRootfs {
    pub async fn new(
        _d: &RwLock<DeviceManager>,
        share_fs: &Arc<dyn ShareFs>,
        _h: &dyn Hypervisor,
        sid: &str,
        cid: &str,
        rootfs: &Mount,
    ) -> Result<Self> {
        let share_fs_mount = share_fs.get_share_fs_mount();
        let extra_options =
            NydusExtraOptions::new(rootfs).context("failed to parse nydus extra options")?;
        info!(sl!(), "extra_option {:?}", &extra_options);

        // Instead of starting our own nydusd and calling mount API,
        // we directly use the snapshot directory that nydus snapshotter has already prepared.
        // The nydus snapshotter handles the fuse mode nydusd and RAFS mounting.
        let rafs_mount_path = extra_options.snapshot_dir.clone();
        
        info!(sl!(), "Using nydus snapshotter prepared mount path: {}", rafs_mount_path);

        let volume_config = ShareFsVolumeConfig {
            cid: cid.to_string(),
            source: rafs_mount_path.clone(),
            target: "".to_string(),
            readonly: true,
            mount_options: vec![],
            mount: oci::Mount::default(),
            is_rafs: true,
        };

        let mount_result = share_fs_mount.share_volume(&volume_config).await?;

        // create rootfs under the share directory
        let container_share_dir = get_host_rw_shared_path(sid)
            .join(PASSTHROUGH_FS_DIR)
            .join(cid);
        let rootfs_dir = container_share_dir.join(ROOTFS);
        fs::create_dir_all(rootfs_dir).context("failed to create directory")?;
        // mount point inside the guest
        let rootfs_guest_path = do_get_guest_path(ROOTFS, cid, false, false);
        // bind mount the snapshot dir under the share directory
        share_fs_mount
            .share_rootfs(&ShareFsRootfsConfig {
                cid: cid.to_string(),
                source: extra_options.snapshot_dir.clone(),
                target: SNAPSHOT_DIR.to_string(),
                readonly: false,
                is_rafs: false,
            })
            .await
            .context("share nydus rootfs")?;
        let mut options: Vec<String> = Vec::new();
        options.push(
            "lowerdir=".to_string()
                + &mount_result.guest_path,
        );
        options.push(
            "workdir=".to_string()
                + &do_get_guest_path(
                    format!("{}/{}", SNAPSHOT_DIR, "work").as_str(),
                    cid,
                    false,
                    false,
                ),
        );
        options.push(
            "upperdir=".to_string()
                + &do_get_guest_path(
                    format!("{}/{}", SNAPSHOT_DIR, "fs").as_str(),
                    cid,
                    false,
                    false,
                ),
        );
        options.push("index=off".to_string());
        let rootfs_storage = Storage {
            driver: KATA_OVERLAY_DEV_TYPE.to_string(),
            source: TYPE_OVERLAY_FS.to_string(),
            fs_type: TYPE_OVERLAY_FS.to_string(),
            options,
            mount_point: rootfs_guest_path.clone(),
            ..Default::default()
        };

        Ok(NydusRootfs {
            guest_path: rootfs_guest_path,
            rootfs: rootfs_storage,
            source: rafs_mount_path,
            share_fs_mount: share_fs_mount.clone(),
        })
    }
}

#[async_trait]
impl Rootfs for NydusRootfs {
    async fn get_guest_rootfs_path(&self) -> Result<String> {
        Ok(self.guest_path.clone())
    }

    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![])
    }

    async fn get_storage(&self) -> Option<Storage> {
        Some(self.rootfs.clone())
    }

    async fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        self.share_fs_mount.umount_volume(&self.source).await?;
        // Note: We don't need to umount nydusd here because the nydus snapshotter
        // manages the lifecycle of fuse mode nydusd instances.
        info!(sl!(), "Nydus rootfs cleanup completed for source: {}", self.source);
        Ok(())
    }
}
