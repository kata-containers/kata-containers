// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
// GPT (GUID Partition Table) disk metadata generation for EROFS multi-layer rootfs.
//
// This module generates a GPT metadata file (gpt_meta_head.img) that is used
// in conjunction with VMDK descriptors to present multiple EROFS layers as a
// single virtual disk with multiple GPT partitions to the guest VM.
// Backup GPT structures are omitted — the virtual disk is ephemeral and
// read-only, so backup recovery serves no purpose.
//
// Key features:
// - Only includes read-only EROFS layers in GPT partitions (rw layer handled separately)
// - Preserves the original order of layers from rootfs_mounts
// - Generates minimal GPT metadata without copying layer data
// - Supports 1MiB alignment for partitions
// - Creates VMDK-compatible descriptor with head/layer/pad extents

use anyhow::{anyhow, Context, Result};
use crc::Crc;
use gpt::{disk::LogicalBlockSize, mbr::ProtectiveMBR, partition_types, GptConfig};
use scopeguard;
use std::convert::TryFrom;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::sl;

/// GPT disk parameters (using gpt crate constants where available)
/// DEFAULT_SECTOR_SIZE is LogicalBlockSize enum, not u64
const SECTOR_SIZE: u64 = 512;
/// 1 MiB alignment start
const FIRST_PARTITION_LBA: u64 = 2048;
/// 1 MiB alignment
const ALIGNMENT_LBA: u64 = 2048;
/// bytes per GPT partition entry (UEFI standard)
const GPT_ENTRY_SIZE: u64 = 128;
/// standard GPT partition entry count
const MAX_GPT_PARTITIONS: usize = 128;
/// 32 sectors for partition entries (128 entries * 128 bytes each / 512 bytes per sector)
const ENTRIES_SECTORS: u64 = (MAX_GPT_PARTITIONS as u64 * GPT_ENTRY_SIZE) / SECTOR_SIZE;
/// GPT header size in bytes (UEFI specification)
const GPT_HEADER_SIZE: usize = 92;
/// Offset (in bytes) of the GPT primary header within the head file (LBA 1)
const GPT_HEADER_FILE_OFFSET: u64 = SECTOR_SIZE;
/// CRC-32/ISO-HDLC — the same algorithm the `gpt` crate uses internally.
const CRC_32: Crc<u32> = Crc::<u32>::new(&crc::CRC_32_ISO_HDLC);

/// GPT head metadata file name
const GPT_META_HEAD_IMG: &str = "gpt_meta_head.img";
/// Temporary full GPT image used to synthesize head metadata
const GPT_META_FULL_IMG: &str = "gpt_meta_full.img";

/// Represents a read-only EROFS layer to be placed in a GPT partition
#[derive(Debug, Clone)]
pub struct ErofsLayer {
    /// Path to the EROFS image file
    pub path: String,
    /// Size in sectors (ceiling division, sector = 512 bytes)
    pub size_sectors: u64,
    /// Snapshot ID extracted from path (for naming)
    pub snapshot_id: String,
}

/// GPT partition layout information for a single layer
#[derive(Debug, Clone)]
pub struct PartitionLayout {
    /// Layer information
    pub layer: ErofsLayer,
    /// Partition number (1-indexed)
    pub partition_number: u32,
    /// First LBA of the partition
    pub start_lba: u64,
    /// Last LBA of the partition
    pub end_lba: u64,
    /// Partition name
    pub name: String,
}

/// Complete GPT disk layout calculation result
#[derive(Debug, Clone)]
pub struct GptDiskLayout {
    /// All partition layouts in order
    pub partitions: Vec<PartitionLayout>,
    /// Total sectors in the virtual disk
    pub total_sectors: u64,
    /// Logical block size in bytes
    pub lb_size: u64,
}

/// Result of GPT metadata file generation
#[derive(Debug)]
pub struct GptMetadataFiles {
    /// Path to generated gpt_meta_head.img
    pub head_path: PathBuf,
    /// Size of head file in sectors
    pub head_sectors: u64,
    /// Paths to generated padding files (between partitions)
    pub pad_paths: Vec<PathBuf>,
}

/// Extract snapshot ID from a source path
///
/// Examples:
///   ".../snapshots/35/layer.erofs" ---> "35"
pub fn extract_snapshot_id(source: &str) -> String {
    Path::new(source)
        .parent()
        .and_then(|p| p.file_name())
        .map(|id| id.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Get file size in bytes
pub fn get_erofs_layer_size(path: &str) -> Result<u64> {
    let metadata = fs::metadata(path).context(format!("failed to stat EROFS file: {}", path))?;
    Ok(metadata.len())
}

/// Align LBA up to the specified alignment
fn align_up(lba: u64, alignment: u64) -> u64 {
    if lba.is_multiple_of(alignment) {
        lba
    } else {
        ((lba / alignment) + 1) * alignment
    }
}

/// Calculate GPT disk layout from EROFS layers
///
/// This function computes the LBA positions for all partitions without
/// modifying any files. It follows the layout:
/// - LBA 0: Protective MBR
/// - LBA 1: Primary GPT Header
/// - LBA 2-33: Primary Partition Entry Array
/// - LBA 34-2047: Reserved/padding
/// - LBA 2048+: Partitions (1MiB aligned)
/// - End: Backup Partition Entry Array + Backup GPT Header
pub fn calculate_gpt_layout(layers: &[ErofsLayer]) -> Result<GptDiskLayout> {
    if layers.is_empty() {
        return Err(anyhow!("no EROFS layers provided for GPT layout"));
    }

    // TODO: Fix the length of partitions exceeding GPT limits.
    // It should be addressed by splitting into multiple GPT disks if needed, but for now we enforce the limit.
    if layers.len() > MAX_GPT_PARTITIONS {
        return Err(anyhow!(
            "The layers for GPT: {} exceeds maximum {} partitions \
             (ENTRIES_SECTORS is sized for {} entries)",
            layers.len(),
            MAX_GPT_PARTITIONS,
            MAX_GPT_PARTITIONS,
        ));
    }

    // Validate that all layers have non-zero size
    for (idx, layer) in layers.iter().enumerate() {
        if layer.size_sectors == 0 {
            return Err(anyhow!(
                "EROFS layer {} ({}) has size_sectors = 0, cannot generate GPT partition",
                idx,
                layer.path
            ));
        }
    }

    let lb_size = SECTOR_SIZE;
    let first_usable_lba = FIRST_PARTITION_LBA;

    // Calculate partition positions
    let mut partitions = Vec::with_capacity(layers.len());
    let mut current_lba = first_usable_lba;

    for (idx, layer) in layers.iter().enumerate() {
        // Align start LBA to 1MiB boundary
        let start_lba = align_up(current_lba, ALIGNMENT_LBA);
        let end_lba = start_lba + layer.size_sectors - 1;

        // Generate partition name: erofs-{index}-s{snapshot_id}
        let name = format!("erofs-{}-s{}", idx, layer.snapshot_id);
        // Truncate to fit GPT name limit without slicing through a UTF-8 codepoint.
        let name = match name.char_indices().nth(36) {
            Some((truncate_at, _)) => name[..truncate_at].to_string(),
            None => name,
        };

        partitions.push(PartitionLayout {
            layer: layer.clone(),
            partition_number: (idx + 1) as u32,
            start_lba,
            end_lba,
            name,
        });

        // Next partition starts after this one
        current_lba = end_lba + 1;
    }

    // Calculate backup GPT position
    // Backup entries are placed after the last partition, aligned
    let backup_entries_lba = align_up(current_lba, ALIGNMENT_LBA);
    let backup_header_lba = backup_entries_lba + ENTRIES_SECTORS;
    let total_sectors = backup_header_lba + 1;

    let last_usable_lba = backup_entries_lba - 1;

    // Validate that all partitions fit in usable area
    for (idx, part) in partitions.iter().enumerate() {
        if part.end_lba > last_usable_lba {
            return Err(anyhow!(
                "partition {} (end_lba={}) exceeds last usable LBA ({})",
                idx,
                part.end_lba,
                last_usable_lba
            ));
        }
    }

    Ok(GptDiskLayout {
        partitions,
        total_sectors,
        lb_size,
    })
}

/// Generate GPT head metadata and return layout information
///
/// This is the main entry point for GPT metadata generation.
/// It creates a temporary full GPT image (needed by the gpt crate to
/// produce valid primary structures), extracts the head region, patches
/// the primary header to remove references to backup GPT, and discards
/// the rest.
///
/// Output:
/// - gpt_meta_head.img: Primary GPT structures (MBR + GPT header + partition entries + padding)
#[allow(unused_variables)]
pub fn generate_gpt_metadata(
    sid: &str,
    cid: &str,
    erofs_layers: Vec<ErofsLayer>,
    container_dir: &Path,
) -> Result<(GptDiskLayout, GptMetadataFiles)> {
    if erofs_layers.is_empty() {
        return Err(anyhow!(
            "no EROFS layers provided for GPT metadata generation"
        ));
    }

    let mut layout = calculate_gpt_layout(&erofs_layers)?;
    if layout.partitions.is_empty() {
        return Err(anyhow!(
            "no partitions in layout, cannot generate GPT metadata"
        ));
    }

    let full_path = container_dir.join(GPT_META_FULL_IMG);
    generate_full_gpt_image(&layout, &full_path).context("failed to generate full GPT image")?;
    let _cleanup = scopeguard::guard((), |_| {
        let _ = fs::remove_file(&full_path);
    });

    // Extract head: LBA 0 to FIRST_PARTITION_LBA (2048 sectors = 1 MiB)
    let lb_size = layout.lb_size;
    let head_sectors = FIRST_PARTITION_LBA;
    let head_size = head_sectors * lb_size;
    let head_path = container_dir.join(GPT_META_HEAD_IMG);
    extract_file_range(&full_path, &head_path, 0, head_size)
        .context("failed to extract GPT head metadata")?;

    // Patch the primary GPT header so AlternateLBA / LastUsableLBA are
    let last_partition_end = layout.partitions.last().unwrap().end_lba;
    patch_primary_gpt_header(&head_path, last_partition_end)
        .context("failed to patch primary GPT header")?;

    // Adjust the layout to reflect the virtual disk size (no backup).
    layout.total_sectors = last_partition_end + 1;

    info!(
        sl!(),
        "Generated GPT head file: {} ({} sectors, {} bytes, virtual disk {} sectors)",
        head_path.display(),
        head_sectors,
        head_size,
        layout.total_sectors
    );

    let metadata_files = GptMetadataFiles {
        head_path,
        head_sectors,
        pad_paths: Vec::new(),
    };

    Ok((layout, metadata_files))
}

fn generate_full_gpt_image(layout: &GptDiskLayout, output_path: &Path) -> Result<()> {
    let lb_size = layout.lb_size;
    let total_size = layout.total_sectors * lb_size;

    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_path)
        .context(format!(
            "failed to create full GPT image: {}",
            output_path.display()
        ))?;

    file.set_len(total_size)
        .context("failed to pre-allocate full GPT image")?;

    let mbr =
        ProtectiveMBR::with_lb_size(u32::try_from(layout.total_sectors - 1).unwrap_or(0xFFFF_FFFF));
    mbr.overwrite_lba0(&mut file)
        .context("failed to write Protective MBR")?;

    let mut gdisk = GptConfig::new()
        .writable(true)
        .logical_block_size(LogicalBlockSize::Lb512)
        .change_partition_count(true)
        .create_from_device(file, None)
        .context("failed to initialize GPT config")?;

    for part_layout in &layout.partitions {
        let part_size_bytes = (part_layout.end_lba - part_layout.start_lba + 1) * lb_size;
        gdisk
            .add_partition(
                &part_layout.name,
                part_size_bytes,
                partition_types::LINUX_FS,
                0,
                Some(ALIGNMENT_LBA),
            )
            .context(format!("failed to add partition '{}'", part_layout.name))?;
    }

    let mut file = gdisk
        .write()
        .context("failed to write GPT partition table")?;
    file.flush().context("failed to flush full GPT image")?;

    Ok(())
}

/// Patch the primary GPT header in the extracted head file to remove
/// backup GPT references.
///
/// Sets `AlternateLBA` to one sector beyond the virtual disk (so the kernel
/// detects "no valid backup" and falls back to the primary) and
/// `LastUsableLBA` to the end of the last partition, then recomputes the
/// header CRC32.
fn patch_primary_gpt_header(head_path: &Path, last_partition_end_lba: u64) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(head_path)
        .context("failed to open head file for patching")?;

    // Read the 92-byte GPT header starting at LBA 1.
    file.seek(SeekFrom::Start(GPT_HEADER_FILE_OFFSET))?;
    let mut header = [0u8; GPT_HEADER_SIZE];
    file.read_exact(&mut header)?;

    // AlternateLBA (offset 32, 8 bytes LE) — point beyond virtual disk
    let alternate_lba = last_partition_end_lba + 1;
    header[32..40].copy_from_slice(&alternate_lba.to_le_bytes());

    // LastUsableLBA (offset 48, 8 bytes LE) — last partition end
    header[48..56].copy_from_slice(&last_partition_end_lba.to_le_bytes());

    // Zero HeaderCRC32 (offset 16, 4 bytes LE) before computing new CRC
    header[16..20].copy_from_slice(&0u32.to_le_bytes());

    let new_crc = {
        let mut digest = CRC_32.digest();
        digest.update(&header);
        digest.finalize()
    };
    header[16..20].copy_from_slice(&new_crc.to_le_bytes());

    // Write patched header back
    file.seek(SeekFrom::Start(GPT_HEADER_FILE_OFFSET))?;
    file.write_all(&header)?;
    file.flush()?;

    info!(
        sl!(),
        "Patched primary GPT header: AlternateLBA={}, LastUsableLBA={}, CRC32={:#010x}",
        alternate_lba,
        last_partition_end_lba,
        new_crc
    );

    Ok(())
}

fn extract_file_range(src: &Path, dst: &Path, offset: u64, size: u64) -> Result<()> {
    let mut src_file = fs::OpenOptions::new()
        .read(true)
        .open(src)
        .context(format!("failed to open source file: {}", src.display()))?;
    src_file
        .seek(SeekFrom::Start(offset))
        .context("failed to seek source file")?;

    let mut dst_file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(dst)
        .context(format!("failed to create output file: {}", dst.display()))?;

    dst_file
        .set_len(size)
        .context("failed to pre-allocate output file")?;

    let mut limited = src_file.take(size);
    std::io::copy(&mut limited, &mut dst_file).context("failed to copy file range")?;
    dst_file.flush().context("failed to flush output file")?;

    Ok(())
}

/// Generate padding file content (all zeros)
///
/// Returns the file path and size in sectors.
pub fn generate_padding_file(output_path: &Path, size_sectors: u64) -> Result<u64> {
    let size_bytes = size_sectors * SECTOR_SIZE;

    if size_bytes == 0 {
        return Err(anyhow!("cannot create zero-size padding file"));
    }

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(output_path)
        .context(format!(
            "failed to create padding file: {}",
            output_path.display()
        ))?;

    // Pre-allocate with zeros
    file.set_len(size_bytes)
        .context("failed to pre-allocate padding file")?;
    file.flush().context("failed to flush padding file")?;
    drop(file);

    info!(
        sl!(),
        "Generated padding file: {} ({} sectors, {} bytes)",
        output_path.display(),
        size_sectors,
        size_bytes
    );

    Ok(size_sectors)
}
