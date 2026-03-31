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
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use kata_types::mount::StorageDevice;
use protocols::agent::Storage;
use tokio::sync::Mutex;

use crate::device::BLOCK;
use crate::mount::baremount;
use crate::sandbox::Sandbox;
use crate::storage::{StorageContext, StorageHandler};
use crate::uevent::{wait_for_uevent, Uevent, UeventMatcher};
use kata_sys_util::mount::create_mount_destination;
use regex::Regex;
use slog::Logger;

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

#[derive(Debug)]
struct VirtioBlkMatcher {
    rex: Regex,
}

impl VirtioBlkMatcher {
    fn new(devname: &str) -> Self {
        let re = format!(r"/virtio[0-9]+/block/{}$", devname);
        VirtioBlkMatcher {
            rex: Regex::new(&re).expect("Failed to compile VirtioBlkMatcher regex"),
        }
    }
}

impl UeventMatcher for VirtioBlkMatcher {
    fn is_match(&self, uev: &Uevent) -> bool {
        uev.subsystem == BLOCK && self.rex.is_match(&uev.devpath) && !uev.devname.is_empty()
    }
}

#[derive(Debug)]
pub struct MultiLayerErofsHandler {}

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
        slog::info!(
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

/// Handle multi-layer EROFS storage by combining multiple storages
pub async fn handle_multi_layer_erofs(
    storages: &[&Storage],
    cid: &Option<String>,
    sandbox: &Arc<Mutex<Sandbox>>,
    logger: &Logger,
) -> Result<String> {
    let logger = logger.new(o!("subsystem" => "multi-layer-erofs"));

    // Find ext4 (upper) and erofs (lower) storages
    let mut ext4_storage: Option<&Storage> = None;
    let mut erofs_storages: Vec<&Storage> = Vec::new();
    let mut mkdir_dirs: Vec<(String, Option<String>)> = Vec::new(); // (path, mode)

    for storage in storages {
        if storage.options.iter().any(|o| o == OPT_OVERLAY_UPPER)
            || (storage.fstype == EXT4_TYPE && storage.options.iter().any(|o| o == OPT_MULTI_LAYER))
        {
            ext4_storage = Some(*storage);

            // Extract mkdir directories from X-kata.mkdir.path options
            for opt in &storage.options {
                if let Some(mkdir_spec) = opt.strip_prefix(OPT_MKDIR_PATH) {
                    // Format: path:mode or path:mode:uid:gid
                    let parts: Vec<&str> = mkdir_spec.splitn(2, ':').collect();
                    if !parts.is_empty() {
                        let path = parts[0].to_string();
                        let mode = parts.get(1).map(|s| s.to_string());
                        mkdir_dirs.push((path, mode));
                    }
                }
            }
        } else if storage.options.iter().any(|o| o == OPT_OVERLAY_LOWER)
            || (storage.fstype == EROFS_TYPE
                && storage.options.iter().any(|o| o == OPT_MULTI_LAYER))
        {
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

    slog::info!(
        logger,
        "handling multi-layer erofs";
        "ext4-device" => &ext4.source,
        "erofs-devices" => erofs_storages.iter().map(|s| s.source.as_str()).collect::<Vec<_>>().join(", "),
        "mount-point" => &ext4.mount_point,
        "ext4-fstype" => &ext4.fstype,
        "ext4-options" => ext4.options.join(","),
        "mkdir-dirs-count" => mkdir_dirs.len(),
    );

    // Create temporary mount points for upper and lower layers
    let cid_str = cid.as_deref().unwrap_or("sandbox");
    let temp_base =
        std::path::PathBuf::from(format!("/run/kata-containers/{}/multi-layer", cid_str));
    fs::create_dir_all(&temp_base).context("failed to create temp mount base")?;

    let upper_mount = temp_base.join("upper");
    let lower_mount = temp_base.join("lower");

    // work_dir MUST be inside the ext4 mount for overlay to work
    // It will be set to upper_mount/work after ext4 is mounted

    slog::info!(
        logger,
        "created temp mount directories";
        "temp-base" => temp_base.display(),
        "upper-mount" => upper_mount.display(),
        "lower-mount" => lower_mount.display(),
    );

    fs::create_dir_all(&upper_mount).context("failed to create upper mount dir")?;
    fs::create_dir_all(&lower_mount).context("failed to create lower mount dir")?;

    // Wait for block devices to be ready before mounting
    // Extract device name from source (e.g., /dev/vda -> vda)
    let ext4_devname = extract_device_name(&ext4.source)?;
    slog::info!(
        logger,
        "waiting for ext4 block device to be ready";
        "device" => &ext4.source,
        "devname" => &ext4_devname,
    );

    let matcher = VirtioBlkMatcher::new(&ext4_devname);
    wait_for_uevent(sandbox, matcher)
        .await
        .context("timeout waiting for ext4 block device")?;

    // Step 1: Mount upper layer (ext4)
    slog::info!(
        logger,
        "mounting ext4 upper layer";
        "device" => &ext4.source,
        "fstype" => &ext4.fstype,
        "mount-point" => upper_mount.display(),
        "options" => ext4.options.join(","),
    );

    create_mount_destination(Path::new(&ext4.source), &upper_mount, "", &ext4.fstype)
        .context("failed to create upper mount destination")?;

    // Filter out X-kata.* custom options before mount
    // These are metadata markers, not actual mount options
    let mount_options: Vec<String> = ext4
        .options
        .iter()
        .filter(|o| !o.starts_with("X-kata."))
        .cloned()
        .collect();

    slog::info!(
        logger,
        "filtered ext4 mount options";
        "original-options" => ext4.options.join(","),
        "mount-options" => mount_options.join(","),
    );

    let (flags, options) = kata_sys_util::mount::parse_mount_options(&mount_options)?;
    baremount(
        Path::new(&ext4.source),
        &upper_mount,
        &ext4.fstype,
        flags,
        options.as_str(),
        &logger,
    )
    .context("failed to mount ext4 upper layer")?;

    slog::info!(
        logger,
        "ext4 upper layer mounted successfully";
        "mount-point" => upper_mount.display(),
    );

    // Step 2: Create mkdir directories specified in X-kata.mkdir.path options
    for (raw_path, _mode) in &mkdir_dirs {
        // Resolve template variables like {{ mount 0 }}/upper
        // Currently we only support {{ mount 0 }} which references upper_mount
        let resolved_path = if raw_path.contains("{{ mount 0 }}") {
            raw_path.replace("{{ mount 0 }}", upper_mount.to_str().unwrap_or(""))
        } else if raw_path.contains("{{ mount 1 }}") {
            // This will be resolved after erofs mount
            slog::warn!(
                logger,
                "mkdir path references unmounted layer, deferring creation";
                "path" => raw_path
            );
            continue;
        } else {
            raw_path.clone()
        };

        slog::info!(
            logger,
            "creating mkdir directory";
            "raw-path" => raw_path,
            "resolved-path" => &resolved_path,
        );

        // Create directory with default permissions (0755)
        // Mode parsing can be added in future if needed
        fs::create_dir_all(&resolved_path).context(format!(
            "failed to create mkdir directory: {}",
            resolved_path
        ))?;
    }

    // Step 3: Mount lower layers (erofs)
    let erofs = erofs_storages[0];

    // Wait for erofs block device to be ready before mounting
    let erofs_devname = extract_device_name(&erofs.source)?;
    slog::info!(
        logger,
        "waiting for erofs block device to be ready";
        "device" => &erofs.source,
        "devname" => &erofs_devname,
    );

    let matcher = VirtioBlkMatcher::new(&erofs_devname);
    wait_for_uevent(sandbox, matcher)
        .await
        .context("timeout waiting for erofs block device")?;

    slog::info!(
        logger,
        "block device is ready, mounting erofs lower layer";
        "device" => &erofs.source,
        "mount-point" => lower_mount.display(),
    );

    create_mount_destination(Path::new(&erofs.source), &lower_mount, "", EROFS_TYPE)
        .context("failed to create lower mount destination")?;

    baremount(
        Path::new(&erofs.source),
        &lower_mount,
        EROFS_TYPE,
        nix::mount::MsFlags::MS_RDONLY,
        "ro",
        &logger,
    )
    .context("failed to mount erofs lower layer")?;

    // Step 4: Create upperdir and workdir within the ext4 mount
    // Both MUST be on the same filesystem (the ext4 mount)
    // mkdir step already created {{ mount 0 }}/upper, so upperdir is upper_mount/upper
    let upperdir = upper_mount.join("upper");
    let workdir = upper_mount.join("work");

    // Ensure upperdir exists (mkdir step should have created it, but check again)
    if !upperdir.exists() {
        fs::create_dir_all(&upperdir).context("failed to create upperdir")?;
    }
    // workdir MUST be created inside ext4 mount, not in temp_base
    fs::create_dir_all(&workdir).context("failed to create workdir")?;

    // Step 5: Create overlay mount at final mount_point
    slog::info!(
        logger,
        "creating overlay mount";
        "upperdir" => upperdir.display(),
        "lowerdir" => lower_mount.display(),
        "workdir" => workdir.display(),
        "target" => &ext4.mount_point,
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
        lower_mount.display(),
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

    slog::info!(
        logger,
        "multi-layer erofs overlay mounted successfully";
        "mount-point" => &ext4.mount_point,
    );

    Ok(ext4.mount_point.clone())
}

/// Extract device name from a device path
///
/// Examples:
/// - "/dev/vda" -> "vda"
/// - "/dev/vdb" -> "vdb"
fn extract_device_name(device_path: &str) -> Result<String> {
    device_path
        .strip_prefix("/dev/")
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("device path '{}' must start with /dev/", device_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_types() {
        let handler = MultiLayerErofsHandler {};
        assert_eq!(handler.driver_types(), &[DRIVER_MULTI_LAYER_EROFS]);
    }

    #[test]
    fn test_constants() {
        assert_eq!(OPT_OVERLAY_UPPER, "X-kata.overlay-upper");
        assert_eq!(OPT_OVERLAY_LOWER, "X-kata.overlay-lower");
        assert_eq!(OPT_MULTI_LAYER, "X-kata.multi-layer=true");
        assert_eq!(OPT_MKDIR_PATH, "X-kata.mkdir.path=");
    }
}
