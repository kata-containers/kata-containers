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

use crate::device::block_device_handler::get_virtio_blk_pci_device_name;
use crate::linux_abi::pcipath_from_dev_tree_path;
use crate::mount::baremount;
use crate::sandbox::Sandbox;
use crate::storage::{StorageContext, StorageHandler};
use anyhow::{anyhow, Context, Result};
use kata_sys_util::mount::create_mount_destination;
use kata_types::mount::StorageDevice;
use protocols::agent::Storage;
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

#[derive(Debug)]
pub struct MultiLayerErofsHandler {}

#[derive(Debug, Clone)]
pub struct MultiLayerErofsResult {
    pub mount_point: String,
    pub processed_mount_points: Vec<String>,
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
    let temp_base = PathBuf::from(format!("/run/kata-containers/{}/multi-layer", cid_str));
    fs::create_dir_all(&temp_base).context("failed to create temp mount base")?;

    let upper_mount = temp_base.join("upper");
    fs::create_dir_all(&upper_mount).context("failed to create upper mount dir")?;

    wait_and_mount_layer(ext4, &upper_mount, sandbox, &logger).await?;

    for mkdir_dir in &mkdir_dirs {
        // As {{ mount 1 }} refers to the first lower layer, which is not available until we mount it.
        // Just skip it for now and handle it in a second pass after mounting the lower layers.
        if mkdir_dir.raw_path.contains("{{ mount 1 }}") {
            continue;
        }
        let resolved_path = resolve_mkdir_path(&mkdir_dir.raw_path, &upper_mount, None);
        info!(
            logger,
            "Creating mkdir directory in upper layer";
            "raw-path" => &mkdir_dir.raw_path,
            "resolved-path" => &resolved_path,
        );

        fs::create_dir_all(&resolved_path)
            .with_context(|| format!("failed to create mkdir directory: {}", resolved_path))?;
    }

    let mut lower_mounts = Vec::new();
    for (index, erofs) in erofs_storages.iter().enumerate() {
        let lower_mount = temp_base.join(format!("lower-{}", index));
        fs::create_dir_all(&lower_mount).with_context(|| {
            format!("failed to create lower mount dir {}", lower_mount.display())
        })?;

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
                resolve_mkdir_path(&mkdir_dir.raw_path, &upper_mount, Some(first_lower));
            info!(
                logger,
                "Creating deferred mkdir directory";
                "raw-path" => &mkdir_dir.raw_path,
                "resolved-path" => &resolved_path,
            );

            fs::create_dir_all(&resolved_path).with_context(|| {
                format!(
                    "failed to create deferred mkdir directory: {}",
                    resolved_path
                )
            })?;
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

    Ok(MultiLayerErofsResult {
        mount_point: ext4.mount_point.clone(),
        processed_mount_points,
    })
}

fn is_upper_storage(storage: &Storage) -> bool {
    storage.options.iter().any(|o| o == OPT_OVERLAY_UPPER)
        || (storage.fstype == EXT4_TYPE && storage.options.iter().any(|o| o == OPT_MULTI_LAYER))
}

fn is_lower_storage(storage: &Storage) -> bool {
    storage.options.iter().any(|o| o == OPT_OVERLAY_LOWER)
        || (storage.fstype == EROFS_TYPE && storage.options.iter().any(|o| o == OPT_MULTI_LAYER))
}

fn parse_mkdir_directive(spec: &str) -> Result<MkdirDirective> {
    let parts: Vec<&str> = spec.splitn(2, ':').collect();
    if parts.is_empty() || parts[0].is_empty() {
        return Err(anyhow!("invalid X-kata.mkdir.path directive: '{}'", spec));
    }

    Ok(MkdirDirective {
        raw_path: parts[0].to_string(),
        mode: parts.get(1).map(|s| s.to_string()),
    })
}

fn resolve_mkdir_path(
    raw_path: &str,
    upper_mount: &Path,
    first_lower_mount: Option<&Path>,
) -> String {
    let mut resolved = raw_path.replace("{{ mount 0 }}", upper_mount.to_str().unwrap_or(""));

    if let Some(lower) = first_lower_mount {
        resolved = resolved.replace("{{ mount 1 }}", lower.to_str().unwrap_or(""));
    }

    resolved
}

async fn wait_and_mount_layer(
    layer: &Storage,
    layer_mount: &Path,
    sandbox: &Arc<Mutex<Sandbox>>,
    logger: &Logger,
) -> Result<()> {
    let (root_complex, pcipath) = pcipath_from_dev_tree_path(&layer.source)?;
    let dev_path = get_virtio_blk_pci_device_name(sandbox, root_complex, &pcipath).await?;

    info!(
        logger,
        "Mounting layer";
        "device" => &layer.source,
        "fstype" => &layer.fstype,
        "devname" => &dev_path,
        "mount-point" => layer_mount.display(),
    );

    create_mount_destination(Path::new(&dev_path), layer_mount, "", &layer.fstype)
        .context("failed to create layer mount destination")?;

    let (flags, options) = if layer.fstype == EROFS_TYPE {
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
        &layer.fstype,
        flags,
        options.as_str(),
        logger,
    )
    .context("failed to mount layer")?;

    Ok(())
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
