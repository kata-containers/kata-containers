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
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{chown, OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

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
use nix::dir::Dir;
use nix::errno::Errno;
use nix::fcntl::{openat, readlinkat, AtFlags, OFlag};
use nix::sys::stat::{fstat, fstatat, FileStat, Mode, SFlag};
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
const EROFS_BLOCK_SIZE: u64 = 4096;

const VERITYSETUP: &str = "veritysetup";

/// veritysetup draws a random salt unless it is given one, which would put the
/// root hash back to differing on every build. A fixed salt costs nothing
/// here: salting defends against precomputation across images, and the root
/// hash is something we publish anyway.
const VERITY_SALT: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// mkfs.erofs draws a random filesystem UUID unless it is given one, which on
/// its own makes two images of identical content differ. Pin it so the bytes
/// depend on the tree alone, which is what lets the image be measured.
const EROFS_IMAGE_UUID: &str = "00000000-0000-0000-0000-000000000000";

/// Kubelet's atomic writer points this at the directory holding the payload.
const ATOMIC_WRITER_DATA_LINK: &str = "..data";

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
        let verity = build_image(&src, &entry_name, &image_path)
            .await
            .with_context(|| format!("build erofs image for {source}"))?;

        // The image must not outlive a failed attach.
        let volume = Self::attach(d, m, sid, &src, &entry_name, &image_path, &verity).await;
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
        verity: &Verity,
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
        let (mut storage, mut mount, device_id) = handle_block_volume(
            device_info,
            m,
            true,
            sid,
            EROFS_FS_TYPE,
            Some(&mount_options),
        )
        .await
        .context("handle erofs block volume")?;

        // Added here rather than through handle_block_volume, whose options
        // also end up on the container's bind mount, where these do not belong.
        storage.options.extend(verity.storage_options());

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

async fn build_image(src: &Path, entry_name: &str, image_path: &Path) -> Result<Verity> {
    let dir = image_path
        .parent()
        .ok_or_else(|| anyhow!("image path {} has no parent", image_path.display()))?;
    fs::create_dir_all(dir)
        .with_context(|| format!("create erofs volume directory {}", dir.display()))?;

    // mkfs.erofs images a directory tree, so a file source needs one of its own.
    let staging = tempfile::tempdir_in(dir).context("create erofs staging directory")?;
    if src.is_dir() {
        stage_directory(src, staging.path())?;
    } else {
        stage_file(src, &staging.path().join(entry_name))?;
    }

    let result = run_mkfs(staging.path(), image_path).await;
    drop(staging);
    result?;

    let verity = append_verity_tree(image_path).await?;

    fs::set_permissions(image_path, fs::Permissions::from_mode(IMAGE_MODE))
        .with_context(|| format!("set permissions on {}", image_path.display()))?;

    Ok(verity)
}

fn stage_file(src: &Path, staged: &Path) -> Result<()> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(src)
        .with_context(|| format!("open {}", src.display()))?;

    copy_file(file, src, staged)
}

/// What the guest needs in order to check the image against what we measured.
pub(crate) struct Verity {
    root_hash: String,
    /// Where the hash tree starts, which is also the size of the image proper.
    hash_offset: u64,
}

impl Verity {
    /// Read by the agent, which builds the dm-verity device from them before
    /// mounting. Not mount options: the agent strips the prefix before it
    /// calls mount().
    fn storage_options(&self) -> Vec<String> {
        vec![
            "X-kata.dmverity-enabled=true".to_string(),
            format!("X-kata.dmverity.roothash={}", self.root_hash),
            format!("X-kata.dmverity.hashoffset={}", self.hash_offset),
            format!("X-kata.dmverity.blocksize={}", EROFS_BLOCK_SIZE),
            format!("X-kata.dmverity.hashsize={}", EROFS_BLOCK_SIZE),
            format!("X-kata.dmverity.salt={}", VERITY_SALT),
        ]
    }
}

/// Append a dm-verity hash tree to the image, so that the guest can check
/// every block it reads against a hash rather than trusting the device.
///
/// Tree and image share one file, which keeps it to a single device: the
/// image occupies everything below the offset and the tree everything above.
///
/// The root hash is worth only as much as the ability to recompute it, and it
/// is tied to the erofs-utils version: 1.7.1 and 1.9.3 number inodes
/// differently, so their images never agree. Two separate builds of one
/// version do agree, so pinning the version is enough, and kata-deploy
/// already pins it through the erofs-utils image.
async fn append_verity_tree(image_path: &Path) -> Result<Verity> {
    // veritysetup wants the hash area block-aligned, and mkfs.erofs does not
    // promise a whole number of blocks. Padding is invisible to erofs, which
    // reads no further than its own superblock says.
    let data_size = pad_to_block(image_path)?;

    let output = Command::new(VERITYSETUP)
        .arg("format")
        .arg(image_path)
        .arg(image_path)
        .arg(format!("--hash-offset={}", data_size))
        .arg(format!("--data-blocks={}", data_size / EROFS_BLOCK_SIZE))
        .arg(format!("--data-block-size={}", EROFS_BLOCK_SIZE))
        .arg(format!("--hash-block-size={}", EROFS_BLOCK_SIZE))
        .arg(format!("--salt={}", VERITY_SALT))
        // veritysetup stamps a random UUID into the hash superblock, which
        // leaves the image differing between builds even though the root
        // hash does not.
        .arg(format!("--uuid={}", EROFS_IMAGE_UUID))
        .output()
        .await
        .with_context(|| format!("run {VERITYSETUP}; is cryptsetup installed?"))?;

    if !output.status.success() {
        return Err(anyhow!(
            "{VERITYSETUP} failed for {} ({}): {}",
            image_path.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(Verity {
        root_hash: parse_root_hash(&String::from_utf8_lossy(&output.stdout))?,
        hash_offset: data_size,
    })
}

fn pad_to_block(image_path: &Path) -> Result<u64> {
    let size = fs::metadata(image_path)?.len();
    let padded = size.div_ceil(EROFS_BLOCK_SIZE) * EROFS_BLOCK_SIZE;

    if padded != size {
        fs::OpenOptions::new()
            .write(true)
            .open(image_path)?
            .set_len(padded)?;
    }

    if padded == 0 {
        return Err(anyhow!("{} is empty", image_path.display()));
    }

    Ok(padded)
}

fn parse_root_hash(output: &str) -> Result<String> {
    output
        .lines()
        .find_map(|line| line.strip_prefix("Root hash:"))
        .map(str::trim)
        .filter(|hash| !hash.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("no root hash in {VERITYSETUP} output: {output}"))
}

/// Copy the tree at `src` into `root`, flattening kubelet's atomic writer
/// layout into the files its links point at.
///
/// Symlinks are not supported. A link staged into the image resolves inside
/// the guest, against whatever the container sees there, and following one
/// here would copy in whatever the host path leads to. Nothing is followed:
/// the walk goes through file descriptors opened with O_NOFOLLOW, so a link
/// swapped in while it runs is refused rather than taken.
fn stage_directory(src: &Path, root: &Path) -> Result<()> {
    let mut dir = Dir::open(src, DIR_FLAGS, Mode::empty())
        .with_context(|| format!("open {}", src.display()))?;
    let data = atomic_writer_payload(&dir, src)?
        .map(|payload| {
            let path = src.join(&payload);
            open_dir_at(&dir, &payload, &path).map(|d| (d, path))
        })
        .transpose()?;

    for name in entry_names(&mut dir)? {
        let from = src.join(&name);
        let to = root.join(&name);

        match &data {
            // ..data and the timestamped payload. Kubelet rejects keys
            // beginning with "..", so none of the pod's keys is dropped.
            Some(_) if name.as_bytes().starts_with(b"..") => {}
            Some((payload, payload_path)) if kind_at(&dir, &name, &from)? == SFlag::S_IFLNK => {
                let target = readlinkat(&dir, name.as_os_str())
                    .with_context(|| format!("read link {}", from.display()))?;
                if Path::new(&target) != Path::new(ATOMIC_WRITER_DATA_LINK).join(&name) {
                    return Err(anyhow!(
                        "{} links to {:?} rather than into {ATOMIC_WRITER_DATA_LINK}, and EROFS volumes do not support symlinks",
                        from.display(),
                        target
                    ));
                }
                copy_entry(payload, &name, &payload_path.join(&name), &to)?;
            }
            _ => copy_entry(&dir, &name, &from, &to)?,
        }
    }

    set_metadata(root, &fstat(&dir)?)
}

const DIR_FLAGS: OFlag = OFlag::O_RDONLY
    .union(OFlag::O_DIRECTORY)
    .union(OFlag::O_NOFOLLOW)
    .union(OFlag::O_CLOEXEC);

/// The payload directory `..data` points at, provided it is a sibling named
/// the way kubelet names it, `..` followed by a timestamp.
fn atomic_writer_payload(dir: &Dir, src: &Path) -> Result<Option<OsString>> {
    match fstatat(dir, ATOMIC_WRITER_DATA_LINK, AtFlags::AT_SYMLINK_NOFOLLOW) {
        Ok(stat) if kind(&stat) == SFlag::S_IFLNK => {}
        Ok(_) | Err(Errno::ENOENT) => return Ok(None),
        Err(e) => return Err(e).context(format!("stat {}", src.display())),
    }

    let payload = readlinkat(dir, ATOMIC_WRITER_DATA_LINK)
        .with_context(|| format!("read {ATOMIC_WRITER_DATA_LINK} in {}", src.display()))?;

    let mut components = Path::new(&payload).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None)
            if name.as_bytes().starts_with(b"..") && name != ATOMIC_WRITER_DATA_LINK =>
        {
            Ok(Some(name.to_owned()))
        }
        _ => Err(anyhow!(
            "{ATOMIC_WRITER_DATA_LINK} in {} points at {:?}, outside the volume",
            src.display(),
            payload
        )),
    }
}

fn entry_names(dir: &mut Dir) -> Result<Vec<OsString>> {
    let mut names = Vec::new();
    for entry in dir.iter() {
        let entry = entry?;
        let name = OsStr::from_bytes(entry.file_name().to_bytes());
        if name != "." && name != ".." {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}

fn copy_entry(parent: &Dir, name: &OsStr, from: &Path, to: &Path) -> Result<()> {
    let entry_kind = kind_at(parent, name, from)?;

    if entry_kind == SFlag::S_IFDIR {
        copy_tree(open_dir_at(parent, name, from)?, from, to)
    } else if entry_kind == SFlag::S_IFREG {
        let fd = openat(
            parent,
            name,
            OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC,
            Mode::empty(),
        )
        .with_context(|| format!("open {}", from.display()))?;
        copy_file(File::from(fd), from, to)
    } else {
        Err(unsupported(from, entry_kind))
    }
}

fn copy_tree(mut dir: Dir, from: &Path, to: &Path) -> Result<()> {
    fs::create_dir(to).with_context(|| format!("create {}", to.display()))?;

    for name in entry_names(&mut dir)? {
        copy_entry(&dir, &name, &from.join(&name), &to.join(&name))?;
    }

    set_metadata(to, &fstat(&dir)?)
}

fn copy_file(mut file: File, from: &Path, to: &Path) -> Result<()> {
    let stat = fstat(&file).with_context(|| format!("stat {}", from.display()))?;
    if kind(&stat) != SFlag::S_IFREG {
        return Err(unsupported(from, kind(&stat)));
    }

    let mut staged = File::create_new(to).with_context(|| format!("create {}", to.display()))?;
    io::copy(&mut file, &mut staged).with_context(|| format!("copy {}", from.display()))?;

    set_metadata(to, &stat)
}

fn open_dir_at(parent: &Dir, name: &OsStr, path: &Path) -> Result<Dir> {
    Dir::openat(parent, name, DIR_FLAGS, Mode::empty())
        .with_context(|| format!("open {}", path.display()))
}

fn kind_at(parent: &Dir, name: &OsStr, path: &Path) -> Result<SFlag> {
    fstatat(parent, name, AtFlags::AT_SYMLINK_NOFOLLOW)
        .map(|stat| kind(&stat))
        .with_context(|| format!("stat {}", path.display()))
}

fn kind(stat: &FileStat) -> SFlag {
    SFlag::from_bits_truncate(stat.st_mode & SFlag::S_IFMT.bits())
}

fn unsupported(path: &Path, kind: SFlag) -> anyhow::Error {
    if kind == SFlag::S_IFLNK {
        anyhow!(
            "{} is a symlink, and EROFS volumes do not support symlinks",
            path.display()
        )
    } else {
        anyhow!(
            "{} is neither a regular file nor a directory, which is all EROFS volumes support",
            path.display()
        )
    }
}

/// Mode and ownership reach the image and the container sees them, so a staged
/// copy has to carry the originals rather than whatever the runtime would
/// create them as.
fn set_metadata(dst: &Path, stat: &FileStat) -> Result<()> {
    fs::set_permissions(dst, fs::Permissions::from_mode(stat.st_mode & 0o7777))?;
    chown(dst, Some(stat.st_uid), Some(stat.st_gid))?;

    Ok(())
}

async fn run_mkfs(tree: &Path, image_path: &Path) -> Result<()> {
    let output = Command::new(MKFS_EROFS)
        .arg("-b")
        .arg(EROFS_BLOCK_SIZE.to_string())
        .arg("-T")
        .arg("0")
        .arg("-U")
        .arg(EROFS_IMAGE_UUID)
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
        tool_available(MKFS_EROFS, "--help") && tool_available(VERITYSETUP, "--version")
    }

    fn tool_available(tool: &str, probe: &str) -> bool {
        StdCommand::new(tool)
            .arg(probe)
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
    async fn test_atomic_writer_volume_is_flattened() {
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
        assert!(fs::symlink_metadata(out.join("key-a")).unwrap().is_file());
        assert_eq!(fs::read(out.join("key-a")).unwrap(), b"value-a");
        assert_eq!(fs::read(out.join("key-b")).unwrap(), b"value-b");

        let mut names: Vec<_> = fs::read_dir(&out)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        names.sort();
        assert_eq!(names, ["key-a", "key-b"]);
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
    async fn test_the_same_tree_images_to_the_same_bytes() {
        if !erofs_utils_available() {
            println!("skipping: erofs-utils not installed");
            return;
        }

        let tmp = TempDir::new().unwrap();
        let tree = tmp.path().join("configmap");
        fs::create_dir_all(&tree).unwrap();
        atomic_writer_tree(&tree);

        let first = tmp.path().join("first.erofs");
        let second = tmp.path().join("second.erofs");
        build_image(&tree, "configmap", &first).await.unwrap();
        build_image(&tree, "configmap", &second).await.unwrap();

        assert_eq!(
            fs::read(&first).unwrap(),
            fs::read(&second).unwrap(),
            "two builds of one tree must produce identical images"
        );
    }

    /// The same configmap, written by kubelet at two different moments.
    fn atomic_writer_tree_stamped(root: &Path, stamp: &str) {
        let data = root.join(stamp);
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("key-a"), b"value-a").unwrap();
        fs::write(data.join("key-b"), b"value-b").unwrap();
        symlink(stamp, root.join("..data")).unwrap();
        symlink("..data/key-a", root.join("key-a")).unwrap();
        symlink("..data/key-b", root.join("key-b")).unwrap();
    }

    #[tokio::test]
    async fn test_kubelet_write_time_does_not_reach_the_image() {
        if !erofs_utils_available() {
            println!("skipping: erofs-utils not installed");
            return;
        }

        let tmp = TempDir::new().unwrap();
        let mut images = Vec::new();

        for stamp in ["..2026_01_01_00_00_00.1234", "..2026_09_22_19_15_00.9876"] {
            let tree = tmp.path().join(stamp.trim_start_matches('.'));
            fs::create_dir_all(&tree).unwrap();
            atomic_writer_tree_stamped(&tree, stamp);

            let image = tmp.path().join(format!("{}.erofs", stamp.len()));
            build_image(&tree, "cm", &image).await.unwrap();
            images.push(fs::read(&image).unwrap());
        }

        assert_eq!(
            images[0], images[1],
            "the atomic writer's timestamp must not change the image"
        );
    }

    fn assert_no_symlinks(dir: &Path) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            assert!(!meta.is_symlink(), "{} is a symlink", path.display());
            if meta.is_dir() {
                assert_no_symlinks(&path);
            }
        }
    }

    #[tokio::test]
    async fn test_images_hold_no_symlinks() {
        if !erofs_utils_available() {
            println!("skipping: erofs-utils not installed");
            return;
        }

        let tmp = TempDir::new().unwrap();
        let tree = tmp.path().join("projected");
        fs::create_dir_all(&tree).unwrap();
        atomic_writer_tree(&tree);
        let data = tree.join("..2026_01_01_00_00_00.1234");
        fs::create_dir_all(data.join("nested")).unwrap();
        fs::write(data.join("nested/key-c"), b"value-c").unwrap();
        symlink("..data/nested", tree.join("nested")).unwrap();
        // Where kubelet mounts a volume nested inside this one.
        fs::create_dir(tree.join("mnt")).unwrap();

        let image = tmp.path().join("out.erofs");
        build_image(&tree, "projected", &image).await.unwrap();

        let out = tmp.path().join("extract");
        extract(&image, &out);
        assert_no_symlinks(&out);
        assert_eq!(fs::read(out.join("nested/key-c")).unwrap(), b"value-c");
        assert!(out.join("mnt").is_dir());
    }

    fn stage(tree: &Path) -> Result<TempDir> {
        let out = TempDir::new().unwrap();
        stage_directory(tree, out.path()).map(|_| out)
    }

    fn assert_rejected(tree: &Path, expected: &str) {
        let err = format!("{:#}", stage(tree).unwrap_err());
        assert!(err.contains(expected), "unexpected error: {}", err);
    }

    #[test]
    fn test_plain_trees_are_copied_with_their_metadata() {
        let tmp = TempDir::new().unwrap();
        let tree = tmp.path().join("plain");
        fs::create_dir_all(tree.join("sub")).unwrap();
        fs::write(tree.join("sub/file"), b"data").unwrap();
        fs::set_permissions(tree.join("sub/file"), fs::Permissions::from_mode(0o640)).unwrap();
        fs::write(tree.join("..data"), b"not kubelet's").unwrap();

        let out = stage(&tree).unwrap();

        let staged = out.path().join("sub/file");
        assert_eq!(fs::read(&staged).unwrap(), b"data");
        assert_eq!(
            fs::metadata(&staged).unwrap().permissions().mode() & 0o7777,
            0o640
        );
        assert_eq!(
            fs::read(out.path().join("..data")).unwrap(),
            b"not kubelet's"
        );
    }

    #[test]
    fn test_symlinks_are_rejected() {
        let tmp = TempDir::new().unwrap();

        let top = tmp.path().join("top");
        fs::create_dir_all(&top).unwrap();
        symlink("/etc/passwd", top.join("passwd")).unwrap();
        assert_rejected(&top, "is a symlink");

        let nested = tmp.path().join("nested");
        fs::create_dir_all(nested.join("sub")).unwrap();
        symlink("../..", nested.join("sub/up")).unwrap();
        assert_rejected(&nested, "is a symlink");

        let dangling = tmp.path().join("dangling");
        fs::create_dir_all(&dangling).unwrap();
        symlink("missing", dangling.join("link")).unwrap();
        assert_rejected(&dangling, "is a symlink");
    }

    #[test]
    fn test_data_link_escaping_the_volume_is_rejected() {
        let tmp = TempDir::new().unwrap();

        for (i, target) in [
            "/etc",
            "../elsewhere",
            "..2026/nested",
            "..2026/../../etc",
            "..2026/..",
            "..data",
            "plain",
        ]
        .iter()
        .enumerate()
        {
            let tree = tmp.path().join(i.to_string());
            fs::create_dir_all(&tree).unwrap();
            symlink(target, tree.join("..data")).unwrap();
            assert_rejected(&tree, "outside the volume");
        }
    }

    #[test]
    fn test_key_links_must_point_into_the_payload() {
        let tmp = TempDir::new().unwrap();

        for (i, target) in [
            "/etc/passwd",
            "..data/../../../etc/passwd",
            "..data/evil/../../../etc/passwd",
            "..data/evil/..",
            "..data/key-a",
            "..2026_01_01_00_00_00.1234/key-a",
        ]
        .iter()
        .enumerate()
        {
            let tree = tmp.path().join(i.to_string());
            fs::create_dir_all(&tree).unwrap();
            atomic_writer_tree(&tree);
            symlink(target, tree.join("evil")).unwrap();
            assert_rejected(&tree, "rather than into ..data");
        }
    }

    #[test]
    fn test_symlinks_inside_the_payload_are_rejected() {
        let tmp = TempDir::new().unwrap();
        let tree = tmp.path().join("configmap");
        fs::create_dir_all(&tree).unwrap();
        atomic_writer_tree(&tree);
        let data = tree.join("..2026_01_01_00_00_00.1234");
        symlink("/etc/shadow", data.join("key-c")).unwrap();
        symlink("..data/key-c", tree.join("key-c")).unwrap();

        assert_rejected(&tree, "is a symlink");
    }

    #[test]
    fn test_special_files_are_rejected() {
        let tmp = TempDir::new().unwrap();
        let tree = tmp.path().join("fifo");
        fs::create_dir_all(&tree).unwrap();
        nix::unistd::mkfifo(&tree.join("pipe"), Mode::from_bits_truncate(0o600)).unwrap();

        assert_rejected(&tree, "neither a regular file nor a directory");
    }

    #[test]
    fn test_symlinked_file_sources_are_rejected() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("real"), b"data").unwrap();
        symlink("real", tmp.path().join("link")).unwrap();

        assert!(stage_file(&tmp.path().join("link"), &tmp.path().join("staged")).is_err());
        assert!(!tmp.path().join("staged").exists());
    }

    #[test]
    fn test_root_hash_is_read_from_veritysetup_output() {
        let output = "VERITY header information for /img\n\
                      UUID:                 8f3f\n\
                      Hash type:            1\n\
                      Data blocks:          1\n\
                      Salt:                 0000\n\
                      Root hash:            9c4a1e2f\n";

        assert_eq!(parse_root_hash(output).unwrap(), "9c4a1e2f");
        assert!(parse_root_hash("Root hash:   \n").is_err());
        assert!(parse_root_hash("no hash here").is_err());
    }

    #[test]
    fn test_verity_storage_options_carry_what_the_guest_needs() {
        let options = Verity {
            root_hash: "9c4a1e2f".to_string(),
            hash_offset: 8192,
        }
        .storage_options();

        assert!(options.contains(&"X-kata.dmverity-enabled=true".to_string()));
        assert!(options.contains(&"X-kata.dmverity.roothash=9c4a1e2f".to_string()));
        assert!(options.contains(&"X-kata.dmverity.hashoffset=8192".to_string()));
        assert!(options.iter().all(|o| o.starts_with("X-kata.")));
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
