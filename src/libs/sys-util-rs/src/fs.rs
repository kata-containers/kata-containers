// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{Error, Result};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{eother, sl};

/// Get bundle path (current working directory).
pub fn get_bundle_path() -> Result<PathBuf> {
    std::env::current_dir()
}

/// Get the basename of the canonicalized path
pub fn get_base_name<P: AsRef<Path>>(src: P) -> Result<OsString> {
    let s = src.as_ref().canonicalize()?;
    s.file_name().map(|v| v.to_os_string()).ok_or_else(|| {
        eother!(
            "failed to get base name of path {}",
            src.as_ref().to_string_lossy()
        )
    })
}

/// Reflink copy src to dst, and falls back to regular copy if reflink copy fails.
pub fn reflink_copy<S: AsRef<Path>, D: AsRef<Path>>(src: S, dst: D) -> Result<()> {
    let src_path = src.as_ref();
    let dst_path = dst.as_ref();
    let src = src_path.to_string_lossy();
    let dst = dst_path.to_string_lossy();

    let src_info = fs::metadata(src_path)?;
    if !src_info.is_file() {
        return Err(eother!("reflink_copy src {} is not a regular file", src));
    }

    // Make sure dst's parent exist. If dst is a regular file, then unlink it for later copy.
    if dst_path.exists() {
        let dst_info = fs::metadata(dst_path)?;
        if !dst_info.is_file() {
            return Err(eother!("reflink_copy dst {} is not a regular file", dst));
        } else {
            nix::unistd::unlink(dst_path)?;
        }
    } else if let Some(dst_parent) = dst_path.parent() {
        if !dst_parent.exists() {
            if let Err(e) = fs::create_dir_all(dst_parent) {
                return Err(eother!(
                    "reflink_copy: create_dir_all {} failed: {:?}",
                    dst_parent.to_str().unwrap(),
                    e
                ));
            }
        } else {
            let md = dst_parent.metadata()?;
            if !md.is_dir() {
                return Err(eother!("reflink_copy parent of {} is not a directory", dst));
            }
        }
    }

    // Reflink copy, and fallback to regular copy if reflink fails.
    let src_file = fs::File::open(src_path)?;
    let dst_file = fs::File::create(dst_path)?;
    if let Err(e) = do_reflink_copy(src_file, dst_file) {
        match e.raw_os_error() {
            // Cross dev copy or filesystem doesn't support reflink, do regular copy
            Some(os_err)
                if os_err == nix::Error::EXDEV as i32
                    || os_err == nix::Error::EOPNOTSUPP as i32 =>
            {
                warn!(
                    sl!(),
                    "reflink_copy: reflink is not supported ({:?}), do regular copy instead", e,
                );
                if let Err(e) = do_regular_copy(src.as_ref(), dst.as_ref()) {
                    return Err(eother!(
                        "reflink_copy: regular copy {} to {} failed: {:?}",
                        src,
                        dst,
                        e
                    ));
                }
            }
            // Reflink copy failed
            _ => {
                return Err(eother!(
                    "reflink_copy: copy {} to {} failed: {:?}",
                    src,
                    dst,
                    e,
                ))
            }
        }
    }

    Ok(())
}

// Copy file using cp command, which handles sparse file copy.
fn do_regular_copy(src: &str, dst: &str) -> Result<()> {
    match Command::new("cp")
        .args(&["--sparse=auto", src, dst])
        .output()
    {
        Ok(output) => match output.status.success() {
            true => Ok(()),
            false => Err(eother!("cp {} {} failed: {:?}", src, dst, output)),
        },
        Err(e) => Err(eother!("cp {} {} failed: {:?}", src, dst, e)),
    }
}

/// Copy file by reflink
fn do_reflink_copy(src: File, dst: File) -> Result<()> {
    use nix::ioctl_write_int;
    // FICLONE ioctl number definition, from include/linux/fs.h
    const FS_IOC_MAGIC: u8 = 0x94;
    const FS_IOC_FICLONE: u8 = 9;
    // Define FICLONE ioctl using nix::ioctl_write_int! macro.
    // The generated function has the following signature:
    // pub unsafe fn ficlone(fd: libc::c_int, data: libc::c_ulang) -> Result<libc::c_int>
    ioctl_write_int!(ficlone, FS_IOC_MAGIC, FS_IOC_FICLONE);

    // Safe because the `src` and `dst` are valid file objects and we have checked the result.
    unsafe { ficlone(dst.as_raw_fd(), src.as_raw_fd() as u64) }
        .map(|_| ())
        .map_err(|e| Error::from_raw_os_error(e as i32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_base_name() {
        assert_eq!(&get_base_name("/etc/hostname").unwrap(), "hostname");
        assert_eq!(&get_base_name("/bin").unwrap(), "bin");
        assert!(&get_base_name("/").is_err());
        assert!(&get_base_name("").is_err());
        assert!(get_base_name("/no/such/path________yeah").is_err());
    }

    #[test]
    fn test_reflink_copy() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().join("mounts");
        reflink_copy("/proc/mounts", &path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty());
        reflink_copy("/proc/mounts", &path).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.is_empty());

        reflink_copy("/proc/mounts", tmpdir.path()).unwrap_err();
        reflink_copy("/proc/mounts_not_exist", &path).unwrap_err();
    }
}
