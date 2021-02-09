use {Error, NixPath, Result};
use errno::Errno;
use fcntl::{self, OFlag};
use libc;
use std::os::unix::io::{AsRawFd, IntoRawFd, RawFd};
use std::{ffi, ptr};
use sys;

#[cfg(target_os = "linux")]
use libc::{dirent64 as dirent, readdir64_r as readdir_r};

#[cfg(not(target_os = "linux"))]
use libc::{dirent, readdir_r};

/// An open directory.
///
/// This is a lower-level interface than `std::fs::ReadDir`. Notable differences:
///    * can be opened from a file descriptor (as returned by `openat`, perhaps before knowing
///      if the path represents a file or directory).
///    * implements `AsRawFd`, so it can be passed to `fstat`, `openat`, etc.
///      The file descriptor continues to be owned by the `Dir`, so callers must not keep a `RawFd`
///      after the `Dir` is dropped.
///    * can be iterated through multiple times without closing and reopening the file
///      descriptor. Each iteration rewinds when finished.
///    * returns entries for `.` (current directory) and `..` (parent directory).
///    * returns entries' names as a `CStr` (no allocation or conversion beyond whatever libc
///      does).
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Dir(
    ptr::NonNull<libc::DIR>
);

impl Dir {
    /// Opens the given path as with `fcntl::open`.
    pub fn open<P: ?Sized + NixPath>(path: &P, oflag: OFlag,
                                     mode: sys::stat::Mode) -> Result<Self> {
        let fd = fcntl::open(path, oflag, mode)?;
        Dir::from_fd(fd)
    }

    /// Opens the given path as with `fcntl::openat`.
    pub fn openat<P: ?Sized + NixPath>(dirfd: RawFd, path: &P, oflag: OFlag,
                                       mode: sys::stat::Mode) -> Result<Self> {
        let fd = fcntl::openat(dirfd, path, oflag, mode)?;
        Dir::from_fd(fd)
    }

    /// Converts from a descriptor-based object, closing the descriptor on success or failure.
    #[inline]
    pub fn from<F: IntoRawFd>(fd: F) -> Result<Self> {
        Dir::from_fd(fd.into_raw_fd())
    }

    /// Converts from a file descriptor, closing it on success or failure.
    pub fn from_fd(fd: RawFd) -> Result<Self> {
        let d = unsafe { libc::fdopendir(fd) };
        if d.is_null() {
            let e = Error::last();
            unsafe { libc::close(fd) };
            return Err(e);
        };
        // Always guaranteed to be non-null by the previous check
        Ok(Dir(ptr::NonNull::new(d).unwrap()))
    }

    /// Returns an iterator of `Result<Entry>` which rewinds when finished.
    pub fn iter(&mut self) -> Iter {
        Iter(self)
    }
}

// `Dir` is not `Sync`. With the current implementation, it could be, but according to
// https://www.gnu.org/software/libc/manual/html_node/Reading_002fClosing-Directory.html,
// future versions of POSIX are likely to obsolete `readdir_r` and specify that it's unsafe to
// call `readdir` simultaneously from multiple threads.
//
// `Dir` is safe to pass from one thread to another, as it's not reference-counted.
unsafe impl Send for Dir {}

impl AsRawFd for Dir {
    fn as_raw_fd(&self) -> RawFd {
        unsafe { libc::dirfd(self.0.as_ptr()) }
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        unsafe { libc::closedir(self.0.as_ptr()) };
    }
}

#[derive(Debug, Eq, Hash, PartialEq)]
pub struct Iter<'d>(&'d mut Dir);

impl<'d> Iterator for Iter<'d> {
    type Item = Result<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            // Note: POSIX specifies that portable applications should dynamically allocate a
            // buffer with room for a `d_name` field of size `pathconf(..., _PC_NAME_MAX)` plus 1
            // for the NUL byte. It doesn't look like the std library does this; it just uses
            // fixed-sized buffers (and libc's dirent seems to be sized so this is appropriate).
            // Probably fine here too then.
            let mut ent = std::mem::MaybeUninit::<dirent>::uninit();
            let mut result = ptr::null_mut();
            if let Err(e) = Errno::result(
                readdir_r((self.0).0.as_ptr(), ent.as_mut_ptr(), &mut result))
            {
                return Some(Err(e));
            }
            if result.is_null() {
                return None;
            }
            assert_eq!(result, ent.as_mut_ptr());
            Some(Ok(Entry(ent.assume_init())))
        }
    }
}

impl<'d> Drop for Iter<'d> {
    fn drop(&mut self) {
        unsafe { libc::rewinddir((self.0).0.as_ptr()) }
    }
}

/// A directory entry, similar to `std::fs::DirEntry`.
///
/// Note that unlike the std version, this may represent the `.` or `..` entries.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct Entry(dirent);

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    Fifo,
    CharacterDevice,
    Directory,
    BlockDevice,
    File,
    Symlink,
    Socket,
}

impl Entry {
    /// Returns the inode number (`d_ino`) of the underlying `dirent`.
    #[cfg(any(target_os = "android",
              target_os = "emscripten",
              target_os = "fuchsia",
              target_os = "haiku",
              target_os = "ios",
              target_os = "l4re",
              target_os = "linux",
              target_os = "macos",
              target_os = "solaris"))]
    pub fn ino(&self) -> u64 {
        self.0.d_ino as u64
    }

    /// Returns the inode number (`d_fileno`) of the underlying `dirent`.
    #[cfg(not(any(target_os = "android",
                  target_os = "emscripten",
                  target_os = "fuchsia",
                  target_os = "haiku",
                  target_os = "ios",
                  target_os = "l4re",
                  target_os = "linux",
                  target_os = "macos",
                  target_os = "solaris")))]
    pub fn ino(&self) -> u64 {
        u64::from(self.0.d_fileno)
    }

    /// Returns the bare file name of this directory entry without any other leading path component.
    pub fn file_name(&self) -> &ffi::CStr {
        unsafe { ::std::ffi::CStr::from_ptr(self.0.d_name.as_ptr()) }
    }

    /// Returns the type of this directory entry, if known.
    ///
    /// See platform `readdir(3)` or `dirent(5)` manpage for when the file type is known;
    /// notably, some Linux filesystems don't implement this. The caller should use `stat` or
    /// `fstat` if this returns `None`.
    pub fn file_type(&self) -> Option<Type> {
        match self.0.d_type {
            libc::DT_FIFO => Some(Type::Fifo),
            libc::DT_CHR => Some(Type::CharacterDevice),
            libc::DT_DIR => Some(Type::Directory),
            libc::DT_BLK => Some(Type::BlockDevice),
            libc::DT_REG => Some(Type::File),
            libc::DT_LNK => Some(Type::Symlink),
            libc::DT_SOCK => Some(Type::Socket),
            /* libc::DT_UNKNOWN | */ _ => None,
        }
    }
}
