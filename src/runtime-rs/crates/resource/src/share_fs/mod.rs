// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod share_virtio_fs;
mod share_virtio_fs_inline;
use share_virtio_fs_inline::ShareVirtioFsInline;
mod share_virtio_fs_standalone;
use share_virtio_fs_standalone::ShareVirtioFsStandalone;
mod utils;
mod virtio_fs_share_mount;
use virtio_fs_share_mount::VirtiofsShareMount;

use std::sync::Arc;

use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::Hypervisor;
use kata_types::config::hypervisor::SharedFsInfo;

const VIRTIO_FS: &str = "virtio-fs";
const INLINE_VIRTIO_FS: &str = "inline-virtio-fs";

const KATA_HOST_SHARED_DIR: &str = "/run/kata-containers/shared/sandboxes/";
const KATA_GUEST_SHARE_DIR: &str = "/run/kata-containers/shared/containers/";
pub(crate) const DEFAULT_KATA_GUEST_SANDBOX_DIR: &str = "/run/kata-containers/sandbox/";

const PASSTHROUGH_FS_DIR: &str = "passthrough";

#[async_trait]
pub trait ShareFs: Send + Sync {
    fn get_share_fs_mount(&self) -> Arc<dyn ShareFsMount>;
    async fn setup_device_before_start_vm(&self, h: &dyn Hypervisor) -> Result<()>;
    async fn setup_device_after_start_vm(&self, h: &dyn Hypervisor) -> Result<()>;
    async fn get_storages(&self) -> Result<Vec<Storage>>;
}

pub struct ShareFsRootfsConfig {
    // TODO: for nydus v5/v6 need to update ShareFsMount
    pub cid: String,
    pub source: String,
    pub target: String,
    pub readonly: bool,
}

pub struct ShareFsVolumeConfig {
    pub cid: String,
    pub source: String,
    pub target: String,
    pub readonly: bool,
}

pub struct ShareFsMountResult {
    pub guest_path: String,
}

#[async_trait]
pub trait ShareFsMount: Send + Sync {
    async fn share_rootfs(&self, config: ShareFsRootfsConfig) -> Result<ShareFsMountResult>;
    async fn share_volume(&self, config: ShareFsVolumeConfig) -> Result<ShareFsMountResult>;
}

pub fn new(id: &str, config: &SharedFsInfo) -> Result<Arc<dyn ShareFs>> {
    let shared_fs = config.shared_fs.clone();
    let shared_fs = shared_fs.unwrap_or_default();
    match shared_fs.as_str() {
        INLINE_VIRTIO_FS => Ok(Arc::new(
            ShareVirtioFsInline::new(id, config).context("new inline virtio fs")?,
        )),
        VIRTIO_FS => Ok(Arc::new(
            ShareVirtioFsStandalone::new(id, config).context("new standalone virtio fs")?,
        )),
        _ => Err(anyhow!("unsupported shred fs {:?}", &shared_fs)),
    }
}
