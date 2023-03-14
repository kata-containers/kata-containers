pub use libc::{dev_t, mode_t};
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "openbsd"))]
pub use libc::c_uint;
#[cfg(any(
    target_os = "netbsd",
    target_os = "freebsd",
    target_os = "dragonfly"
))]
pub use libc::c_ulong;
pub use libc::stat as FileStat;

use crate::{Result, NixPath, errno::Errno};
#[cfg(not(target_os = "redox"))]
use crate::fcntl::{AtFlags, at_rawfd};
use std::mem;
use std::os::unix::io::RawFd;
use crate::sys::time::{TimeSpec, TimeVal};

libc_bitflags!(
    /// "File type" flags for `mknod` and related functions.
    pub struct SFlag: mode_t {
        S_IFIFO;
        S_IFCHR;
        S_IFDIR;
        S_IFBLK;
        S_IFREG;
        S_IFLNK;
        S_IFSOCK;
        S_IFMT;
    }
);

libc_bitflags! {
    /// "File mode / permissions" flags.
    pub struct Mode: mode_t {
        S_IRWXU;
        S_IRUSR;
        S_IWUSR;
        S_IXUSR;
        S_IRWXG;
        S_IRGRP;
        S_IWGRP;
        S_IXGRP;
        S_IRWXO;
        S_IROTH;
        S_IWOTH;
        S_IXOTH;
        S_ISUID as mode_t;
        S_ISGID as mode_t;
        S_ISVTX as mode_t;
    }
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os="openbsd"))]
pub type type_of_file_flag = c_uint;
#[cfg(any(
    target_os = "netbsd",
    target_os = "freebsd",
    target_os = "dragonfly"
))]
pub type type_of_file_flag = c_ulong;

#[cfg(any(
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "macos",
    target_os = "ios"
))]
libc_bitflags! {
    /// File flags.
    #[cfg_attr(docsrs, doc(cfg(all())))]
    pub struct FileFlag: type_of_file_flag {
        /// The file may only be appended to.
        SF_APPEND;
        /// The file has been archived.
        SF_ARCHIVED;
        #[cfg(any(target_os = "dragonfly"))]
        SF_CACHE;
        /// The file may not be changed.
        SF_IMMUTABLE;
        /// Indicates a WAPBL journal file.
        #[cfg(any(target_os = "netbsd"))]
        SF_LOG;
        /// Do not retain history for file
        #[cfg(any(target_os = "dragonfly"))]
        SF_NOHISTORY;
        /// The file may not be renamed or deleted.
        #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
        SF_NOUNLINK;
        /// Mask of superuser changeable flags
        SF_SETTABLE;
        /// Snapshot is invalid.
        #[cfg(any(target_os = "netbsd"))]
        SF_SNAPINVAL;
        /// The file is a snapshot file.
        #[cfg(any(target_os = "netbsd", target_os = "freebsd"))]
        SF_SNAPSHOT;
        #[cfg(any(target_os = "dragonfly"))]
        SF_XLINK;
        /// The file may only be appended to.
        UF_APPEND;
        /// The file needs to be archived.
        #[cfg(any(target_os = "freebsd"))]
        UF_ARCHIVE;
        #[cfg(any(target_os = "dragonfly"))]
        UF_CACHE;
        /// File is compressed at the file system level.
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        UF_COMPRESSED;
        /// The file may be hidden from directory listings at the application's
        /// discretion.
        #[cfg(any(
            target_os = "freebsd",
            target_os = "macos",
            target_os = "ios",
        ))]
        UF_HIDDEN;
        /// The file may not be changed.
        UF_IMMUTABLE;
        /// Do not dump the file.
        UF_NODUMP;
        #[cfg(any(target_os = "dragonfly"))]
        UF_NOHISTORY;
        /// The file may not be renamed or deleted.
        #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
        UF_NOUNLINK;
        /// The file is offline, or has the Windows and CIFS
        /// `FILE_ATTRIBUTE_OFFLINE` attribute.
        #[cfg(any(target_os = "freebsd"))]
        UF_OFFLINE;
        /// The directory is opaque when viewed through a union stack.
        UF_OPAQUE;
        /// The file is read only, and may not be written or appended.
        #[cfg(any(target_os = "freebsd"))]
        UF_READONLY;
        /// The file contains a Windows reparse point.
        #[cfg(any(target_os = "freebsd"))]
        UF_REPARSE;
        /// Mask of owner changeable flags.
        UF_SETTABLE;
        /// The file has the Windows `FILE_ATTRIBUTE_SPARSE_FILE` attribute.
        #[cfg(any(target_os = "freebsd"))]
        UF_SPARSE;
        /// The file has the DOS, Windows and CIFS `FILE_ATTRIBUTE_SYSTEM`
        /// attribute.
        #[cfg(any(target_os = "freebsd"))]
        UF_SYSTEM;
        /// File renames and deletes are tracked.
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        UF_TRACKED;
        #[cfg(any(target_os = "dragonfly"))]
        UF_XLINK;
    }
}

/// Create a special or ordinary file, by pathname.
pub fn mknod<P: ?Sized + NixPath>(path: &P, kind: SFlag, perm: Mode, dev: dev_t) -> Result<()> {
    let res = path.with_nix_path(|cstr| unsafe {
        libc::mknod(cstr.as_ptr(), kind.bits | perm.bits() as mode_t, dev)
    })?;

    Errno::result(res).map(drop)
}

/// Create a special or ordinary file, relative to a given directory.
#[cfg(not(any(target_os = "ios", target_os = "macos", target_os = "redox", target_os = "haiku")))]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub fn mknodat<P: ?Sized + NixPath>(
    dirfd: RawFd,
    path: &P,
    kind: SFlag,
    perm: Mode,
    dev: dev_t,
) -> Result<()> {
    let res = path.with_nix_path(|cstr| unsafe {
        libc::mknodat(dirfd, cstr.as_ptr(), kind.bits | perm.bits() as mode_t, dev)
    })?;

    Errno::result(res).map(drop)
}

#[cfg(target_os = "linux")]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub const fn major(dev: dev_t) -> u64 {
    ((dev >> 32) & 0xffff_f000) |
    ((dev >>  8) & 0x0000_0fff)
}

#[cfg(target_os = "linux")]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub const fn minor(dev: dev_t) -> u64 {
    ((dev >> 12) & 0xffff_ff00) |
    ((dev      ) & 0x0000_00ff)
}

#[cfg(target_os = "linux")]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub const fn makedev(major: u64, minor: u64) -> dev_t {
    ((major & 0xffff_f000) << 32) |
    ((major & 0x0000_0fff) <<  8) |
    ((minor & 0xffff_ff00) << 12) |
     (minor & 0x0000_00ff)
}

pub fn umask(mode: Mode) -> Mode {
    let prev = unsafe { libc::umask(mode.bits() as mode_t) };
    Mode::from_bits(prev).expect("[BUG] umask returned invalid Mode")
}

pub fn stat<P: ?Sized + NixPath>(path: &P) -> Result<FileStat> {
    let mut dst = mem::MaybeUninit::uninit();
    let res = path.with_nix_path(|cstr| {
        unsafe {
            libc::stat(cstr.as_ptr(), dst.as_mut_ptr())
        }
    })?;

    Errno::result(res)?;

    Ok(unsafe{dst.assume_init()})
}

pub fn lstat<P: ?Sized + NixPath>(path: &P) -> Result<FileStat> {
    let mut dst = mem::MaybeUninit::uninit();
    let res = path.with_nix_path(|cstr| {
        unsafe {
            libc::lstat(cstr.as_ptr(), dst.as_mut_ptr())
        }
    })?;

    Errno::result(res)?;

    Ok(unsafe{dst.assume_init()})
}

pub fn fstat(fd: RawFd) -> Result<FileStat> {
    let mut dst = mem::MaybeUninit::uninit();
    let res = unsafe { libc::fstat(fd, dst.as_mut_ptr()) };

    Errno::result(res)?;

    Ok(unsafe{dst.assume_init()})
}

#[cfg(not(target_os = "redox"))]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub fn fstatat<P: ?Sized + NixPath>(dirfd: RawFd, pathname: &P, f: AtFlags) -> Result<FileStat> {
    let mut dst = mem::MaybeUninit::uninit();
    let res = pathname.with_nix_path(|cstr| {
        unsafe { libc::fstatat(dirfd, cstr.as_ptr(), dst.as_mut_ptr(), f.bits() as libc::c_int) }
    })?;

    Errno::result(res)?;

    Ok(unsafe{dst.assume_init()})
}

/// Change the file permission bits of the file specified by a file descriptor.
///
/// # References
///
/// [fchmod(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/fchmod.html).
pub fn fchmod(fd: RawFd, mode: Mode) -> Result<()> {
    let res = unsafe { libc::fchmod(fd, mode.bits() as mode_t) };

    Errno::result(res).map(drop)
}

/// Flags for `fchmodat` function.
#[derive(Clone, Copy, Debug)]
pub enum FchmodatFlags {
    FollowSymlink,
    NoFollowSymlink,
}

/// Change the file permission bits.
///
/// The file to be changed is determined relative to the directory associated
/// with the file descriptor `dirfd` or the current working directory
/// if `dirfd` is `None`.
///
/// If `flag` is `FchmodatFlags::NoFollowSymlink` and `path` names a symbolic link,
/// then the mode of the symbolic link is changed.
///
/// `fchmodat(None, path, mode, FchmodatFlags::FollowSymlink)` is identical to
/// a call `libc::chmod(path, mode)`. That's why `chmod` is unimplemented
/// in the `nix` crate.
///
/// # References
///
/// [fchmodat(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/fchmodat.html).
#[cfg(not(target_os = "redox"))]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub fn fchmodat<P: ?Sized + NixPath>(
    dirfd: Option<RawFd>,
    path: &P,
    mode: Mode,
    flag: FchmodatFlags,
) -> Result<()> {
    let atflag =
        match flag {
            FchmodatFlags::FollowSymlink => AtFlags::empty(),
            FchmodatFlags::NoFollowSymlink => AtFlags::AT_SYMLINK_NOFOLLOW,
        };
    let res = path.with_nix_path(|cstr| unsafe {
        libc::fchmodat(
            at_rawfd(dirfd),
            cstr.as_ptr(),
            mode.bits() as mode_t,
            atflag.bits() as libc::c_int,
        )
    })?;

    Errno::result(res).map(drop)
}

/// Change the access and modification times of a file.
///
/// `utimes(path, times)` is identical to
/// `utimensat(None, path, times, UtimensatFlags::FollowSymlink)`. The former
/// is a deprecated API so prefer using the latter if the platforms you care
/// about support it.
///
/// # References
///
/// [utimes(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/utimes.html).
pub fn utimes<P: ?Sized + NixPath>(path: &P, atime: &TimeVal, mtime: &TimeVal) -> Result<()> {
    let times: [libc::timeval; 2] = [*atime.as_ref(), *mtime.as_ref()];
    let res = path.with_nix_path(|cstr| unsafe {
        libc::utimes(cstr.as_ptr(), &times[0])
    })?;

    Errno::result(res).map(drop)
}

/// Change the access and modification times of a file without following symlinks.
///
/// `lutimes(path, times)` is identical to
/// `utimensat(None, path, times, UtimensatFlags::NoFollowSymlink)`. The former
/// is a deprecated API so prefer using the latter if the platforms you care
/// about support it.
///
/// # References
///
/// [lutimes(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/lutimes.html).
#[cfg(any(target_os = "linux",
          target_os = "haiku",
          target_os = "ios",
          target_os = "macos",
          target_os = "freebsd",
          target_os = "netbsd"))]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub fn lutimes<P: ?Sized + NixPath>(path: &P, atime: &TimeVal, mtime: &TimeVal) -> Result<()> {
    let times: [libc::timeval; 2] = [*atime.as_ref(), *mtime.as_ref()];
    let res = path.with_nix_path(|cstr| unsafe {
        libc::lutimes(cstr.as_ptr(), &times[0])
    })?;

    Errno::result(res).map(drop)
}

/// Change the access and modification times of the file specified by a file descriptor.
///
/// # References
///
/// [futimens(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/futimens.html).
#[inline]
pub fn futimens(fd: RawFd, atime: &TimeSpec, mtime: &TimeSpec) -> Result<()> {
    let times: [libc::timespec; 2] = [*atime.as_ref(), *mtime.as_ref()];
    let res = unsafe { libc::futimens(fd, &times[0]) };

    Errno::result(res).map(drop)
}

/// Flags for `utimensat` function.
// TODO: replace with fcntl::AtFlags
#[derive(Clone, Copy, Debug)]
pub enum UtimensatFlags {
    FollowSymlink,
    NoFollowSymlink,
}

/// Change the access and modification times of a file.
///
/// The file to be changed is determined relative to the directory associated
/// with the file descriptor `dirfd` or the current working directory
/// if `dirfd` is `None`.
///
/// If `flag` is `UtimensatFlags::NoFollowSymlink` and `path` names a symbolic link,
/// then the mode of the symbolic link is changed.
///
/// `utimensat(None, path, times, UtimensatFlags::FollowSymlink)` is identical to
/// `utimes(path, times)`. The latter is a deprecated API so prefer using the
/// former if the platforms you care about support it.
///
/// # References
///
/// [utimensat(2)](https://pubs.opengroup.org/onlinepubs/9699919799/functions/utimens.html).
#[cfg(not(target_os = "redox"))]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub fn utimensat<P: ?Sized + NixPath>(
    dirfd: Option<RawFd>,
    path: &P,
    atime: &TimeSpec,
    mtime: &TimeSpec,
    flag: UtimensatFlags
) -> Result<()> {
    let atflag =
        match flag {
            UtimensatFlags::FollowSymlink => AtFlags::empty(),
            UtimensatFlags::NoFollowSymlink => AtFlags::AT_SYMLINK_NOFOLLOW,
        };
    let times: [libc::timespec; 2] = [*atime.as_ref(), *mtime.as_ref()];
    let res = path.with_nix_path(|cstr| unsafe {
        libc::utimensat(
            at_rawfd(dirfd),
            cstr.as_ptr(),
            &times[0],
            atflag.bits() as libc::c_int,
        )
    })?;

    Errno::result(res).map(drop)
}

#[cfg(not(target_os = "redox"))]
#[cfg_attr(docsrs, doc(cfg(all())))]
pub fn mkdirat<P: ?Sized + NixPath>(fd: RawFd, path: &P, mode: Mode) -> Result<()> {
    let res = path.with_nix_path(|cstr| {
        unsafe { libc::mkdirat(fd, cstr.as_ptr(), mode.bits() as mode_t) }
    })?;

    Errno::result(res).map(drop)
}
