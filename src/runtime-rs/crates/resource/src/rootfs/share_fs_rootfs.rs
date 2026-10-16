// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use super::{Rootfs, ROOTFS};
use crate::share_fs::{ShareFs, ShareFsRootfsConfig};
use agent::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::{umount_timeout, Mounter};
use kata_types::mount::Mount;
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

pub(crate) struct ShareFsRootfs {
    guest_path: String,
    share_fs: Arc<dyn ShareFs>,
    config: ShareFsRootfsConfig,
    mounted_bundle_rootfs: bool,
}

impl ShareFsRootfs {
    pub async fn new(
        share_fs: &Arc<dyn ShareFs>,
        cid: &str,
        bundle_path: &str,
        rootfs: Option<&Mount>,
    ) -> Result<Self> {
        let bundle_rootfs = if let Some(rootfs) = rootfs {
            let bundle_rootfs = format!("{bundle_path}/{ROOTFS}");
            rootfs.mount(&bundle_rootfs).context(format!(
                "mount rootfs from {:?} to {}",
                rootfs, bundle_rootfs
            ))?;
            bundle_rootfs
        } else {
            bundle_path.to_string()
        };

        let share_fs_mount = share_fs.get_share_fs_mount();
        let config = ShareFsRootfsConfig {
            cid: cid.to_string(),
            source: bundle_rootfs.to_string(),
            target: ROOTFS.to_string(),
            readonly: false,
            is_rafs: false,
        };

        let mount_result = share_fs_mount
            .share_rootfs(&config)
            .await
            .context("share rootfs")?;

        Ok(ShareFsRootfs {
            guest_path: mount_result.guest_path,
            share_fs: Arc::clone(share_fs),
            config,
            mounted_bundle_rootfs: rootfs.is_some(),
        })
    }
}

#[async_trait]
impl Rootfs for ShareFsRootfs {
    async fn get_guest_rootfs_path(&self) -> Result<String> {
        Ok(self.guest_path.clone())
    }

    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>> {
        todo!()
    }

    async fn get_storage(&self) -> Option<Vec<Storage>> {
        None
    }

    async fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        // Umount the mount point shared to guest
        let share_fs_mount = self.share_fs.get_share_fs_mount();
        share_fs_mount
            .umount_rootfs(&self.config)
            .await
            .context("umount shared rootfs")?;

        // CRI-O supplies an already mounted rootfs with no rootfs mounts in the
        // create request. Only unmount the source when we mounted it ourselves.
        if self.mounted_bundle_rootfs {
            umount_timeout(&self.config.source, 0).context("umount bundle rootfs")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share_fs::do_get_host_path;
    use crate::test_utils::{
        device_manager, in_private_mount_namespace, inline_share_fs, is_mounted, set_immutable,
        SharedTmpfs,
    };
    use std::{fs, path::Path};

    // Fail removal after detaching the share to test cleanup of the remaining bundle rootfs.
    #[actix_rt::test]
    #[ignore = "requires root and mount namespace capabilities"]
    async fn test_cleanup_retry_after_partial_failure() {
        if !in_private_mount_namespace(
            "rootfs::share_fs_rootfs::tests::test_cleanup_retry_after_partial_failure",
        ) {
            return;
        }
        let tmpfs = SharedTmpfs::new();
        let sid = tmpfs.sandbox();
        let share_fs = inline_share_fs(&sid);
        let device_manager = device_manager().await;

        let layers = tmpfs.path().join("layers");
        for dir in ["lower", "upper", "work"] {
            fs::create_dir_all(layers.join(dir)).unwrap();
        }
        let sentinel = layers.join("lower/sentinel");
        fs::write(&sentinel, b"image data").unwrap();
        let bundle = tmpfs.path().join("bundle");
        let bundle_rootfs = bundle.join(ROOTFS);
        fs::create_dir_all(&bundle_rootfs).unwrap();
        let mount = Mount {
            source: "overlay".into(),
            fs_type: "overlay".into(),
            options: vec![
                format!("lowerdir={}", layers.join("lower").display()),
                format!("upperdir={}", layers.join("upper").display()),
                format!("workdir={}", layers.join("work").display()),
            ],
            ..Default::default()
        };
        let rootfs = ShareFsRootfs::new(&share_fs, "cid", bundle.to_str().unwrap(), Some(&mount))
            .await
            .unwrap();
        let shared_rootfs = do_get_host_path(ROOTFS, &sid, "cid", false, false);
        assert!(is_mounted(&bundle_rootfs));
        assert!(is_mounted(&shared_rootfs));

        let container_dir = Path::new(&shared_rootfs).parent().unwrap().to_owned();
        set_immutable(&container_dir, true);
        let result = rootfs.cleanup(&device_manager).await;
        set_immutable(&container_dir, false);
        assert!(result.is_err());
        assert!(!is_mounted(&shared_rootfs));
        assert!(is_mounted(&bundle_rootfs));

        rootfs.cleanup(&device_manager).await.unwrap();
        assert!(!is_mounted(&bundle_rootfs));
        assert!(!Path::new(&shared_rootfs).exists());
        assert_eq!(fs::read(&sentinel).unwrap(), b"image data");

        rootfs.cleanup(&device_manager).await.unwrap();
    }
}
