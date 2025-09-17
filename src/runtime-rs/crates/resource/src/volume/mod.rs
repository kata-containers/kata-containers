// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod block_volume;
mod default_volume;
mod ephemeral_volume;
pub mod hugepage;
mod share_fs_volume;
mod shm_volume;
pub mod utils;

pub mod direct_volume;
use crate::volume::{direct_volume::is_direct_volume, share_fs_volume::VolumeManager};
pub mod direct_volumes;

use std::{sync::Arc, vec::Vec};

use self::hugepage::{get_huge_page_limits_map, get_huge_page_option};
use crate::{share_fs::ShareFs, volume::block_volume::is_block_volume};
use agent::Agent;
use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::get_mount_options;
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

const BIND: &str = "bind";

#[async_trait]
pub trait Volume: Send + Sync {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>>;
    fn get_storage(&self) -> Result<Vec<agent::Storage>>;
    fn get_device_id(&self) -> Result<Option<String>>;
    async fn cleanup(&self, device_manager: &RwLock<DeviceManager>) -> Result<()>;
}

#[derive(Default)]
pub struct VolumeResourceInner {
    volumes: Vec<Arc<dyn Volume>>,
}

#[derive(Default)]
pub struct VolumeResource {
    inner: Arc<RwLock<VolumeResourceInner>>,
    // The core purpose of introducing `volume_manager` to `VolumeResource` is to centralize the management of shared file system volumes.
    // By creating a single VolumeManager instance within VolumeResource, all shared file volumes are managed by one central entity.
    // This single volume_manager can accurately track the references of all ShareFsVolume instances to the shared volumes,
    // ensuring correct reference counting, proper volume lifecycle management, and preventing issues like volumes being overwritten.
    volume_manager: Arc<VolumeManager>,
}

impl VolumeResource {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(VolumeResourceInner::default())),
            volume_manager: Arc::new(VolumeManager::new()),
        }
    }

    pub async fn handler_volumes(
        &self,
        share_fs: &Option<Arc<dyn ShareFs>>,
        cid: &str,
        spec: &oci::Spec,
        d: &RwLock<DeviceManager>,
        sid: &str,
        agent: Arc<dyn Agent>,
    ) -> Result<Vec<Arc<dyn Volume>>> {
        let mut volumes: Vec<Arc<dyn Volume>> = vec![];
        let oci_mounts = &spec.mounts().clone().unwrap_or_default();
        info!(sl!(), " oci mount is : {:?}", oci_mounts.clone());
        // handle mounts
        for m in oci_mounts {
            let read_only = get_mount_options(m.options()).iter().any(|opt| opt == "ro");
            let volume: Arc<dyn Volume> = if shm_volume::is_shm_volume(m) {
                Arc::new(
                    shm_volume::ShmVolume::new(m)
                        .with_context(|| format!("new shm volume {:?}", m))?,
                )
            } else if ephemeral_volume::is_ephemeral_volume(m) {
                Arc::new(
                    ephemeral_volume::EphemeralVolume::new(m)
                        .with_context(|| format!("new ephemeral volume {:?}", m))?,
                )
            } else if is_block_volume(m) {
                // handle block volume
                Arc::new(
                    block_volume::BlockVolume::new(d, m, read_only, sid)
                        .await
                        .with_context(|| format!("new block volume {:?}", m))?,
                )
            } else if is_direct_volume(m)? {
                // handle direct volumes
                match direct_volume::handle_direct_volume(d, m, read_only, sid)
                    .await
                    .context("handle direct volume")?
                {
                    Some(directvol) => directvol,
                    None => continue,
                }
            } else if let Some(options) =
                get_huge_page_option(m).context("failed to check huge page")?
            {
                // get hugepage limits from oci
                let hugepage_limits =
                    get_huge_page_limits_map(spec).context("get huge page option")?;
                // handle container hugepage
                Arc::new(
                    hugepage::Hugepage::new(m, hugepage_limits, options)
                        .with_context(|| format!("handle hugepages {:?}", m))?,
                )
            } else if share_fs_volume::is_share_fs_volume(m) {
                Arc::new(
                    share_fs_volume::ShareFsVolume::new(
                        share_fs,
                        m,
                        cid,
                        read_only,
                        agent.clone(),
                        self.volume_manager.clone(),
                    )
                    .await
                    .with_context(|| format!("new share fs volume {:?}", m))?,
                )
            } else if is_skip_volume(m) {
                info!(sl!(), "skip volume {:?}", m);
                continue;
            } else {
                Arc::new(
                    default_volume::DefaultVolume::new(m)
                        .with_context(|| format!("new default volume {:?}", m))?,
                )
            };

            volumes.push(volume.clone());
            let mut inner = self.inner.write().await;
            inner.volumes.push(volume);
        }

        Ok(volumes)
    }

    pub async fn dump(&self) {
        let inner = self.inner.read().await;
        for v in &inner.volumes {
            info!(
                sl!(),
                "volume mount {:?}: count {}",
                v.get_volume_mount(),
                Arc::strong_count(v)
            );
        }
    }
}

fn is_skip_volume(_m: &oci::Mount) -> bool {
    // TODO: support volume check
    false
}
