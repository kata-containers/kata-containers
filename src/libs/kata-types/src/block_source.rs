// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
//! Opening the host files behind a block device.
//!
//! The shim opens block sources itself, while it still has host privileges,
//! and hands the hypervisor the open files rather than paths.

use anyhow::{anyhow, Context, Result};
use nix::fcntl::OFlag;
use std::fs::{File, Metadata, OpenOptions};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};

/// Open the block source at `path` and return it with the metadata of the
/// opened file.
///
/// The file is opened read-only if `is_readonly`, and with `O_DIRECT` if
/// `is_direct`. It must be a regular file or, unless `regular_file_only`, a
/// block device.
pub fn open_block_source(
    path: &str,
    is_readonly: bool,
    is_direct: bool,
    regular_file_only: bool,
) -> Result<(File, Metadata)> {
    let direct_flag = if is_direct {
        OFlag::O_DIRECT
    } else {
        OFlag::empty()
    };
    let file = OpenOptions::new()
        .read(true)
        .write(!is_readonly)
        // Rust opens files close-on-exec by default; specify it here as part of
        // this security boundary. The hypervisor backend clears it only for
        // descriptors that the hypervisor process inherits at startup.
        .custom_flags((direct_flag | OFlag::O_CLOEXEC).bits())
        .open(path)
        .with_context(|| format!("open block source {path}"))?;
    let metadata = file
        .metadata()
        .with_context(|| format!("stat opened block source {path}"))?;
    let file_type = metadata.file_type();
    if regular_file_only && !file_type.is_file() {
        return Err(anyhow!("block source {path} is not a regular file"));
    }
    if !regular_file_only && !file_type.is_file() && !file_type.is_block_device() {
        return Err(anyhow!(
            "block source {path} is neither an allowed regular file nor block device"
        ));
    }

    Ok((file, metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::fcntl::{fcntl, FcntlArg};
    use tempfile::tempdir;

    #[test]
    fn test_open_block_source() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("disk.img");
        std::fs::write(&file, vec![0_u8; 4096]).unwrap();
        let file = file.to_str().unwrap();
        let dir = dir.path().to_str().unwrap();

        struct TestData<'a> {
            path: &'a str,
            is_readonly: bool,
            is_direct: bool,
            regular_file_only: bool,
            // Ok: the expected access mode. Err: the expected error.
            result: std::result::Result<OFlag, String>,
        }
        let tests = [
            TestData {
                path: file,
                is_readonly: true,
                is_direct: false,
                regular_file_only: false,
                result: Ok(OFlag::O_RDONLY),
            },
            TestData {
                path: file,
                is_readonly: false,
                is_direct: true,
                regular_file_only: true,
                result: Ok(OFlag::O_RDWR),
            },
            TestData {
                path: "/dev/null",
                is_readonly: true,
                is_direct: false,
                regular_file_only: false,
                result: Err(
                    "block source /dev/null is neither an allowed regular file nor block device"
                        .to_string(),
                ),
            },
            TestData {
                path: "/dev/null",
                is_readonly: true,
                is_direct: false,
                regular_file_only: true,
                result: Err("block source /dev/null is not a regular file".to_string()),
            },
            TestData {
                path: dir,
                is_readonly: true,
                is_direct: false,
                regular_file_only: true,
                result: Err(format!("block source {dir} is not a regular file")),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let result = open_block_source(d.path, d.is_readonly, d.is_direct, d.regular_file_only);
            let msg = format!("test[{}] {}: result: {:?}", i, d.path, result);
            match &d.result {
                Ok(access) => {
                    let (file, metadata) = result.unwrap();
                    let flags = OFlag::from_bits_truncate(fcntl(&file, FcntlArg::F_GETFL).unwrap());
                    assert_eq!(flags & OFlag::O_ACCMODE, *access, "{}", msg);
                    assert_eq!(flags.contains(OFlag::O_DIRECT), d.is_direct, "{}", msg);
                    assert!(metadata.is_file(), "{}", msg);
                }
                Err(expected) => {
                    assert_eq!(&format!("{:#}", result.unwrap_err()), expected, "{}", msg)
                }
            }
        }
    }
}
