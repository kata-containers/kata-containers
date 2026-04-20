// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
// Handle multi-layer EROFS rootfs:
// Mount[0]: ext4 rw layer -> virtio-blk device (writable)
// Mount[1]: erofs with device= -> virtio-blk via VMDK (read-only)
// Mount[2]: overlay (format/mkdir/overlay) -> host mount OR guest agent
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
use kata_types::mount::Mount;
use oci_spec::runtime as oci;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::{BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// EROFS rootfs type identifier
pub(crate) const EROFS_ROOTFS_TYPE: &str = "erofs";
/// RW layer rootfs type identifier, used for multi-layer EROFS as the writable upper layer
/// Typically ext4 format, but can be extended to other fs types in the future.
pub(crate) const RW_LAYER_ROOTFS_TYPE: &str = "ext4";
/// MKFS type prefix for unformatted rwlayer (to be formatted in VM guest)
pub(crate) const MKFS_TYPE_PREFIX: &str = "mkfs/";
/// VMDK file extension for merged EROFS image
const EROFS_MERGED_VMDK: &str = "merged_fs.vmdk";
/// Maximum number of virtio-blk devices allowed for multi-layer EROFS rootfs.
///
/// This defensive limit prevents exhausting PCI slot resources, especially on
/// lightweight VMMs (Dragonball, Cloud Hypervisor) where the PCIe root bus has
/// only 32 slots (PCIE_ROOT_BUS_SLOTS_CAPACITY). For QEMU with PCI bridges
/// (30 slots/bridge), this limit is conservative but still applies as a uniform
/// safeguard across all hypervisor backends.
const MAX_VIRTIO_BLK_DEVICES: usize = 10;
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
const DEFAULT_KATA_GUEST_ROOT_SHARED_FS: &str = "/run/kata-containers/";
/// Template for mkdir option in overlay mount (X-containerd.mkdir.path)
const X_CONTAINERD_MKDIR_PATH: &str = "X-containerd.mkdir.path=";
/// Template for mkdir option passed to guest agent (X-kata.mkdir.path)
const X_KATA_MKDIR_PATH: &str = "X-kata.mkdir.path=";
/// Template for mkfs size option (X-containerd.mkfs.size)
const X_CONTAINERD_MKFS_SIZE: &str = "X-containerd.mkfs.size=";
/// Template for mkfs filesystem type option (X-containerd.mkfs.fs)
const X_CONTAINERD_MKFS_FS: &str = "X-containerd.mkfs.fs=";
/// Template for mkfs UUID option (X-containerd.mkfs.uuid)
const X_CONTAINERD_MKFS_UUID: &str = "X-containerd.mkfs.uuid=";
/// Template for mkfs options passed to guest agent (X-kata.mkfs.*)
const X_KATA_MKFS_SIZE: &str = "X-kata.mkfs.size=";
const X_KATA_MKFS_FS: &str = "X-kata.mkfs.fs=";
const X_KATA_MKFS_UUID: &str = "X-kata.mkfs.uuid=";

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum RawFileMode {
    Sparse,
    #[cfg(target_os = "linux")]
    Allocate,
}

pub fn create_raw_file<P: AsRef<Path>>(path: P, size: u64, mode: RawFileMode) -> io::Result<()> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(true)
        .open(path)?;

    match mode {
        RawFileMode::Sparse => {
            file.set_len(size)?;
        }
        #[cfg(target_os = "linux")]
        RawFileMode::Allocate => {
            use std::os::fd::AsRawFd;
            let ret = unsafe { libc::fallocate(file.as_raw_fd(), 0, 0, size as libc::off_t) };
            if ret != 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }

    file.sync_all()?;
    Ok(())
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
    let sandbox_dir =
        PathBuf::from(kata_types::build_path(DEFAULT_KATA_GUEST_ROOT_SHARED_FS)).join(sid);
    let container_dir = sandbox_dir.join(cid);
    fs::create_dir_all(&container_dir).context(format!(
        "failed to create container directory: {}",
        container_dir.display()
    ))?;

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

    // collect extent information without writing anything.
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

        // round up to whole sectors to avoid losing tail bytes on non-aligned files.
        // VMDK extents are measured in 512-byte sectors; a file that is not sector-aligned
        // still needs the last partial sector to be addressable by the VM.
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

    // write descriptor to a temp file, then atomically rename.
    let tmp_path = vmdk_path.with_extension("vmdk.tmp");
    // Prevent path traversal attacks by rejecting paths containing '..'.
    if tmp_path.components().any(|c| c == Component::ParentDir) {
        return Err(anyhow!("Invalid input: {}", tmp_path.display()));
    }
    let file = fs::File::create(&tmp_path).context(format!(
        "failed to create temp VMDK file: {}",
        tmp_path.display()
    ))?;
    let mut writer = BufWriter::new(file);

    // Header
    writeln!(writer, "# Disk DescriptorFile")?;
    writeln!(writer, "version=1")?;
    writeln!(writer, "CID=fffffffe")?;
    writeln!(writer, "parentCID=ffffffff")?;
    writeln!(writer, "createType=\"{}\"", VMDK_SUBFORMAT)?;
    writeln!(writer)?;

    // Extent descriptions
    writeln!(writer, "# Extent description")?;
    for extent in &extents {
        let mut remaining = extent.total_sectors;
        let mut file_offset: u64 = 0;

        while remaining > 0 {
            let chunk = remaining.min(MAX_2GB_EXTENT_SECTORS);
            writeln!(
                writer,
                "RW {} FLAT \"{}\" {}",
                chunk, extent.path, file_offset
            )?;
            file_offset += chunk;
            remaining -= chunk;
        }

        info!(
            sl!(),
            "VMDK extent: {} ({} sectors, {} extent chunk(s))",
            extent.path,
            extent.total_sectors,
            extent.total_sectors.div_ceil(MAX_2GB_EXTENT_SECTORS)
        );
    }
    writeln!(writer)?;

    // Disk Data Base (DDB)
    // Geometry: cylinders = ceil(total_sectors / (sectors_per_track * heads))
    let cylinders = total_sectors.div_ceil(SECTORS_PER_TRACK * NUMBER_HEADS);

    writeln!(writer, "# The Disk Data Base")?;
    writeln!(writer, "#DDB")?;
    writeln!(writer)?;
    writeln!(writer, "ddb.virtualHWVersion = \"{}\"", VMDK_HW_VERSION)?;
    writeln!(writer, "ddb.geometry.cylinders = \"{}\"", cylinders)?;
    writeln!(writer, "ddb.geometry.heads = \"{}\"", NUMBER_HEADS)?;
    writeln!(writer, "ddb.geometry.sectors = \"{}\"", SECTORS_PER_TRACK)?;
    writeln!(writer, "ddb.adapterType = \"{}\"", VMDK_ADAPTER_TYPE)?;

    // Flush the BufWriter to ensure all data is written before rename.
    writer.flush().context("failed to flush VMDK descriptor")?;
    // Explicitly drop to close the file handle before rename.
    drop(writer);

    // atomic rename: tmp -> final path.
    fs::rename(&tmp_path, vmdk_path).context(format!(
        "failed to rename temp VMDK {} -> {}",
        tmp_path.display(),
        vmdk_path.display()
    ))?;

    info!(
        sl!(),
        "VMDK descriptor created: {} (total {} sectors, {} extents, {} cylinders)",
        vmdk_path.display(),
        total_sectors,
        extents.len(),
        cylinders
    );

    Ok(())
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
/// - Mount[0]: ext4 rw layer (writable container layer) -> virtio-blk device
/// - Mount[1]: erofs layers (fsmeta + flattened layers) -> virtio-blk via VMDK
/// - Mount[2]: overlay (to combine ext4 upper + erofs lower)
pub(crate) struct ErofsMultiLayerRootfs {
    guest_path: String,
    device_ids: Vec<String>,
    rwlayer_storage: Option<Storage>, // Writable layer storage (upper layer), typically ext4
    erofs_storage: Option<Storage>,
    /// Path to generated VMDK descriptor (only set when multiple EROFS devices are merged)
    vmdk_path: Option<PathBuf>,
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
        let mut erofs_storage: Option<Storage> = None;
        let mut vmdk_path: Option<PathBuf> = None;

        // Directories to create (X-containerd.mkdir.path)
        let mut mkdir_dirs: Vec<String> = Vec::new();

        let blkdev_info = get_block_device_info(device_manager).await;
        let block_driver = blkdev_info.block_device_driver.clone();

        // Check block device count limit
        let expected_device_count = rootfs_mounts
            .iter()
            .filter(|m| matches!(m.fs_type.as_str(), RW_LAYER_ROOTFS_TYPE | EROFS_ROOTFS_TYPE))
            .count();
        if expected_device_count > MAX_VIRTIO_BLK_DEVICES {
            return Err(anyhow!(
                "exceeded maximum block devices for multi-layer EROFS: {} > {}",
                expected_device_count,
                MAX_VIRTIO_BLK_DEVICES
            ));
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

                    // let (device_id, guest_path, blk_driver) =
                    //     extract_block_device_info(&device_info, &block_driver)?;
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
                    info!(
                        sl!(),
                        "multi-layer erofs: adding erofs layers: {}", mount.source
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

                    let (mut rolayer, device_id) = extract_block_device_info(&device_info, true)?;
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
                            *o != "loop" && !o.starts_with("device=") && !o.starts_with("X-kata.")
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

                    erofs_storage = Some(rolayer);
                    device_ids.push(device_id);
                }
                fmt if fmt.eq_ignore_ascii_case("overlay")
                    || fmt.eq_ignore_ascii_case("format/overlay")
                    || fmt.eq_ignore_ascii_case("format/mkdir/overlay") =>
                {
                    // Mount[2]: overlay to combine rwlayer (upper) + erofs (lower)
                    info!(
                        sl!(),
                        "multi-layer erofs: parsing overlay mount, options: {:?}", mount.options
                    );

                    // Parse mkdir options (X-containerd.mkdir.path)
                    for opt in &mount.options {
                        if let Some(mkdir_spec) = opt.strip_prefix(X_CONTAINERD_MKDIR_PATH) {
                            // Keep the full spec (path:mode or path:mode:uid:gid) for guest agent
                            mkdir_dirs.push(mkdir_spec.to_string());
                        }
                    }
                }
                fmt if fmt.starts_with(MKFS_TYPE_PREFIX) => {
                    // mkfs/* type: unformatted rwlayer to be formatted in VM guest
                    // This is an alternative to the pre-formatted ext4 rwlayer.
                    // containerd creates an unformatted image, kata-agent formats it in VM.
                    info!(
                        sl!(),
                        "multi-layer erofs: adding mkfs rw layer (unformatted): {}", mount.source
                    );

                    let rwlayer_path = PathBuf::from(&mount.source);
                    if !rwlayer_path.exists() {
                        // Create an empty file for the unformatted rw layer if it doesn't exist.
                        // The file will be formatted in the VM guest, so it can start as an empty file.
                        info!(
                            sl!(),
                            "multi-layer erofs: creating unformatted rw layer file: {}",
                            rwlayer_path.display()
                        );
                        // 10GB is an arbitrary default size for the unformatted rw layer; it can be adjusted as needed.
                        create_raw_file(rwlayer_path, 10 * 1024 * 1024 * 1024, RawFileMode::Sparse)
                            .context(format!(
                                "failed to create unformatted rw layer file: {}",
                                mount.source
                            ))?;
                    }
                    let device_config = &mut BlockConfig {
                        driver_option: block_driver.clone(),
                        format: BlockDeviceFormat::Raw, // raw format, unformatted content
                        path_on_host: mount.source.clone(),
                        blkdev_aio: BlockDeviceAio::new(&blkdev_info.block_device_aio),
                        ..Default::default()
                    };

                    let device_info = do_handle_device(
                        device_manager,
                        &DeviceConfig::BlockCfg(device_config.clone()),
                    )
                    .await
                    .context("failed to attach mkfs block device")?;

                    let (mut rwlayer, device_id) =
                        extract_block_device_info(&device_info, false)
                            .context("failed to get block device for mkfs layer")?;
                    info!(
                        sl!(),
                        "mkfs block device attached - device_id: {} guest_path: {}",
                        device_id,
                        rwlayer.source
                    );

                    // Filter out "loop" option and containerd mkfs options
                    // Convert X-containerd.mkfs.* to X-kata.mkfs.* for guest agent
                    let mut options: Vec<String> = mount
                        .options
                        .iter()
                        .filter(|o| {
                            *o != "loop"
                                && !o.starts_with(X_CONTAINERD_MKFS_SIZE)
                                && !o.starts_with(X_CONTAINERD_MKFS_FS)
                                && !o.starts_with(X_CONTAINERD_MKFS_UUID)
                        })
                        .cloned()
                        .collect();

                    // Parse X-containerd.mkfs.* options and convert to X-kata.mkfs.*
                    for opt in &mount.options {
                        if let Some(size) = opt.strip_prefix(X_CONTAINERD_MKFS_SIZE) {
                            options.push(format!("{}{}", X_KATA_MKFS_SIZE, size));
                        } else if let Some(fs) = opt.strip_prefix(X_CONTAINERD_MKFS_FS) {
                            options.push(format!("{}{}", X_KATA_MKFS_FS, fs));
                        } else if let Some(uuid) = opt.strip_prefix(X_CONTAINERD_MKFS_UUID) {
                            options.push(format!("{}{}", X_KATA_MKFS_UUID, uuid));
                        }
                    }

                    // Mark as overlay upper layer and multi-layer
                    options.push("X-kata.overlay-upper".to_string());
                    options.push("X-kata.multi-layer=true".to_string());
                    // Mark as mkfs type - guest agent should format this device
                    options.push("X-kata.mkfs=true".to_string());

                    // Set up storage for mkfs rw layer
                    rwlayer.fs_type = fmt.to_string(); // Keep mkfs/* type for guest agent
                    rwlayer.mount_point = container_path.clone();
                    rwlayer.options = options;

                    rwlayer_storage = Some(rwlayer);
                    device_ids.push(device_id);
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

        // Check device count limit
        if device_ids.len() > MAX_VIRTIO_BLK_DEVICES {
            return Err(anyhow!(
                "exceeded maximum virtio disk count: {} > {}",
                device_ids.len(),
                MAX_VIRTIO_BLK_DEVICES
            ));
        }

        // Add mkdir directives to rwlayer storage options for guest agent
        if let Some(ref mut rwlayer) = rwlayer_storage {
            rwlayer.options.extend(
                mkdir_dirs
                    .iter()
                    .map(|dir| format!("{}{}", X_KATA_MKDIR_PATH, dir)),
            );
        }

        Ok(Self {
            guest_path: container_path,
            device_ids,
            rwlayer_storage,
            erofs_storage,
            vmdk_path,
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
        // Return all storages for multi-layer EROFS (rw layer + erofs layer) to guest agent.
        // Guest agent needs both to create overlay mount
        let mut storages = Vec::new();

        if let Some(rwlayer) = self.rwlayer_storage.clone() {
            storages.push(rwlayer);
        }

        if let Some(erofs) = self.erofs_storage.clone() {
            storages.push(erofs);
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
        let mut dm = device_manager.write().await;
        for device_id in &self.device_ids {
            dm.try_remove_device(device_id).await?;
        }

        // Clean up generated VMDK descriptor file if it exists (only for multi-device case)
        if let Some(ref vmdk) = self.vmdk_path {
            if vmdk.exists() {
                if let Err(e) = fs::remove_file(vmdk) {
                    warn!(
                        sl!(),
                        "failed to remove VMDK descriptor {}: {}",
                        vmdk.display(),
                        e
                    );
                }
            }
        }

        Ok(())
    }
}

/// Check if mounts represent a multi-layer EROFS rootfs.
///
/// Returns `true` when `rootfs_mounts` contains at least two entries:
/// an ext4 rw layer (upper) and an erofs layer (lower).
pub fn is_erofs_multi_layer(rootfs_mounts: &[Mount]) -> bool {
    if rootfs_mounts.len() < 2 {
        return false;
    }

    let has_rwlayer = rootfs_mounts.iter().any(|m| {
        // Match pre-formatted ext4 rwlayer (with "rw" option)
        (m.fs_type.eq_ignore_ascii_case(RW_LAYER_ROOTFS_TYPE)
            && m.options.iter().any(|o| o == "rw"))
            // Match unformatted mkfs/* rwlayer (e.g., mkfs/ext4)
            || m.fs_type.starts_with(MKFS_TYPE_PREFIX)
    });

    let has_erofs = rootfs_mounts
        .iter()
        .any(|m| m.fs_type.eq_ignore_ascii_case(EROFS_ROOTFS_TYPE));

    // Must have rwlayer + erofs (multi-layer or single-layer)
    has_rwlayer && has_erofs
}
