// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! Path resolution within a directory tree using libpathrs.

use anyhow::{bail, Context, Result};
use pathrs::error::ErrorKind;
use pathrs::procfs::{ProcfsBase, ProcfsHandle};
use pathrs::Root;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

// Linux permits at most 40 symlink traversals during one path lookup.
const MAX_DANGLING_SYMLINKS: usize = 40;

fn handle_path(procfs: &ProcfsHandle, handle: impl AsFd) -> Result<PathBuf> {
    Ok(procfs.readlink(
        ProcfsBase::ProcSelf,
        format!("fd/{}", handle.as_fd().as_raw_fd()),
    )?)
}

/// Join a path to an existing root, resolving symlinks as if inside a chroot.
///
/// Absolute paths and `..` components are constrained to `root`. Missing
/// components are allowed so callers can construct paths before creating them.
/// Symlink loops and non-directory intermediate components return errors.
/// No filesystem objects are created by this function.
///
/// # Security
///
/// This returns a pathname, not a pinned file descriptor. The caller must ensure
/// the directory tree cannot change between resolution and use. For operations
/// on a tree that can change concurrently, use [`pathrs::Root`] and its handles
/// directly instead.
pub fn scoped_join(root: impl AsRef<Path>, path: impl AsRef<Path>) -> Result<PathBuf> {
    if path.as_ref().as_os_str().as_bytes().contains(&0) {
        bail!("path contains a NUL byte");
    }
    let root_path = root
        .as_ref()
        .canonicalize()
        .context("canonicalize path resolution root")?;
    let root = Root::open(&root_path).context("open path resolution root")?;
    let procfs = ProcfsHandle::new()?;
    let root_path = handle_path(&procfs, &root)?;
    let mut relative = PathBuf::new();

    let mut remaining = path.as_ref().to_path_buf();
    let mut dangling_links = 0;
    'resolve: loop {
        let mut components = remaining.components();
        while let Some(component) = components.next() {
            match component {
                Component::RootDir => relative.clear(),
                Component::CurDir => continue,
                Component::ParentDir => {
                    relative.pop();
                    continue;
                }
                Component::Normal(name) => relative.push(name),
                Component::Prefix(_) => bail!("invalid Linux path prefix"),
            }

            match root.resolve(&relative) {
                Ok(handle) => {
                    let resolved = handle_path(&procfs, &handle)?;
                    relative = resolved
                        .strip_prefix(&root_path)
                        .context("resolved path moved outside root")?
                        .to_path_buf();
                }
                Err(error) if error.kind() == ErrorKind::OsError(Some(nix::libc::ENOENT)) => {
                    // libpathrs requires existing targets. Distinguish a missing
                    // component from a symlink whose target is missing, and use
                    // libpathrs to read that link before resolving its target.
                    match root.resolve_nofollow(&relative) {
                        Err(missing)
                            if missing.kind() == ErrorKind::OsError(Some(nix::libc::ENOENT)) => {}
                        Err(other) => return Err(other.into()),
                        Ok(_) => {
                            dangling_links += 1;
                            if dangling_links > MAX_DANGLING_SYMLINKS {
                                bail!("too many dangling symlinks in path");
                            }
                            let target = root.readlink(&relative)?;
                            relative.pop();
                            remaining = target.join(components.as_path());
                            continue 'resolve;
                        }
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
        break;
    }

    Ok(root_path.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::tempdir;

    #[test]
    fn missing_paths_are_scoped_without_creating_them() {
        let root = tempdir().unwrap();
        for (input, expected) in [
            ("", ""),
            ("/", ""),
            ("..", ""),
            ("../../../a/b", "a/b"),
            ("/a/./b/../../c", "c"),
            ("missing/../other/child", "other/child"),
        ] {
            assert_eq!(
                scoped_join(root.path(), input).unwrap(),
                root.path().join(expected),
                "{input}"
            );
        }
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn symlinks_are_resolved_inside_root() {
        let root = tempdir().unwrap();
        fs::create_dir_all(root.path().join("real/child")).unwrap();
        symlink("/real/child", root.path().join("absolute")).unwrap();
        symlink("../../real", root.path().join("relative")).unwrap();
        for (input, expected) in [
            ("absolute/missing", "real/child/missing"),
            ("absolute/../missing", "real/missing"),
            ("relative/child", "real/child"),
            ("missing/../absolute", "real/child"),
            ("absolute/../../../../missing", "missing"),
        ] {
            assert_eq!(
                scoped_join(root.path(), input).unwrap(),
                root.path().join(expected),
                "{input}"
            );
        }
    }

    #[test]
    fn dangling_symlink_targets_remain_scoped() {
        let root = tempdir().unwrap();
        fs::create_dir(root.path().join("dir")).unwrap();
        symlink("/missing", root.path().join("absolute")).unwrap();
        symlink("../../missing", root.path().join("dir/relative")).unwrap();
        symlink("/absolute", root.path().join("chain")).unwrap();
        for input in ["absolute/child", "dir/relative/child", "chain/child"] {
            assert_eq!(
                scoped_join(root.path(), input).unwrap(),
                root.path().join("missing/child"),
                "{}",
                input
            );
        }
        assert_eq!(
            scoped_join(root.path(), "absolute/../other").unwrap(),
            root.path().join("other")
        );
        assert!(!root.path().join("missing").exists());
    }

    #[test]
    fn invalid_components_are_not_treated_as_missing() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("file"), b"").unwrap();
        symlink("loop", root.path().join("loop")).unwrap();
        symlink("missing/../cycle", root.path().join("cycle")).unwrap();
        for input in ["file/child", "loop", "cycle", "a\0b"] {
            assert!(scoped_join(root.path(), input).is_err(), "{:?}", input);
        }
        assert!(scoped_join(root.path().join("missing"), "child").is_err());
    }

    #[test]
    fn magic_links_are_rejected() {
        let root = tempdir().unwrap();
        let file = fs::File::create(root.path().join("file")).unwrap();
        assert!(scoped_join("/proc", format!("self/fd/{}", file.as_raw_fd())).is_err());
    }

    #[test]
    fn root_symlinks_and_non_utf8_names_are_supported() {
        let parent = tempdir().unwrap();
        let root = parent.path().join("root");
        fs::create_dir(&root).unwrap();
        let alias = parent.path().join("alias");
        symlink(&root, &alias).unwrap();
        let name = OsStr::from_bytes(b"name\xff");
        fs::write(root.join(name), b"").unwrap();
        assert_eq!(scoped_join(&alias, name).unwrap(), root.join(name));
    }
}
