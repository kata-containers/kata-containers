// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod share_virtio_fs;
pub use share_virtio_fs::rafs_mount;
mod share_virtio_fs_inline;
use share_virtio_fs_inline::ShareVirtioFsInline;
mod share_virtio_fs_standalone;
use share_virtio_fs_standalone::ShareVirtioFsStandalone;
mod utils;
use tokio::sync::Mutex;
pub use utils::{
    do_get_guest_path, do_get_guest_share_path, do_get_host_path, get_host_rw_shared_path,
};
mod virtio_fs_share_mount;
use virtio_fs_share_mount::VirtiofsShareMount;
pub use virtio_fs_share_mount::EPHEMERAL_PATH;
pub mod sandbox_bind_mounts;

use std::{collections::HashMap, fmt::Debug, path::PathBuf, sync::Arc};

use agent::Storage;
use anyhow::{anyhow, Context, Ok, Result};
use async_trait::async_trait;
use kata_types::config::hypervisor::SharedFsInfo;
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

use hypervisor::{device::device_manager::DeviceManager, Hypervisor};

const VIRTIO_FS: &str = "virtio-fs";
const _VIRTIO_FS_NYDUS: &str = "virtio-fs-nydus";
const INLINE_VIRTIO_FS: &str = "inline-virtio-fs";

const KATA_HOST_SHARED_DIR: &str = "/run/kata-containers/shared/sandboxes/";

/// share fs (for example virtio-fs) mount path in the guest
const KATA_GUEST_SHARE_DIR: &str = "/run/kata-containers/shared/containers/";

pub(crate) const DEFAULT_KATA_GUEST_SANDBOX_DIR: &str = "/run/kata-containers/sandbox/";

pub const PASSTHROUGH_FS_DIR: &str = "passthrough";
const RAFS_DIR: &str = "rafs";

#[async_trait]
pub trait ShareFs: Send + Sync {
    fn get_share_fs_mount(&self) -> Arc<dyn ShareFsMount>;
    async fn setup_device_before_start_vm(
        &self,
        h: &dyn Hypervisor,
        d: &RwLock<DeviceManager>,
    ) -> Result<()>;
    async fn setup_device_after_start_vm(
        &self,
        h: &dyn Hypervisor,
        d: &RwLock<DeviceManager>,
    ) -> Result<()>;
    async fn get_storages(&self) -> Result<Vec<Storage>>;
    fn mounted_info_set(&self) -> Arc<Mutex<HashMap<String, MountedInfo>>>;
}

#[derive(Debug, Clone)]
pub struct ShareFsRootfsConfig {
    // TODO: for nydus v5/v6 need to update ShareFsMount
    pub cid: String,
    pub source: String,
    pub target: String,
    pub readonly: bool,
    pub is_rafs: bool,
}

#[derive(Debug)]
pub struct ShareFsVolumeConfig {
    pub cid: String,
    pub source: String,
    pub target: String,
    pub readonly: bool,
    pub mount_options: Vec<String>,
    pub mount: oci::Mount,
    pub is_rafs: bool,
}

pub struct ShareFsMountResult {
    pub guest_path: String,
    pub storages: Vec<agent::Storage>,
}

/// Save mounted info for sandbox-level shared files.
#[derive(Clone, Debug)]
pub struct MountedInfo {
    // Guest path
    pub guest_path: PathBuf,
    // Ref count of containers that uses this volume with read only permission
    pub ro_ref_count: usize,
    // Ref count of containers that uses this volume with read write permission
    pub rw_ref_count: usize,
}

impl MountedInfo {
    pub fn new(guest_path: PathBuf, readonly: bool) -> Self {
        Self {
            guest_path,
            ro_ref_count: readonly.into(),
            rw_ref_count: (!readonly).into(),
        }
    }

    /// Check if the mount has read only permission
    pub fn readonly(&self) -> bool {
        self.rw_ref_count == 0
    }

    /// Ref count for all permissions
    pub fn ref_count(&self) -> usize {
        self.ro_ref_count + self.rw_ref_count
    }

    // File/dir name in the form of "sandbox-<uuid>-<file/dir name>"
    pub fn file_name(&self) -> Result<String> {
        match self.guest_path.file_name() {
            Some(file_name) => match file_name.to_str() {
                Some(file_name) => Ok(file_name.to_owned()),
                None => Err(anyhow!("failed to get string from {:?}", file_name)),
            },
            None => Err(anyhow!(
                "failed to get file name from the guest_path {:?}",
                self.guest_path
            )),
        }
    }
}

#[async_trait]
pub trait ShareFsMount: Send + Sync {
    async fn share_rootfs(&self, config: &ShareFsRootfsConfig) -> Result<ShareFsMountResult>;
    async fn share_volume(&self, config: &ShareFsVolumeConfig) -> Result<ShareFsMountResult>;
    /// Upgrade to readwrite permission
    async fn upgrade_to_rw(&self, file_name: &str) -> Result<()>;
    /// Downgrade to readonly permission
    async fn downgrade_to_ro(&self, file_name: &str) -> Result<()>;
    /// Umount the volume
    async fn umount_volume(&self, file_name: &str) -> Result<()>;
    /// Umount the rootfs
    async fn umount_rootfs(&self, config: &ShareFsRootfsConfig) -> Result<()>;
    /// Clean up share fs mount
    async fn cleanup(&self, sid: &str) -> Result<()>;
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
