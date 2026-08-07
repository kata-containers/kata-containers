// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::{BlockDeviceFormat, VmdkConfig};

use anyhow::{anyhow, Context, Result};
use nix::sys::memfd::{memfd_create, MFdFlags};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};

// A GPT-composed rootfs can require one layer and one padding file per
// partition, plus metadata and the VMDK descriptor. This accommodates the
// current 128-layer limit while bounding descriptor consumption.
pub(super) const MAX_BLOCK_SOURCE_FDS: usize = 512;
// Leave room for the composed rootfs and additional block devices, but prevent
// an unbounded number of descriptors from being delegated to one QEMU process.
pub(super) const MAX_QEMU_BLOCK_FDSETS: usize = 1024;
const BLOCK_FD_OPAQUE_PREFIX: &str = "kata-block:";

pub(super) fn block_fd_opaque(node_name: &str, label: &str) -> String {
    format!("{BLOCK_FD_OPAQUE_PREFIX}{node_name}:{label}")
}

pub(super) fn block_fd_node_name(opaque: &str) -> Option<&str> {
    opaque
        .strip_prefix(BLOCK_FD_OPAQUE_PREFIX)?
        .split_once(':')
        .map(|(node_name, _)| node_name)
        .filter(|node_name| !node_name.is_empty())
}

#[derive(Debug)]
pub(super) struct PreparedBlockSource {
    pub filename: String,
    pub is_regular_file: bool,
}

fn open_block_source(
    path: &str,
    is_readonly: bool,
    is_direct: bool,
    regular_file_only: bool,
) -> Result<(File, std::fs::Metadata)> {
    let mut options = OpenOptions::new();
    let direct_flag = if is_direct { libc::O_DIRECT } else { 0 };
    options
        .read(true)
        .write(!is_readonly)
        // Rust opens files close-on-exec by default; specify it here as part of
        // this security boundary and clear it only for descriptors intentionally
        // inherited by QEMU at startup.
        .custom_flags(direct_flag | libc::O_CLOEXEC);

    let file = options
        .open(path)
        .with_context(|| format!("open QEMU block source {path}"))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("stat opened QEMU block source {path}"))?;
    let file_type = metadata.file_type();
    if regular_file_only && !file_type.is_file() {
        return Err(anyhow!("QEMU block source {path} is not a regular file"));
    }
    if !regular_file_only && !file_type.is_file() && !file_type.is_block_device() {
        return Err(anyhow!(
            "QEMU block source {path} is neither an allowed regular file nor block device"
        ));
    }

    Ok((file, metadata))
}

pub(super) fn block_source_fd_count(
    format: &BlockDeviceFormat,
    vmdk: Option<&VmdkConfig>,
) -> Result<usize> {
    if *format == BlockDeviceFormat::Raw {
        return Ok(1);
    }

    let vmdk = vmdk.ok_or_else(|| anyhow!("VMDK block source is missing its extent layout"))?;
    if vmdk.extents.is_empty() {
        return Err(anyhow!("VMDK contains no extents"));
    }

    let unique_extents = vmdk
        .extents
        .iter()
        .map(|extent| &extent.path_on_host)
        .collect::<HashSet<_>>()
        .len();
    let count = unique_extents
        .checked_add(1)
        .ok_or_else(|| anyhow!("VMDK descriptor count overflow"))?;
    if count > MAX_BLOCK_SOURCE_FDS {
        return Err(anyhow!(
            "QEMU block source requires {count} descriptors, exceeding the limit of {MAX_BLOCK_SOURCE_FDS}"
        ));
    }

    Ok(count)
}

fn render_vmdk_descriptor(
    config: &VmdkConfig,
    backing_paths: &HashMap<String, String>,
) -> Result<String> {
    let total_sectors = config
        .total_sectors()
        .ok_or_else(|| anyhow!("VMDK total sector count overflow"))?;
    if total_sectors == 0 {
        return Err(anyhow!("VMDK contains no non-empty extents"));
    }

    let mut descriptor = String::new();
    writeln!(descriptor, "# Disk DescriptorFile")?;
    writeln!(descriptor, "version=1")?;
    writeln!(descriptor, "CID=fffffffe")?;
    writeln!(descriptor, "parentCID=ffffffff")?;
    writeln!(descriptor, "createType=\"twoGbMaxExtentFlat\"")?;
    writeln!(descriptor)?;
    writeln!(descriptor, "# Extent description")?;
    for extent in &config.extents {
        let path = backing_paths
            .get(&extent.path_on_host)
            .ok_or_else(|| anyhow!("missing prepared VMDK extent {}", extent.path_on_host))?;
        writeln!(
            descriptor,
            "RW {} FLAT \"{}\" {}",
            extent.sectors, path, extent.file_offset
        )?;
    }

    let cylinders = total_sectors.div_ceil(63 * 16);
    writeln!(descriptor)?;
    writeln!(descriptor, "# The Disk Data Base")?;
    writeln!(descriptor, "#DDB")?;
    writeln!(descriptor)?;
    writeln!(descriptor, "ddb.virtualHWVersion = \"4\"")?;
    writeln!(descriptor, "ddb.geometry.cylinders = \"{cylinders}\"")?;
    writeln!(descriptor, "ddb.geometry.heads = \"16\"")?;
    writeln!(descriptor, "ddb.geometry.sectors = \"63\"")?;
    writeln!(descriptor, "ddb.adapterType = \"ide\"")?;

    Ok(descriptor)
}

fn create_vmdk_descriptor_file(
    descriptor: &str,
    is_readonly: bool,
    is_direct: bool,
) -> Result<File> {
    let mut writable = if is_direct {
        tempfile::tempfile().context("create anonymous QEMU VMDK descriptor file")?
    } else {
        let name = CString::new("kata-qemu-vmdk")?;
        let fd = memfd_create(name.as_c_str(), MFdFlags::MFD_CLOEXEC)
            .context("create QEMU VMDK descriptor memfd")?;
        File::from(fd)
    };
    writable
        .write_all(descriptor.as_bytes())
        .context("write anonymous QEMU VMDK descriptor")?;

    // QEMU's fdset lookup requires the descriptor's access and O_DIRECT flags
    // to match the flags requested by the block layer. Reopen the completed
    // anonymous file with the requested access rather than passing its writable
    // construction descriptor.
    let fd_path = format!("/proc/self/fd/{}", writable.as_raw_fd());
    OpenOptions::new()
        .read(true)
        .write(!is_readonly)
        .custom_flags(if is_direct { libc::O_DIRECT } else { 0 })
        .open(&fd_path)
        .context("reopen anonymous QEMU VMDK descriptor read-only")
}

/// Open a block source while the shim still has host privileges and register
/// the resulting descriptors with QEMU. Structured VMDK layouts are serialized
/// once after their backing extents have been registered.
pub(super) fn prepare_block_source<F>(
    path: &str,
    format: &BlockDeviceFormat,
    vmdk: Option<&VmdkConfig>,
    is_readonly: bool,
    is_direct: bool,
    mut register: F,
) -> Result<PreparedBlockSource>
where
    F: FnMut(File, &str) -> Result<String>,
{
    block_source_fd_count(format, vmdk)?;

    if *format == BlockDeviceFormat::Raw {
        let (file, metadata) = open_block_source(path, is_readonly, is_direct, false)?;
        let is_regular_file = metadata.is_file();
        let filename = register(file, "block-source")?;
        return Ok(PreparedBlockSource {
            filename,
            is_regular_file,
        });
    }

    let vmdk = vmdk.ok_or_else(|| anyhow!("VMDK block source is missing its extent layout"))?;
    let mut backing_paths = HashMap::new();
    for extent in &vmdk.extents {
        if backing_paths.contains_key(&extent.path_on_host) {
            continue;
        }
        let (file, metadata) =
            open_block_source(&extent.path_on_host, is_readonly, is_direct, true)?;
        let required_sectors = vmdk
            .extents
            .iter()
            .filter(|candidate| candidate.path_on_host == extent.path_on_host)
            .map(|candidate| candidate.file_offset.checked_add(candidate.sectors))
            .try_fold(0_u64, |maximum, end| end.map(|end| maximum.max(end)))
            .ok_or_else(|| anyhow!("VMDK extent sector count overflow"))?;
        if metadata.len().div_ceil(512) < required_sectors {
            return Err(anyhow!(
                "VMDK extent {} is shorter than its declared layout",
                extent.path_on_host
            ));
        }
        let index = backing_paths.len();
        let fd_path = register(file, &format!("vmdk-extent-{index}"))?;
        backing_paths.insert(extent.path_on_host.clone(), fd_path);
    }

    let descriptor = render_vmdk_descriptor(vmdk, &backing_paths)?;
    let descriptor = create_vmdk_descriptor_file(&descriptor, is_readonly, is_direct)?;
    let filename = register(descriptor, "vmdk-descriptor")?;

    Ok(PreparedBlockSource {
        filename,
        is_regular_file: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    use std::cell::RefCell;
    use std::io::{Read, Seek, SeekFrom};
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn renders_structured_vmdk_layout() {
        let mut config = VmdkConfig::default();
        config.push_extent("/images/first.raw", 8, 0);
        config.push_extent("/images/first.raw", 4, 8);
        config.push_extent("/images/second.raw", 16, 0);
        let backing_paths = HashMap::from([
            ("/images/first.raw".to_string(), "/dev/fdset/1".to_string()),
            ("/images/second.raw".to_string(), "/dev/fdset/2".to_string()),
        ]);

        let descriptor = render_vmdk_descriptor(&config, &backing_paths).unwrap();
        assert!(descriptor.contains("RW 8 FLAT \"/dev/fdset/1\" 0"));
        assert!(descriptor.contains("RW 4 FLAT \"/dev/fdset/1\" 8"));
        assert!(descriptor.contains("RW 16 FLAT \"/dev/fdset/2\" 0"));
        assert!(descriptor.contains("ddb.geometry.cylinders = \"1\""));
    }

    #[test]
    fn prepares_raw_source_with_requested_access() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("disk.img");
        std::fs::write(&path, b"disk").unwrap();

        let prepared = prepare_block_source(
            path.to_str().unwrap(),
            &BlockDeviceFormat::Raw,
            None,
            true,
            false,
            |file, _| {
                assert_eq!(file.metadata()?.len(), 4);
                Ok("/dev/fdset/7".to_string())
            },
        )
        .unwrap();

        assert_eq!(prepared.filename, "/dev/fdset/7");
        assert!(prepared.is_regular_file);
    }

    #[test]
    fn opens_writable_raw_source_read_write() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("writable.raw");
        std::fs::write(&path, b"disk").unwrap();

        prepare_block_source(
            path.to_str().unwrap(),
            &BlockDeviceFormat::Raw,
            None,
            false,
            false,
            |mut file, _| {
                file.seek(SeekFrom::End(0))?;
                file.write_all(b"-writable")?;
                Ok("/dev/fdset/8".to_string())
            },
        )
        .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"disk-writable");
    }

    #[test]
    fn opens_readonly_raw_source_without_write_access() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("readonly.raw");
        std::fs::write(&path, b"readonly").unwrap();

        prepare_block_source(
            path.to_str().unwrap(),
            &BlockDeviceFormat::Raw,
            None,
            true,
            false,
            |mut file, _| {
                assert!(file.write_all(b"no").is_err());
                Ok("/dev/fdset/8".to_string())
            },
        )
        .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"readonly");
    }

    #[test]
    fn prepares_every_structured_vmdk_extent() {
        let dir = tempdir().unwrap();
        let first = dir.path().join("first.raw");
        let second = dir.path().join("second.raw");
        std::fs::write(&first, vec![0u8; 512]).unwrap();
        std::fs::write(&second, vec![0u8; 1024]).unwrap();

        let mut vmdk = VmdkConfig::default();
        vmdk.push_extent(first.to_str().unwrap(), 1, 0);
        vmdk.push_extent(second.to_str().unwrap(), 2, 0);

        let registered = RefCell::new(Vec::new());
        let prepared = prepare_block_source(
            "merged.vmdk",
            &BlockDeviceFormat::Vmdk,
            Some(&vmdk),
            true,
            false,
            |mut file, label| {
                let index = registered.borrow().len() + 10;
                if label == "vmdk-descriptor" {
                    let flags = OFlag::from_bits_truncate(fcntl(&file, FcntlArg::F_GETFL)?);
                    assert_eq!(flags & OFlag::O_ACCMODE, OFlag::O_RDONLY);
                    let mut descriptor = String::new();
                    file.read_to_string(&mut descriptor)?;
                    assert!(descriptor.contains("\"/dev/fdset/10\""));
                    assert!(descriptor.contains("\"/dev/fdset/11\""));
                    assert!(!descriptor.contains(first.to_str().unwrap()));
                    assert!(!descriptor.contains(second.to_str().unwrap()));
                }
                registered.borrow_mut().push(label.to_string());
                Ok(format!("/dev/fdset/{index}"))
            },
        )
        .unwrap();

        assert_eq!(prepared.filename, "/dev/fdset/12");
        assert_eq!(
            registered.into_inner(),
            vec!["vmdk-extent-0", "vmdk-extent-1", "vmdk-descriptor"]
        );
    }

    #[test]
    fn preserves_direct_io_on_registered_files() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("disk.img");
        std::fs::write(&path, vec![0u8; 4096]).unwrap();

        prepare_block_source(
            path.to_str().unwrap(),
            &BlockDeviceFormat::Raw,
            None,
            true,
            true,
            |file, _| {
                let flags = OFlag::from_bits_truncate(fcntl(&file, FcntlArg::F_GETFL)?);
                assert!(flags.contains(OFlag::O_DIRECT));
                Ok("/dev/fdset/8".to_string())
            },
        )
        .unwrap();
    }

    #[test]
    fn preserves_direct_io_on_vmdk_descriptor_and_extents() {
        let dir = tempdir().unwrap();
        let extent = dir.path().join("extent.raw");
        std::fs::write(&extent, vec![0u8; 4096]).unwrap();
        let mut vmdk = VmdkConfig::default();
        vmdk.push_extent(extent.to_str().unwrap(), 8, 0);

        prepare_block_source(
            "merged.vmdk",
            &BlockDeviceFormat::Vmdk,
            Some(&vmdk),
            true,
            true,
            |file, _| {
                let flags = OFlag::from_bits_truncate(fcntl(&file, FcntlArg::F_GETFL)?);
                assert!(flags.contains(OFlag::O_DIRECT));
                Ok("/dev/fdset/9".to_string())
            },
        )
        .unwrap();
    }

    #[test]
    fn follows_final_component_symlink() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target.raw");
        let link = dir.path().join("source.raw");
        std::fs::write(&target, b"disk").unwrap();
        symlink(&target, &link).unwrap();

        let prepared = prepare_block_source(
            link.to_str().unwrap(),
            &BlockDeviceFormat::Raw,
            None,
            true,
            false,
            |file, _| {
                assert!(file.metadata()?.is_file());
                Ok("/dev/fdset/1".to_string())
            },
        )
        .unwrap();

        assert_eq!(prepared.filename, "/dev/fdset/1");
        assert!(prepared.is_regular_file);
    }

    #[test]
    fn rejects_short_vmdk_extent() {
        let dir = tempdir().unwrap();
        let extent = dir.path().join("extent.raw");
        std::fs::write(&extent, vec![0_u8; 512]).unwrap();
        let mut vmdk = VmdkConfig::default();
        vmdk.push_extent(extent.to_str().unwrap(), 2, 0);

        let error = prepare_block_source(
            "disk.vmdk",
            &BlockDeviceFormat::Vmdk,
            Some(&vmdk),
            true,
            false,
            |_, _| Ok("/dev/fdset/1".to_string()),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("shorter than its declared layout"));
    }

    #[test]
    fn bounds_vmdk_descriptor_count() {
        let mut vmdk = VmdkConfig::default();
        for index in 0..MAX_BLOCK_SOURCE_FDS {
            vmdk.push_extent(&format!("/images/{index}.raw"), 1, 0);
        }

        let error = block_source_fd_count(&BlockDeviceFormat::Vmdk, Some(&vmdk)).unwrap_err();

        assert!(error.to_string().contains("exceeding the limit"));
    }

    #[test]
    fn registered_source_survives_path_removal() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("source.raw");
        std::fs::write(&path, b"persistent").unwrap();
        let received = RefCell::new(None);

        prepare_block_source(
            path.to_str().unwrap(),
            &BlockDeviceFormat::Raw,
            None,
            true,
            false,
            |file, _| {
                *received.borrow_mut() = Some(file.try_clone()?);
                Ok("/dev/fdset/1".to_string())
            },
        )
        .unwrap();
        std::fs::remove_file(&path).unwrap();

        let mut contents = String::new();
        received
            .borrow_mut()
            .as_mut()
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        assert_eq!(contents, "persistent");
    }

    #[test]
    fn identifies_owned_block_fdsets() {
        let opaque = block_fd_opaque("drive-3", "vmdk-extent-0");

        assert_eq!(block_fd_node_name(&opaque), Some("drive-3"));
        assert_eq!(block_fd_node_name("unrelated"), None);
        assert_eq!(block_fd_node_name("kata-block::source"), None);
    }
}
