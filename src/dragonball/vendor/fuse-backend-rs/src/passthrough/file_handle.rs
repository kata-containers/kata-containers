// Copyright 2021 Red Hat, Inc. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::CStr;
use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::passthrough::PassthroughFs;

/// An arbitrary maximum size for CFileHandle::f_handle.
///
/// According to Linux ABI, struct file_handle has a flexible array member 'f_handle', but it's
/// hard-coded here for simplicity.
pub const MAX_HANDLE_SZ: usize = 128;

#[derive(Clone, Copy)]
#[repr(C)]
pub(crate) struct CFileHandle {
    // Size of f_handle [in, out]
    pub(crate) handle_bytes: libc::c_uint,
    // Handle type [out]
    pub(crate) handle_type: libc::c_int,
    // File identifier (sized by caller) [out]
    pub(crate) f_handle: [libc::c_char; MAX_HANDLE_SZ],
}

impl CFileHandle {
    fn new() -> Self {
        CFileHandle {
            handle_bytes: MAX_HANDLE_SZ as libc::c_uint,
            handle_type: 0,
            f_handle: [0; MAX_HANDLE_SZ],
        }
    }
}

impl Ord for CFileHandle {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.handle_bytes != other.handle_bytes {
            return self.handle_bytes.cmp(&other.handle_bytes);
        }
        if self.handle_type != other.handle_type {
            return self.handle_type.cmp(&other.handle_type);
        }

        self.f_handle
            .iter()
            .zip(other.f_handle.iter())
            .map(|(x, y)| x.cmp(y))
            .find(|&ord| ord != std::cmp::Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for CFileHandle {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for CFileHandle {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for CFileHandle {}

impl Debug for CFileHandle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "File handle: type {}, len {}",
            self.handle_type, self.handle_bytes
        )
    }
}

/// Struct to maintain information for a file handle.
#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug)]
pub struct FileHandle {
    pub(crate) mnt_id: u64,
    pub(crate) handle: CFileHandle,
}

extern "C" {
    fn name_to_handle_at(
        dirfd: libc::c_int,
        pathname: *const libc::c_char,
        file_handle: *mut CFileHandle,
        mount_id: *mut libc::c_int,
        flags: libc::c_int,
    ) -> libc::c_int;

    // Technically `file_handle` should be a `mut` pointer, but `open_by_handle_at()` is specified
    // not to change it, so we can declare it `const`.
    fn open_by_handle_at(
        mount_fd: libc::c_int,
        file_handle: *const CFileHandle,
        flags: libc::c_int,
    ) -> libc::c_int;
}

impl FileHandle {
    /// Create a file handle for the given file.
    pub fn from_name_at(dir_fd: RawFd, path: &CStr) -> io::Result<Self> {
        let mut mount_id: libc::c_int = 0;
        let mut c_fh = CFileHandle::new();

        let ret = unsafe {
            name_to_handle_at(
                dir_fd,
                path.as_ptr(),
                &mut c_fh,
                &mut mount_id,
                libc::AT_EMPTY_PATH,
            )
        };
        if ret == 0 {
            Ok(FileHandle {
                mnt_id: mount_id as u64,
                handle: c_fh,
            })
        } else {
            let e = io::Error::last_os_error();
            Err(e)
        }
    }

    /// Create a file handle for the given file.
    ///
    /// Also ensure that `mount_fds` contains a valid fd for the mount the file is on (so that
    /// `handle.open_with_mount_fds()` will work).
    ///
    /// If `path` is empty, `reopen_dir` may be invoked to duplicate `dir` with custom
    /// `libc::open()` flags.
    pub fn from_name_at_with_mount_fds<F>(
        dir_fd: RawFd,
        path: &CStr,
        mount_fds: &MountFds,
        reopen_dir: F,
    ) -> io::Result<Self>
    where
        F: FnOnce(RawFd, libc::c_int, u32) -> io::Result<File>,
    {
        let handle = Self::from_name_at(dir_fd, path).map_err(|e| {
            error!("from_name_at failed error {:?}", e);
            e
        })?;

        mount_fds.ensure_mount_point(handle.mnt_id, dir_fd, path, reopen_dir)?;

        Ok(handle)
    }

    /// Open a file handle (low-level wrapper).
    ///
    /// `mount_fd` must be an open non-`O_PATH` file descriptor for an inode on the same mount as
    /// the file to be opened, i.e. the mount given by `self.mnt_id`.
    fn open(&self, mount_fd: &impl AsRawFd, flags: libc::c_int) -> io::Result<File> {
        let ret = unsafe { open_by_handle_at(mount_fd.as_raw_fd(), &self.handle, flags) };
        if ret >= 0 {
            // Safe because `open_by_handle_at()` guarantees this is a valid fd
            let file = unsafe { File::from_raw_fd(ret) };
            Ok(file)
        } else {
            let e = io::Error::last_os_error();
            error!("open_by_handle_at failed error {:?}", e);
            Err(e)
        }
    }

    /// Open a file handle, using the given `mount_fds` hash map.
    ///
    /// Look up `self.mnt_id` in `mount_fds`, and pass the result to `self.open()`.
    pub fn open_with_mount_fds(
        &self,
        mount_fds: &MountFds,
        flags: libc::c_int,
    ) -> io::Result<File> {
        let mount_fds_locked = mount_fds.map.read().unwrap();
        let mount_file = mount_fds_locked.get(&self.mnt_id).ok_or_else(|| {
            error!(
                "open_with_mount_fds: mnt_id {:?} is not found.",
                &self.mnt_id
            );
            io::Error::from_raw_os_error(libc::ENODEV)
        })?;

        self.open(mount_file, flags)
    }
}

/// Struct to maintain <mount ID, mountpoint file> mapping for open_by_handle_at().
///
/// Creating a file handle only returns a mount ID; opening a file handle requires an open fd on the
/// respective mount.  This is a type in which we can store fds that we know are associated with a
/// given mount ID, so that when opening a handle we can look it up.
#[derive(Default)]
pub struct MountFds {
    pub(crate) map: RwLock<HashMap<u64, File>>,
}

impl MountFds {
    pub fn new() -> Self {
        MountFds::default()
    }

    #[allow(dead_code)]
    pub fn get_map(&self) -> RwLockReadGuard<'_, HashMap<u64, std::fs::File>> {
        self.map.read().unwrap()
    }

    #[allow(dead_code)]
    pub fn get_map_mut(&self) -> RwLockWriteGuard<'_, HashMap<u64, std::fs::File>> {
        self.map.write().unwrap()
    }

    fn ensure_mount_point<F>(
        &self,
        mnt_id: u64,
        dir_fd: RawFd,
        path: &CStr,
        reopen_dir: F,
    ) -> io::Result<()>
    where
        F: FnOnce(RawFd, libc::c_int, u32) -> io::Result<File>,
    {
        if self.map.read().unwrap().contains_key(&mnt_id) {
            return Ok(());
        }

        let (path_fd, _path_file) = if path.to_bytes().is_empty() {
            // `open_by_handle_at()` needs a non-`O_PATH` fd, and `dir` may be `O_PATH`, so we
            // have to open a new fd here
            // We do not know whether `dir`/`path` is a special file, though, and we must not open
            // special files with anything but `O_PATH`, so we have to get some `O_PATH` fd first
            // that we can stat to find out whether it is safe to open.
            // (When opening a new fd here, keep a `File` object around so the fd is closed when it
            // goes out of scope.)
            (dir_fd, None)
        } else {
            let f = PassthroughFs::<()>::open_file(
                dir_fd,
                path,
                libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0,
            )
            .map_err(|e| {
                error!(
                    "from_name_at_with_mount_fds: open_file on {:?} failed error {:?}",
                    path, e
                );
                e
            })?;
            (f.as_raw_fd(), Some(f))
        };

        // liubo: TODO find mnt id
        // Ensure that `file` refers to an inode with the mount ID we need
        // if statx(&file, None)?.mnt_id != handle.mnt_id {
        //     return Err(io::Error::from_raw_os_error(libc::EIO));
        // }

        let st = PassthroughFs::<()>::stat_fd(path_fd, None)?;
        // Ensure that we can safely reopen `path_fd` with `O_RDONLY`
        let file_type = st.st_mode & libc::S_IFMT;
        if file_type != libc::S_IFREG && file_type != libc::S_IFDIR {
            error!(
                "from_name_at_with_mount_fds: file {:?} is special file",
                path
            );
            return Err(io::Error::from_raw_os_error(libc::EIO));
        }

        let file = reopen_dir(
            path_fd,
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            st.st_mode,
        )?;
        self.map.write().unwrap().insert(mnt_id, file);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_handle_derives() {
        let mut h1 = CFileHandle {
            handle_bytes: 128,
            handle_type: 3,
            f_handle: [0; MAX_HANDLE_SZ],
        };
        let h2 = CFileHandle {
            handle_bytes: 129,
            handle_type: 3,
            f_handle: [0; MAX_HANDLE_SZ],
        };
        let h3 = CFileHandle {
            handle_bytes: 128,
            handle_type: 4,
            f_handle: [0; MAX_HANDLE_SZ],
        };
        let h4 = CFileHandle {
            handle_bytes: 128,
            handle_type: 4,
            f_handle: [1; MAX_HANDLE_SZ],
        };
        let mut h5 = CFileHandle {
            handle_bytes: 128,
            handle_type: 3,
            f_handle: [0; MAX_HANDLE_SZ],
        };

        assert!(h1 < h2);
        assert!(h1 != h2);
        assert!(h1 < h3);
        assert!(h1 != h3);
        assert!(h1 < h4);
        assert!(h1 != h4);

        assert!(h1 == h5);
        h1.f_handle[0] = 1;
        assert!(h1 > h5);
        h5.f_handle[0] = 1;
        assert!(h1 == h5);
    }
}
