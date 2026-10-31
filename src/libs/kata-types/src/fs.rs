// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

//! Filesystem-related constants shared across Kata components.

/// Root filesystem type: ext4
pub const VM_ROOTFS_FILESYSTEM_EXT4: &str = "ext4";

/// Root filesystem type: xfs
pub const VM_ROOTFS_FILESYSTEM_XFS: &str = "xfs";

/// Root filesystem type: erofs
pub const VM_ROOTFS_FILESYSTEM_EROFS: &str = "erofs";

/// Read kernel PARTUUID identifiers from a raw disk image with 512-byte sectors.
///
/// Kata's image builder creates DOS partition tables, whose identifiers combine
/// the little-endian disk signature and partition number. GPT images instead
/// carry a unique GUID for each partition. Filesystem UUIDs cannot be used here:
/// resolving those requires userspace, which is unavailable before mounting root.
pub fn partition_uuids(
    path: &std::path::Path,
) -> anyhow::Result<std::collections::BTreeMap<u32, String>> {
    use anyhow::{ensure, Context};
    use std::collections::BTreeMap;
    use std::convert::TryInto;
    use std::io::Read;

    let mut file = std::fs::File::open(path)
        .with_context(|| format!("open rootfs image {}", path.display()))?;
    let mut mbr = [0u8; 512];
    file.read_exact(&mut mbr)
        .context("read rootfs partition table")?;
    ensure!(
        mbr[510..512] == [0x55, 0xaa],
        "invalid rootfs MBR signature"
    );

    let entries = &mbr[446..510];
    if entries.chunks_exact(16).any(|entry| entry[4] == 0xee) {
        let disk = gpt::GptConfig::new()
            .logical_block_size(gpt::disk::LogicalBlockSize::Lb512)
            .open(path)
            .context("read rootfs GPT")?;
        return Ok(disk
            .partitions()
            .iter()
            .map(|(&number, partition)| (number, partition.part_guid.to_string()))
            .collect());
    }

    let signature = u32::from_le_bytes(mbr[440..444].try_into().unwrap());
    // An unset disk signature is not a useful identity when several images
    // are attached. Fail before starting the VM instead of risking a collision.
    ensure!(signature != 0, "rootfs image has no DOS disk signature");
    let mut uuids = BTreeMap::new();
    for (index, entry) in entries.chunks_exact(16).enumerate() {
        if entry[4] != 0 && entry[12..16] != [0; 4] {
            let number = index as u32 + 1;
            uuids.insert(number, format!("{signature:08x}-{number:02x}"));
        }
    }
    Ok(uuids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn dos_image(signature: u32) -> tempfile::NamedTempFile {
        let mut image = tempfile::NamedTempFile::new().unwrap();
        let mut mbr = [0u8; 512];
        mbr[440..444].copy_from_slice(&signature.to_le_bytes());
        for offset in [446, 462] {
            mbr[offset + 4] = 0x83;
            mbr[offset + 12..offset + 16].copy_from_slice(&2048u32.to_le_bytes());
        }
        mbr[510..512].copy_from_slice(&[0x55, 0xaa]);
        image.write_all(&mbr).unwrap();
        image
    }

    #[test]
    fn test_dos_partition_uuids() {
        let image = dos_image(0x603f58ab);
        let uuids = partition_uuids(image.path()).unwrap();
        assert_eq!(uuids.len(), 2);
        assert_eq!(uuids[&1], "603f58ab-01");
        assert_eq!(uuids[&2], "603f58ab-02");
    }

    #[test]
    fn test_invalid_partition_uuids() {
        let image = dos_image(0);
        assert!(partition_uuids(image.path()).is_err());
        image.as_file().set_len(511).unwrap();
        assert!(partition_uuids(image.path()).is_err());
        let mut image = dos_image(1);
        use std::io::{Seek, SeekFrom};
        image.seek(SeekFrom::Start(510)).unwrap();
        image.write_all(&[0; 2]).unwrap();
        assert!(partition_uuids(image.path()).is_err());
    }

    #[test]
    fn test_gpt_partition_uuids() {
        let image = tempfile::NamedTempFile::new().unwrap();
        image.as_file().set_len(8 * 1024 * 1024).unwrap();
        let mut file = image.reopen().unwrap();
        gpt::mbr::ProtectiveMBR::with_lb_size(16383)
            .overwrite_lba0(&mut file)
            .unwrap();
        let mut disk = gpt::GptConfig::new()
            .writable(true)
            .logical_block_size(gpt::disk::LogicalBlockSize::Lb512)
            .create_from_device(file, None)
            .unwrap();
        let number = disk
            .add_partition("root", 1024 * 1024, gpt::partition_types::LINUX_FS, 0, None)
            .unwrap();
        let expected = disk.partitions()[&number].part_guid.to_string();
        disk.write().unwrap();
        assert_eq!(partition_uuids(image.path()).unwrap()[&number], expected);
    }
}
