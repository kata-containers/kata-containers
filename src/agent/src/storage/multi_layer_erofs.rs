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
    // Validate container ID to prevent path traversal via crafted cid values
    validate_container_id(cid_str)?;
    let temp_base = PathBuf::from(format!("/run/kata-containers/{}/multi-layer", cid_str));
    fs::create_dir_all(&temp_base).context("failed to create temp mount base")?;

    // Validate mount point to prevent path traversal via crafted mount_point values
    validate_mount_point(&ext4.mount_point)?;

    let upper_mount = temp_base.join("upper");
    fs::create_dir_all(&upper_mount).context("failed to create upper mount dir")?;

    wait_and_mount_layer(ext4, &upper_mount, sandbox, &logger).await?;

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
}

fn is_lower_storage(storage: &Storage) -> bool {
    storage.options.iter().any(|o| o == OPT_OVERLAY_LOWER)
        || (storage.fstype == EROFS_TYPE && storage.options.iter().any(|o| o == OPT_MULTI_LAYER))
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

    // After successfully mounting the layer, we track the mount point for cleanup.
    track_temporary_mount_for_cleanup(sandbox, layer_mount, logger).await?;

    Ok(())
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
