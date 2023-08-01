// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod nydus_rootfs;
mod share_fs_rootfs;
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use kata_types::mount::Mount;
mod block_rootfs;
use hypervisor::{device::device_manager::DeviceManager, Hypervisor};
use std::{sync::Arc, vec::Vec};
use tokio::sync::RwLock;

use crate::share_fs::ShareFs;

use self::{block_rootfs::is_block_rootfs, nydus_rootfs::NYDUS_ROOTFS_TYPE};

const ROOTFS: &str = "rootfs";
const HYBRID_ROOTFS_LOWER_DIR: &str = "rootfs_lower";
const TYPE_OVERLAY_FS: &str = "overlay";
#[async_trait]
pub trait Rootfs: Send + Sync {
    async fn get_guest_rootfs_path(&self) -> Result<String>;
    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>>;
    async fn get_storage(&self) -> Option<Storage>;
    async fn cleanup(&self, device_manager: &RwLock<DeviceManager>) -> Result<()>;
    async fn get_device_id(&self) -> Result<Option<String>>;
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

    #[allow(clippy::too_many_arguments)]
    pub async fn handler_rootfs(
        &self,
        share_fs: &Option<Arc<dyn ShareFs>>,
        device_manager: &RwLock<DeviceManager>,
        h: &dyn Hypervisor,
        sid: &str,
        cid: &str,
        root_path: &str,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
        is_image_offload: bool,
    ) -> Result<Option<Arc<dyn Rootfs>>> {
        match rootfs_mounts {
            mounts_vec if mounts_vec.is_empty() => {
                // No rootfs_mounts when creating container from bundle or creating confidential containers.
                self.handle_empty_rootfs(share_fs, cid, root_path, is_image_offload)
                    .await
            }
            mounts_vec if is_single_layer_rootfs(mounts_vec) => {
                // Safe as single_layer_rootfs must have one layer
                let layer = &mounts_vec[0];
                self.handle_single_layer_rootfs(
                    share_fs,
                    device_manager,
                    h,
                    sid,
                    cid,
                    bundle_path,
                    layer,
                )
                .await
            }
            _ => Err(anyhow!(
                "unsupported rootfs mounts count {}",
                rootfs_mounts.len()
            )),
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

    async fn handle_empty_rootfs(
        &self,
        share_fs: &Option<Arc<dyn ShareFs>>,
        cid: &str,
        root_path: &str,
        is_image_offload: bool,
    ) -> Result<Option<Arc<dyn Rootfs>>> {
        // In condfidential computing, there is no image information on the host,
        // so there is no Rootfs.
        if is_image_offload {
            return Ok(None);
        }

        if let Some(share_fs) = share_fs {
            // sharefs rootfs
            let rootfs = Arc::new(
                share_fs_rootfs::ShareFsRootfs::new(share_fs, cid, root_path, None)
                    .await
                    .context("new share fs rootfs")?,
            );
            Ok(Some(rootfs))
        } else {
            Err(anyhow!("share fs is unavailable"))
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_single_layer_rootfs(
        &self,
        share_fs: &Option<Arc<dyn ShareFs>>,
        device_manager: &RwLock<DeviceManager>,
        hypervisor: &dyn Hypervisor,
        sid: &str,
        cid: &str,
        bundle_path: &str,
        layer: &Mount,
    ) -> Result<Option<Arc<dyn Rootfs>>> {
        let rootfs: Arc<dyn Rootfs> = if let Some(dev_id) = is_block_rootfs(&layer.source) {
            // handle block rootfs
            info!(sl!(), "block device: {}", dev_id);
            Arc::new(
                block_rootfs::BlockRootfs::new(device_manager, sid, cid, dev_id, layer)
                    .await
                    .context("new block rootfs")?,
            )
        } else if let Some(share_fs) = share_fs {
            if layer.fs_type == NYDUS_ROOTFS_TYPE {
                // nydus rootfs
                Arc::new(
                    nydus_rootfs::NydusRootfs::new(share_fs, hypervisor, sid, cid, layer)
                        .await
                        .context("new nydus rootfs")?,
                )
            } else {
                // share fs rootfs
                Arc::new(
                    share_fs_rootfs::ShareFsRootfs::new(share_fs, cid, bundle_path, Some(layer))
                        .await
                        .context("new share fs rootfs")?,
                )
            }
        } else {
            return Err(anyhow!("unsupported rootfs {:?}", &layer));
        };

        let mut inner = self.inner.write().await;
        inner.rootfs.push(Arc::clone(&rootfs));
        Ok(Some(rootfs))
    }
}

fn is_single_layer_rootfs(rootfs_mounts: &[Mount]) -> bool {
    rootfs_mounts.len() == 1
}
