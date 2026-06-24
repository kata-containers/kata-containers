// Copyright (c) 2019-2026 Alibaba Cloud
// Copyright (c) 2019-2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! Nydus Rootfs Implementation
//!
//! This module provides a unified implementation for nydus rootfs that supports two modes:
//! - **Inline mode**: Used with Dragonball VMM where nydus is built-in
//! - **Standalone mode**: Used with QEMU/Cloud-Hypervisor where nydusd runs as a separate process
//!
//! The mode is determined by whether a `NydusShareFs` instance is provided:
//! - `Some(nydus_fs)`: Standalone mode (external nydusd with guest kernel overlay)
//! - `None`: Inline mode (built-in nydusd with guest kernel overlay)

use std::path::PathBuf;
use std::{fs, path::Path, sync::Arc};

use super::{Rootfs, TYPE_OVERLAY_FS};
use crate::rootfs::HYBRID_ROOTFS_LOWER_DIR;
use crate::{
    rootfs::ROOTFS,
    share_fs::{
        do_get_guest_path, get_host_rw_shared_path, kata_guest_nydus_root_dir,
        kata_guest_share_dir, NydusShareFs, ShareFs, ShareFsRootfsConfig, PASSTHROUGH_FS_DIR,
    },
};
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{device::device_manager::DeviceManager, Hypervisor};
use kata_types::mount::{Mount, NydusExtraOptions};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

/// Used for nydus rootfs type detection
pub(crate) const NYDUS_ROOTFS_TYPE: &str = "fuse.nydus-overlayfs";

/// Nydus v5 rootfs version
const NYDUS_ROOTFS_V5: &str = "v5";
/// Nydus v6 rootfs version
const NYDUS_ROOTFS_V6: &str = "v6";

/// Snapshot directory name
const SNAPSHOT_DIR: &str = "snapshotdir";
/// Overlay device type for kata-agent
const KATA_OVERLAY_DEV_TYPE: &str = "overlayfs";
/// Nydus prefetch file list name
const NYDUS_PREFETCH_FILE_LIST: &str = "prefetch_file.list";
/// The lower directory name used in the rafs mountpoint path within the nydusd namespace.
const LOWER_DIR: &str = "lowerdir";
/// The nydus image directory under the guest share root: /run/kata-containers/shared/rafs/.
const NYDUS_RAFS_DIR: &str = "rafs";

/// Unified Nydus Rootfs implementation supporting both inline and standalone modes.
pub(crate) struct NydusRootfs {
    guest_path: String,
    rootfs: Option<Storage>,
    sid: String,
    cid: String,
    /// Nydus-specific share fs reference for standalone mode cleanup.
    /// None in inline mode.
    nydus_share_fs: Option<Arc<dyn NydusShareFs>>,
    /// It's used to track the rafs mount point for cleanup.
    /// None in inline mode.
    rafs_mountpoint: Option<String>,
}

impl NydusRootfs {
    pub async fn new(
        device_manager: &RwLock<DeviceManager>,
        share_fs: &Arc<dyn ShareFs>,
        nydus_share_fs: &Option<Arc<dyn NydusShareFs>>,
        h: &dyn Hypervisor,
        sid: &str,
        cid: &str,
        rootfs: &Mount,
    ) -> Result<Self> {
        let prefetch_list_path =
            get_nydus_prefetch_files(h.hypervisor_config().await.prefetch_list_path).await;

        let extra_options =
            NydusExtraOptions::new(rootfs).context("failed to parse nydus extra options")?;
        info!(
            sl!(),
            "nydus rootfs extra_options: {:?}, is_standalone_nydus: {}",
            &extra_options,
            nydus_share_fs.is_some()
        );

        let rafs_meta = &extra_options.source;
        let (rootfs_storage, rootfs_guest_path, rafs_mountpoint) =
            match extra_options.fs_version.as_str() {
                // both nydus v5 and v6 can be handled by the builtin nydus in dragonball by using the rafs mode.
                // nydus v6 could also be handled by the guest kernel as well, but some kernel patch is not support in the upstream community. We will add an option to let runtime-rs handle nydus v6 in the guest kernel optionally once the patch is ready
                // see this issue (https://github.com/kata-containers/kata-containers/issues/5143)
                NYDUS_ROOTFS_V5 | NYDUS_ROOTFS_V6 => {
                    // Determine the mode based on whether NydusShareFs is available
                    if let Some(nydus_fs) = nydus_share_fs {
                        // Standalone mode: external nydusd with guest kernel overlay
                        Self::create_standalone_rootfs(
                            nydus_fs.as_ref(),
                            sid,
                            cid,
                            rafs_meta,
                            &extra_options,
                        )
                        .await?
                    } else {
                        // Inline mode: built-in nydusd with guest kernel overlay
                        Self::create_inline_rootfs(
                            device_manager,
                            share_fs,
                            sid,
                            cid,
                            rafs_meta,
                            &extra_options,
                            prefetch_list_path,
                        )
                        .await?
                    }
                }
                _ => {
                    let errstr = "invalid nydus rootfs version, expected v5 or v6";
                    error!(sl!(), "{}", errstr);
                    return Err(anyhow!(errstr));
                }
            };

        info!(
            sl!(),
            "nydus rootfs created: guest_path={}, storage={:?}", rootfs_guest_path, rootfs_storage
        );

        Ok(NydusRootfs {
            guest_path: rootfs_guest_path,
            rootfs: rootfs_storage,
            sid: sid.to_string(),
            cid: cid.to_string(),
            nydus_share_fs: nydus_share_fs.clone(),
            rafs_mountpoint,
        })
    }

    /// Create rootfs in standalone mode (external nydusd).
    ///
    /// In this mode:
    /// - nydusd runs as a separate process
    /// - nydusd exposes the RAFS lowerdir
    /// - the writable overlay is assembled by kata-agent in the guest kernel
    /// - virtiofs is mounted at `/run/kata-containers/shared/`
    /// - passthrough_fs is mounted at `/containers` within nydusd namespace
    async fn create_standalone_rootfs(
        nydus_fs: &dyn crate::share_fs::NydusShareFs,
        sid: &str,
        cid: &str,
        rafs_meta: &str,
        extra_options: &NydusExtraOptions,
    ) -> Result<(Option<Storage>, String, Option<String>)> {
        // Create rootfs directory on the host under the share directory.
        // Host/Guest Mapping in Standalone Mode:
        // - Host: get_host_rw_shared_path(sid)/<cid>/rootfs = .../rw/<cid>/rootfs
        // - Guest: /run/kata-containers/shared/containers/<cid>/rootfs
        let container_share_dir = get_host_rw_shared_path(sid).join(cid);
        let rootfs_dir = container_share_dir.join(ROOTFS);
        fs::create_dir_all(&rootfs_dir).context("failed to create rootfs directory")?;

        // The guest mount point for the overlay rootfs: /run/kata-containers/shared/containers/<cid>/rootfs
        let rootfs_guest_path = Self::guest_shared_path(cid, ROOTFS);

        // Bind mount the snapshot dir (allocated by the snapshotter on the host) to the shared directory
        // so it becomes visible in the guest.
        let snapshot_share_dir = container_share_dir.join(SNAPSHOT_DIR);
        kata_sys_util::mount::bind_mount_unchecked(
            &extra_options.snapshot_dir,
            &snapshot_share_dir,
            false,
            nix::mount::MsFlags::MS_SLAVE,
        )
        .context("failed to bind mount snapshot dir")?;

        // Guest paths for overlay upper and work directories.
        let upper_dir_guest = Self::guest_shared_path(cid, &format!("{}/{}", SNAPSHOT_DIR, "fs"));
        let work_dir_guest = Self::guest_shared_path(cid, &format!("{}/{}", SNAPSHOT_DIR, "work"));

        info!(
            sl!(),
            "mounting rafs (standalone mode): source={}, cid={}", rafs_meta, cid,
        );

        // Go-runtime parity: mount plain RAFS via nydusd. The writable overlay
        // is assembled by kata-agent in the guest kernel using the Storage below.
        // If the rafs mount fails, undo the snapshot bind mount above so we don't
        // leak a mount that would block later cleanup/retries.
        let rafs_mnt = match nydus_fs
            .mount_rafs(cid, rafs_meta, &extra_options.config)
            .await
        {
            Ok(mnt) => mnt,
            Err(e) => {
                if let Err(umount_err) = nix::mount::umount(&snapshot_share_dir) {
                    warn!(
                        sl!(),
                        "failed to umount snapshot dir {:?} after rafs mount failure: {}",
                        snapshot_share_dir,
                        umount_err
                    );
                }
                return Err(e).context("failed to mount rafs in standalone mode");
            }
        };

        let lowerdir_guest = Self::guest_nydus_image_path(cid);
        let options = vec![
            format!("upperdir={}", upper_dir_guest),
            format!("workdir={}", work_dir_guest),
            format!("lowerdir={}", lowerdir_guest),
            "index=off".to_string(),
        ];

        info!(
            sl!(),
            "nydus standalone overlay storage: mount_point={}, lowerdir={}, upperdir={}, workdir={}",
            rootfs_guest_path,
            lowerdir_guest,
            upper_dir_guest,
            work_dir_guest
        );

        Ok((
            Storage {
                driver: KATA_OVERLAY_DEV_TYPE.to_string(),
                source: TYPE_OVERLAY_FS.to_string(),
                fs_type: TYPE_OVERLAY_FS.to_string(),
                options,
                mount_point: rootfs_guest_path.clone(),
                ..Default::default()
            }
            .into(),
            rootfs_guest_path,
            Some(rafs_mnt),
        ))
    }

    /// Create rootfs in inline mode (built-in nydusd).
    ///
    /// In this mode:
    /// - nydus is built into Dragonball VMM
    /// - overlay is assembled by guest kernel
    /// - virtiofs is mounted at `/run/kata-containers/shared/containers/`
    /// - passthrough_fs uses PASSTHROUGH_FS_DIR subdirectory
    async fn create_inline_rootfs(
        device_manager: &RwLock<DeviceManager>,
        share_fs: &Arc<dyn ShareFs>,
        sid: &str,
        cid: &str,
        rafs_meta: &str,
        extra_options: &NydusExtraOptions,
        prefetch_list_path: Option<String>,
    ) -> Result<(Option<Storage>, String, Option<String>)> {
        let share_fs_mount = share_fs.get_share_fs_mount();

        // Mount rafs via DeviceManager (inline mode uses built-in nydusd).
        // This is different from standalone mode which uses nydusd API.
        let rafs_mnt = crate::share_fs::do_get_guest_share_path(HYBRID_ROOTFS_LOWER_DIR, cid, true);
        crate::share_fs::rafs_mount(
            device_manager,
            sid,
            rafs_meta.to_string(),
            rafs_mnt.clone(),
            extra_options.config.clone(),
            prefetch_list_path,
        )
        .await
        .context("failed to do rafs mount")?;

        // Create rootfs directory on the host side.
        // In inline mode, we use PASSTHROUGH_FS_DIR subdirectory.
        let container_share_dir = get_host_rw_shared_path(sid)
            .join(PASSTHROUGH_FS_DIR)
            .join(cid);
        let rootfs_dir = container_share_dir.join(ROOTFS);
        fs::create_dir_all(rootfs_dir).context("failed to create directory")?;

        // Guest mount point
        let rootfs_guest_path = do_get_guest_path(ROOTFS, cid, false, false);

        // Bind mount the snapshot dir under the share directory
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

        // Build overlay options for guest kernel overlay
        let options = vec![
            format!(
                "lowerdir={}",
                do_get_guest_path(HYBRID_ROOTFS_LOWER_DIR, cid, false, true)
            ),
            format!(
                "workdir={}",
                do_get_guest_path(
                    format!("{}/{}", SNAPSHOT_DIR, "work").as_str(),
                    cid,
                    false,
                    false
                )
            ),
            format!(
                "upperdir={}",
                do_get_guest_path(
                    format!("{}/{}", SNAPSHOT_DIR, "fs").as_str(),
                    cid,
                    false,
                    false
                )
            ),
            "index=off".to_string(),
        ];

        info!(
            sl!(),
            "nydus inline overlay storage: mount_point={}, rafs_mnt={}",
            rootfs_guest_path,
            rafs_mnt
        );

        Ok((
            Storage {
                driver: KATA_OVERLAY_DEV_TYPE.to_string(),
                source: TYPE_OVERLAY_FS.to_string(),
                fs_type: TYPE_OVERLAY_FS.to_string(),
                options,
                mount_point: rootfs_guest_path.clone(),
                ..Default::default()
            }
            .into(),
            rootfs_guest_path,
            None,
        ))
    }

    /// Generate the nydus image guest path for lowerdir:
    /// `/run/kata-containers/shared/rafs/<cid>/lowerdir`
    fn guest_nydus_image_path(cid: &str) -> String {
        PathBuf::from(kata_guest_nydus_root_dir())
            .join(NYDUS_RAFS_DIR)
            .join(cid)
            .join(LOWER_DIR)
            .to_str()
            .unwrap()
            .to_string()
    }

    /// Generate the guest shared dir path for containers: `/run/kata-containers/shared/containers/<cid>/<suffix>`
    fn guest_shared_path(cid: &str, suffix: &str) -> String {
        let guest_shared_dir = kata_guest_share_dir();
        PathBuf::from(&guest_shared_dir)
            .join(cid)
            .join(suffix)
            .to_str()
            .unwrap()
            .to_string()
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

    async fn get_storage(&self) -> Option<Vec<Storage>> {
        self.rootfs.clone().map(|rootfs| vec![rootfs])
    }

    async fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        if let (Some(nydus_fs), Some(rafs_mnt)) = (&self.nydus_share_fs, &self.rafs_mountpoint) {
            if let Err(e) = nydus_fs.umount_rafs(rafs_mnt).await {
                warn!(
                    sl!(),
                    "failed to umount rafs at {} with err {}", rafs_mnt, e
                );
            }

            let sn_shared_path = get_host_rw_shared_path(&self.sid)
                .join(&self.cid)
                .join(SNAPSHOT_DIR);
            if sn_shared_path.exists() {
                if let Err(e) = nix::mount::umount(&sn_shared_path) {
                    warn!(
                        sl!(),
                        "failed to umount snapshot mount at {:?} with err {}", sn_shared_path, e
                    );
                }
            }
        }

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
    if !nydus_prefetch_path.ends_with(format!("/{NYDUS_PREFETCH_FILE_LIST}").as_str()) {
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

    Some(prefetch_list_path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, path::PathBuf};
    use tempfile::tempdir;

    #[test]
    fn test_guest_shared_path() {
        // "/run/kata-containers/shared/containers/<cid>/<suffix>"
        let cid = "nydustester";
        let path = NydusRootfs::guest_shared_path(cid, "rootfs");
        assert_eq!(
            path,
            "/run/kata-containers/shared/containers/nydustester/rootfs"
        );

        let upper_path = NydusRootfs::guest_shared_path(cid, &format!("{}/{}", SNAPSHOT_DIR, "fs"));
        assert_eq!(
            upper_path,
            "/run/kata-containers/shared/containers/nydustester/snapshotdir/fs"
        );

        let work_path =
            NydusRootfs::guest_shared_path(cid, &format!("{}/{}", SNAPSHOT_DIR, "work"));
        assert_eq!(
            work_path,
            "/run/kata-containers/shared/containers/nydustester/snapshotdir/work"
        );
    }

    #[test]
    fn test_guest_nydus_image_path() {
        let cid = "nydustester";
        let path = NydusRootfs::guest_nydus_image_path(cid);
        assert_eq!(
            path,
            "/run/kata-containers/shared/rafs/nydustester/lowerdir"
        );
    }

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
