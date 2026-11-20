// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
//! Opening the host files behind a block device.
//!
//! The shim opens block sources itself, while it still has host privileges,
//! and hands the hypervisor the open files rather than paths.

use anyhow::{anyhow, Context, Result};
use nix::fcntl::{fcntl, FcntlArg, OFlag};
use std::fs::{File, Metadata, OpenOptions};
use std::os::unix::fs::{FileTypeExt, OpenOptionsExt};

/// Open the block source at `path` and return it with the metadata of the
/// opened file.
///
/// The file is opened read-only if `is_readonly`, and with `O_DIRECT` if
/// `is_direct`. It must be a regular file or, unless `regular_file_only`, a
/// block device. Any other type is rejected without blocking.
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
        //
        // Opening a FIFO read-only blocks until a writer appears, so the type
        // check below would never be reached. O_NONBLOCK makes the open return
        // at once; it is cleared again once the type is known to be allowed.
        .custom_flags((direct_flag | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK).bits())
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

    // Hand the hypervisor the descriptor with the flags it asked for.
    let flags = OFlag::from_bits_truncate(
        fcntl(&file, FcntlArg::F_GETFL).with_context(|| format!("get flags of {path}"))?,
    );
    fcntl(&file, FcntlArg::F_SETFL(flags - OFlag::O_NONBLOCK))
        .with_context(|| format!("clear O_NONBLOCK on {path}"))?;

    Ok((file, metadata))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::sys::stat::Mode;
    use nix::unistd::mkfifo;
    use tempfile::tempdir;

    #[test]
    fn test_open_block_source() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("disk.img");
        std::fs::write(&file, vec![0_u8; 4096]).unwrap();
        let file = file.to_str().unwrap();
        let fifo = dir.path().join("fifo");
        mkfifo(&fifo, Mode::S_IRUSR | Mode::S_IWUSR).unwrap();
        let fifo = fifo.to_str().unwrap();
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
            // Opening a FIFO read-only would block without O_NONBLOCK.
            TestData {
                path: fifo,
                is_readonly: true,
                is_direct: false,
                regular_file_only: false,
                result: Err(format!(
                    "block source {fifo} is neither an allowed regular file nor block device"
                )),
            },
            TestData {
                path: fifo,
                is_readonly: true,
                is_direct: false,
                regular_file_only: true,
                result: Err(format!("block source {fifo} is not a regular file")),
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
                    assert!(!flags.contains(OFlag::O_NONBLOCK), "{}", msg);
                    assert!(metadata.is_file(), "{}", msg);
                }
                Err(expected) => {
                    assert_eq!(&format!("{:#}", result.unwrap_err()), expected, "{}", msg)
                }
            }
        }
    }
}
