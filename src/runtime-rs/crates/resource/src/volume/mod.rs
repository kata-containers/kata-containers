// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod block_volume;
mod default_volume;
pub mod hugepage;
mod share_fs_volume;
mod shm_volume;
use async_trait::async_trait;

use anyhow::{Context, Result};
use std::{sync::Arc, vec::Vec};
use tokio::sync::RwLock;

use crate::share_fs::ShareFs;

use self::hugepage::{get_huge_page_limits_map, get_huge_page_option};

const BIND: &str = "bind";
#[async_trait]
pub trait Volume: Send + Sync {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>>;
    fn get_storage(&self) -> Result<Vec<agent::Storage>>;
    async fn cleanup(&self) -> Result<()>;
}

#[derive(Default)]
pub struct VolumeResourceInner {
    volumes: Vec<Arc<dyn Volume>>,
}

#[derive(Default)]
pub struct VolumeResource {
    inner: Arc<RwLock<VolumeResourceInner>>,
}

impl VolumeResource {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn handler_volumes(
        &self,
        share_fs: &Option<Arc<dyn ShareFs>>,
        cid: &str,
        spec: &oci::Spec,
    ) -> Result<Vec<Arc<dyn Volume>>> {
        let mut volumes: Vec<Arc<dyn Volume>> = vec![];
        let oci_mounts = &spec.mounts;
        // handle mounts
        for m in oci_mounts {
            let volume: Arc<dyn Volume> = if shm_volume::is_shim_volume(m) {
                let shm_size = shm_volume::DEFAULT_SHM_SIZE;
                Arc::new(
                    shm_volume::ShmVolume::new(m, shm_size)
                        .with_context(|| format!("new shm volume {:?}", m))?,
                )
            } else if share_fs_volume::is_share_fs_volume(m) {
                Arc::new(
                    share_fs_volume::ShareFsVolume::new(share_fs, m, cid)
                        .await
                        .with_context(|| format!("new share fs volume {:?}", m))?,
                )
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
            } else if block_volume::is_block_volume(m) {
                Arc::new(
                    block_volume::BlockVolume::new(m)
                        .with_context(|| format!("new block volume {:?}", m))?,
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
