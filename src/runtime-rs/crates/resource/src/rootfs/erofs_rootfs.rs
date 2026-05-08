// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
// Handle multi-layer EROFS rootfs.
//
// The containerd erofs snapshotter sends the active snapshot as either:
// - ext4 rwlayer.img + erofs lower + overlay when host rw backing is enabled.
// - erofs lower + overlay when default_size="0"; the agent then uses a
//   guest-memory upper directory under /run.
//
// The overlay mount may be handled by the guest agent if it contains "{{"
// templates in upperdir/workdir.

use super::{Rootfs, ROOTFS};
use crate::share_fs::{do_get_guest_path, do_get_host_path};
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_device_info, DeviceManager},
        DeviceConfig, DeviceType,
    },
    BlockConfig, BlockDeviceAio, BlockDeviceFormat, KATA_SCSI_DEV_TYPE,
};
use kata_types::device::{
    DRIVER_BLK_CCW_TYPE as KATA_CCW_DEV_TYPE, DRIVER_BLK_PCI_TYPE as KATA_BLK_DEV_TYPE,
};
use kata_types::gpt_disk::{
    extract_snapshot_id, generate_gpt_metadata, generate_padding_file, get_erofs_layer_size,
    ErofsLayer, GptDiskLayout, GptMetadataFiles,
};
use kata_types::mount::Mount;
use oci_spec::runtime as oci;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// EROFS rootfs type identifier
pub(crate) const EROFS_ROOTFS_TYPE: &str = "erofs";
/// RW layer rootfs type identifier, used for multi-layer EROFS as the writable upper layer
/// Typically ext4 format, but can be extended to other fs types in the future.
pub(crate) const RW_LAYER_ROOTFS_TYPE: &str = "ext4";
/// VMDK file extension for merged EROFS image
const EROFS_MERGED_VMDK: &str = "merged_fs.vmdk";

/// Maximum number of rootfs layer devices (erofs + rw layer) allowed in multi-layer EROFS mode.
/// This is a pre-flight sanity check before VMDK merging, to prevent excessive block devices
/// when many layers are used without fsmerge.
const MAX_ROOTFS_LAYER_DEVICES: usize = 129; // 128 EROFS layers + 1 rw layer (129 total)
/// Maximum sectors per 2GB extent (2GB / 512 bytes per sector)
const MAX_2GB_EXTENT_SECTORS: u64 = 0x8000_0000 >> 9;
/// Sectors per track for VMDK geometry
const SECTORS_PER_TRACK: u64 = 63;
/// Number of heads for VMDK geometry
const NUMBER_HEADS: u64 = 16;
/// VMDK subformat type (twoGbMaxExtentFlat for large files)
const VMDK_SUBFORMAT: &str = "twoGbMaxExtentFlat";
/// VMDK adapter type
const VMDK_ADAPTER_TYPE: &str = "ide";
/// VMDK hardware version
const VMDK_HW_VERSION: &str = "4";
/// Default shared directory for guest rootfs VMDK files (for multi-layer EROFS)
pub(crate) const DEFAULT_KATA_GUEST_ROOT_SHARED_FS: &str = "/run/kata-containers/";
/// Template for mkdir option in overlay mount (X-containerd.mkdir.path)
const X_CONTAINERD_MKDIR_PATH: &str = "X-containerd.mkdir.path=";
/// Template for mkdir option passed to guest agent (X-kata.mkdir.path)
const X_KATA_MKDIR_PATH: &str = "X-kata.mkdir.path=";

/// Create the per-container directory under the shared filesystem root.
pub(crate) fn ensure_container_dir(sid: &str, cid: &str) -> Result<PathBuf> {
    let dir = PathBuf::from(kata_types::build_path(DEFAULT_KATA_GUEST_ROOT_SHARED_FS))
        .join(sid)
        .join(cid);
    fs::create_dir_all(&dir).context(format!(
        "failed to create container directory: {}",
        dir.display()
    ))?;

    Ok(dir)
}

/// Generate merged VMDK file from multiple EROFS devices
///
/// Creates a VMDK descriptor that combines multiple EROFS images into a single
/// virtual block device (flatten device). For a single device, the EROFS image
/// is used directly without a VMDK wrapper.
///
/// And `erofs_devices` are for host paths to EROFS image files (from `source` and `device=` options)
async fn generate_merged_erofs_vmdk(
    sid: &str,
    cid: &str,
    erofs_devices: &[String],
) -> Result<(String, BlockDeviceFormat)> {
    if erofs_devices.is_empty() {
        return Err(anyhow!("no EROFS devices provided"));
    }

    // Validate all device paths exist and are regular files before proceeding.
    for dev_path in erofs_devices {
        let metadata = fs::metadata(dev_path)
            .context(format!("EROFS device path not accessible: {}", dev_path))?;
        if !metadata.is_file() {
            return Err(anyhow!(
                "EROFS device path is not a regular file: {}",
                dev_path
            ));
        }
    }

    // For single device, use it directly with Raw format (no need for VMDK descriptor)
    if erofs_devices.len() == 1 {
        info!(
            sl!(),
            "single EROFS device, using directly with Raw format: {}", erofs_devices[0]
        );
        return Ok((erofs_devices[0].clone(), BlockDeviceFormat::Raw));
    }

    // For multiple devices, create VMDK descriptor
    let container_dir = ensure_container_dir(sid, cid)?;
    let vmdk_path = container_dir.join(EROFS_MERGED_VMDK);

    info!(
        sl!(),
        "creating VMDK descriptor for {} EROFS devices: {}",
        erofs_devices.len(),
        vmdk_path.display()
    );

    // create_vmdk_descriptor uses atomic write (temp + rename) internally,
    // so a failure will not leave a corrupt descriptor file.
    create_vmdk_descriptor(&vmdk_path, erofs_devices)
        .context("failed to create VMDK descriptor")?;

    Ok((vmdk_path.display().to_string(), BlockDeviceFormat::Vmdk))
}

/// Helper struct for writing VMDK descriptor files atomically.
///
/// Encapsulates the common VMDK descriptor format: header, extent descriptions,
/// DDB footer, and atomic write (temp file + rename). Used by both fsmerge mode
/// (`create_vmdk_descriptor`) and GPT mode (`create_gpt_vmdk_descriptor`).
struct VmdkDescriptorWriter {
    writer: BufWriter<fs::File>,
    temp_path: PathBuf,
    final_path: PathBuf,
}

impl VmdkDescriptorWriter {
    fn new(vmdk_path: &Path) -> Result<Self> {
        let temp_path = vmdk_path.with_extension("vmdk.tmp");
        if temp_path.components().any(|c| c == Component::ParentDir) {
            return Err(anyhow!("Invalid input: {}", temp_path.display()));
        }
        let file = fs::File::create(&temp_path).context(format!(
            "failed to create temp VMDK file: {}",
            temp_path.display()
        ))?;
        let mut writer = BufWriter::new(file);

        writeln!(writer, "# Disk DescriptorFile")?;
        writeln!(writer, "version=1")?;
        writeln!(writer, "CID=fffffffe")?;
        writeln!(writer, "parentCID=ffffffff")?;
        writeln!(writer, "createType=\"{}\"", VMDK_SUBFORMAT)?;
        writeln!(writer)?;
        writeln!(writer, "# Extent description")?;

        Ok(Self {
            writer,
            temp_path,
            final_path: vmdk_path.to_path_buf(),
        })
    }

    // Write a single extent line (no 2GB chunking).
    fn write_extent(&mut self, path: &str, sectors: u64, file_offset: u64) -> Result<()> {
        writeln!(
            self.writer,
            "RW {} FLAT \"{}\" {}",
            sectors, path, file_offset
        )?;
        Ok(())
    }

    // Write extent lines with 2GB chunking for large files.
    fn write_extent_chunked(&mut self, path: &str, total_sectors: u64) -> Result<()> {
        let mut remaining = total_sectors;
        let mut file_offset: u64 = 0;
        while remaining > 0 {
            let chunk = remaining.min(MAX_2GB_EXTENT_SECTORS);
            self.write_extent(path, chunk, file_offset)?;
            file_offset += chunk;
            remaining -= chunk;
        }
        Ok(())
    }

    // Write DDB footer, flush, and atomically rename to final path.
    fn finalize(mut self, total_sectors: u64) -> Result<()> {
        writeln!(self.writer)?;

        let cylinders = total_sectors.div_ceil(SECTORS_PER_TRACK * NUMBER_HEADS);

        writeln!(self.writer, "# The Disk Data Base")?;
        writeln!(self.writer, "#DDB")?;
        writeln!(self.writer)?;
        writeln!(
            self.writer,
            "ddb.virtualHWVersion = \"{}\"",
            VMDK_HW_VERSION
        )?;
        writeln!(self.writer, "ddb.geometry.cylinders = \"{}\"", cylinders)?;
        writeln!(self.writer, "ddb.geometry.heads = \"{}\"", NUMBER_HEADS)?;
        writeln!(
            self.writer,
            "ddb.geometry.sectors = \"{}\"",
            SECTORS_PER_TRACK
        )?;
        writeln!(self.writer, "ddb.adapterType = \"{}\"", VMDK_ADAPTER_TYPE)?;

        self.writer
            .flush()
            .context("failed to flush VMDK descriptor")?;
        drop(self.writer);

        fs::rename(&self.temp_path, &self.final_path).context(format!(
            "failed to rename temp VMDK {} -> {}",
            self.temp_path.display(),
            self.final_path.display()
        ))?;

        Ok(())
    }
}

/// Create VMDK descriptor for multiple EROFS extents (flatten device)
///
/// Generates a VMDK descriptor file (twoGbMaxExtentFlat format) that references
/// multiple EROFS images as flat extents, allowing them to be treated as a single
/// contiguous block device in the VM.
fn create_vmdk_descriptor(vmdk_path: &Path, erofs_paths: &[String]) -> Result<()> {
    if erofs_paths.is_empty() {
        return Err(anyhow!(
            "empty EROFS path list, cannot create VMDK descriptor"
        ));
    }

    struct ExtentInfo {
        path: String,
        total_sectors: u64,
    }

    let mut extents: Vec<ExtentInfo> = Vec::with_capacity(erofs_paths.len());
    let mut total_sectors: u64 = 0;

    for erofs_path in erofs_paths {
        let metadata = fs::metadata(erofs_path)
            .context(format!("failed to stat EROFS file: {}", erofs_path))?;

        let file_size = metadata.len();
        if file_size == 0 {
            warn!(sl!(), "EROFS file {} is zero-length, skipping", erofs_path);
            continue;
        }

        let sectors = file_size.div_ceil(512);

        if file_size % 512 != 0 {
            warn!(
                sl!(),
                "EROFS file {} size ({} bytes) is not 512-byte aligned, \
                 rounding up to {} sectors ({} bytes addressable)",
                erofs_path,
                file_size,
                sectors,
                sectors * 512
            );
        }

        total_sectors = total_sectors.checked_add(sectors).ok_or_else(|| {
            anyhow!(
                "total sector count overflow when adding {} ({} sectors)",
                erofs_path,
                sectors
            )
        })?;

        extents.push(ExtentInfo {
            path: erofs_path.clone(),
            total_sectors: sectors,
        });
    }

    if total_sectors == 0 {
        return Err(anyhow!(
            "no valid EROFS files to create VMDK descriptor (all files are empty)"
        ));
    }

    let mut vmdk = VmdkDescriptorWriter::new(vmdk_path)?;
    for extent in &extents {
        vmdk.write_extent_chunked(&extent.path, extent.total_sectors)?;
        info!(
            sl!(),
            "VMDK extent: {} ({} sectors, {} extent chunk(s))",
            extent.path,
            extent.total_sectors,
            extent.total_sectors.div_ceil(MAX_2GB_EXTENT_SECTORS)
        );
    }

    vmdk.finalize(total_sectors)?;

    info!(
        sl!(),
        "VMDK descriptor created: {} (total {} sectors, {} extents)",
        vmdk_path.display(),
        total_sectors,
        extents.len()
    );

    Ok(())
}

/// Generate GPT-partitioned VMDK and return layout information for per-partition storage creation
///
/// Returns: (vmdk_path, BlockDeviceFormat::Vmdk, GptDiskLayout, GptMetadataFiles)
fn generate_gpt_vmdk_with_layout(
    sid: &str,
    cid: &str,
    erofs_layers: Vec<ErofsLayer>,
) -> Result<(String, BlockDeviceFormat, GptDiskLayout, GptMetadataFiles)> {
    if erofs_layers.is_empty() {
        return Err(anyhow!("no EROFS layers provided for GPT VMDK generation"));
    }

    // Validate all layer paths exist and are regular files
    for layer in &erofs_layers {
        let metadata = fs::metadata(&layer.path)
            .context(format!("EROFS layer path not accessible: {}", layer.path))?;
        if !metadata.is_file() {
            return Err(anyhow!(
                "EROFS layer path is not a regular file: {}",
                layer.path
            ));
        }
    }

    // Create container directory
    let container_dir = ensure_container_dir(sid, cid)?;
    let vmdk_path = container_dir.join(EROFS_MERGED_VMDK);

    info!(
        sl!(),
        "creating GPT-partitioned VMDK for {} EROFS layers: {}",
        erofs_layers.len(),
        vmdk_path.display()
    );

    // Generate GPT metadata files
    let (layout, mut gpt_files) = generate_gpt_metadata(sid, cid, erofs_layers, &container_dir)
        .context("failed to generate GPT metadata")?;

    // Create VMDK descriptor with GPT layout and collect generated padding paths
    let pad_paths = create_gpt_vmdk_descriptor(&vmdk_path, &layout, &gpt_files)
        .context("failed to create GPT VMDK descriptor")?;
    gpt_files.pad_paths = pad_paths;

    Ok((
        vmdk_path.display().to_string(),
        BlockDeviceFormat::Vmdk,
        layout,
        gpt_files,
    ))
}

/// Create VMDK descriptor for GPT-partitioned disk
///
/// Returns the list of generated padding file paths for cleanup tracking.
fn create_gpt_vmdk_descriptor(
    vmdk_path: &Path,
    layout: &GptDiskLayout,
    gpt_files: &GptMetadataFiles,
) -> Result<Vec<PathBuf>> {
    let mut vmdk = VmdkDescriptorWriter::new(vmdk_path)?;
    let mut pad_paths: Vec<PathBuf> = Vec::new();

    // 1. GPT head metadata
    vmdk.write_extent(
        &gpt_files.head_path.display().to_string(),
        gpt_files.head_sectors,
        0,
    )?;
    info!(
        sl!(),
        "VMDK extent: GPT head ({} sectors) at {}",
        gpt_files.head_sectors,
        gpt_files.head_path.display()
    );

    // 2. Layer extents with padding gaps
    // head ends at LBA 2047, so first gap starts at LBA 2048.
    let mut prev_end_lba = gpt_files.head_sectors - 1;

    let metadata_dir = gpt_files.head_path.parent().ok_or_else(|| {
        anyhow!(
            "GPT head file has no parent directory: {}",
            gpt_files.head_path.display()
        )
    })?;

    for (idx, part) in layout.partitions.iter().enumerate() {
        let gap_start_lba = prev_end_lba + 1;
        if part.start_lba > gap_start_lba {
            let gap_sectors = part.start_lba - gap_start_lba;
            let pad_path = metadata_dir.join(format!("pad-{}.img", idx));

            generate_padding_file(&pad_path, gap_sectors).context(format!(
                "failed to generate padding file: {}",
                pad_path.display()
            ))?;

            vmdk.write_extent(&pad_path.display().to_string(), gap_sectors, 0)?;
            pad_paths.push(pad_path);
        }

        vmdk.write_extent_chunked(&part.layer.path, part.layer.size_sectors)?;
        info!(
            sl!(),
            "VMDK extent: {} (partition {}, LBA {}-{}, {} sectors)",
            part.layer.path,
            part.partition_number,
            part.start_lba,
            part.end_lba,
            part.layer.size_sectors
        );

        prev_end_lba = part.end_lba;
    }

    vmdk.finalize(layout.total_sectors)?;

    info!(
        sl!(),
        "GPT VMDK descriptor created: {} (total {} sectors, {} partitions)",
        vmdk_path.display(),
        layout.total_sectors,
        layout.partitions.len()
    );

    Ok(pad_paths)
}

fn extract_block_device_info(
    device_info: &DeviceType,
    read_only: bool,
) -> Result<(agent::Storage, String)> {
    // storage
    let mut storage = agent::Storage {
        options: if read_only {
            vec!["ro".to_string()]
        } else {
            Vec::new()
        },
        ..Default::default()
    };
    let mut device_id = String::new();
    if let DeviceType::Block(device) = device_info.clone() {
        let blk_driver = device.config.driver_option;
        // blk, mmioblk
        storage.driver = blk_driver.clone();
        storage.source = match blk_driver.as_str() {
            KATA_BLK_DEV_TYPE => {
                if let Some(pci_path) = device.config.pci_path {
                    pci_path.to_string()
                } else {
                    return Err(anyhow!("block driver is blk but no pci path exists"));
                }
            }
            KATA_SCSI_DEV_TYPE => {
                if let Some(scsi_addr) = device.config.scsi_addr {
                    scsi_addr.to_string()
                } else {
                    return Err(anyhow!("block driver is scsi but no scsi address exists"));
                }
            }
            KATA_CCW_DEV_TYPE => {
                if let Some(ccw_addr) = device.config.ccw_addr {
                    ccw_addr.to_string()
                } else {
                    return Err(anyhow!("block driver is ccw but no ccw address exists"));
                }
            }
            _ => device.config.virt_path,
        };
        device_id = device.device_id;
    }

    Ok((storage, device_id))
}

/// EROFS Multi-Layer Rootfs with overlay support
///
/// Handles the EROFS Multi-Layer where rootfs consists of:
/// - Optional ext4 rw disk -> virtio-blk when host rw backing exists.
/// - EROFS layers (fsmeta + flattened layers) -> virtio-blk via VMDK.
/// - Overlay metadata that combines the writable upper with the EROFS lower.
pub(crate) struct ErofsMultiLayerRootfs {
    guest_path: String,
    device_ids: Vec<String>,
    // Writable layer storage (upper layer), typically ext4 and optional when
    // the agent creates a /run-backed upper.
    rwlayer_storage: Option<Storage>,
    // Read-only EROFS layer storages (lower layers), one per partition in GPT mode
    erofs_storages: Vec<Storage>,
    // Path to generated VMDK descriptor (only set when multiple EROFS devices are merged)
    vmdk_path: Option<PathBuf>,
    // Paths to generated GPT metadata files (head, padding) for cleanup
    gpt_metadata_paths: Vec<PathBuf>,
    // Container-scoped runtime directory that may only contain generated helper artifacts.
    generated_artifacts_dir: PathBuf,
}

impl ErofsMultiLayerRootfs {
    pub async fn new(
        device_manager: &RwLock<DeviceManager>,
        sid: &str,
        cid: &str,
        rootfs_mounts: &[Mount],
        _share_fs: &Option<Arc<dyn crate::share_fs::ShareFs>>,
    ) -> Result<Self> {
        let container_path = do_get_guest_path(ROOTFS, cid, false, false);
        let host_path = do_get_host_path(ROOTFS, sid, cid, false, false);

        fs::create_dir_all(&host_path)
            .map_err(|e| anyhow!("failed to create rootfs dir {}: {:?}", host_path, e))?;

        let mut device_ids = Vec::new();
        let mut rwlayer_storage: Option<Storage> = None;
        let mut erofs_storages: Vec<Storage> = Vec::new();
        let mut vmdk_path: Option<PathBuf> = None;
        let mut gpt_metadata_paths: Vec<PathBuf> = Vec::new();
        // Track whether GPT+VMDK erofs layers have already been processed in bulk.
        let mut gpt_erofs_processed = false;

        // Directories to create (X-containerd.mkdir.path)
        let mut mkdir_dirs: Vec<String> = Vec::new();

        let blkdev_info = get_block_device_info(device_manager).await;
        let block_driver = blkdev_info.block_device_driver.clone();

        // Check block device count limit
        let expected_device_count = rootfs_mounts
            .iter()
            .filter(|m| {
                m.fs_type.eq_ignore_ascii_case(RW_LAYER_ROOTFS_TYPE)
                    || m.fs_type.eq_ignore_ascii_case(EROFS_ROOTFS_TYPE)
            })
            .count();

        // TODO(Alex Lyn): fsmerge mode with single erofs mount and multiple device= options
        // may require multiple block devices if containerd does not merge layers into one file.
        // This is a fallback or default mode if fsmerge is not enabled.
        if expected_device_count > MAX_ROOTFS_LAYER_DEVICES {
            return Err(anyhow!(
                "exceeded maximum block devices for multi-layer EROFS: {} > {}",
                expected_device_count,
                MAX_ROOTFS_LAYER_DEVICES
            ));
        }

        // Pre-extract mkdir directives from overlay mounts before the main loop,
        // so they are available regardless of mount ordering.
        for mount in rootfs_mounts {
            if matches!(
                mount.fs_type.as_str(),
                "overlay" | "format/overlay" | "format/mkdir/overlay"
            ) {
                for opt in &mount.options {
                    if let Some(mkdir_spec) = opt.strip_prefix(X_CONTAINERD_MKDIR_PATH) {
                        mkdir_dirs.push(mkdir_spec.to_string());
                    }
                }
            }
        }

        // Process each mount in rootfs_mounts to set up devices and storages
        for mount in rootfs_mounts {
            match mount.fs_type.as_str() {
                fmt if fmt.eq_ignore_ascii_case(RW_LAYER_ROOTFS_TYPE) => {
                    // Mount[0]: rw layer -> virtio-blk device /dev/vdX1
                    info!(
                        sl!(),
                        "multi-layer erofs: adding rw layer: {}", mount.source
                    );

                    let device_config = &mut BlockConfig {
                        driver_option: block_driver.clone(),
                        format: BlockDeviceFormat::Raw, // rw layer should be raw format
                        path_on_host: mount.source.clone(),
                        blkdev_aio: BlockDeviceAio::new(&blkdev_info.block_device_aio),
                        ..Default::default()
                    };

                    let device_info = do_handle_device(
                        device_manager,
                        &DeviceConfig::BlockCfg(device_config.clone()),
                    )
                    .await
                    .context("failed to attach rw block device")?;

                    let (mut rwlayer, device_id) =
                        extract_block_device_info(&device_info, false)
                            .context("failed to get block device for rw layer")?;
                    info!(
                        sl!(),
                        "writable block device attached - device_id: {} guest_path: {}",
                        device_id,
                        rwlayer.source
                    );

                    // Filter out "loop" option which is not needed in VM (device is already /dev/vdX)
                    let mut options: Vec<String> = mount
                        .options
                        .iter()
                        .filter(|o| *o != "loop")
                        .cloned()
                        .collect();

                    // RW layer is the writable upper layer (marked with X-kata.overlay-upper)
                    options.push("X-kata.overlay-upper".to_string());
                    options.push("X-kata.multi-layer=true".to_string());

                    // Set up storage for rw layer (upper layer)
                    rwlayer.fs_type = RW_LAYER_ROOTFS_TYPE.to_string();
                    rwlayer.mount_point = container_path.clone();
                    rwlayer.options = options;

                    rwlayer_storage = Some(rwlayer);
                    device_ids.push(device_id);
                }
                fmt if fmt.eq_ignore_ascii_case(EROFS_ROOTFS_TYPE) => {
                    // Mount[1]: erofs layers -> virtio-blk via VMDK /dev/vdX2
                    //
                    // Two modes are supported:
                    // 1. fsmerge mode: Single erofs mount with `device=` options pointing to additional files.
                    //    This is used when containerd has already merged layers into a single file.
                    // 2. GPT+VMDK mode: Multiple independent erofs mounts (each mount is a separate layer file).
                    //    This is used when containerd does NOT use fsmerge, and we need to create GPT partitions.

                    // In GPT mode, all erofs layers are processed in bulk on the first
                    // encounter. Skip subsequent erofs mounts but continue iterating
                    // so that later ext4 rw-layer and overlay mounts are still handled.
                    if gpt_erofs_processed {
                        info!(
                            sl!(),
                            "multi-layer erofs: skipping already-processed erofs mount: {}",
                            mount.source
                        );
                        continue;
                    }

                    // Collect all EROFS mounts once with their original indices.
                    let erofs_mounts_indexed: Vec<(usize, &Mount)> = rootfs_mounts
                        .iter()
                        .enumerate()
                        .filter(|(_, m)| m.fs_type.eq_ignore_ascii_case(EROFS_ROOTFS_TYPE))
                        .collect();
                    let total_erofs_mounts = erofs_mounts_indexed.len();

                    // GPT+VMDK mode: Multiple independent erofs layer files
                    if total_erofs_mounts > 1 {
                        info!(
                            sl!(),
                            "multi-layer erofs: using GPT+VMDK mode for {} independent layers",
                            total_erofs_mounts
                        );

                        let mut erofs_layers = Vec::new();

                        for (_mount_idx, erofs_mount) in &erofs_mounts_indexed {
                            let layer_path = erofs_mount.source.clone();
                            let size_bytes = get_erofs_layer_size(&layer_path).context(format!(
                                "gptdisk: failed to get size of EROFS layer: {}",
                                layer_path
                            ))?;

                            if size_bytes == 0 {
                                warn!(
                                    sl!(),
                                    "gptdisk: EROFS layer {} is zero-length, skipping", layer_path
                                );
                                continue;
                            }

                            let size_sectors = size_bytes.div_ceil(512);
                            let snapshot_id = extract_snapshot_id(&layer_path);

                            erofs_layers.push(ErofsLayer {
                                path: layer_path,
                                size_sectors,
                                snapshot_id,
                            });
                        }

                        if erofs_layers.is_empty() {
                            return Err(anyhow!(
                                "gptdisk: no valid EROFS layers found for GPT VMDK"
                            ));
                        }

                        // Generate GPT-partitioned VMDK and get layout information
                        let (erofs_path, erofs_format, layout, gpt_files) =
                            generate_gpt_vmdk_with_layout(sid, cid, erofs_layers)
                                .context("gptdisk: failed to generate GPT VMDK")?;

                        // Track VMDK path for cleanup
                        vmdk_path = Some(PathBuf::from(&erofs_path));

                        // Track GPT metadata files (head + padding) for cleanup
                        gpt_metadata_paths.push(gpt_files.head_path.clone());
                        gpt_metadata_paths.extend(gpt_files.pad_paths.iter().cloned());

                        info!(
                            sl!(),
                            "GPT VMDK created - path: {}, format: {:?}, {} partitions",
                            erofs_path,
                            erofs_format,
                            layout.partitions.len()
                        );

                        let device_config = &mut BlockConfig {
                            driver_option: block_driver.clone(),
                            format: erofs_format,
                            path_on_host: erofs_path,
                            is_readonly: true,
                            blkdev_aio: BlockDeviceAio::new(&blkdev_info.block_device_aio),
                            ..Default::default()
                        };

                        let device_info = do_handle_device(
                            device_manager,
                            &DeviceConfig::BlockCfg(device_config.clone()),
                        )
                        .await
                        .context("failed to attach GPT VMDK block device")?;

                        let (base_device, device_id) =
                            extract_block_device_info(&device_info, true)?;
                        info!(
                            sl!(),
                            "GPT VMDK device attached - device_id: {} guest_path: {}",
                            device_id,
                            &base_device.source
                        );

                        device_ids.push(device_id);

                        // Create a storage entry for each GPT partition.
                        for (idx, part) in layout.partitions.iter().enumerate() {
                            let mut rolayer = base_device.clone();
                            let options: Vec<String> = vec![
                                "X-kata.overlay-lower".to_string(),
                                "X-kata.multi-layer=true".to_string(),
                                "X-kata.gpt-partitioned=true".to_string(),
                                format!("X-kata.partition-number={}", part.partition_number),
                            ];

                            rolayer.fs_type = EROFS_ROOTFS_TYPE.to_string();
                            rolayer.mount_point = container_path.clone();
                            rolayer.options = options;
                            rolayer.source = base_device.source.clone();

                            info!(
                                sl!(),
                                "Created storage for GPT partition {} (partition number {}, LBA {}-{})",
                                idx, part.partition_number, part.start_lba, part.end_lba
                            );

                            erofs_storages.push(rolayer);
                        }

                        // Mark GPT erofs as processed so subsequent erofs mounts
                        // in the loop are skipped, while still allowing ext4 and
                        // overlay mounts to be visited.
                        gpt_erofs_processed = true;
                    } else {
                        // fsmerge mode: Single erofs mount with device= options
                        info!(
                            sl!(),
                            "multi-layer erofs: using fsmerge mode for erofs layers: {}",
                            mount.source
                        );

                        // Collect all EROFS devices: source + `device=` options
                        let mut erofs_devices = vec![mount.source.clone()];
                        for opt in &mount.options {
                            if let Some(device_path) = opt.strip_prefix("device=") {
                                erofs_devices.push(device_path.to_string());
                            }
                        }

                        info!(sl!(), "EROFS devices count: {}", erofs_devices.len());

                        // Generate merged VMDK file from all EROFS devices
                        // Returns (path, format) - format is Vmdk for multiple devices, Raw for single device
                        let (erofs_path, erofs_format) =
                            generate_merged_erofs_vmdk(sid, cid, &erofs_devices)
                                .await
                                .context("failed to generate EROFS VMDK")?;

                        // Track VMDK path for cleanup (only when VMDK is actually created)
                        if erofs_format == BlockDeviceFormat::Vmdk {
                            vmdk_path = Some(PathBuf::from(&erofs_path));
                        }

                        info!(
                            sl!(),
                            "EROFS block device config - path: {}, format: {:?}",
                            erofs_path,
                            erofs_format
                        );

                        let device_config = &mut BlockConfig {
                            driver_option: block_driver.clone(),
                            format: erofs_format, // Vmdk for multiple devices, Raw for single device
                            path_on_host: erofs_path,
                            is_readonly: true, // EROFS layers are read-only, must set to avoid "resize" lock errors
                            blkdev_aio: BlockDeviceAio::new(&blkdev_info.block_device_aio),
                            ..Default::default()
                        };

                        let device_info = do_handle_device(
                            device_manager,
                            &DeviceConfig::BlockCfg(device_config.clone()),
                        )
                        .await
                        .context("failed to attach erofs block device")?;

                        let (mut rolayer, device_id) =
                            extract_block_device_info(&device_info, true)?;
                        info!(
                            sl!(),
                            "erofs device attached - device_id: {} guest_path: {}",
                            device_id,
                            &rolayer.source
                        );

                        let mut options: Vec<String> = mount
                            .options
                            .iter()
                            .filter(|o| {
                                // Filter out options that are not valid erofs mount parameters:
                                // 1. "loop" - not needed in VM, device is already /dev/vdX
                                // 2. "device=" prefix - used for VMDK generation only, not for mount
                                // 3. "X-kata." prefix - metadata markers for kata internals
                                *o != "loop"
                                    && !o.starts_with("device=")
                                    && !o.starts_with("X-kata.")
                            })
                            .cloned()
                            .collect();

                        // Erofs layers are read-only lower layers (marked with X-kata.overlay-lower)
                        options.push("X-kata.overlay-lower".to_string());
                        options.push("X-kata.multi-layer=true".to_string());

                        info!(
                            sl!(),
                            "erofs storage options filtered: {:?} -> {:?}", mount.options, options
                        );

                        rolayer.fs_type = EROFS_ROOTFS_TYPE.to_string();
                        rolayer.mount_point = container_path.clone();
                        rolayer.options = options;

                        erofs_storages.push(rolayer);
                        device_ids.push(device_id);
                    }
                }
                fmt if fmt.eq_ignore_ascii_case("overlay")
                    || fmt.eq_ignore_ascii_case("format/overlay")
                    || fmt.eq_ignore_ascii_case("format/mkdir/overlay") =>
                {
                    // Mount[2]: overlay to combine rwlayer (upper) + erofs (lower)
                    // mkdir directives already extracted before the main loop
                    info!(
                        sl!(),
                        "multi-layer erofs: overlay mount (mkdir directives pre-extracted)"
                    );
                }
                _ => {
                    info!(
                        sl!(),
                        "multi-layer erofs: ignoring unknown mount type: {}", mount.fs_type
                    );
                }
            }
        }

        if device_ids.is_empty() {
            return Err(anyhow!("no devices attached for multi-layer erofs rootfs"));
        }

        // Forward overlay mkdir hints on the EROFS Storage only. The guest agent scans
        // every multi-layer storage for X-kata.mkdir.path; attaching here avoids splitting
        // the same metadata across rwlayer vs erofs when an ext4 upper exists.
        let mkdir_options = mkdir_dirs
            .iter()
            .map(|dir| format!("{}{}", X_KATA_MKDIR_PATH, dir))
            .collect::<Vec<_>>();
        if let Some(erofs) = erofs_storages.first_mut() {
            erofs.options.extend(mkdir_options);
        }

        Ok(Self {
            guest_path: container_path,
            device_ids,
            rwlayer_storage,
            erofs_storages,
            vmdk_path,
            gpt_metadata_paths,
            generated_artifacts_dir: PathBuf::from(
                kata_types::build_path(DEFAULT_KATA_GUEST_ROOT_SHARED_FS),
            )
            .join(sid)
            .join(cid),
        })
    }
}

#[async_trait]
impl Rootfs for ErofsMultiLayerRootfs {
    async fn get_guest_rootfs_path(&self) -> Result<String> {
        Ok(self.guest_path.clone())
    }

    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(Vec::new()) // For multi-layer EROFS, the actual mount is handled by guest agent, so return empty here.
    }

    async fn get_storage(&self) -> Option<Vec<Storage>> {
        // Return all storages for multi-layer EROFS. The rw layer is optional;
        // when absent, the agent creates a /run-backed upper dir. In GPT mode,
        // each partition has its own EROFS storage entry.
        let mut storages = Vec::new();

        if let Some(rwlayer) = self.rwlayer_storage.clone() {
            storages.push(rwlayer);
        }

        // Add all EROFS layer storages (single storage in fsmerge mode, multiple in GPT mode)
        for erofs in &self.erofs_storages {
            storages.push(erofs.clone());
        }

        if storages.is_empty() {
            None
        } else {
            Some(storages)
        }
    }

    async fn get_device_id(&self) -> Result<Option<String>> {
        Ok(self.device_ids.first().cloned())
    }

    async fn cleanup(&self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        // Helper function to safely remove a file if it exists and is within the specified directory.
        let safely_remove_file = |path: &Path, dir: &Path| -> Result<()> {
            if path.starts_with(dir) && path.exists() {
                fs::remove_file(path).context(format!("failed to remove file: {}", path.display()))?;
            }
            Ok(())
        };

        let mut dm = device_manager.write().await;
        for device_id in &self.device_ids {
            dm.try_remove_device(device_id).await?;
        }

        // Clean up generated VMDK descriptor file if it exists.
        if let Some(ref vmdk) = self.vmdk_path {
            safely_remove_file(vmdk, &self.generated_artifacts_dir)?;
        }

        // Clean up GPT metadata files (head, padding).
        for metadata_path in &self.gpt_metadata_paths {
            safely_remove_file(metadata_path, &self.generated_artifacts_dir)?;
        }

        Ok(())
    }
}

fn overlay_like(fs_type: &str) -> bool {
    matches!(
        fs_type.to_ascii_lowercase().as_str(),
        "overlay" | "format/overlay" | "format/mkdir/overlay"
    )
}

/// Check if mounts represent a multi-layer EROFS rootfs.
///
/// Matches what the containerd erofs snapshotter sends for an active snapshot:
/// an EROFS lower layer plus an overlay mount. With host rw backing enabled,
/// the mount list also includes an ext4 `rwlayer.img`; with `default_size="0"`
/// it does not, and the agent creates the writable upper under `/run`.
///
/// This is only the coarse dispatcher check; `ErofsMultiLayerRootfs::new`
/// parses the optional rwlayer and overlay metadata.
pub fn is_erofs_multi_layer(rootfs_mounts: &[Mount]) -> bool {
    if rootfs_mounts.len() < 2 {
        return false;
    }

    let has_erofs = rootfs_mounts
        .iter()
        .any(|m| m.fs_type.eq_ignore_ascii_case(EROFS_ROOTFS_TYPE));

    if !has_erofs {
        return false;
    }

    rootfs_mounts.iter().any(|m| overlay_like(&m.fs_type))
}

#[cfg(test)]
mod tests {
    use super::{is_erofs_multi_layer, EROFS_ROOTFS_TYPE, RW_LAYER_ROOTFS_TYPE};
    use kata_types::mount::Mount;
    use std::path::PathBuf;

    fn mount(fs_type: &str, options: &[&str]) -> Mount {
        Mount {
            fs_type: fs_type.to_string(),
            options: options.iter().map(|s| (*s).to_string()).collect(),
            destination: PathBuf::from("/"),
            ..Default::default()
        }
    }

    #[test]
    fn is_erofs_multi_layer_rejects_short_list() {
        assert!(!is_erofs_multi_layer(&[]));
        assert!(!is_erofs_multi_layer(&[mount(EROFS_ROOTFS_TYPE, &[])]));
    }

    #[test]
    fn is_erofs_multi_layer_requires_erofs() {
        let mounts = vec![mount(RW_LAYER_ROOTFS_TYPE, &["rw"]), mount("overlay", &[])];
        assert!(!is_erofs_multi_layer(&mounts));
    }

    #[test]
    fn is_erofs_multi_layer_ext4_rw_erofs_and_overlay() {
        let mounts = vec![
            mount(RW_LAYER_ROOTFS_TYPE, &["rw"]),
            mount(EROFS_ROOTFS_TYPE, &[]),
            mount("overlay", &[]),
        ];
        assert!(is_erofs_multi_layer(&mounts));
    }

    #[test]
    fn is_erofs_multi_layer_implicit_upper_erofs_and_overlay_variants() {
        for overlay_type in ["overlay", "format/overlay", "format/mkdir/overlay"] {
            let mounts = vec![mount(EROFS_ROOTFS_TYPE, &[]), mount(overlay_type, &[])];
            assert!(
                is_erofs_multi_layer(&mounts),
                "expected multi-layer for overlay type {}",
                overlay_type
            );
        }
    }

    #[test]
    fn is_erofs_multi_layer_erofs_without_overlay_or_rw_is_false() {
        let mounts = vec![mount(EROFS_ROOTFS_TYPE, &[]), mount("btrfs", &[])];
        assert!(!is_erofs_multi_layer(&mounts));
    }

    #[test]
    fn is_erofs_multi_layer_does_not_validate_optional_rwlayer_options() {
        // The dispatcher only requires EROFS + overlay. Detailed rwlayer
        // interpretation is handled by ErofsMultiLayerRootfs::new.
        let mounts = vec![
            mount(RW_LAYER_ROOTFS_TYPE, &["ro"]),
            mount(EROFS_ROOTFS_TYPE, &[]),
            mount("overlay", &[]),
        ];
        assert!(is_erofs_multi_layer(&mounts));
    }
}
