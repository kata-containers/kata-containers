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
use super::share_virtio_fs::{
    prepare_virtiofs, FS_TYPE_VIRTIO_FS, KATA_VIRTIO_FS_DEV_TYPE, MOUNT_GUEST_TAG,
};
use super::utils::get_host_rw_shared_path;
use super::virtio_fs_share_mount::VirtiofsShareMount;
use super::{kata_guest_nydus_root_dir, MountedInfo, NydusShareFs, ShareFs, ShareFsMount};

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
    nydusd: Arc<RwLock<Option<Arc<Nydusd>>>>,
    share_fs_mount: Arc<dyn ShareFsMount>,
    mounted_info_set: Arc<Mutex<HashMap<String, MountedInfo>>>,
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
        })
    }

    async fn setup_nydusd(&self, h: &dyn Hypervisor) -> Result<()> {
        let jailer_root = h.get_jailer_root().await?;
        let sock_path = Path::new(&jailer_root).join("virtiofsd.sock");
        let api_sock_path = Path::new(&jailer_root).join(NYDUSD_API_SOCK);

        // new and validate nydusd config
        let nydusd_config = NydusdConfig::new(
            self.config.virtio_fs_daemon.clone(),
            sock_path,
            api_sock_path,
            get_host_rw_shared_path(&self.config.id),
            self.config.debug,
            self.config.virtio_fs_extra_args.clone(),
        )
        .validate()
        .context("validate nydusd config")?;

        let mut nydusd_guard = self.nydusd.write().await;
        if nydusd_guard.is_some() {
            return Err(anyhow!("nydusd already initialized"));
        }
        let nydusd = nydusd_guard.insert(Arc::new(Nydusd::new(nydusd_config)));
        let pid = nydusd.start().await.context("failed to start nydusd")?;

        info!(sl!(), "nydusd started with pid {}", pid);

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

        prepare_virtiofs(d, KATA_VIRTIO_FS_DEV_TYPE, &self.config.id, &jailer_root)
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
            shared: false,
        };

        storages.push(shared_volume);
        Ok(storages)
    }

    fn mounted_info_set(&self) -> Arc<Mutex<HashMap<String, MountedInfo>>> {
        self.mounted_info_set.clone()
    }

    async fn stop(&self) -> Result<()> {
        info!(sl!(), "stopping nydusd daemon");
        let nydusd = self.nydusd.read().await.clone();
        if let Some(nydusd) = nydusd {
            nydusd.stop().await.context("failed to stop nydusd")?;
            let mut nydusd_guard = self.nydusd.write().await;
            if nydusd_guard
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &nydusd))
            {
                *nydusd_guard = None;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl NydusShareFs for ShareVirtioFsNydus {
    async fn mount_rafs(&self, cid: &str, rafs_meta: &str, config: &str) -> Result<String> {
        let mountpoint = format!("/rafs/{}/lowerdir", cid);
        let nydusd = self
            .nydusd
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow!("nydusd not initialized"))?;

        nydusd
            .mount_rafs(&mountpoint, &PathBuf::from(rafs_meta), config)
            .await
            .context("failed to mount rafs via nydusd API")?;

        Ok(mountpoint)
    }

    async fn umount_rafs(&self, mountpoint: &str) -> Result<()> {
        let nydusd = self
            .nydusd
            .read()
            .await
            .clone()
            .ok_or_else(|| anyhow!("nydusd not initialized"))?;
        nydusd
            .umount(mountpoint)
            .await
            .context("failed to umount rafs via nydusd API")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn stop_keeps_nydusd_after_cancellation() {
        let share = ShareVirtioFsNydus::new("test", &SharedFsInfo::default()).unwrap();
        let nydusd = Nydusd::new(NydusdConfig::new(
            PathBuf::new(),
            PathBuf::new(),
            PathBuf::new(),
            PathBuf::new(),
            false,
            vec![],
        ));
        let lock = nydusd.hold_stop_lock_for_test().await;
        *share.nydusd.write().await = Some(Arc::new(nydusd));

        assert!(timeout(Duration::from_millis(10), share.stop())
            .await
            .is_err());
        drop(lock);

        assert!(share.nydusd.read().await.is_some());
        share.stop().await.unwrap();
        assert!(share.nydusd.read().await.is_none());
        share.stop().await.unwrap();
    }

    #[tokio::test]
    async fn stop_does_not_block_nydusd_access_while_waiting_for_exit() {
        let share = ShareVirtioFsNydus::new("test", &SharedFsInfo::default()).unwrap();
        let nydusd = Arc::new(Nydusd::new(NydusdConfig::new(
            PathBuf::new(),
            PathBuf::new(),
            PathBuf::new(),
            PathBuf::new(),
            false,
            vec![],
        )));
        let stop_lock = nydusd.hold_stop_lock_for_test().await;
        *share.nydusd.write().await = Some(nydusd);

        let stopping = share.stop();
        tokio::pin!(stopping);
        assert!(matches!(
            futures::poll!(stopping.as_mut()),
            std::task::Poll::Pending
        ));
        assert!(timeout(Duration::from_millis(50), share.nydusd.read())
            .await
            .is_ok());

        drop(stop_lock);
        stopping.await.unwrap();
        assert!(share.nydusd.read().await.is_none());
    }
}
