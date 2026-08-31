// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

//! Read-only volumes handed to the guest as EROFS images rather than through
//! agent `copy_file` calls, for sandboxes without filesystem sharing.
//!
//! This costs one device and one mount, instead of a request per file, symlink
//! and directory.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_device_info, DeviceManager},
        DeviceConfig,
    },
    BlockConfigModern, BlockDeviceAio,
};
use kata_sys_util::mount::get_mount_path;
use kata_types::k8s::{is_configmap, is_downward_api, is_projected, is_secret};
use kata_types::prefix_with_rootless_dir;
use oci_spec::runtime as oci;
use tokio::process::Command;
use tokio::sync::RwLock;

use super::share_fs_volume::is_watchable_volume;
use super::utils::handle_block_volume;
use super::Volume;

pub const DEFAULT_KATA_SHARED_EROFS_VOLUME_PATH: &str = "/run/kata-containers/shared/erofs-volumes";

/// Opt-in name for the `[runtime] experimental` list.
pub const EROFS_VOLUMES_FEATURE: &str = "erofs_volumes";

const EROFS_FS_TYPE: &str = "erofs";
const MKFS_EROFS: &str = "mkfs.erofs";

/// The kernel requires the block size to match the guest page size.
const EROFS_BLOCK_SIZE: &str = "4096";

const IMAGE_MODE: u32 = 0o600;

pub fn kata_shared_erofs_volume_path() -> String {
    prefix_with_rootless_dir(DEFAULT_KATA_SHARED_EROFS_VOLUME_PATH)
}

pub(crate) struct ErofsVolume {
    storage: agent::Storage,
    mount: oci::Mount,
    device_id: String,
    image_path: PathBuf,
}

impl ErofsVolume {
    pub(crate) async fn new(
        d: &RwLock<DeviceManager>,
        m: &oci::Mount,
        sid: &str,
        cid: &str,
    ) -> Result<Self> {
        let source = get_mount_path(m.source());
        let src = fs::canonicalize(&source)
            .with_context(|| format!("canonicalize mount source {source}"))?;
        let entry_name = entry_name(m.destination())?;

        let image_path = image_path(sid, cid, m.destination())?;
        build_image(&src, &entry_name, &image_path)
            .await
            .with_context(|| format!("build erofs image for {source}"))?;

        // The image must not outlive a failed attach.
        let volume = Self::attach(d, m, sid, &src, &entry_name, &image_path).await;
        if volume.is_err() {
            remove_image(&image_path);
        }
        volume
    }

    async fn attach(
        d: &RwLock<DeviceManager>,
        m: &oci::Mount,
        sid: &str,
        src: &Path,
        entry_name: &str,
        image_path: &Path,
    ) -> Result<Self> {
        let blkdev_info = get_block_device_info(d).await;
        let block_device_config = BlockConfigModern {
            path_on_host: image_path.display().to_string(),
            is_readonly: true,
            driver_option: blkdev_info.block_device_driver,
            // /run is tmpfs and rejects O_DIRECT, which in turn rules out
            // aio=native: QEMU only accepts it together with cache.direct.
            is_direct: Some(false),
            blkdev_aio: BlockDeviceAio::Threads,
            num_queues: blkdev_info.num_queues,
            queue_size: blkdev_info.queue_size,
            ..Default::default()
        };

        let device_info = do_handle_device(
            d,
            &DeviceConfig::BlockCfgModern(block_device_config.clone()),
        )
        .await
        .context("attach erofs volume device")?;

        let mount_options = mount_options_for(src);
        let (storage, mut mount, device_id) = handle_block_volume(
            device_info,
            m,
            true,
            sid,
            EROFS_FS_TYPE,
            Some(&mount_options),
        )
        .await
        .context("handle erofs block volume")?;

        // An image root is always a directory, so a file source sits one level
        // inside the mount point.
        if !src.is_dir() {
            mount.set_source(Some(Path::new(&storage.mount_point).join(entry_name)));
        }

        info!(
            sl!(),
            "erofs volume {:?} -> {:?} via {}",
            m.destination(),
            mount.source(),
            image_path.display()
        );

        // copy_file re-sends changed files as inotify reports them, but a
        // mounted image cannot be updated in place.
        if is_watchable_volume(&src.to_path_buf()) {
            warn!(
                sl!(),
                "erofs volume {:?} will not see updates to {}",
                m.destination(),
                src.display()
            );
        }

        Ok(Self {
            storage,
            mount,
            device_id,
            image_path: image_path.to_path_buf(),
        })
    }
}

fn remove_image(image_path: &Path) {
    if let Err(e) = fs::remove_file(image_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            warn!(
                sl!(),
                "failed to remove erofs volume image {}: {:?}",
                image_path.display(),
                e
            );
        }
    }
}

#[async_trait]
impl Volume for ErofsVolume {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(vec![self.storage.clone()])
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(Some(self.device_id.clone()))
    }

    async fn cleanup(&self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        device_manager
            .write()
            .await
            .try_remove_device(&self.device_id)
            .await?;

        remove_image(&self.image_path);

        Ok(())
    }
}

/// The image is immutable, so only read-only mounts qualify. Kubernetes
/// content volumes are read-only by construction even when the OCI spec
/// carries no `ro` option.
pub(crate) fn is_erofs_candidate(m: &oci::Mount, read_only: bool) -> bool {
    let source = get_mount_path(m.source());
    if source.is_empty() {
        return false;
    }
    let src = Path::new(&source);

    if !(read_only || is_k8s_content_volume(src)) {
        return false;
    }

    src.is_file() || src.is_dir()
}

fn is_k8s_content_volume(src: &Path) -> bool {
    is_configmap(src) || is_secret(src) || is_projected(src) || is_downward_api(src)
}

/// A host-built image could otherwise smuggle in setuid binaries or device
/// nodes. Kubernetes content volumes hold data rather than programs, so they
/// get noexec on top; a plain read-only bind mount may legitimately carry
/// binaries.
fn mount_options_for(src: &Path) -> Vec<String> {
    let mut options = vec!["ro".to_string(), "nosuid".to_string(), "nodev".to_string()];

    if is_k8s_content_volume(src) {
        options.push("noexec".to_string());
    }

    options
}

fn entry_name(destination: &Path) -> Result<String> {
    destination
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| anyhow!("mount destination {destination:?} has no file name"))
}

/// Hashing the destination keeps the name short and unique: a container cannot
/// mount two volumes at the same destination.
fn image_path(sid: &str, cid: &str, destination: &Path) -> Result<PathBuf> {
    let mut hasher = DefaultHasher::new();
    destination.hash(&mut hasher);

    Ok(Path::new(&kata_shared_erofs_volume_path())
        .join(sid)
        .join(format!(
            "{cid}-{:016x}-{}.erofs",
            hasher.finish(),
            entry_name(destination)?
        )))
}

async fn build_image(src: &Path, entry_name: &str, image_path: &Path) -> Result<()> {
    let dir = image_path
        .parent()
        .ok_or_else(|| anyhow!("image path {} has no parent", image_path.display()))?;
    fs::create_dir_all(dir)
        .with_context(|| format!("create erofs volume directory {}", dir.display()))?;

    // mkfs.erofs images a directory tree, so a file source needs one of its own.
    let mut staging = None;
    let tree = if src.is_dir() {
        src.to_path_buf()
    } else {
        let scratch = tempfile::tempdir_in(dir).context("create erofs staging directory")?;
        stage_file(src, &scratch.path().join(entry_name))?;
        let path = scratch.path().to_path_buf();
        staging = Some(scratch);
        path
    };

    let result = run_mkfs(&tree, image_path).await;
    drop(staging);
    result?;

    fs::set_permissions(image_path, fs::Permissions::from_mode(IMAGE_MODE))
        .with_context(|| format!("set permissions on {}", image_path.display()))?;

    Ok(())
}

fn stage_file(src: &Path, staged: &Path) -> Result<()> {
    // A hard link avoids the copy, but only within one filesystem.
    if fs::hard_link(src, staged).is_ok() {
        return Ok(());
    }

    fs::copy(src, staged)
        .with_context(|| format!("stage {} for imaging", src.display()))
        .map(|_| ())
}

async fn run_mkfs(tree: &Path, image_path: &Path) -> Result<()> {
    let output = Command::new(MKFS_EROFS)
        .arg("-b")
        .arg(EROFS_BLOCK_SIZE)
        .arg("-T")
        .arg("0")
        .arg(image_path)
        .arg(tree)
        .output()
        .await
        .with_context(|| format!("run {MKFS_EROFS}; is erofs-utils installed?"))?;

    if !output.status.success() {
        return Err(anyhow!(
            "{MKFS_EROFS} failed for {} ({}): {}",
            tree.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::process::Command as StdCommand;
    use tempfile::TempDir;

    fn erofs_utils_available() -> bool {
        StdCommand::new(MKFS_EROFS)
            .arg("--help")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// The layout kubelet's atomic writer produces for a configmap.
    fn atomic_writer_tree(root: &Path) {
        let data = root.join("..2026_01_01_00_00_00.1234");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("key-a"), b"value-a").unwrap();
        fs::write(data.join("key-b"), b"value-b").unwrap();
        symlink("..2026_01_01_00_00_00.1234", root.join("..data")).unwrap();
        symlink("..data/key-a", root.join("key-a")).unwrap();
        symlink("..data/key-b", root.join("key-b")).unwrap();
    }

    fn mount_of(destination: &str, source: &Path) -> oci::Mount {
        let mut m = oci::Mount::default();
        m.set_destination(PathBuf::from(destination));
        m.set_source(Some(source.to_path_buf()));
        m.set_typ(Some("bind".to_string()));
        m
    }

    fn extract(image: &Path, into: &Path) {
        let output = StdCommand::new("fsck.erofs")
            .arg(format!("--extract={}", into.display()))
            .arg("--overwrite")
            .arg(image)
            .output()
            .expect("run fsck.erofs --extract");
        assert!(
            output.status.success(),
            "fsck.erofs rejected the image: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn test_directory_source_preserves_symlink_layout() {
        if !erofs_utils_available() {
            println!("skipping: erofs-utils not installed");
            return;
        }

        let tmp = TempDir::new().unwrap();
        let tree = tmp.path().join("configmap");
        fs::create_dir_all(&tree).unwrap();
        atomic_writer_tree(&tree);

        let image = tmp.path().join("out.erofs");
        build_image(&tree, "configmap", &image).await.unwrap();

        assert_eq!(&fs::read(&image).unwrap()[1024..1028], b"\xe2\xe1\xf5\xe0");
        assert_eq!(
            fs::metadata(&image).unwrap().permissions().mode() & 0o777,
            IMAGE_MODE
        );

        let out = tmp.path().join("extract");
        extract(&image, &out);
        assert_eq!(
            fs::read_link(out.join("key-a")).unwrap(),
            Path::new("..data/key-a")
        );
        assert_eq!(fs::read(out.join("key-a")).unwrap(), b"value-a");
    }

    #[tokio::test]
    async fn test_file_source_is_staged_under_destination_name() {
        if !erofs_utils_available() {
            println!("skipping: erofs-utils not installed");
            return;
        }

        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("etc-hosts");
        fs::write(&src, b"127.0.0.1\tlocalhost\n").unwrap();

        let image = tmp.path().join("out.erofs");
        build_image(&src, "hosts", &image).await.unwrap();

        let out = tmp.path().join("extract");
        extract(&image, &out);
        assert_eq!(
            fs::read(out.join("hosts")).unwrap(),
            b"127.0.0.1\tlocalhost\n"
        );
    }

    #[tokio::test]
    async fn test_staging_leaves_nothing_behind() {
        if !erofs_utils_available() {
            println!("skipping: erofs-utils not installed");
            return;
        }

        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("resolv.conf");
        fs::write(&src, b"nameserver 10.96.0.10\n").unwrap();

        let dir = tmp.path().join("images");
        let image = dir.join("out.erofs");
        build_image(&src, "resolv.conf", &image).await.unwrap();

        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p != &image)
            .collect();
        assert!(leftovers.is_empty(), "leftover staging: {:?}", leftovers);
    }

    #[test]
    fn test_read_write_mounts_are_rejected() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("data");
        fs::create_dir_all(&src).unwrap();

        let m = mount_of("/data", &src);
        assert!(!is_erofs_candidate(&m, false));
        assert!(is_erofs_candidate(&m, true));
    }

    #[test]
    fn test_missing_and_sourceless_mounts_are_rejected() {
        let tmp = TempDir::new().unwrap();

        let missing = mount_of("/data", &tmp.path().join("does-not-exist"));
        assert!(!is_erofs_candidate(&missing, true));

        let mut sourceless = oci::Mount::default();
        sourceless.set_destination(PathBuf::from("/data"));
        assert!(!is_erofs_candidate(&sourceless, true));
    }

    #[test]
    fn test_k8s_content_volumes_are_accepted_without_ro() {
        let tmp = TempDir::new().unwrap();
        let src = tmp
            .path()
            .join("pods/6dad7281/volumes/kubernetes.io~configmap/cm");
        fs::create_dir_all(&src).unwrap();

        assert!(is_k8s_content_volume(&src));
        assert!(is_erofs_candidate(&mount_of("/cm", &src), false));
    }

    #[test]
    fn test_content_volumes_are_mounted_noexec() {
        let tmp = TempDir::new().unwrap();

        for kind in [
            "kubernetes.io~configmap",
            "kubernetes.io~secret",
            "kubernetes.io~projected",
            "kubernetes.io~downward-api",
        ] {
            let src = tmp.path().join(format!("pods/6dad7281/volumes/{kind}/v"));
            fs::create_dir_all(&src).unwrap();

            let options = mount_options_for(&src);
            assert!(
                options.contains(&"noexec".to_string()),
                "{} should be noexec, got {:?}",
                kind,
                options
            );
            for expected in ["ro", "nosuid", "nodev"] {
                assert!(options.contains(&expected.to_string()));
            }
        }
    }

    #[test]
    fn test_plain_read_only_mounts_keep_exec() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("rootfs-overlay");
        fs::create_dir_all(&src).unwrap();

        let options = mount_options_for(&src);

        assert!(!options.contains(&"noexec".to_string()));
        assert!(options.contains(&"nosuid".to_string()));
        assert!(options.contains(&"nodev".to_string()));
    }

    #[test]
    fn test_image_paths_are_unique_per_destination() {
        let a = image_path("sid", "cid", Path::new("/etc/hosts")).unwrap();
        let b = image_path("sid", "cid", Path::new("/etc/resolv.conf")).unwrap();
        let c = image_path("sid", "cid", Path::new("/other/hosts")).unwrap();

        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(
            a,
            image_path("sid", "cid", Path::new("/etc/hosts")).unwrap()
        );
        assert!(a.to_string_lossy().ends_with("-hosts.erofs"));
    }

    #[test]
    fn test_entry_name_requires_a_file_name() {
        assert_eq!(entry_name(Path::new("/etc/hosts")).unwrap(), "hosts");
        assert!(entry_name(Path::new("/")).is_err());
    }
}
