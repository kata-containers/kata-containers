// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::ffi::OsString;
use std::fs;
use std::io::Result;
use std::path::{Path, PathBuf};

use crate::eother;

// from linux.git/fs/fuse/inode.c: #define FUSE_SUPER_MAGIC 0x65735546
const FUSE_SUPER_MAGIC: u32 = 0x65735546;

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

/// Check whether `path` is on a fuse filesystem.
pub fn is_fuse_fs<P: AsRef<Path>>(path: P) -> bool {
    if let Ok(st) = nix::sys::statfs::statfs(path.as_ref()) {
        if st.filesystem_type().0 == FUSE_SUPER_MAGIC as i64 {
            return true;
        }
    }
    false
}

/// Check whether `path` is on a overlay filesystem.
pub fn is_overlay_fs<P: AsRef<Path>>(path: P) -> bool {
    if let Ok(st) = nix::sys::statfs::statfs(path.as_ref()) {
        if st.filesystem_type() == nix::sys::statfs::OVERLAYFS_SUPER_MAGIC {
            return true;
        }
    }
    false
}

/// Check whether the given path is a symlink.
pub fn is_symlink<P: AsRef<Path>>(path: P) -> std::io::Result<bool> {
    let path = path.as_ref();
    let meta = fs::symlink_metadata(path)?;

    Ok(meta.file_type().is_symlink())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mount::umount_all;
    use std::process::Command;
    use thiserror::private::PathAsDisplay;

    #[test]
    fn test_get_base_name() {
        assert_eq!(&get_base_name("/etc/hostname").unwrap(), "hostname");
        assert_eq!(&get_base_name("/bin").unwrap(), "bin");
        assert!(&get_base_name("/").is_err());
        assert!(&get_base_name("").is_err());
        assert!(get_base_name("/no/such/path________yeah").is_err());
    }

    #[test]
    fn test_is_symlink() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path();

        std::os::unix::fs::symlink(path, path.join("a")).unwrap();
        assert!(is_symlink(path.join("a")).unwrap());
    }

    #[test]
    fn test_is_overlayfs() {
        let tmpdir1 = tempfile::tempdir().unwrap();
        let tmpdir2 = tempfile::tempdir().unwrap();
        let tmpdir3 = tempfile::tempdir().unwrap();
        let tmpdir4 = tempfile::tempdir().unwrap();

        let option = format!(
            "-o lowerdir={},upperdir={},workdir={}",
            tmpdir1.path().as_display(),
            tmpdir2.path().display(),
            tmpdir3.path().display()
        );
        let target = format!("{}", tmpdir4.path().display());

        Command::new("/bin/mount")
            .arg("-t overlay")
            .arg(option)
            .arg("overlay")
            .arg(target)
            .output()
            .unwrap();
        assert!(is_overlay_fs(tmpdir4.path()));
        umount_all(tmpdir4.path(), false).unwrap();
    }
}
