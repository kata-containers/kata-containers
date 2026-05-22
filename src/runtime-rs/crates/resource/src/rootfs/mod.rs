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
mod erofs_rootfs;
pub mod virtual_volume;

use hypervisor::{device::device_manager::DeviceManager, Hypervisor};
use virtual_volume::{is_kata_virtual_volume, VirtualVolume};

use std::{collections::HashMap, sync::Arc, vec::Vec};
use tokio::sync::RwLock;

use self::{
    block_rootfs::is_block_rootfs, erofs_rootfs::ErofsMultiLayerRootfs,
    nydus_rootfs::NYDUS_ROOTFS_TYPE,
};
use crate::{rootfs::erofs_rootfs::is_erofs_multi_layer, share_fs::ShareFs};
use oci_spec::runtime as oci;

const ROOTFS: &str = "rootfs";
const HYBRID_ROOTFS_LOWER_DIR: &str = "rootfs_lower";
const TYPE_OVERLAY_FS: &str = "overlay";

#[async_trait]
pub trait Rootfs: Send + Sync {
    async fn get_guest_rootfs_path(&self) -> Result<String>;
    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>>;
    async fn get_storage(&self) -> Option<Vec<Storage>>;
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
        root: &oci::Root,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
        annotations: &HashMap<String, String>,
    ) -> Result<Arc<dyn Rootfs>> {
        match rootfs_mounts {
            // if rootfs_mounts is empty
            [] => {
                if let Some(share_fs) = share_fs {
                    // handle share fs rootfs
                    Ok(Arc::new(
                        share_fs_rootfs::ShareFsRootfs::new(
                            share_fs,
                            cid,
                            root.path().display().to_string().as_str(),
                            None,
                        )
                        .await
                        .context("new share fs rootfs")?,
                    ))
                } else {
                    Err(anyhow!("share fs is unavailable"))
                }
            }
            _ if is_erofs_multi_layer(rootfs_mounts) => {
                info!(
                    sl!(),
                    "handling multi-layer erofs rootfs with {} mounts",
                    rootfs_mounts.len()
                );

                let multi_layer =
                    ErofsMultiLayerRootfs::new(device_manager, sid, cid, rootfs_mounts, share_fs)
                        .await
                        .context("new multi-layer erofs rootfs")?;

                let ret = Arc::new(multi_layer);
                let mut inner = self.inner.write().await;
                inner.rootfs.push(ret.clone());
                Ok(ret)
            }
            _ if is_single_layer_rootfs(rootfs_mounts) => {
                // Safe as single_layer_rootfs must have one layer
                let layer = &rootfs_mounts[0];
                let mut inner = self.inner.write().await;

                if is_guest_pull_volume(share_fs, layer) {
                    let mount_options = layer.options.clone();
                    let virtual_volume: Arc<dyn Rootfs> = Arc::new(
                        VirtualVolume::new(cid, annotations, mount_options.to_vec())
                            .await
                            .context("kata virtual volume failed.")?,
                    );
                    return Ok(virtual_volume);
                }

                let rootfs = if let Some((dev_id, layer)) = is_block_rootfs(layer) {
                    // handle block rootfs
                    info!(sl!(), "block device: {}", dev_id);
                    let block_rootfs: Arc<dyn Rootfs> = Arc::new(
                        block_rootfs::BlockRootfs::new(device_manager, sid, cid, dev_id, &layer)
                            .await
                            .context("new block rootfs")?,
                    );
                    Ok(block_rootfs)
                } else if let Some(share_fs) = share_fs {
                    // handle nydus rootfs
                    let share_rootfs: Arc<dyn Rootfs> = if layer.fs_type == NYDUS_ROOTFS_TYPE {
                        Arc::new(
                            nydus_rootfs::NydusRootfs::new(
                                device_manager,
                                share_fs,
                                h,
                                sid,
                                cid,
                                layer,
                            )
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

pub fn is_guest_pull_volume(
    share_fs: &Option<Arc<dyn ShareFs>>,
    m: &kata_types::mount::Mount,
) -> bool {
    share_fs.is_none() && is_kata_virtual_volume(m)
}
