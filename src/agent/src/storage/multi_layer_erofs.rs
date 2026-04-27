// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! Multi-layer EROFS storage handler
//!
//! This handler implements the guest-side processing of multi-layer EROFS rootfs:
//! - Storage with X-kata.overlay-upper: ext4 rw layer (upperdir)
//! - Storage with X-kata.overlay-lower: erofs layers (lowerdir)
//! - Creates overlay to combine them
//! - Supports X-kata.mkdir.path options to create directories in upper layer before overlay mount

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::confidential_data_hub;
use crate::device::block_device_handler::get_virtio_blk_pci_device_name;
use crate::device::scsi_device_handler::get_scsi_device_name;
use crate::linux_abi::pcipath_from_dev_tree_path;
use crate::mount::baremount;
use crate::sandbox::Sandbox;
use crate::storage::{StorageContext, StorageHandler};
use crate::AGENT_CONFIG;
use anyhow::{anyhow, Context, Result};
use kata_sys_util::mount::create_mount_destination;
use kata_types::device::{DRIVER_BLK_PCI_TYPE, DRIVER_SCSI_TYPE};
use kata_types::mount::StorageDevice;
use protocols::agent::Storage;
use safe_path::scoped_join;
use slog::Logger;
use tokio::sync::Mutex;

/// EROFS Type
const EROFS_TYPE: &str = "erofs";
/// ext4 Type
const EXT4_TYPE: &str = "ext4";
/// Overlay Type
const OVERLAY_TYPE: &str = "overlay";

/// Driver type for multi-layer EROFS
pub const DRIVER_MULTI_LAYER_EROFS: &str = "erofs.multi-layer";

/// Custom storage option markers
const OPT_OVERLAY_UPPER: &str = "X-kata.overlay-upper";
const OPT_OVERLAY_LOWER: &str = "X-kata.overlay-lower";
const OPT_MULTI_LAYER: &str = "X-kata.multi-layer=true";
const OPT_MKDIR_PATH: &str = "X-kata.mkdir.path=";

/// MKFS options for unformatted rwlayer
const OPT_MKFS: &str = "X-kata.mkfs=true";
const OPT_MKFS_FS: &str = "X-kata.mkfs.fs=";
const OPT_MKFS_SIZE: &str = "X-kata.mkfs.size=";
const OPT_MKFS_UUID: &str = "X-kata.mkfs.uuid=";

/// MKFS type prefix from containerd
const MKFS_TYPE_PREFIX: &str = "mkfs/";
/// LUKS type (luks2 is the modern standard)
const LUKS_TYPE: &str = "luks2";
/// cryptsetup binary path (used for cleanup: cryptsetup close)
const CRYPTSETUP_BIN: &str = "cryptsetup";
/// Device mapper path prefix
const DEV_MAPPER_PATH: &str = "/dev/mapper/";

#[derive(Debug)]
pub struct MultiLayerErofsHandler {}
/// LUKS encrypted device information for cleanup.
///
/// When CDH sets up LUKS encryption, we discover the mapper device name
/// from /proc/mounts so that cleanup can call `cryptsetup close`.
/// The encryption key is managed entirely by CDH/KBS and is not held by the agent.
#[derive(Debug, Clone)]
pub struct LuksInfo {
    /// Device mapper name (e.g., "kata-rwlayer-xxx")
    pub mapper_name: String,
}

/// Storage device that handles LUKS encrypted device cleanup
///
/// This wraps a base storage device and ensures LUKS devices are properly
/// closed when the storage is cleaned up.
///
/// Cleanup order:
/// 1. Unmount overlay (base_device.cleanup)
/// 2. Unmount upper layer temporary mount (must happen before LUKS close)
/// 3. Close LUKS devices (cryptsetup close)
pub struct LuksStorageDevice {
    /// Base storage device
    base_device: Arc<dyn StorageDevice>,
    /// LUKS mapper names that need to be closed on cleanup
    luks_devices: Vec<String>,
    /// Upper layer temporary mount point that sits on top of the LUKS device.
    /// Must be unmounted before closing the LUKS device.
    upper_mount_path: Option<String>,
    /// Logger for cleanup operations
    logger: Logger,
}

impl std::fmt::Debug for LuksStorageDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuksStorageDevice")
            .field("luks_devices", &self.luks_devices)
            .field("upper_mount_path", &self.upper_mount_path)
            .finish()
    }
}

impl LuksStorageDevice {
    /// Create a new LUKS storage device wrapper
    pub fn new(
        base_device: Arc<dyn StorageDevice>,
        luks_devices: Vec<String>,
        upper_mount_path: Option<String>,
        logger: Logger,
    ) -> Self {
        Self {
            base_device,
            luks_devices,
            upper_mount_path,
            logger,
        }
    }
}

impl StorageDevice for LuksStorageDevice {
    fn path(&self) -> Option<&str> {
        self.base_device.path()
    }

    fn cleanup(&self) -> Result<()> {
        // Step 1: Cleanup the base device (unmount overlay, etc.)
        if let Err(e) = self.base_device.cleanup() {
            warn!(
                self.logger,
                "Error cleaning up base storage device: {:?}", e
            );
            // Continue with LUKS cleanup even if base cleanup fails
        }

        // Step 2: Unmount the upper layer temporary mount point.
        // The upper layer is mounted on the LUKS mapper device, so it MUST
        // be unmounted before we can close the LUKS device.
        if let Some(ref upper_path) = self.upper_mount_path {
            info!(
                self.logger,
                "Unmounting upper layer before LUKS close";
                "upper_mount_path" => upper_path,
            );
            if let Ok(true) = crate::mount::is_mounted(upper_path) {
                let mounts = vec![upper_path.to_string()];
                if let Err(e) = crate::mount::remove_mounts(&mounts) {
                    warn!(
                        self.logger,
                        "Error unmounting upper layer {}: {:?}", upper_path, e
                    );
                }
            }
        }

        // Step 3: Close all LUKS devices
        for mapper_name in &self.luks_devices {
            info!(
                self.logger,
                "Cleaning up LUKS device during storage cleanup";
                "mapper_name" => mapper_name,
            );
            if let Err(e) = luks_close(mapper_name, &self.logger) {
                warn!(
                    self.logger,
                    "Error closing LUKS device {}: {:?}", mapper_name, e
                );
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MultiLayerErofsResult {
    pub mount_point: String,
    pub processed_mount_points: Vec<String>,
    /// Temporary mount points (upper, lower-0, lower-1, …) that back the
    /// overlay.  These must be tracked so they are unmounted *after* the
    /// overlay target during container teardown.
    pub temp_mount_points: Vec<String>,
    /// Upper layer temporary mount point path (sits on LUKS device, needs unmount before LUKS close)
    pub upper_mount_path: Option<String>,
    /// LUKS encrypted devices that need cleanup
    pub luks_devices: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct MkdirDirective {
    raw_path: String,
    mode: Option<String>,
}

#[async_trait::async_trait]
impl StorageHandler for MultiLayerErofsHandler {
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_MULTI_LAYER_EROFS]
    }

    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        // This is called when a single storage has driver="erofs.multi-layer"
        // For now, treat it as a regular mount point
        info!(
            ctx.logger,
            "multi-layer EROFS handler invoked for single storage";
            "driver" => &storage.driver,
            "source" => &storage.source,
            "fstype" => &storage.fstype,
            "mount-point" => &storage.mount_point,
        );

        let path = crate::storage::common_storage_handler(ctx.logger, &storage)?;
        crate::storage::new_device(path)
    }
}

pub fn is_multi_layer_storage(storage: &Storage) -> bool {
    storage.options.iter().any(|o| o == OPT_MULTI_LAYER)
        || storage.driver == DRIVER_MULTI_LAYER_EROFS
}

pub async fn handle_multi_layer_erofs_group(
    trigger: &Storage,
    storages: &[Storage],
    cid: &Option<String>,
    sandbox: &Arc<Mutex<Sandbox>>,
    logger: &Logger,
) -> Result<MultiLayerErofsResult> {
    let logger = logger.new(o!(
        "subsystem" => "multi-layer-erofs",
        "trigger-mount-point" => trigger.mount_point.clone(),
    ));

    let multi_layer_storages: Vec<&Storage> = storages
        .iter()
        .filter(|s| is_multi_layer_storage(s))
        .collect();

    if multi_layer_storages.is_empty() {
        return Err(anyhow!("no multi-layer storages found"));
    }

    info!(
        logger,
        "handle_multi_layer_erofs_group: found multi-layer storages";
        "count" => multi_layer_storages.len(),
        "trigger-mount-point" => trigger.mount_point.clone(),
        "storages" => multi_layer_storages.iter().map(|s| format!("{}:{}", s.fstype, s.mount_point)).collect::<Vec<_>>().join(","),
    );

    let mut ext4_storage: Option<&Storage> = None;
    let mut erofs_storages: Vec<&Storage> = Vec::new();
    let mut mkdir_dirs: Vec<MkdirDirective> = Vec::new();

    for storage in &multi_layer_storages {
        if is_upper_storage(storage) {
            if ext4_storage.is_some() {
                return Err(anyhow!(
                    "multi-layer erofs currently supports exactly one ext4 upper layer"
                ));
            }
            ext4_storage = Some(*storage);

            // Extract mkdir directories from X-kata.mkdir.path options
            for opt in &storage.options {
                if let Some(mkdir_spec) = opt.strip_prefix(OPT_MKDIR_PATH) {
                    mkdir_dirs.push(parse_mkdir_directive(mkdir_spec)?);
                }
            }
        } else if is_lower_storage(storage) {
            erofs_storages.push(*storage);
        }
    }

    let ext4 = ext4_storage
        .ok_or_else(|| anyhow!("multi-layer erofs missing ext4 upper layer storage"))?;

    if erofs_storages.is_empty() {
        return Err(anyhow!(
            "multi-layer erofs missing erofs lower layer storage"
        ));
    }

    info!(
        logger,
        "Handling multi-layer erofs group";
        "ext4-device" => &ext4.source,
        "erofs-devices" => erofs_storages
            .iter()
            .map(|s| s.source.as_str())
            .collect::<Vec<_>>()
            .join(","),
        "mount-point" => &ext4.mount_point,
        "mkdir-dirs-count" => mkdir_dirs.len(),
    );

    // Create temporary mount points for upper and lower layers
    let cid_str = cid.as_deref().unwrap_or("sandbox");
    // Validate container ID to prevent path traversal via crafted cid values
    validate_container_id(cid_str)?;
    let temp_base = PathBuf::from(format!("/run/kata-containers/{}/multi-layer", cid_str));
    fs::create_dir_all(&temp_base).context("failed to create temp mount base")?;

    // Validate mount point to prevent path traversal via crafted mount_point values
    validate_mount_point(&ext4.mount_point)?;

    let upper_mount = temp_base.join("upper");
    fs::create_dir_all(&upper_mount).context("failed to create upper mount dir")?;

    // Track LUKS devices for cleanup
    let mut luks_devices: Vec<String> = Vec::new();

    // Mount the upper layer (rwlayer) - this may be LUKS encrypted if mkfs type
    let luks_info = wait_and_mount_layer(ext4, &upper_mount, sandbox, &logger).await?;
    if let Some(info) = luks_info {
        info!(
            logger,
            "Tracking LUKS encrypted rwlayer for cleanup";
            "mapper_name" => &info.mapper_name,
        );
        luks_devices.push(info.mapper_name);
    }

    for mkdir_dir in &mkdir_dirs {
        // As {{ mount 1 }} refers to the first lower layer, which is not available until we mount it.
        // Just skip it for now and handle it in a second pass after mounting the lower layers.
        if mkdir_dir.raw_path.contains("{{ mount 1 }}") {
            continue;
        }
        let resolved_path = resolve_mkdir_path(&mkdir_dir.raw_path, &upper_mount, None)?;
        info!(
            logger,
            "Creating mkdir directory in upper layer";
            "raw-path" => &mkdir_dir.raw_path,
            "resolved-path" => resolved_path.display().to_string(),
        );

        fs::create_dir_all(&resolved_path).context(format!(
            "failed to create mkdir directory: {}",
            resolved_path.display()
        ))?;
    }

    let mut lower_mounts = Vec::new();
    for (index, erofs) in erofs_storages.iter().enumerate() {
        let lower_mount = temp_base.join(format!("lower-{}", index));
        fs::create_dir_all(&lower_mount).context(format!(
            "failed to create lower mount dir {}",
            lower_mount.display()
        ))?;

        // EROFS layers are not encrypted, so we ignore the returned LuksInfo
        wait_and_mount_layer(erofs, &lower_mount, sandbox, &logger).await?;
        lower_mounts.push(lower_mount);
    }

    // If any mkdir directive refers to {{ mount 1 }}, resolve it now using the first lower mount.
    // This matches current supported placeholder behavior without inventing a broader template scheme.
    for mkdir_dir in &mkdir_dirs {
        if mkdir_dir.raw_path.contains("{{ mount 1 }}") {
            let first_lower = lower_mounts
                .first()
                .ok_or_else(|| anyhow!("lower mount is missing while resolving mkdir path"))?;
            let resolved_path =
                resolve_mkdir_path(&mkdir_dir.raw_path, &upper_mount, Some(first_lower))?;
            info!(
                logger,
                "Creating deferred mkdir directory";
                "raw-path" => &mkdir_dir.raw_path,
                "resolved-path" => resolved_path.display().to_string(),
            );

            fs::create_dir_all(&resolved_path).context(format!(
                "failed to create deferred mkdir directory: {}",
                resolved_path.display()
            ))?;
        }
    }

    let upperdir = upper_mount.join("upper");
    let workdir = upper_mount.join("work");

    if !upperdir.exists() {
        fs::create_dir_all(&upperdir).context("failed to create upperdir")?;
    }
    fs::create_dir_all(&workdir).context("failed to create workdir")?;

    let lowerdir = lower_mounts
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(":");

    info!(
        logger,
        "Creating overlay mount";
        "upperdir" => upperdir.display(),
        "lowerdir" => &lowerdir,
        "workdir" => workdir.display(),
        "target" => &ext4.mount_point,
        "luks-devices" => luks_devices.join(","),
    );

    create_mount_destination(
        Path::new(OVERLAY_TYPE),
        Path::new(&ext4.mount_point),
        "",
        OVERLAY_TYPE,
    )
    .context("failed to create overlay mount destination")?;

    let overlay_options = format!(
        "upperdir={},lowerdir={},workdir={}",
        upperdir.display(),
        lowerdir,
        workdir.display()
    );

    baremount(
        Path::new(OVERLAY_TYPE),
        Path::new(&ext4.mount_point),
        OVERLAY_TYPE,
        nix::mount::MsFlags::empty(),
        &overlay_options,
        &logger,
    )
    .context("failed to mount overlay")?;

    info!(
        logger,
        "Multi-layer EROFS overlay mounted successfully";
        "mount-point" => &ext4.mount_point,
        "luks-devices-count" => luks_devices.len(),
    );

    // Collect all unique mount points to maintain a clean resource state.
    //
    // In multi-layer EROFS configurations, upper and lower storages may share
    // the same mount point.
    // We must deduplicate these entries before processing to prevent:
    // 1. Double-incrementing sandbox refcounts for the same resource.
    // 2. Redundant bookkeeping operations that could lead to state inconsistency.
    //
    // Note: We maintain the original order of insertion, which is essential for
    // ensuring a predictable and correct sequence during resource cleanup.
    let processed_mount_points = multi_layer_storages.iter().fold(Vec::new(), |mut acc, s| {
        if !acc.contains(&s.mount_point) {
            acc.push(s.mount_point.clone());
        }
        acc
    });

    // Collect the temporary mount points (upper first, then lowers) so the
    // caller can register them in container_mounts for proper cleanup.
    let mut temp_mount_points = Vec::with_capacity(1 + lower_mounts.len());
    temp_mount_points.push(upper_mount.display().to_string());
    for lm in &lower_mounts {
        temp_mount_points.push(lm.display().to_string());
    }

    Ok(MultiLayerErofsResult {
        mount_point: ext4.mount_point.clone(),
        processed_mount_points,
        temp_mount_points,
        luks_devices: luks_devices.clone(),
        upper_mount_path: if luks_devices.is_empty() {
            None
        } else {
            Some(upper_mount.display().to_string())
        },
    })
}

async fn track_temporary_mount_for_cleanup(
    sandbox: &Arc<tokio::sync::Mutex<Sandbox>>,
    mount_point: &Path,
    logger: &Logger,
) -> Result<()> {
    let mount_point_str = mount_point.display().to_string();
    let mut sandbox = sandbox.lock().await;
    if !sandbox.storages.contains_key(&mount_point_str) {
        sandbox.add_sandbox_storage(&mount_point_str, false).await;

        let device = crate::storage::StorageDeviceGeneric::new(mount_point_str.clone());
        sandbox
            .update_sandbox_storage(&mount_point_str, Arc::new(device))
            .map_err(|_| anyhow!("failed to update sandbox storage for {}", mount_point_str))?;

        info!(
            logger,
            "Tracking temporary mount point for cleanup";
            "mount-point" => &mount_point_str
        );
    }
    Ok(())
}

fn is_upper_storage(storage: &Storage) -> bool {
    storage.options.iter().any(|o| o == OPT_OVERLAY_UPPER)
        || (storage.fstype == EXT4_TYPE && storage.options.iter().any(|o| o == OPT_MULTI_LAYER))
        || (storage.options.iter().any(|o| o == OPT_MKFS)
            && storage.options.iter().any(|o| o == OPT_MULTI_LAYER))
}

fn is_lower_storage(storage: &Storage) -> bool {
    storage.options.iter().any(|o| o == OPT_OVERLAY_LOWER)
        || (storage.fstype == EROFS_TYPE && storage.options.iter().any(|o| o == OPT_MULTI_LAYER))
}

/// Get device number in "major:minor" format from a device path.
/// CDH secure_mount requires device_id in "major:minor" format.
/// This reads the device metadata to obtain major/minor numbers.
fn get_device_number_from_path(dev_path: &str) -> Result<String> {
    use std::os::unix::fs::MetadataExt;

    let metadata =
        std::fs::metadata(dev_path).context(format!("failed to stat device: {}", dev_path))?;
    let rdev = metadata.rdev();
    Ok(format!(
        "{}:{}",
        nix::sys::stat::major(rdev),
        nix::sys::stat::minor(rdev)
    ))
}

/// Find the LUKS device mapper name for a given mount point by inspecting /proc/mounts.
/// Returns None if the mount source is not a /dev/mapper/ device.
fn find_luks_mapper_for_mount(mount_point: &str, logger: &Logger) -> Result<Option<String>> {
    let mount_info = kata_sys_util::mount::get_linux_mount_info(mount_point)
        .context(format!("failed to get mount info for: {}", mount_point))?;

    info!(
        logger,
        "Mount info for LUKS mapper discovery";
        "mount_point" => mount_point,
        "device" => &mount_info.device,
        "fs_type" => &mount_info.fs_type,
    );

    if let Some(mapper_name) = mount_info.device.strip_prefix(DEV_MAPPER_PATH) {
        if !mapper_name.is_empty() {
            return Ok(Some(mapper_name.to_string()));
        }
    }

    Ok(None)
}

/// Try to set up LUKS encryption using CDH (Confidential Data Hub) service.
/// CDH handles the entire LUKS lifecycle in one call:
/// LUKS format -> LUKS open -> mkfs -> mount (one-shot operation).
async fn try_cdh_luks_setup(
    dev_path: &str,
    actual_fstype: &str,
    layer: &Storage,
    mount_point: &Path,
    logger: &Logger,
) -> Result<Option<LuksInfo>> {
    // Check if CDH is available
    if !confidential_data_hub::is_cdh_client_initialized() {
        return Ok(None);
    }

    info!(
        logger,
        "CDH is available, attempting LUKS encryption via CDH secure_mount";
        "device" => dev_path,
        "fstype" => actual_fstype,
    );

    // Get device major:minor number for CDH
    let device_id = get_device_number_from_path(dev_path)
        .context("failed to get device number for CDH secure_mount")?;

    // Build mkfs options string from layer options (e.g., "-U <uuid>")
    let mut mkfs_opts_parts: Vec<String> = Vec::new();
    if let Some(uuid) = layer
        .options
        .iter()
        .find_map(|o| o.strip_prefix(OPT_MKFS_UUID))
    {
        mkfs_opts_parts.push("-U".to_string());
        mkfs_opts_parts.push(uuid.to_string());
    }
    let mkfs_opts = mkfs_opts_parts.join(" ");

    let integrity = AGENT_CONFIG.secure_storage_integrity.to_string();

    // Construct CDH options HashMap following the same format as rpc::cdh_secure_mount
    let options = std::collections::HashMap::from([
        ("deviceId".to_string(), device_id.clone()),
        ("sourceType".to_string(), "empty".to_string()),
        ("targetType".to_string(), "fileSystem".to_string()),
        ("filesystemType".to_string(), actual_fstype.to_string()),
        ("mkfsOpts".to_string(), mkfs_opts.clone()),
        ("encryptionType".to_string(), LUKS_TYPE.to_string()),
        ("dataIntegrity".to_string(), integrity.clone()),
    ]);

    let mount_point_str = mount_point.to_str().unwrap_or("");

    info!(
        logger,
        "Calling CDH secure_mount for LUKS encrypted rwlayer";
        "device_id" => &device_id,
        "filesystem_type" => actual_fstype,
        "encryption_type" => LUKS_TYPE,
        "data_integrity" => &integrity,
        "mkfs_opts" => &mkfs_opts,
        "mount_point" => mount_point_str,
    );

    // CDH secure_mount handles everything: LUKS format + open + mkfs + mount
    confidential_data_hub::secure_mount("block-device", &options, vec![], mount_point_str)
        .await
        .context("CDH secure_mount failed for LUKS encrypted rwlayer")?;

    info!(
        logger,
        "CDH secure_mount completed successfully, discovering LUKS mapper device";
        "mount_point" => mount_point_str,
    );

    let luks_info = match find_luks_mapper_for_mount(mount_point_str, logger)? {
        Some(mapper_name) => {
            info!(
                logger,
                "Discovered LUKS mapper device from CDH mount";
                "mapper_name" => &mapper_name,
            );
            Some(LuksInfo { mapper_name })
        }
        None => {
            info!(
                logger,
                "CDH mount source is not a /dev/mapper device";
                "mount_point" => mount_point_str,
            );
            None
        }
    };

    Ok(luks_info)
}

/// Close a LUKS encrypted device
fn luks_close(mapper_name: &str, logger: &Logger) -> Result<()> {
    info!(logger, "Closing LUKS device"; "mapper_name" => mapper_name);

    let output = std::process::Command::new(CRYPTSETUP_BIN)
        .args(["close", mapper_name])
        .output()
        .context("failed to execute cryptsetup close")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Log warning but don't fail - device might already be closed
        warn!(
            logger,
            "cryptsetup close warning for {}: {}", mapper_name, stderr
        );
    }

    Ok(())
}

/// Validate that a container ID does not contain path traversal sequences.
///
/// Container IDs are used to construct filesystem paths. A malicious ID containing
/// path separators or ".." components could be used to escape the intended directory.
fn validate_container_id(cid: &str) -> Result<()> {
    if cid.is_empty() {
        return Err(anyhow!("container ID must not be empty"));
    }
    if cid.contains('/') || cid.contains('\\') || cid.contains("..") || cid.contains('\0') {
        return Err(anyhow!(
            "container ID contains invalid characters (path separators, '..', or null bytes): '{}'",
            cid
        ));
    }
    Ok(())
}

/// Validate that a mount point path is absolute and does not contain path traversal sequences.
fn validate_mount_point(mount_point: &str) -> Result<()> {
    if mount_point.is_empty() {
        return Err(anyhow!("mount point must not be empty"));
    }
    if !mount_point.starts_with('/') {
        return Err(anyhow!(
            "mount point must be an absolute path, got: '{}'",
            mount_point
        ));
    }
    if mount_point.contains("..") {
        return Err(anyhow!(
            "mount point must not contain path traversal sequences: '{}'",
            mount_point
        ));
    }
    Ok(())
}

fn parse_mkdir_directive(spec: &str) -> Result<MkdirDirective> {
    let parts: Vec<&str> = spec.splitn(2, ':').collect();
    if parts.is_empty() || parts[0].is_empty() {
        return Err(anyhow!("invalid X-kata.mkdir.path directive: '{}'", spec));
    }

    let raw_path = parts[0];

    // Reject null bytes
    if raw_path.contains('\0') {
        return Err(anyhow!("X-kata.mkdir.path contains null bytes: '{}'", spec));
    }

    Ok(MkdirDirective {
        raw_path: raw_path.to_string(),
        mode: parts.get(1).map(|s| s.to_string()),
    })
}

/// Resolve a mkdir path template and ensure it is safely scoped under the given root.
///
/// Templates may contain `{{ mount 0 }}` (upper layer) and `{{ mount 1 }}` (first lower layer)
/// placeholders. After substitution, the resolved path is validated using `safe_path::scoped_join`
/// to prevent path traversal attacks.
fn resolve_mkdir_path(
    raw_path: &str,
    upper_mount: &Path,
    first_lower_mount: Option<&Path>,
) -> Result<PathBuf> {
    let mut resolved = raw_path.replace("{{ mount 0 }}", upper_mount.to_str().unwrap_or(""));

    if let Some(lower) = first_lower_mount {
        resolved = resolved.replace("{{ mount 1 }}", lower.to_str().unwrap_or(""));
    }

    let resolved_path = Path::new(&resolved);

    // Determine the scoping root: the resolved path should be under one of the known mount points.
    // We use the upper_mount as the default scope root when the path references it,
    // and the first_lower_mount when the path references that instead.
    let scope_root = if let Some(lower) = first_lower_mount {
        if resolved.starts_with(lower.to_str().unwrap_or("")) {
            lower
        } else {
            upper_mount
        }
    } else {
        upper_mount
    };

    // Extract the relative portion after the scope root prefix
    let relative = if let Ok(rel) = resolved_path.strip_prefix(scope_root) {
        rel.to_path_buf()
    } else {
        // If the path doesn't start with any known root, treat the whole path as unsafe
        PathBuf::from(&resolved)
    };

    // Use scoped_join to ensure the final path cannot escape the scope root.
    // This handles "..", symlinks, and other traversal techniques.
    let safe = scoped_join(scope_root, &relative).context(format!(
        "path traversal detected in mkdir path: raw='{}', resolved='{}', scope_root='{}'",
        raw_path,
        resolved,
        scope_root.display()
    ))?;

    Ok(safe)
}

async fn wait_and_mount_layer(
    layer: &Storage,
    layer_mount: &Path,
    sandbox: &Arc<Mutex<Sandbox>>,
    logger: &Logger,
) -> Result<Option<LuksInfo>> {
    info!(
        logger,
        "Waiting for layer device";
        "device" => &layer.source,
        "driver" => &layer.driver,
        "mount-point" => layer_mount.display(),
    );
    let dev_path = match layer.driver.as_str() {
        DRIVER_SCSI_TYPE => {
            // For SCSI devices, we need to wait for the device to appear and get its path before mounting.
            get_scsi_device_name(sandbox, &layer.source).await?
        }
        DRIVER_BLK_PCI_TYPE => {
            let (root_complex, pcipath) = pcipath_from_dev_tree_path(&layer.source)?;
            get_virtio_blk_pci_device_name(sandbox, root_complex, &pcipath).await?
        }
        _ => {
            // For non-SCSI devices, we can assume the source is directly mountable.
            return Err(anyhow!(
                "unsupported driver type '{}' for multi-layer erofs",
                layer.driver
            ));
        }
    };

    // Check if this is an mkfs type that needs formatting before mount
    let is_mkfs =
        layer.fstype.starts_with(MKFS_TYPE_PREFIX) || layer.options.iter().any(|o| o == OPT_MKFS);

    // Determine the actual filesystem type to use
    let actual_fstype = if is_mkfs {
        // Extract filesystem type from mkfs options or use default ext4
        let fs_from_option = layer
            .options
            .iter()
            .find_map(|o| o.strip_prefix(OPT_MKFS_FS));
        fs_from_option.unwrap_or(EXT4_TYPE).to_string()
    } else {
        layer.fstype.clone()
    };

    info!(
        logger,
        "Mounting layer";
        "device" => &layer.source,
        "fstype" => &layer.fstype,
        "actual-fstype" => &actual_fstype,
        "devname" => &dev_path,
        "mount-point" => layer_mount.display(),
        "is-mkfs" => is_mkfs,
    );

    // LUKS encryption via CDH (Confidential Data Hub).
    if is_mkfs {
        if let Some(luks_info) =
            try_cdh_luks_setup(&dev_path, &actual_fstype, layer, layer_mount, logger).await?
        {
            // CDH handled everything: LUKS + mkfs + mount
            track_temporary_mount_for_cleanup(sandbox, layer_mount, logger).await?;
            return Ok(Some(luks_info));
        }

        // CDH not available, proceed without encryption (mkfs + mount only)
        info!(
            logger,
            "CDH not available, proceeding without encryption for mkfs rwlayer"
        );
    }

    // Non-encrypted path: format (if mkfs) and mount the device directly.
    create_mount_destination(Path::new(&dev_path), layer_mount, "", &actual_fstype)
        .context("failed to create layer mount destination")?;

    // If this is an mkfs type, format the device before mounting
    if is_mkfs {
        info!(
            logger,
            "Formatting unencrypted rwlayer device";
            "device" => &dev_path,
            "fstype" => &actual_fstype,
        );

        // Build mkfs command arguments
        let mut mkfs_args = vec![];

        // Add UUID if specified
        if let Some(uuid) = layer
            .options
            .iter()
            .find_map(|o| o.strip_prefix(OPT_MKFS_UUID))
        {
            mkfs_args.push("-U".to_string());
            mkfs_args.push(uuid.to_string());
        }

        // Add size if specified (for ext4, this would be the filesystem size)
        // Note: Most filesystems use the entire device by default, so size is optional
        if let Some(_size) = layer
            .options
            .iter()
            .find_map(|o| o.strip_prefix(OPT_MKFS_SIZE))
        {
            warn!(
                logger,
                "MKFS size option is specified but not implemented in this example; ignoring size limit"
            );
        }

        // Add the device path
        mkfs_args.push(dev_path.clone());

        // Execute mkfs command based on filesystem type
        let mkfs_cmd = match actual_fstype.as_str() {
            EXT4_TYPE => "mkfs.ext4",
            "xfs" => "mkfs.xfs",
            "btrfs" => "mkfs.btrfs",
            _ => {
                return Err(anyhow!(
                    "unsupported filesystem type for mkfs: '{}'",
                    actual_fstype
                ));
            }
        };

        info!(
            logger,
            "Executing mkfs command";
            "cmd" => mkfs_cmd,
            "args" => mkfs_args.join(" "),
        );

        let output = std::process::Command::new(mkfs_cmd)
            .args(&mkfs_args)
            .output()
            .context(format!("failed to execute {} command", mkfs_cmd))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("{} command failed: {}", mkfs_cmd, stderr));
        }

        info!(
            logger,
            "Successfully formatted device";
            "device" => &dev_path,
            "fstype" => &actual_fstype,
        );
    }

    let (flags, options) = if actual_fstype == EROFS_TYPE {
        info!(
            logger,
            "Mounting EROFS layer";
            "device" => &layer.source,
            "devname" => &dev_path,
            "mount-point" => layer_mount.display(),
        );
        // EROFS layers must be mounted read-only
        (nix::mount::MsFlags::MS_RDONLY, "ro".to_string())
    } else {
        // For non-EROFS layers, we can apply any specified mount options.
        // Filter out X-kata.* custom options before mount
        let mount_options: Vec<String> = layer
            .options
            .iter()
            .filter(|o| !o.starts_with("X-kata."))
            .cloned()
            .collect();
        info!(
            logger,
            "Mounting rwlayer";
            "device" => &layer.source,
            "devname" => &dev_path,
            "original-options" => layer.options.join(","),
            "mount-point" => layer_mount.display(),
        );
        kata_sys_util::mount::parse_mount_options(&mount_options)?
    };

    baremount(
        Path::new(&dev_path),
        layer_mount,
        &actual_fstype,
        flags,
        options.as_str(),
        logger,
    )
    .context("failed to mount layer")?;

    // After successfully mounting the layer, we track the mount point for cleanup.
    track_temporary_mount_for_cleanup(sandbox, layer_mount, logger).await?;

    // No LUKS encryption in this path (CDH was not available or not mkfs type)
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // --- validate_container_id ---

    #[rstest]
    #[case("abc123", true)]
    #[case("container-id-with-dashes", true)]
    #[case("UPPER", true)]
    #[case("a", true)]
    #[case("", false)]
    #[case("../escape", false)]
    #[case("foo/bar", false)]
    #[case("foo\\bar", false)]
    #[case("foo\0bar", false)]
    #[case("a..b", false)]
    fn test_validate_container_id(#[case] cid: &str, #[case] should_pass: bool) {
        let result = validate_container_id(cid);
        assert_eq!(
            result.is_ok(),
            should_pass,
            "validate_container_id({:?}) = {:?}",
            cid,
            result
        );
    }

    // --- validate_mount_point ---

    #[rstest]
    #[case("/mnt/foo", true)]
    #[case("/", true)]
    #[case("/a/b/c", true)]
    #[case("", false)]
    #[case("relative/path", false)]
    #[case("/mnt/../escape", false)]
    #[case("/mnt/a..b", false)]
    fn test_validate_mount_point(#[case] mp: &str, #[case] should_pass: bool) {
        let result = validate_mount_point(mp);
        assert_eq!(
            result.is_ok(),
            should_pass,
            "validate_mount_point({:?}) = {:?}",
            mp,
            result
        );
    }

    // --- parse_mkdir_directive ---

    #[rstest]
    #[case("some/path", true, "some/path", None)]
    #[case("some/path:0755", true, "some/path", Some("0755"))]
    #[case("path:mode:extra", true, "path", Some("mode:extra"))]
    #[case("", false, "", None)]
    fn test_parse_mkdir_directive(
        #[case] spec: &str,
        #[case] should_pass: bool,
        #[case] expected_path: &str,
        #[case] expected_mode: Option<&str>,
    ) {
        let result = parse_mkdir_directive(spec);
        if should_pass {
            let d = result.expect("expected Ok");
            assert_eq!(d.raw_path, expected_path);
            assert_eq!(d.mode.as_deref(), expected_mode);
        } else {
            assert!(result.is_err(), "expected Err for spec {:?}", spec);
        }
    }

    #[test]
    fn test_parse_mkdir_directive_rejects_null_bytes() {
        assert!(parse_mkdir_directive("foo\0bar").is_err());
    }

    // --- resolve_mkdir_path ---

    #[test]
    fn test_resolve_mkdir_path_upper_only() {
        let upper = PathBuf::from("/tmp/test-upper");
        std::fs::create_dir_all(&upper).unwrap();

        let result = resolve_mkdir_path("{{ mount 0 }}/subdir", &upper, None);
        let resolved = result.expect("expected Ok");
        assert!(
            resolved.starts_with(&upper),
            "resolved path {:?} should be under upper {:?}",
            resolved,
            upper
        );
        assert!(resolved.ends_with("subdir"));

        let _ = std::fs::remove_dir_all(&upper);
    }

    #[test]
    fn test_resolve_mkdir_path_with_lower() {
        let upper = PathBuf::from("/tmp/test-resolve-upper");
        let lower = PathBuf::from("/tmp/test-resolve-lower");
        std::fs::create_dir_all(&upper).unwrap();
        std::fs::create_dir_all(&lower).unwrap();

        let result = resolve_mkdir_path("{{ mount 1 }}/data", &upper, Some(&lower));
        let resolved = result.expect("expected Ok");
        assert!(
            resolved.starts_with(&lower),
            "resolved path {:?} should be under lower {:?}",
            resolved,
            lower
        );

        let _ = std::fs::remove_dir_all(&upper);
        let _ = std::fs::remove_dir_all(&lower);
    }

    // --- is_upper_storage / is_lower_storage ---

    #[test]
    fn test_is_upper_storage() {
        let mut s = Storage::default();
        assert!(!is_upper_storage(&s));

        s.options.push(OPT_OVERLAY_UPPER.to_string());
        assert!(is_upper_storage(&s));

        let s2 = Storage {
            fstype: EXT4_TYPE.to_string(),
            options: vec![OPT_MULTI_LAYER.to_string()],
            ..Default::default()
        };
        assert!(is_upper_storage(&s2));
    }

    #[test]
    fn test_is_lower_storage() {
        let mut s = Storage::default();
        assert!(!is_lower_storage(&s));

        s.options.push(OPT_OVERLAY_LOWER.to_string());
        assert!(is_lower_storage(&s));

        let s2 = Storage {
            fstype: EROFS_TYPE.to_string(),
            options: vec![OPT_MULTI_LAYER.to_string()],
            ..Default::default()
        };
        assert!(is_lower_storage(&s2));
    }

    // --- is_multi_layer_storage ---

    #[rstest]
    #[case(vec![], "", false)]
    #[case(vec![OPT_MULTI_LAYER.to_string()], "", true)]
    #[case(vec![], DRIVER_MULTI_LAYER_EROFS, true)]
    #[case(vec!["ro".to_string()], "virtio-blk", false)]
    fn test_is_multi_layer_storage(
        #[case] options: Vec<String>,
        #[case] driver: &str,
        #[case] expected: bool,
    ) {
        let s = Storage {
            options,
            driver: driver.to_string(),
            ..Default::default()
        };
        assert_eq!(
            is_multi_layer_storage(&s),
            expected,
            "is_multi_layer_storage with driver={:?}, options={:?}",
            s.driver,
            s.options
        );
    }
}
