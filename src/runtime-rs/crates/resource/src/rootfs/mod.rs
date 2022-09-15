// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod share_fs_rootfs;

use std::{sync::Arc, vec::Vec};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use kata_types::mount::Mount;
use tokio::sync::RwLock;

use crate::share_fs::ShareFs;

const ROOTFS: &str = "rootfs";

#[async_trait]
pub trait Rootfs: Send + Sync {
    async fn get_guest_rootfs_path(&self) -> Result<String>;
    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>>;
}

#[derive(Default)]
struct RootFsResourceInner {
    rootfs: Vec<Arc<dyn Rootfs>>,
}

pub struct RootFsResource {
    inner: Arc<RwLock<RootFsResourceInner>>,
}

impl Default for RootFsResource {
    fn default() -> Self {
        Self::new()
    }
}

impl RootFsResource {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RootFsResourceInner::default())),
        }
    }

    pub async fn handler_rootfs(
        &self,
        share_fs: &Option<Arc<dyn ShareFs>>,
        cid: &str,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
    ) -> Result<Arc<dyn Rootfs>> {
        match rootfs_mounts {
            mounts_vec if is_single_layer_rootfs(mounts_vec) => {
                // Safe as single_layer_rootfs must have one layer
                let layer = &mounts_vec[0];

                let rootfs = if let Some(share_fs) = share_fs {
                    // share fs rootfs
                    let share_fs_mount = share_fs.get_share_fs_mount();
                    share_fs_rootfs::ShareFsRootfs::new(&share_fs_mount, cid, bundle_path, layer)
                        .await
                        .context("new share fs rootfs")?
                } else {
                    return Err(anyhow!("unsupported rootfs {:?}", &layer));
                };

                let mut inner = self.inner.write().await;
                let r = Arc::new(rootfs);
                inner.rootfs.push(r.clone());
                Ok(r)
            }
            _ => {
                return Err(anyhow!(
                    "unsupported rootfs mounts count {}",
                    rootfs_mounts.len()
                ))
            }
        }
    }

    pub async fn dump(&self) {
        let inner = self.inner.read().await;
        for r in &inner.rootfs {
            info!(
                sl!(),
                "rootfs {:?}: count {}",
                r.get_guest_rootfs_path().await,
                Arc::strong_count(r)
            );
        }
    }
}

fn is_single_layer_rootfs(rootfs_mounts: &[Mount]) -> bool {
    rootfs_mounts.len() == 1
}
