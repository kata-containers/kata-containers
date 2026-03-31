// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};
use std::{collections::HashMap, sync::Arc};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use tokio::sync::{Mutex, RwLock};

use agent::Storage;
use hypervisor::{device::device_manager::DeviceManager, Hypervisor};
use kata_types::config::hypervisor::SharedFsInfo;

use super::nydus::nydus_daemon::{Nydusd, NydusdConfig};
use super::share_virtio_fs::{prepare_virtiofs, FS_TYPE_VIRTIO_FS, KATA_VIRTIO_FS_DEV_TYPE, MOUNT_GUEST_TAG};
use super::utils::{ensure_dir_exist, get_host_rw_shared_path};
use super::virtio_fs_share_mount::VirtiofsShareMount;
use super::{MountedInfo, NydusShareFs, ShareFs, ShareFsMount, VIRTIO_FS_NYDUS, kata_guest_nydus_root_dir};

const NYDUSD_API_SOCK: &str = "nydusd-api.sock";

#[derive(Debug, Clone)]
pub struct ShareVirtioFsNydusConfig {
    id: String,
    pub virtio_fs_daemon: PathBuf,
    pub virtio_fs_extra_args: Vec<String>,
    pub debug: bool,
}

pub struct ShareVirtioFsNydus {
    config: ShareVirtioFsNydusConfig,
    nydusd: Arc<RwLock<Option<Nydusd>>>,
    share_fs_mount: Arc<dyn ShareFsMount>,
    mounted_info_set: Arc<Mutex<HashMap<String, MountedInfo>>>,
    jailer_root: RwLock<String>,
}

impl ShareVirtioFsNydus {
    pub fn new(id: &str, config: &SharedFsInfo) -> Result<Self> {
        Ok(Self {
            config: ShareVirtioFsNydusConfig {
                id: id.to_string(),
                virtio_fs_daemon: config.virtio_fs_daemon.clone().into(),
                virtio_fs_extra_args: config.virtio_fs_extra_args.clone(),
                debug: false,
            },
            nydusd: Arc::new(RwLock::new(None)),
            share_fs_mount: Arc::new(VirtiofsShareMount::new(id)),
            mounted_info_set: Arc::new(Mutex::new(HashMap::new())),
            jailer_root: RwLock::new(String::new()),
        })
    }

    async fn setup_nydusd(&self, h: &dyn Hypervisor) -> Result<()> {
        let jailer_root = h.get_jailer_root().await?;
        {
            let mut root = self.jailer_root.write().await;
            *root = jailer_root.clone();
        }
        let sock_path = Path::new(&jailer_root).join("virtiofsd.sock");
        let api_sock_path = Path::new(&jailer_root).join(NYDUSD_API_SOCK);

        // Use the RW path for nydusd passthrough_fs source.
        // This is critical because the passthrough_fs needs to support
        // write operations for container overlay upperdir and workdir.
        let source_path = get_host_rw_shared_path(&self.config.id);
        ensure_dir_exist(&source_path)?;

        let nydusd_config = NydusdConfig {
            path: self.config.virtio_fs_daemon.clone(),
            sock_path: sock_path.clone(),
            api_sock_path: api_sock_path.clone(),
            source_path,
            debug: self.config.debug,
            extra_args: self.config.virtio_fs_extra_args.clone(),
        };

        let nydusd = Nydusd::new(nydusd_config);
        let pid = nydusd.start().await.context("failed to start nydusd")?;

        info!(sl!(), "nydusd started with pid {}", pid);

        {
            let mut n = self.nydusd.write().await;
            *n = Some(nydusd);
        }

        Ok(())
    }
}

#[async_trait]
impl ShareFs for ShareVirtioFsNydus {
    fn get_share_fs_mount(&self) -> Arc<dyn ShareFsMount> {
        self.share_fs_mount.clone()
    }

    async fn setup_device_before_start_vm(
        &self,
        h: &dyn Hypervisor,
        d: &RwLock<DeviceManager>,
    ) -> Result<()> {
        let jailer_root = h.get_jailer_root().await?;

        prepare_virtiofs(d, VIRTIO_FS_NYDUS, &self.config.id, &jailer_root)
            .await
            .context("prepare virtiofs for nydus")?;

        self.setup_nydusd(h).await.context("setup nydusd")?;

        Ok(())
    }

    async fn setup_device_after_start_vm(
        &self,
        _h: &dyn Hypervisor,
        _d: &RwLock<DeviceManager>,
    ) -> Result<()> {
        Ok(())
    }

    async fn get_storages(&self) -> Result<Vec<Storage>> {
        let mut storages: Vec<Storage> = Vec::new();

        // In nydusd mode, virtiofs is mounted at `/run/kata-containers/shared/`, because nydusd's
        // internal passthrough_fs is mounted at `/containers` within the virtiofs namespace, which
        // maps to `/run/kata-containers/shared/containers/` in the guest.
        let shared_volume = Storage {
            driver: String::from(KATA_VIRTIO_FS_DEV_TYPE),
            driver_options: Vec::new(),
            source: String::from(MOUNT_GUEST_TAG),
            fs_type: String::from(FS_TYPE_VIRTIO_FS),
            fs_group: None,
            options: vec![String::from("nodev")],
            mount_point: kata_guest_nydus_root_dir(),
        };

        storages.push(shared_volume);
        Ok(storages)
    }

    fn mounted_info_set(&self) -> Arc<Mutex<HashMap<String, MountedInfo>>> {
        self.mounted_info_set.clone()
    }

    async fn stop(&self) -> Result<()> {
        info!(sl!(), "stopping nydusd daemon");
        let mut nydusd_guard = self.nydusd.write().await;
        if let Some(nydusd) = nydusd_guard.take() {
            nydusd.stop().await.context("failed to stop nydusd")?;
        }
        Ok(())
    }
}

#[async_trait]
impl NydusShareFs for ShareVirtioFsNydus {
    async fn mount_rafs(
        &self,
        cid: &str,
        rafs_meta: &str,
        config: &str,
        overlay_config: &str,
    ) -> Result<String> {
        // Standalone mode: mount via nydusd API with native overlay support
        let rafs_mnt = format!("/rafs/{}/lowerdir", cid);
        self.do_mount_rafs_with_overlay(&rafs_mnt, &PathBuf::from(rafs_meta), config, overlay_config)
            .await
            .context("failed to mount rafs with overlay via nydusd API")?;
        Ok(rafs_mnt)
    }

    async fn umount_rafs(&self, mountpoint: &str) -> Result<()> {
        let nydusd_guard = self.nydusd.read().await;
        let nydusd = nydusd_guard
            .as_ref()
            .ok_or_else(|| anyhow!("nydusd not initialized"))?;
        nydusd
            .umount(mountpoint)
            .await
            .context("failed to umount rafs via nydusd API")
    }
}

impl ShareVirtioFsNydus {
    /// Mount rafs with nydusd native overlay support
    /// This creates a writable overlay filesystem directly in nydusd
    /// The overlay_config should be a JSON string containing:
    /// - upper_dir: path to the upper directory in the guest
    /// - work_dir: path to the work directory in the guest
    async fn do_mount_rafs_with_overlay(
        &self,
        mountpoint: &str,
        source: &PathBuf,
        config: &str,
        overlay_config: &str,
    ) -> Result<()> {
        let nydusd_guard = self.nydusd.read().await;
        let nydusd = nydusd_guard
            .as_ref()
            .ok_or_else(|| anyhow!("nydusd not initialized"))?;
        nydusd
            .mount_rafs_with_overlay(mountpoint, source, config, overlay_config)
            .await
    }
}