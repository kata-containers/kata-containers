// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::BlockDeviceFormat;

use anyhow::{anyhow, Context, Result};
use std::fs::{File, OpenOptions};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};

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

/// Open a raw block source while the shim still has host privileges and
/// register the resulting descriptor with QEMU. Other formats retain their
/// existing path-based transport.
pub(super) fn prepare_block_source<F>(
    path: &str,
    format: &BlockDeviceFormat,
    is_readonly: bool,
    is_direct: bool,
    mut register: F,
) -> Result<PreparedBlockSource>
where
    F: FnMut(File, &str) -> Result<String>,
{
    if *format != BlockDeviceFormat::Raw {
        let metadata =
            std::fs::metadata(path).with_context(|| format!("stat QEMU block source {path}"))?;
        return Ok(PreparedBlockSource {
            filename: path.to_string(),
            is_regular_file: metadata.is_file(),
        });
    }

    let (file, metadata) = open_block_source(path, is_readonly, is_direct, false)?;
    let is_regular_file = metadata.is_file();
    let filename = register(file, "block-source")?;
    Ok(PreparedBlockSource {
        filename,
        is_regular_file,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::fcntl::{fcntl, FcntlArg, OFlag};
    use std::cell::RefCell;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn prepares_raw_source_with_requested_access() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("disk.img");
        std::fs::write(&path, b"disk").unwrap();

        let prepared = prepare_block_source(
            path.to_str().unwrap(),
            &BlockDeviceFormat::Raw,
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
    fn preserves_direct_io_on_registered_files() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("disk.img");
        std::fs::write(&path, vec![0u8; 4096]).unwrap();

        prepare_block_source(
            path.to_str().unwrap(),
            &BlockDeviceFormat::Raw,
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
    fn follows_final_component_symlink() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target.raw");
        let link = dir.path().join("source.raw");
        std::fs::write(&target, b"disk").unwrap();
        symlink(&target, &link).unwrap();

        let prepared = prepare_block_source(
            link.to_str().unwrap(),
            &BlockDeviceFormat::Raw,
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
    fn registered_source_survives_path_removal() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("source.raw");
        std::fs::write(&path, b"persistent").unwrap();
        let received = RefCell::new(None);

        prepare_block_source(
            path.to_str().unwrap(),
            &BlockDeviceFormat::Raw,
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
        let opaque = block_fd_opaque("drive-3", "block-source");

        assert_eq!(block_fd_node_name(&opaque), Some("drive-3"));
        assert_eq!(block_fd_node_name("unrelated"), None);
        assert_eq!(block_fd_node_name("kata-block::source"), None);
    }
}
