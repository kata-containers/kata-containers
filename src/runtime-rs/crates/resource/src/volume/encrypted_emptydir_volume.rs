// Copyright (c) 2025 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! Block-encrypted emptyDir volume handler for runtime-rs.
//!
//! When `emptydir_mode = "block-encrypted"` is set in the runtime configuration,
//! each Kubernetes emptyDir volume backed by a host directory is handled here
//! instead of the normal "local" shared-filesystem path.
//!
//! For every such volume this module:
//!
//!   1. Creates a sparse `disk.img` file inside the kubelet emptyDir folder
//!      so that Kubelet can enforce `sizeLimit` (idempotent: skipped if a
//!      previous container in the same pod already did it).
//!   2. Writes a `mountInfo.json` file (direct-volume metadata) that records
//!      the block device path, filesystem type, and `encryption_key=ephemeral`.
//!   3. Plugs the disk image into the VM as a virtio-blk block device via the
//!      hypervisor device manager.
//!   4. Sends an `agent::Storage` with `driver_options: ["encryption_key=ephemeral"]`
//!      and `shared: true` to the kata-agent.  The agent delegates formatting and
//!      mounting to the Confidential Data Hub (CDH) using LUKS2.
//!
//! The `shared: true` flag instructs the agent to keep the storage alive until
//! the sandbox is destroyed.  Correspondingly, `EncryptedEmptyDirVolume::cleanup()`
//! is a deliberate no-op: the host-side `disk.img` and `mountInfo.json` are
//! removed at sandbox teardown by `VolumeResource::cleanup_ephemeral_disks()`.

use std::{collections::HashMap, fs, io::ErrorKind, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use base64::{encode_config, URL_SAFE};
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_device_info, DeviceManager},
        DeviceConfig, DeviceType,
    },
    BlockConfigModern,
};
use kata_sys_util::k8s::is_host_empty_dir;
use kata_types::{
    config::EMPTYDIR_MODE_BLOCK_ENCRYPTED,
    device::{
        DRIVER_BLK_CCW_TYPE as KATA_CCW_DEV_TYPE, DRIVER_BLK_PCI_TYPE as KATA_BLK_DEV_TYPE,
        DRIVER_SCSI_TYPE as KATA_SCSI_DEV_TYPE,
    },
    mount::{
        join_path, kata_direct_volume_root_path, kata_guest_sandbox_dir, DirectVolumeMountInfo,
        KATA_MOUNT_INFO_FILE_NAME,
    },
};
use nix::sys::{stat::stat, statvfs::statvfs};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

use crate::volume::Volume;

/// OCI mount type for bind mounts.
const MOUNT_BIND_TYPE: &str = "bind";

/// Sub-directory of the guest sandbox dir used for block device mounts.
/// Matches genpolicy `spath = /run/kata-containers/sandbox/storage`.
const KATA_GUEST_SANDBOX_STORAGE_DIR: &str = "storage";

/// File name of the sparse disk image created inside each emptyDir folder.
const EMPTYDIR_DISK_IMAGE_NAME: &str = "disk.img";

/// The driver option that tells the kata-agent to encrypt the device via CDH.
const ENCRYPTION_KEY_EPHEMERAL: &str = "encryption_key=ephemeral";

/// Volume-type value written into `mountInfo.json`.
const EMPTYDIR_VOLUME_TYPE_BLK: &str = "blk";

/// Filesystem type used when formatting the encrypted block device.
const EMPTYDIR_FSTYPE: &str = "ext4";

/// Key / value written into the mountInfo.json `metadata` map.
/// Must match Go runtime's direct-volume schema (src/runtime/pkg/direct-volume/utils.go).
const EMPTYDIR_MKFS_METADATA_KEY: &str = "encryptionKey";
const EMPTYDIR_MKFS_METADATA_VAL: &str = "ephemeral";

/// Key for fsGroup metadata in mountInfo.json.
/// Must match Go runtime's direct-volume schema (src/runtime/pkg/direct-volume/utils.go).
const FSGID_KEY: &str = "fsGroup";

// ──────────────────────────────────────────────────────────────────────────────
// Public types
// ──────────────────────────────────────────────────────────────────────────────

/// Descriptor of a block-encrypted emptyDir disk created on the host during a
/// sandbox lifetime.  Instances are collected in `VolumeResource` so the
/// sandbox teardown path can delete the sparse image and its metadata.
#[derive(Debug, Clone)]
pub struct EphemeralDisk {
    /// Absolute path to the `disk.img` file on the host.
    pub disk_path: String,
    /// Absolute path to the kubelet emptyDir folder (also the direct-volume key).
    pub source_path: String,
}

// ──────────────────────────────────────────────────────────────────────────────
// Volume implementation
// ──────────────────────────────────────────────────────────────────────────────

/// Handles a single Kubernetes emptyDir volume in `block-encrypted` mode.
#[derive(Clone)]
pub(crate) struct EncryptedEmptyDirVolume {
    storage: Option<agent::Storage>,
    mount: oci::Mount,
    device_id: String,
}

impl EncryptedEmptyDirVolume {
    pub(crate) async fn new(
        d: &RwLock<DeviceManager>,
        m: &oci::Mount,
        _sid: &str,
        ephemeral_disks: Arc<RwLock<Vec<EphemeralDisk>>>,
    ) -> Result<Self> {
        let source_path = m
            .source()
            .as_ref()
            .and_then(|p| p.to_str())
            .context("block-encrypted emptyDir mount has no source path")?
            .to_string();

        let disk_path = format!("{}/{}", source_path, EMPTYDIR_DISK_IMAGE_NAME);

        // Idempotency: if mountInfo.json already exists, a previous container
        // in this pod already set the volume up.  Skip creation.
        let is_new_disk = match kata_types::mount::get_volume_mount_info(&source_path) {
            Ok(_) => {
                if !std::path::Path::new(&disk_path).exists() {
                    return Err(anyhow!(
                        "mountInfo.json exists but disk image {} is missing",
                        disk_path
                    ));
                }
                info!(
                    sl!(),
                    "encrypted emptyDir: reusing existing disk at {}", disk_path
                );
                false
            }
            Err(e) => {
                let is_not_found = e
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound);
                if !is_not_found {
                    return Err(e).context("failed to read mountInfo for emptyDir");
                }
                setup_ephemeral_disk(&source_path, &disk_path)
                    .with_context(|| format!("setup ephemeral disk at {disk_path}"))?;
                true
            }
        };

        // Register the disk image as a virtio-blk block device.
        let blkdev_info = get_block_device_info(d).await;
        let block_config = BlockConfigModern {
            path_on_host: disk_path.clone(),
            driver_option: blkdev_info.block_device_driver,
            num_queues: blkdev_info.num_queues,
            queue_size: blkdev_info.queue_size,
            ..Default::default()
        };
        let device_info = do_handle_device(d, &DeviceConfig::BlockCfgModern(block_config))
            .await
            .context("register encrypted emptyDir block device with hypervisor")?;

        // Extract the guest-visible device address (PCI path, SCSI addr, etc.)
        // and the hypervisor driver string.
        let (source, driver, device_id) = extract_block_source(device_info).await?;

        // Compute the stable in-guest mount path:
        //   /run/kata-containers/sandbox/storage/<base64url(source)>
        //
        // This satisfies the genpolicy rules:
        //   i_storage.mount_point == $(spath)/base64url.encode(i_storage.source)
        //   i_storage.mount_point == i_mount.source
        let spath = format!(
            "{}/{}",
            kata_guest_sandbox_dir(),
            KATA_GUEST_SANDBOX_STORAGE_DIR
        );
        let b64_source = encode_config(source.as_bytes(), URL_SAFE);
        let mount_point = format!("{}/{}", spath, b64_source);

        let storage = agent::Storage {
            driver,
            source,
            fs_type: EMPTYDIR_FSTYPE.to_string(),
            mount_point: mount_point.clone(),
            driver_options: vec![ENCRYPTION_KEY_EPHEMERAL.to_string()],
            // shared=true: the agent keeps this storage alive until the sandbox
            // is destroyed, not just until the first container using it exits.
            shared: true,
            ..Default::default()
        };

        // The OCI mount source is the in-guest mount_point.  The agent mounts
        // the LUKS2-formatted device there; the container bind-mounts from
        // that path to its own destination.
        let mut mount = oci::Mount::default();
        mount.set_destination(m.destination().clone());
        mount.set_typ(Some(MOUNT_BIND_TYPE.to_string()));
        mount.set_source(Some(PathBuf::from(&mount_point)));
        mount.set_options(m.options().clone());

        if is_new_disk {
            info!(sl!(), "encrypted emptyDir: created disk at {}", disk_path);
            ephemeral_disks.write().await.push(EphemeralDisk {
                disk_path,
                source_path,
            });
        }

        Ok(Self {
            storage: Some(storage),
            mount,
            device_id,
        })
    }
}

#[async_trait]
impl Volume for EncryptedEmptyDirVolume {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(self.storage.iter().cloned().collect())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(Some(self.device_id.clone()))
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        // Intentional no-op: this volume is shared across all containers in
        // the pod.  Host-side cleanup (disk.img + mountInfo.json) is deferred
        // to VolumeResource::cleanup_ephemeral_disks() at sandbox teardown.
        Ok(())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Detection predicate
// ──────────────────────────────────────────────────────────────────────────────

/// Returns `true` when `m` is a host emptyDir bind mount that should be
/// handled as a block-encrypted volume.
pub(crate) fn is_encrypted_emptydir_volume(m: &oci::Mount, emptydir_mode: &str) -> bool {
    if emptydir_mode != EMPTYDIR_MODE_BLOCK_ENCRYPTED {
        return false;
    }

    // update_ephemeral_storage_type() leaves host emptyDirs as "bind" in
    // block-encrypted mode rather than rewriting them to "local".
    if m.typ().as_deref() != Some(MOUNT_BIND_TYPE) {
        return false;
    }

    m.source()
        .as_ref()
        .and_then(|p| p.to_str())
        .map(is_host_empty_dir)
        .unwrap_or(false)
}

// ──────────────────────────────────────────────────────────────────────────────
// Sandbox-level cleanup helper
// ──────────────────────────────────────────────────────────────────────────────

/// Removes the direct-volume `mountInfo.json` directory for `volume_path`.
///
/// Called at sandbox teardown for each `EphemeralDisk` registered during the
/// sandbox lifetime.
pub(crate) fn remove_volume_mount_info(volume_path: &str) -> Result<()> {
    let dir = join_path(kata_direct_volume_root_path().as_str(), volume_path)
        .with_context(|| format!("build direct-volume path for {volume_path}"))?;
    match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("remove direct-volume dir {dir:?}")),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Private helpers
// ──────────────────────────────────────────────────────────────────────────────

/// Creates the sparse `disk.img` and writes `mountInfo.json`.
fn setup_ephemeral_disk(source_path: &str, disk_path: &str) -> Result<()> {
    // Use the capacity of the filesystem that backs the emptyDir so that
    // Kubelet's sizeLimit enforcement still works correctly.
    let vfs =
        statvfs(source_path).with_context(|| format!("statvfs on emptyDir {source_path}"))?;
    let capacity = vfs
        .blocks()
        .checked_mul(vfs.fragment_size())
        .context("emptyDir capacity overflow")?;

    // Create a sparse file: it appears `capacity` bytes large to Kubelet but
    // consumes negligible real disk space until the guest writes into it.
    let f = fs::File::create(disk_path).with_context(|| format!("create {disk_path}"))?;
    f.set_len(capacity)
        .with_context(|| format!("truncate {disk_path} to {capacity}"))?;
    drop(f);

    // Capture the directory's gid to honour fsGroup semantics later.
    let dir_stat =
        stat(source_path).with_context(|| format!("stat emptyDir {source_path}"))?;
    let mut metadata: HashMap<String, String> = HashMap::new();
    metadata.insert(
        EMPTYDIR_MKFS_METADATA_KEY.to_string(),
        EMPTYDIR_MKFS_METADATA_VAL.to_string(),
    );
    if dir_stat.st_gid != 0 {
        metadata.insert(FSGID_KEY.to_string(), dir_stat.st_gid.to_string());
    }

    let mount_info = DirectVolumeMountInfo {
        volume_type: EMPTYDIR_VOLUME_TYPE_BLK.to_string(),
        device: disk_path.to_string(),
        fs_type: EMPTYDIR_FSTYPE.to_string(),
        metadata,
        options: vec![],
    };

    write_volume_mount_info(source_path, &mount_info)
        .with_context(|| format!("write mountInfo.json for {source_path}"))
}

/// Serialises `info` and writes it as `mountInfo.json` under the Kata
/// direct-volume root directory, keyed by `volume_path`.
fn write_volume_mount_info(volume_path: &str, info: &DirectVolumeMountInfo) -> Result<()> {
    let dir = join_path(kata_direct_volume_root_path().as_str(), volume_path)
        .with_context(|| format!("build direct-volume path for {volume_path}"))?;
    fs::create_dir_all(&dir).with_context(|| format!("create dir {dir:?}"))?;
    let file_path = dir.join(KATA_MOUNT_INFO_FILE_NAME);
    let json = serde_json::to_string(info).context("serialise DirectVolumeMountInfo")?;
    fs::write(&file_path, &json).with_context(|| format!("write {file_path:?}"))?;
    Ok(())
}

/// Extracts `(source, driver, device_id)` from a `DeviceType` returned by
/// `do_handle_device`.  Mirrors the logic in `utils::handle_block_volume`.
async fn extract_block_source(device_info: DeviceType) -> Result<(String, String, String)> {
    if let DeviceType::BlockModern(device_mod) = device_info.clone() {
        let device = device_mod.lock().await;
        let driver = device.config.driver_option.clone();
        let source = match driver.as_str() {
            KATA_BLK_DEV_TYPE => device
                .config
                .pci_path
                .as_ref()
                .map(|p| p.to_string())
                .ok_or_else(|| anyhow!("blk device has no PCI path"))?,
            KATA_SCSI_DEV_TYPE => device
                .config
                .scsi_addr
                .clone()
                .ok_or_else(|| anyhow!("SCSI device has no SCSI address"))?,
            _ => device.config.virt_path.clone(),
        };
        return Ok((source, driver, device.device_id.clone()));
    }

    if let DeviceType::Block(device) = device_info {
        let driver = device.config.driver_option.clone();
        let source = match driver.as_str() {
            KATA_BLK_DEV_TYPE => device
                .config
                .pci_path
                .as_ref()
                .map(|p| p.to_string())
                .ok_or_else(|| anyhow!("blk device has no PCI path"))?,
            KATA_SCSI_DEV_TYPE => device
                .config
                .scsi_addr
                .clone()
                .ok_or_else(|| anyhow!("SCSI device has no SCSI address"))?,
            KATA_CCW_DEV_TYPE => device
                .config
                .ccw_addr
                .clone()
                .ok_or_else(|| anyhow!("CCW device has no CCW address"))?,
            _ => device.config.virt_path.clone(),
        };
        return Ok((source, driver, device.device_id));
    }

    Err(anyhow!(
        "encrypted emptyDir: unsupported device type from do_handle_device"
    ))
}
