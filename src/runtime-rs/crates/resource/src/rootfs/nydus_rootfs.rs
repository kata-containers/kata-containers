// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::{fs, path::Path, sync::Arc};

use super::{Rootfs, TYPE_OVERLAY_FS};
use crate::{
    rootfs::{HYBRID_ROOTFS_LOWER_DIR, ROOTFS},
    share_fs::{
        do_get_guest_path, do_get_guest_share_path, get_host_rw_shared_path, rafs_mount, ShareFs,
        ShareFsRootfsConfig, PASSTHROUGH_FS_DIR,
    },
};
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{device::device_manager::DeviceManager, Hypervisor};
use kata_types::mount::{Mount, NydusExtraOptions};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;
// Used for nydus rootfs
pub(crate) const NYDUS_ROOTFS_TYPE: &str = "fuse.nydus-overlayfs";
// Used for Nydus v5 rootfs version
const NYDUS_ROOTFS_V5: &str = "v5";
// Used for Nydus v6 rootfs version
const NYDUS_ROOTFS_V6: &str = "v6";

const SNAPSHOT_DIR: &str = "snapshotdir";
const KATA_OVERLAY_DEV_TYPE: &str = "overlayfs";
// nydus prefetch file list name
const NYDUS_PREFETCH_FILE_LIST: &str = "prefetch_file.list";

pub(crate) struct NydusRootfs {
    guest_path: String,
    rootfs: Storage,
}

impl NydusRootfs {
    pub async fn new(
        d: &RwLock<DeviceManager>,
        share_fs: &Arc<dyn ShareFs>,
        h: &dyn Hypervisor,
        sid: &str,
        cid: &str,
        rootfs: &Mount,
    ) -> Result<Self> {
        let prefetch_list_path =
            get_nydus_prefetch_files(h.hypervisor_config().await.prefetch_list_path).await;

        let share_fs_mount = share_fs.get_share_fs_mount();
        let extra_options =
            NydusExtraOptions::new(rootfs).context("failed to parse nydus extra options")?;
        info!(sl!(), "extra_option {:?}", &extra_options);
        let rafs_meta = &extra_options.source;
        let (rootfs_storage, rootfs_guest_path) = match extra_options.fs_version.as_str() {
            // both nydus v5 and v6 can be handled by the builtin nydus in dragonball by using the rafs mode.
            // nydus v6 could also be handled by the guest kernel as well, but some kernel patch is not support in the upstream community. We will add an option to let runtime-rs handle nydus v6 in the guest kernel optionally once the patch is ready
            // see this issue (https://github.com/kata-containers/kata-containers/issues/5143)
            NYDUS_ROOTFS_V5 | NYDUS_ROOTFS_V6 => {
                // rafs mount the metadata of nydus rootfs
                let rafs_mnt = do_get_guest_share_path(HYBRID_ROOTFS_LOWER_DIR, cid, true);
                rafs_mount(
                    d,
                    sid,
                    rafs_meta.to_string(),
                    rafs_mnt,
                    extra_options.config.clone(),
                    prefetch_list_path,
                )
                .await
                .context("failed to do rafs mount")?;
                // create rootfs under the share directory
                let container_share_dir = get_host_rw_shared_path(sid)
                    .join(PASSTHROUGH_FS_DIR)
                    .join(cid);
                let rootfs_dir = container_share_dir.join(ROOTFS);
                fs::create_dir_all(rootfs_dir).context("failed to create directory")?;
                // mount point inside the guest
                let rootfs_guest_path = do_get_guest_path(ROOTFS, cid, false, false);
                // bind mount the snapshot dir under the share directory
                share_fs_mount
                    .share_rootfs(&ShareFsRootfsConfig {
                        cid: cid.to_string(),
                        source: extra_options.snapshot_dir.clone(),
                        target: SNAPSHOT_DIR.to_string(),
                        readonly: false,
                        is_rafs: false,
                    })
                    .await
                    .context("share nydus rootfs")?;
                let mut options: Vec<String> = Vec::new();
                options.push(
                    "lowerdir=".to_string()
                        + &do_get_guest_path(HYBRID_ROOTFS_LOWER_DIR, cid, false, true),
                );
                options.push(
                    "workdir=".to_string()
                        + &do_get_guest_path(
                            format!("{}/{}", SNAPSHOT_DIR, "work").as_str(),
                            cid,
                            false,
                            false,
                        ),
                );
                options.push(
                    "upperdir=".to_string()
                        + &do_get_guest_path(
                            format!("{}/{}", SNAPSHOT_DIR, "fs").as_str(),
                            cid,
                            false,
                            false,
                        ),
                );
                options.push("index=off".to_string());
                Ok((
                    Storage {
                        driver: KATA_OVERLAY_DEV_TYPE.to_string(),
                        source: TYPE_OVERLAY_FS.to_string(),
                        fs_type: TYPE_OVERLAY_FS.to_string(),
                        options,
                        mount_point: rootfs_guest_path.clone(),
                        ..Default::default()
                    },
                    rootfs_guest_path,
                ))
            }
            _ => {
                let errstr: &str = "new_nydus_rootfs: invalid nydus rootfs type";
                error!(sl!(), "{}", errstr);
                Err(anyhow!(errstr))
            }
        }?;
        Ok(NydusRootfs {
            guest_path: rootfs_guest_path,
            rootfs: rootfs_storage,
        })
    }
}

#[async_trait]
impl Rootfs for NydusRootfs {
    async fn get_guest_rootfs_path(&self) -> Result<String> {
        Ok(self.guest_path.clone())
    }

    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![])
    }

    async fn get_storage(&self) -> Option<Storage> {
        Some(self.rootfs.clone())
    }

    async fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        // TODO: Clean up NydusRootfs after the container is killed
        warn!(sl!(), "Cleaning up NydusRootfs is still unimplemented.");
        Ok(())
    }
}

// Check prefetch files list path, and if invalid, discard it directly.
// As the result of caller `rafs_mount`, it returns `Option<String>`.
async fn get_nydus_prefetch_files(nydus_prefetch_path: String) -> Option<String> {
    // nydus_prefetch_path is an annotation and pod with it will indicate
    // that prefetch_files will be included.
    if nydus_prefetch_path.is_empty() {
        info!(sl!(), "nydus prefetch files path not set, just skip it.");

        return None;
    }

    // Ensure the string ends with "/prefetch_files.list"
    if !nydus_prefetch_path.ends_with(format!("/{}", NYDUS_PREFETCH_FILE_LIST).as_str()) {
        info!(
            sl!(),
            "nydus prefetch file path no {:?} file exist.", NYDUS_PREFETCH_FILE_LIST
        );

        return None;
    }

    // ensure the prefetch_list_path is a regular file.
    let prefetch_list_path = Path::new(nydus_prefetch_path.as_str());
    if !prefetch_list_path.is_file() {
        info!(
            sl!(),
            "nydus prefetch list file {:?} not a regular file", &prefetch_list_path
        );

        return None;
    }

    return Some(prefetch_list_path.display().to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, path::PathBuf};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_get_nydus_prefetch_files() {
        let temp_dir = tempdir().unwrap();
        let prefetch_list_path01 = temp_dir.path().join("nydus_prefetch_files");
        // /tmp_dir/nydus_prefetch_files/
        std::fs::create_dir_all(prefetch_list_path01.clone()).unwrap();
        // /tmp_dir/nydus_prefetch_files/prefetch_file.list
        let prefetch_list_path02 = prefetch_list_path01
            .as_path()
            .join(NYDUS_PREFETCH_FILE_LIST);
        let file = File::create(prefetch_list_path02.clone());
        assert!(file.is_ok());

        let prefetch_file =
            get_nydus_prefetch_files(prefetch_list_path02.as_path().display().to_string()).await;
        assert!(prefetch_file.is_some());
        assert_eq!(PathBuf::from(prefetch_file.unwrap()), prefetch_list_path02);

        drop(file);
        temp_dir.close().unwrap_or_default();
    }
}
