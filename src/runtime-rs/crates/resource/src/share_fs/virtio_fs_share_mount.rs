// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::utils::remove_dir_all_if_exists;
use kata_sys_util::mount::{bind_remount, get_mount_points_under, umount_all, umount_timeout};
use kata_types::k8s::is_watchable_mount;
use std::fs;
use std::path::Path;

const WATCHABLE_PATH_NAME: &str = "watchable";
const WATCHABLE_BIND_DEV_TYPE: &str = "watchable-bind";
const DEFAULT_EPHEMERAL_PATH: &str = "/run/kata-containers/sandbox/ephemeral";

use crate::share_fs::kata_guest_share_dir;

use super::{
    get_host_rw_shared_path,
    utils::{
        self, do_get_host_path, get_host_ro_shared_path, get_host_shared_path,
        mkdir_with_permissions,
    },
    ShareFsMount, ShareFsMountResult, ShareFsRootfsConfig, ShareFsVolumeConfig, PASSTHROUGH_FS_DIR,
};

pub fn ephemeral_path() -> String {
    DEFAULT_EPHEMERAL_PATH.to_string()
}

#[derive(Debug)]
pub struct VirtiofsShareMount {
    id: String,
}

impl VirtiofsShareMount {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

#[async_trait]
impl ShareFsMount for VirtiofsShareMount {
    async fn share_rootfs(&self, config: &ShareFsRootfsConfig) -> Result<ShareFsMountResult> {
        // TODO: select virtiofs or support nydus
        let guest_path = utils::share_to_guest(
            &config.source,
            &config.target,
            &self.id,
            &config.cid,
            config.readonly,
            false,
            config.is_rafs,
        )
        .context("share to guest")?;
        Ok(ShareFsMountResult {
            guest_path,
            storages: vec![],
        })
    }

    async fn share_volume(&self, config: &ShareFsVolumeConfig) -> Result<ShareFsMountResult> {
        let mut guest_path = utils::share_to_guest(
            &config.source,
            &config.target,
            &self.id,
            &config.cid,
            config.readonly,
            true,
            config.is_rafs,
        )
        .context("share to guest")?;

        // watchable mounts
        if is_watchable_mount(&config.source) {
            // Create path in shared directory for creating watchable mount:
            let host_rw_path = utils::get_host_rw_shared_path(&self.id);

            // "/run/kata-containers/shared/sandboxes/$sid/rw/passthrough/watchable"
            let watchable_host_path = Path::new(&host_rw_path)
                .join(PASSTHROUGH_FS_DIR)
                .join(WATCHABLE_PATH_NAME);

            mkdir_with_permissions(watchable_host_path.clone(), 0o750).context(format!(
                "unable to create watchable path {watchable_host_path:?}"
            ))?;

            // path: /run/kata-containers/shared/containers/passthrough/watchable/config-map-name
            let file_name = Path::new(&guest_path)
                .file_name()
                .context("get file name from guest path")?;
            let watchable_guest_mount = Path::new(kata_guest_share_dir().as_str())
                .join(PASSTHROUGH_FS_DIR)
                .join(WATCHABLE_PATH_NAME)
                .join(file_name)
                .into_os_string()
                .into_string()
                .map_err(|e| anyhow!("failed to get watchable guest mount path {:?}", e))?;

            let watchable_storage: Storage = Storage {
                driver: String::from(WATCHABLE_BIND_DEV_TYPE),
                driver_options: Vec::new(),
                source: guest_path,
                fs_type: String::from("bind"),
                fs_group: None,
                options: config.mount_options.clone(),
                mount_point: watchable_guest_mount.clone(),
                shared: false,
            };

            // Update the guest_path, in order to identify what will
            // change in the OCI spec.
            guest_path = watchable_guest_mount;

            let storages = vec![watchable_storage];

            return Ok(ShareFsMountResult {
                guest_path,
                storages,
            });
        }

        Ok(ShareFsMountResult {
            guest_path,
            storages: vec![],
        })
    }

    async fn upgrade_to_rw(&self, file_name: &str) -> Result<()> {
        // Remount readonly directory with readwrite permission
        let host_dest = do_get_host_path(file_name, &self.id, "", true, true);
        bind_remount(host_dest, false)
            .context("remount readonly directory with readwrite permission")?;
        // Remount readwrite directory with readwrite permission
        let host_dest = do_get_host_path(file_name, &self.id, "", true, false);
        bind_remount(host_dest, false)
            .context("remount readwrite directory with readwrite permission")?;
        Ok(())
    }

    async fn downgrade_to_ro(&self, file_name: &str) -> Result<()> {
        // Remount readwrite directory with readonly permission
        let host_dest = do_get_host_path(file_name, &self.id, "", true, false);
        bind_remount(host_dest, true)
            .context("remount readwrite directory with readonly permission")?;
        // Remount readonly directory with readonly permission
        let host_dest = do_get_host_path(file_name, &self.id, "", true, true);
        bind_remount(host_dest, true)
            .context("remount readonly directory with readonly permission")?;
        Ok(())
    }

    async fn umount_volume(&self, file_name: &str) -> Result<()> {
        let host_dest = do_get_host_path(file_name, &self.id, "", true, false);
        umount_timeout(&host_dest, 0).context("umount volume")?;
        // Umount event will be propagated to ro directory

        // Remove the directory of mointpoint
        if let Ok(md) = fs::metadata(&host_dest) {
            if md.is_file() {
                fs::remove_file(&host_dest).context("remove the volume mount point as a file")?;
            }
            if md.is_dir() {
                fs::remove_dir(&host_dest).context("remove the volume mount point as a dir")?;
            }
        }
        Ok(())
    }

    async fn umount_rootfs(&self, config: &ShareFsRootfsConfig) -> Result<()> {
        let host_dest = do_get_host_path(&config.target, &self.id, &config.cid, false, false);
        umount_timeout(&host_dest, 0).context("umount rootfs")?;

        // Remove the directory of mointpoint
        if let Ok(md) = fs::metadata(&host_dest) {
            if md.is_dir() {
                fs::remove_dir(&host_dest).context("remove the rootfs mount point as a dir")?;
            }
        }

        if !config.cid.is_empty() {
            if let Some(container_dir) = Path::new(&host_dest).parent() {
                // Remove only the empty container directory. If entries remain, leave them
                // for their owning cleanup paths rather than deleting them recursively.
                match fs::remove_dir(container_dir) {
                    Err(e)
                        if e.kind() != std::io::ErrorKind::NotFound
                            && e.kind() != std::io::ErrorKind::DirectoryNotEmpty =>
                    {
                        return Err(e).context("remove the container directory");
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    async fn cleanup(&self, sid: &str) -> Result<()> {
        // Unmount ro path
        let host_ro_dest = get_host_ro_shared_path(sid);
        umount_all(host_ro_dest.clone(), true).context("failed to umount ro path")?;

        // Recursive removal through a surviving bind mount would delete host data.
        let host_path = get_host_shared_path(sid);
        let mount_points =
            get_mount_points_under(&host_path).context("failed to list shared path mounts")?;
        if !mount_points.is_empty() {
            warn!(
                sl!(),
                "unmounting leftover shared path mounts: {mount_points:?}"
            );
        }
        for mount_point in &mount_points {
            umount_all(mount_point, true)
                .with_context(|| format!("failed to umount {}", mount_point.display()))?;
        }
        let mount_points =
            get_mount_points_under(&host_path).context("failed to list shared path mounts")?;
        if !mount_points.is_empty() {
            return Err(anyhow!(
                "shared path {} still has mounts: {mount_points:?}",
                host_path.display()
            ));
        }

        remove_dir_all_if_exists(host_ro_dest).context("failed to remove ro path")?;
        let host_rw_dest = get_host_rw_shared_path(sid);
        remove_dir_all_if_exists(host_rw_dest).context("failed to remove rw path")?;
        // remove the host share directory
        remove_dir_all_if_exists(host_path).context("failed to remove host shared path")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ephemeral_path() {
        assert_eq!(ephemeral_path(), DEFAULT_EPHEMERAL_PATH);
    }

    // Isolate mounts so a failed assertion cannot affect other tests or the host.
    #[test]
    #[ignore = "requires root and mount namespace capabilities"]
    fn test_cleanup_preserves_mounted_volume_source() {
        const CHILD: &str = "KATA_SHARED_TREE_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new("unshare")
                .args(["--mount", "--propagation", "private", "--"])
                .arg(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "share_fs::virtio_fs_share_mount::tests::test_cleanup_preserves_mounted_volume_source",
                    "--ignored",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .expect("start private mount namespace (requires unshare)");
            assert!(
                output.status.success(),
                "isolated mount regression failed: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            return;
        }

        use nix::mount::{mount, umount2, MntFlags, MsFlags};

        // Shared propagation reproduces volume mounts appearing in the read-only export.
        let temp = tempfile::tempdir().unwrap();
        mount(
            Some("tmpfs"),
            temp.path(),
            Some("tmpfs"),
            MsFlags::empty(),
            None::<&str>,
        )
        .unwrap();
        // Detach before TempDir cleanup so a failed assertion cannot remove mounted data.
        let _unmount = scopeguard::guard(temp.path().to_path_buf(), |root| {
            umount2(&root, MntFlags::MNT_DETACH).unwrap();
        });
        mount(
            None::<&str>,
            temp.path(),
            None::<&str>,
            MsFlags::MS_SHARED,
            None::<&str>,
        )
        .unwrap();

        let source = temp.path().join("external-source");
        fs::create_dir(&source).unwrap();
        mount(
            Some("tmpfs"),
            &source,
            Some("tmpfs"),
            MsFlags::empty(),
            None::<&str>,
        )
        .unwrap();
        let sentinel = source.join("sentinel");
        fs::write(&sentinel, b"externally owned source data").unwrap();
        let rootfs_source = temp.path().join("rootfs-source");
        fs::create_dir(&rootfs_source).unwrap();
        let rootfs_sentinel = rootfs_source.join("sentinel");
        fs::write(&rootfs_sentinel, b"container rootfs data").unwrap();

        // Use an absolute ID to isolate paths without changing the global rootless flag.
        let sandbox = temp.path().join("sandbox");
        let sid = sandbox.to_str().unwrap();
        let rw = get_host_rw_shared_path(sid);
        let ro = get_host_ro_shared_path(sid);
        fs::create_dir_all(&rw).unwrap();
        fs::create_dir_all(&ro).unwrap();
        kata_sys_util::mount::bind_mount_unchecked(&rw, &ro, true, MsFlags::MS_SLAVE).unwrap();

        let volume = do_get_host_path("volume", sid, "container", true, false);
        let ro_volume = do_get_host_path("volume", sid, "container", true, true);
        let rootfs = do_get_host_path("rootfs", sid, "container", false, false);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let share = VirtiofsShareMount::new(sid);
        runtime
            .block_on(share.share_volume(&ShareFsVolumeConfig {
                cid: "container".into(),
                source: source.to_str().unwrap().into(),
                target: "volume".into(),
                readonly: false,
                mount_options: vec![],
                mount: Default::default(),
                is_rafs: false,
            }))
            .unwrap();
        runtime
            .block_on(share.share_rootfs(&ShareFsRootfsConfig {
                cid: "container".into(),
                source: rootfs_source.to_str().unwrap().into(),
                target: "rootfs".into(),
                readonly: false,
                is_rafs: false,
            }))
            .unwrap();

        let is_mounted = |path: &str| kata_sys_util::mount::get_linux_mount_info(path).is_ok();
        assert!(is_mounted(&volume));
        assert!(is_mounted(&ro_volume));
        assert!(is_mounted(&rootfs));
        // Leave mounts behind to reproduce unfinished container cleanup.
        let result = runtime.block_on(share.cleanup(sid));
        let preserved = || {
            fs::read(&sentinel).ok().as_deref() == Some(b"externally owned source data".as_slice())
                && fs::read(&rootfs_sentinel).ok().as_deref()
                    == Some(b"container rootfs data".as_slice())
        };
        assert!(
            preserved(),
            "shared cleanup deleted source data: {:?}",
            result
        );
        result.unwrap();
        assert_eq!(
            kata_sys_util::mount::get_mount_points_under(&sandbox).unwrap(),
            Vec::<std::path::PathBuf>::new()
        );
        assert!(!sandbox.exists());
        assert!(is_mounted(source.to_str().unwrap()));

        runtime.block_on(share.cleanup(sid)).unwrap();
        assert!(preserved());
        assert!(is_mounted(source.to_str().unwrap()));
    }
}
