// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod block_rootfs;
mod nydus_rootfs;
mod share_fs_rootfs;

use std::{sync::Arc, vec::Vec};

use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{device::device_manager::DeviceManager, Hypervisor};
use kata_sys_util::rand::RandomBytes;
use kata_types::mount::Mount;
use tokio::sync::RwLock;

use self::{block_rootfs::is_block_rootfs, nydus_rootfs::NYDUS_ROOTFS_TYPE};
use crate::share_fs::ShareFs;

const ROOTFS: &str = "rootfs";
const HYBRID_ROOTFS_LOWER_DIR: &str = "rootfs_lower";
const TYPE_OVERLAY_FS: &str = "overlay";
#[async_trait]
pub trait Rootfs: Send + Sync {
    fn id(&self) -> String;

    async fn get_guest_rootfs_path(&self) -> Result<String>;
    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>>;
    async fn get_storage(&self) -> Option<Storage>;
    async fn get_device_id(&self) -> Result<Option<String>>;
    async fn cleanup(&self, device_manager: &RwLock<DeviceManager>) -> Result<()>;
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
        root: &oci::Root,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
    ) -> Result<Arc<dyn Rootfs>> {
        match rootfs_mounts {
            // if rootfs_mounts is empty
            mounts_vec if mounts_vec.is_empty() => {
                if let Some(share_fs) = share_fs {
                    // handle share fs rootfs
                    Ok(Arc::new(
                        share_fs_rootfs::ShareFsRootfs::new(
                            share_fs,
                            cid,
                            root.path.as_str(),
                            None,
                        )
                        .await
                        .context("new share fs rootfs")?,
                    ))
                } else {
                    Err(anyhow!("share fs is unavailable"))
                }
            }
            mounts_vec if is_single_layer_rootfs(mounts_vec) => {
                // Safe as single_layer_rootfs must have one layer
                let layer = &mounts_vec[0];
                let mut inner = self.inner.write().await;
                let rootfs = if let Some(dev_id) = is_block_rootfs(&layer.source) {
                    // handle block rootfs
                    info!(sl!(), "block device: {}", dev_id);
                    let block_rootfs: Arc<dyn Rootfs> = Arc::new(
                        block_rootfs::BlockRootfs::new(device_manager, sid, cid, dev_id, layer)
                            .await
                            .context("new block rootfs")?,
                    );
                    Ok(block_rootfs)
                } else if let Some(share_fs) = share_fs {
                    // handle nydus rootfs
                    let share_rootfs: Arc<dyn Rootfs> = if layer.fs_type == NYDUS_ROOTFS_TYPE {
                        Arc::new(
                            nydus_rootfs::NydusRootfs::new(share_fs, h, sid, cid, layer)
                                .await
                                .context("new nydus rootfs")?,
                        )
                    }
                    // handle sharefs rootfs
                    else {
                        Arc::new(
                            share_fs_rootfs::ShareFsRootfs::new(
                                share_fs,
                                cid,
                                bundle_path,
                                Some(layer),
                            )
                            .await
                            .context("new share fs rootfs")?,
                        )
                    };
                    Ok(share_rootfs)
                } else {
                    Err(anyhow!("unsupported rootfs {:?}", &layer))
                }?;
                inner.rootfs.push(rootfs.clone());
                Ok(rootfs)
            }
            _ => Err(anyhow!(
                "unsupported rootfs mounts count {}",
                rootfs_mounts.len()
            )),
        }
    }

    pub async fn remove(
        &self,
        device_manager: &RwLock<DeviceManager>,
        cid: String,
        rootfs: Vec<Arc<dyn Rootfs>>,
    ) -> Result<Vec<Arc<dyn Rootfs>>> {
        let mut handled = Vec::new();
        let mut unhandled = Vec::new();
        for rootfs in rootfs.iter() {
            if let Err(err) = rootfs.cleanup(device_manager).await {
                warn!(
                    sl!(),
                    "Failed to umount rootfs, cid = {:?}, error = {:?}", cid, err
                );
                unhandled.push(Arc::clone(rootfs));
                continue;
            }
            handled.push(Arc::clone(rootfs));
        }

        let removed_ids: Vec<String> = handled.iter().map(|x| x.id()).collect();

        // clear the cleaned up rootfs in the vector
        let mut inner = self.inner.write().await;
        inner.rootfs.retain(|x| !removed_ids.contains(&x.id()));

        Ok(unhandled)
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

pub(crate) fn generate_rootfs_id() -> String {
    let random_bytes = RandomBytes::new(8);
    format!("rootfs-{:x}", random_bytes)
}
