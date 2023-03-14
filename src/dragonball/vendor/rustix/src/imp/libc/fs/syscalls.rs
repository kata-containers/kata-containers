//! libc syscalls supporting `rustix::fs`.

use super::super::c;
use super::super::conv::{
    borrowed_fd, c_str, ret, ret_c_int, ret_off_t, ret_owned_fd, ret_ssize_t,
};
#[cfg(any(target_os = "android", target_os = "linux"))]
use super::super::conv::{syscall_ret, syscall_ret_owned_fd, syscall_ret_ssize_t};
#[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
use super::super::offset::libc_fallocate;
#[cfg(not(any(
    target_os = "dragonfly",
    target_os = "illumos",
    target_os = "ios",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "redox",
)))]
use super::super::offset::libc_posix_fadvise;
#[cfg(not(any(
    target_os = "android",
    target_os = "dragonfly",
    target_os = "fuchsia",
    target_os = "illumos",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "redox",
)))]
use super::super::offset::libc_posix_fallocate;
use super::super::offset::{libc_fstat, libc_fstatat, libc_ftruncate, libc_lseek, libc_off_t};
#[cfg(not(any(
    target_os = "illumos",
    target_os = "netbsd",
    target_os = "redox",
    target_os = "wasi"
)))]
use super::super::offset::{libc_fstatfs, libc_statfs};
use crate::as_ptr;
use crate::fd::BorrowedFd;
#[cfg(not(target_os = "wasi"))]
use crate::fd::RawFd;
use crate::ffi::ZStr;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use crate::ffi::ZString;
#[cfg(not(target_os = "illumos"))]
use crate::fs::Access;
#[cfg(not(any(
    target_os = "dragonfly",
    target_os = "illumos",
    target_os = "ios",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "redox",
)))]
use crate::fs::Advice;
#[cfg(not(any(
    target_os = "dragonfly",
    target_os = "illumos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "redox"
)))]
use crate::fs::FallocateFlags;
#[cfg(not(target_os = "wasi"))]
use crate::fs::FlockOperation;
#[cfg(any(target_os = "android", target_os = "freebsd", target_os = "linux"))]
use crate::fs::MemfdFlags;
#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "linux",
))]
use crate::fs::SealFlags;
#[cfg(not(any(
    target_os = "illumos",
    target_os = "netbsd",
    target_os = "redox",
    target_os = "wasi"
)))]
// not implemented in libc for netbsd yet
use crate::fs::StatFs;
#[cfg(not(any(
    target_os = "ios",
    target_os = "macos",
    target_os = "redox",
    target_os = "wasi",
)))]
use crate::fs::{Dev, FileType};
use crate::fs::{FdFlags, Mode, OFlags, Stat, Timestamps};
#[cfg(any(target_os = "android", target_os = "linux"))]
use crate::fs::{RenameFlags, ResolveFlags, Statx, StatxFlags};
use crate::io::{self, OwnedFd, SeekFrom};
#[cfg(not(target_os = "wasi"))]
use crate::process::{Gid, Uid};
use core::convert::TryInto;
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
use core::mem::size_of;
#[cfg(target_os = "linux")]
use core::mem::transmute;
use core::mem::MaybeUninit;
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos"
))]
use core::ptr::null_mut;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use {
    super::super::conv::nonnegative_ret,
    crate::fs::{copyfile_state_t, CloneFlags, CopyfileFlags},
};
#[cfg(not(target_os = "redox"))]
use {super::super::offset::libc_openat, crate::fs::AtFlags};

#[cfg(all(unix, target_env = "gnu"))]
/// Use direct syscall (via libc) for openat().
/// Only currently necessary as a workaround for old glibc; see below.
fn openat_via_syscall(
    dirfd: BorrowedFd<'_>,
    path: &ZStr,
    oflags: OFlags,
    mode: Mode,
) -> io::Result<OwnedFd> {
    unsafe {
        let dirfd = borrowed_fd(dirfd);
        let path = c_str(path);
        let oflags = oflags.bits();
        let mode = c::c_uint::from(mode.bits());
        ret_owned_fd(libc::syscall(
            libc::SYS_openat,
            dirfd as c::c_long,
            path,
            oflags as c::c_long,
            mode as c::c_long,
        ) as c::c_int)
    }
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn openat(
    dirfd: BorrowedFd<'_>,
    path: &ZStr,
    oflags: OFlags,
    mode: Mode,
) -> io::Result<OwnedFd> {
    // Work around https://sourceware.org/bugzilla/show_bug.cgi?id=17523
    // Basically old glibc versions don't handle O_TMPFILE correctly.
    #[cfg(all(unix, target_env = "gnu"))]
    if oflags.contains(OFlags::TMPFILE) && crate::imp::if_glibc_is_less_than_2_25() {
        return openat_via_syscall(dirfd, path, oflags, mode);
    }
    unsafe {
        // Pass `mode` as a `c_uint` even if `mode_t` is narrower, since
        // `libc_openat` is declared as a variadic function and narrower
        // arguments are promoted.
        ret_owned_fd(libc_openat(
            borrowed_fd(dirfd),
            c_str(path),
            oflags.bits(),
            c::c_uint::from(mode.bits()),
        ))
    }
}

#[cfg(not(any(
    target_os = "illumos",
    target_os = "netbsd",
    target_os = "redox",
    target_os = "wasi"
)))]
#[inline]
pub(crate) fn statfs(filename: &ZStr) -> io::Result<StatFs> {
    unsafe {
        let mut result = MaybeUninit::<StatFs>::uninit();
        ret(libc_statfs(c_str(filename), result.as_mut_ptr()))?;
        Ok(result.assume_init())
    }
}

#[cfg(not(target_os = "redox"))]
#[inline]
pub(crate) fn readlinkat(dirfd: BorrowedFd<'_>, path: &ZStr, buf: &mut [u8]) -> io::Result<usize> {
    unsafe {
        ret_ssize_t(c::readlinkat(
            borrowed_fd(dirfd),
            c_str(path),
            buf.as_mut_ptr().cast::<c::c_char>(),
            buf.len(),
        ))
        .map(|nread| nread as usize)
    }
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn mkdirat(dirfd: BorrowedFd<'_>, path: &ZStr, mode: Mode) -> io::Result<()> {
    unsafe { ret(c::mkdirat(borrowed_fd(dirfd), c_str(path), mode.bits())) }
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn linkat(
    old_dirfd: BorrowedFd<'_>,
    old_path: &ZStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &ZStr,
    flags: AtFlags,
) -> io::Result<()> {
    unsafe {
        ret(c::linkat(
            borrowed_fd(old_dirfd),
            c_str(old_path),
            borrowed_fd(new_dirfd),
            c_str(new_path),
            flags.bits(),
        ))
    }
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn unlinkat(dirfd: BorrowedFd<'_>, path: &ZStr, flags: AtFlags) -> io::Result<()> {
    unsafe { ret(c::unlinkat(borrowed_fd(dirfd), c_str(path), flags.bits())) }
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn renameat(
    old_dirfd: BorrowedFd<'_>,
    old_path: &ZStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &ZStr,
) -> io::Result<()> {
    unsafe {
        ret(c::renameat(
            borrowed_fd(old_dirfd),
            c_str(old_path),
            borrowed_fd(new_dirfd),
            c_str(new_path),
        ))
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub(crate) fn renameat2(
    old_dirfd: BorrowedFd<'_>,
    old_path: &ZStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &ZStr,
    flags: RenameFlags,
) -> io::Result<()> {
    // `getrandom` wasn't supported in glibc until 2.28.
    weak_or_syscall! {
        fn renameat2(
            olddirfd: c::c_int,
            oldpath: *const c::c_char,
            newdirfd: c::c_int,
            newpath: *const c::c_char,
            flags: c::c_uint
        ) via SYS_renameat2 -> c::c_int
    }

    unsafe {
        ret(renameat2(
            borrowed_fd(old_dirfd),
            c_str(old_path),
            borrowed_fd(new_dirfd),
            c_str(new_path),
            flags.bits(),
        ))
    }
}

/// At present, `libc` only has `renameat2` defined for glibc. On other
/// ABIs, `RenameFlags` has no flags defined, and we use plain `renameat`.
#[cfg(any(
    target_os = "android",
    all(target_os = "linux", not(target_env = "gnu"))
))]
#[inline]
pub(crate) fn renameat2(
    old_dirfd: BorrowedFd<'_>,
    old_path: &ZStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &ZStr,
    flags: RenameFlags,
) -> io::Result<()> {
    assert!(flags.is_empty());
    renameat(old_dirfd, old_path, new_dirfd, new_path)
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn symlinkat(
    old_path: &ZStr,
    new_dirfd: BorrowedFd<'_>,
    new_path: &ZStr,
) -> io::Result<()> {
    unsafe {
        ret(c::symlinkat(
            c_str(old_path),
            borrowed_fd(new_dirfd),
            c_str(new_path),
        ))
    }
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn statat(dirfd: BorrowedFd<'_>, path: &ZStr, flags: AtFlags) -> io::Result<Stat> {
    let mut stat = MaybeUninit::<Stat>::uninit();
    unsafe {
        ret(libc_fstatat(
            borrowed_fd(dirfd),
            c_str(path),
            stat.as_mut_ptr(),
            flags.bits(),
        ))?;
        Ok(stat.assume_init())
    }
}

#[cfg(not(any(target_os = "emscripten", target_os = "illumos", target_os = "redox")))]
pub(crate) fn accessat(
    dirfd: BorrowedFd<'_>,
    path: &ZStr,
    access: Access,
    flags: AtFlags,
) -> io::Result<()> {
    unsafe {
        ret(c::faccessat(
            borrowed_fd(dirfd),
            c_str(path),
            access.bits(),
            flags.bits(),
        ))
    }
}

#[cfg(target_os = "emscripten")]
pub(crate) fn accessat(
    _dirfd: BorrowedFd<'_>,
    _path: &ZStr,
    _access: Access,
    _flags: AtFlags,
) -> io::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "redox"))]
pub(crate) fn utimensat(
    dirfd: BorrowedFd<'_>,
    path: &ZStr,
    times: &Timestamps,
    flags: AtFlags,
) -> io::Result<()> {
    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    unsafe {
        // Assert that `Timestamps` has the expected layout.
        let _ = core::mem::transmute::<Timestamps, [c::timespec; 2]>(times.clone());

        ret(c::utimensat(
            borrowed_fd(dirfd),
            c_str(path),
            as_ptr(times).cast(),
            flags.bits(),
        ))
    }

    // `utimensat` was introduced in macOS 10.13.
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    unsafe {
        // ABI details
        weak! {
            fn utimensat(
                c::c_int,
                *const c::c_char,
                *const c::timespec,
                c::c_int
            ) -> c::c_int
        }
        extern "C" {
            fn setattrlist(
                path: *const c::c_char,
                attr_list: *const Attrlist,
                attr_buf: *const c::c_void,
                attr_buf_size: c::size_t,
                options: c::c_ulong,
            ) -> c::c_int;
        }
        const FSOPT_NOFOLLOW: c::c_ulong = 0x00000001;

        // If we have `utimensat`, use it.
        if let Some(have_utimensat) = utimensat.get() {
            // Assert that `Timestamps` has the expected layout.
            let _ = core::mem::transmute::<Timestamps, [c::timespec; 2]>(times.clone());

            return ret(have_utimensat(
                borrowed_fd(dirfd),
                c_str(path),
                as_ptr(times).cast(),
                flags.bits(),
            ));
        }

        // `setattrlistat` was introduced in 10.13 along with `utimensat`, so if
        // we don't have `utimensat`, we don't have `setattrlistat` either.
        // Emulate it using `fork`, and `fchdir` and [`setattrlist`].
        //
        // [`setattrlist`]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/setattrlist.2.html
        match c::fork() {
            -1 => Err(io::Error::IO),
            0 => {
                if c::fchdir(borrowed_fd(dirfd)) != 0 {
                    let code = match errno::errno().0 {
                        libc::EACCES => 2,
                        libc::ENOTDIR => 3,
                        _ => 1,
                    };
                    c::_exit(code);
                }

                let mut flags_arg = 0;
                if flags.contains(AtFlags::SYMLINK_NOFOLLOW) {
                    flags_arg |= FSOPT_NOFOLLOW;
                }

                let (attrbuf_size, times, attrs) = times_to_attrlist(times);

                if setattrlist(
                    c_str(path),
                    &attrs,
                    as_ptr(&times).cast(),
                    attrbuf_size,
                    flags_arg,
                ) != 0
                {
                    // Translate expected errno codes into ad-hoc integer
                    // values suitable for exit statuses.
                    let code = match errno::errno().0 {
                        libc::EACCES => 2,
                        libc::ENOTDIR => 3,
                        libc::EPERM => 4,
                        libc::EROFS => 5,
                        libc::ELOOP => 6,
                        libc::ENOENT => 7,
                        libc::ENAMETOOLONG => 8,
                        libc::EINVAL => 9,
                        libc::ESRCH => 10,
                        libc::ENOTSUP => 11,
                        _ => 1,
                    };
                    c::_exit(code);
                }

                c::_exit(0);
            }
            child_pid => {
                let mut wstatus = 0;
                let _ = ret_c_int(c::waitpid(child_pid, &mut wstatus, 0))?;
                if c::WIFEXITED(wstatus) {
                    // Translate our ad-hoc exit statuses back to errno codes.
                    match c::WEXITSTATUS(wstatus) {
                        0 => Ok(()),
                        2 => Err(io::Error::ACCESS),
                        3 => Err(io::Error::NOTDIR),
                        4 => Err(io::Error::PERM),
                        5 => Err(io::Error::ROFS),
                        6 => Err(io::Error::LOOP),
                        7 => Err(io::Error::NOENT),
                        8 => Err(io::Error::NAMETOOLONG),
                        9 => Err(io::Error::INVAL),
                        10 => Err(io::Error::SRCH),
                        11 => Err(io::Error::NOTSUP),
                        _ => Err(io::Error::IO),
                    }
                } else {
                    Err(io::Error::IO)
                }
            }
        }
    }
}

#[cfg(not(any(
    target_os = "android",
    target_os = "linux",
    target_os = "redox",
    target_os = "wasi",
)))]
pub(crate) fn chmodat(dirfd: BorrowedFd<'_>, path: &ZStr, mode: Mode) -> io::Result<()> {
    unsafe { ret(c::fchmodat(borrowed_fd(dirfd), c_str(path), mode.bits(), 0)) }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub(crate) fn chmodat(dirfd: BorrowedFd<'_>, path: &ZStr, mode: Mode) -> io::Result<()> {
    // Note that Linux's `fchmodat` does not have a flags argument.
    unsafe {
        // Pass `mode` as a `c_uint` even if `mode_t` is narrower, since
        // `libc_openat` is declared as a variadic function and narrower
        // arguments are promoted.
        syscall_ret(c::syscall(
            c::SYS_fchmodat,
            borrowed_fd(dirfd),
            c_str(path),
            c::c_uint::from(mode.bits()),
        ))
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn fclonefileat(
    srcfd: BorrowedFd<'_>,
    dst_dirfd: BorrowedFd<'_>,
    dst: &ZStr,
    flags: CloneFlags,
) -> io::Result<()> {
    syscall! {
        fn fclonefileat(
            srcfd: BorrowedFd<'_>,
            dst_dirfd: BorrowedFd<'_>,
            dst: *const c::c_char,
            flags: c::c_int
        ) via SYS_fclonefileat -> c::c_int
    }

    unsafe { ret(fclonefileat(srcfd, dst_dirfd, c_str(dst), flags.bits())) }
}

#[cfg(not(any(target_os = "redox", target_os = "wasi")))]
pub(crate) fn chownat(
    dirfd: BorrowedFd<'_>,
    path: &ZStr,
    owner: Uid,
    group: Gid,
    flags: AtFlags,
) -> io::Result<()> {
    unsafe {
        ret(c::fchownat(
            borrowed_fd(dirfd),
            c_str(path),
            owner.as_raw(),
            group.as_raw(),
            flags.bits(),
        ))
    }
}

#[cfg(not(any(
    target_os = "ios",
    target_os = "macos",
    target_os = "redox",
    target_os = "wasi",
)))]
pub(crate) fn mknodat(
    dirfd: BorrowedFd<'_>,
    path: &ZStr,
    file_type: FileType,
    mode: Mode,
    dev: Dev,
) -> io::Result<()> {
    unsafe {
        ret(c::mknodat(
            borrowed_fd(dirfd),
            c_str(path),
            mode.bits() | file_type.as_raw_mode(),
            dev,
        ))
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub(crate) fn copy_file_range(
    fd_in: BorrowedFd<'_>,
    off_in: Option<&mut u64>,
    fd_out: BorrowedFd<'_>,
    off_out: Option<&mut u64>,
    len: u64,
) -> io::Result<u64> {
    assert_eq!(size_of::<c::loff_t>(), size_of::<u64>());

    let mut off_in_val: c::loff_t = 0;
    let mut off_out_val: c::loff_t = 0;
    // Silently cast; we'll get `EINVAL` if the value is negative.
    let off_in_ptr = if let Some(off_in) = &off_in {
        off_in_val = (**off_in) as i64;
        &mut off_in_val
    } else {
        null_mut()
    };
    let off_out_ptr = if let Some(off_out) = &off_out {
        off_out_val = (**off_out) as i64;
        &mut off_out_val
    } else {
        null_mut()
    };
    let len: usize = len.try_into().unwrap_or(usize::MAX);
    let copied = unsafe {
        syscall_ret_ssize_t(c::syscall(
            c::SYS_copy_file_range,
            borrowed_fd(fd_in),
            off_in_ptr,
            borrowed_fd(fd_out),
            off_out_ptr,
            len,
            0, // no flags are defined yet
        ))?
    };
    if let Some(off_in) = off_in {
        *off_in = off_in_val as u64;
    }
    if let Some(off_out) = off_out {
        *off_out = off_out_val as u64;
    }
    Ok(copied as u64)
}

#[cfg(not(any(
    target_os = "dragonfly",
    target_os = "illumos",
    target_os = "ios",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "redox"
)))]
pub(crate) fn fadvise(fd: BorrowedFd<'_>, offset: u64, len: u64, advice: Advice) -> io::Result<()> {
    let offset = offset as i64;
    let len = len as i64;

    // FreeBSD returns `EINVAL` on invalid offsets; emulate the POSIX behavior.
    #[cfg(target_os = "freebsd")]
    let offset = if (offset as i64) < 0 {
        i64::MAX
    } else {
        offset
    };

    // FreeBSD returns `EINVAL` on overflow; emulate the POSIX behavior.
    #[cfg(target_os = "freebsd")]
    let len = if len > 0 && offset.checked_add(len).is_none() {
        i64::MAX - offset
    } else {
        len
    };

    let err = unsafe { libc_posix_fadvise(borrowed_fd(fd), offset, len, advice as c::c_int) };

    // `posix_fadvise` returns its error status rather than using `errno`.
    if err == 0 {
        Ok(())
    } else {
        Err(io::Error(err))
    }
}

pub(crate) fn fcntl_getfd(fd: BorrowedFd<'_>) -> io::Result<FdFlags> {
    unsafe { ret_c_int(c::fcntl(borrowed_fd(fd), c::F_GETFD)).map(FdFlags::from_bits_truncate) }
}

pub(crate) fn fcntl_setfd(fd: BorrowedFd<'_>, flags: FdFlags) -> io::Result<()> {
    unsafe { ret(c::fcntl(borrowed_fd(fd), c::F_SETFD, flags.bits())) }
}

pub(crate) fn fcntl_getfl(fd: BorrowedFd<'_>) -> io::Result<OFlags> {
    unsafe { ret_c_int(c::fcntl(borrowed_fd(fd), c::F_GETFL)).map(OFlags::from_bits_truncate) }
}

pub(crate) fn fcntl_setfl(fd: BorrowedFd<'_>, flags: OFlags) -> io::Result<()> {
    unsafe { ret(c::fcntl(borrowed_fd(fd), c::F_SETFL, flags.bits())) }
}

#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "linux",
))]
pub(crate) fn fcntl_get_seals(fd: BorrowedFd<'_>) -> io::Result<SealFlags> {
    unsafe {
        ret_c_int(c::fcntl(borrowed_fd(fd), c::F_GET_SEALS))
            .map(|flags| SealFlags::from_bits_unchecked(flags))
    }
}

#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "linux",
))]
pub(crate) fn fcntl_add_seals(fd: BorrowedFd<'_>, seals: SealFlags) -> io::Result<()> {
    unsafe { ret(c::fcntl(borrowed_fd(fd), c::F_ADD_SEALS, seals.bits())) }
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn fcntl_dupfd_cloexec(fd: BorrowedFd<'_>, min: RawFd) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(c::fcntl(borrowed_fd(fd), c::F_DUPFD_CLOEXEC, min)) }
}

pub(crate) fn seek(fd: BorrowedFd<'_>, pos: SeekFrom) -> io::Result<u64> {
    let (whence, offset): (c::c_int, libc_off_t) = match pos {
        SeekFrom::Start(pos) => {
            let pos: u64 = pos;
            // Silently cast; we'll get `EINVAL` if the value is negative.
            (c::SEEK_SET, pos as i64)
        }
        SeekFrom::End(offset) => (c::SEEK_END, offset),
        SeekFrom::Current(offset) => (c::SEEK_CUR, offset),
    };
    let offset = unsafe { ret_off_t(libc_lseek(borrowed_fd(fd), offset, whence))? };
    Ok(offset as u64)
}

pub(crate) fn tell(fd: BorrowedFd<'_>) -> io::Result<u64> {
    let offset = unsafe { ret_off_t(libc_lseek(borrowed_fd(fd), 0, c::SEEK_CUR))? };
    Ok(offset as u64)
}

#[cfg(not(any(target_os = "android", target_os = "linux", target_os = "wasi")))]
pub(crate) fn fchmod(fd: BorrowedFd<'_>, mode: Mode) -> io::Result<()> {
    unsafe { ret(c::fchmod(borrowed_fd(fd), mode.bits())) }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub(crate) fn fchmod(fd: BorrowedFd<'_>, mode: Mode) -> io::Result<()> {
    // Use `c::syscall` rather than `c::fchmod` because some libc
    // implementations, such as musl, add extra logic to `fchmod` to emulate
    // support for `O_PATH`, which uses `/proc` outside our control and
    // interferes with our own use of `O_PATH`.
    unsafe {
        syscall_ret(c::syscall(
            c::SYS_fchmod,
            borrowed_fd(fd),
            c::c_uint::from(mode.bits()),
        ))
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub(crate) fn fchown(fd: BorrowedFd<'_>, owner: Uid, group: Gid) -> io::Result<()> {
    unsafe {
        syscall_ret(c::syscall(
            c::SYS_fchown,
            borrowed_fd(fd),
            owner.as_raw(),
            group.as_raw(),
        ))
    }
}

#[cfg(not(any(target_os = "android", target_os = "linux", target_os = "wasi")))]
pub(crate) fn fchown(fd: BorrowedFd<'_>, owner: Uid, group: Gid) -> io::Result<()> {
    unsafe { ret(c::fchown(borrowed_fd(fd), owner.as_raw(), group.as_raw())) }
}

#[cfg(not(target_os = "wasi"))]
pub(crate) fn flock(fd: BorrowedFd<'_>, operation: FlockOperation) -> io::Result<()> {
    unsafe { ret(c::flock(borrowed_fd(fd), operation as c::c_int)) }
}

pub(crate) fn fstat(fd: BorrowedFd<'_>) -> io::Result<Stat> {
    let mut stat = MaybeUninit::<Stat>::uninit();
    unsafe {
        ret(libc_fstat(borrowed_fd(fd), stat.as_mut_ptr()))?;
        Ok(stat.assume_init())
    }
}

#[cfg(not(any(
    target_os = "illumos",
    target_os = "netbsd",
    target_os = "redox",
    target_os = "wasi"
)))] // not implemented in libc for netbsd yet
pub(crate) fn fstatfs(fd: BorrowedFd<'_>) -> io::Result<StatFs> {
    let mut statfs = MaybeUninit::<StatFs>::uninit();
    unsafe {
        ret(libc_fstatfs(borrowed_fd(fd), statfs.as_mut_ptr()))?;
        Ok(statfs.assume_init())
    }
}

pub(crate) fn futimens(fd: BorrowedFd<'_>, times: &Timestamps) -> io::Result<()> {
    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    unsafe {
        // Assert that `Timestamps` has the expected layout.
        let _ = core::mem::transmute::<Timestamps, [c::timespec; 2]>(times.clone());

        ret(c::futimens(borrowed_fd(fd), as_ptr(times).cast()))
    }

    // `futimens` was introduced in macOS 10.13.
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    unsafe {
        // ABI details.
        weak! {
            fn futimens(c::c_int, *const c::timespec) -> c::c_int
        }
        extern "C" {
            fn fsetattrlist(
                fd: c::c_int,
                attr_list: *const Attrlist,
                attr_buf: *const c::c_void,
                attr_buf_size: c::size_t,
                options: c::c_ulong,
            ) -> c::c_int;
        }

        // If we have `futimens`, use it.
        if let Some(have_futimens) = futimens.get() {
            // Assert that `Timestamps` has the expected layout.
            let _ = core::mem::transmute::<Timestamps, [c::timespec; 2]>(times.clone());

            return ret(have_futimens(borrowed_fd(fd), as_ptr(times).cast()));
        }

        // Otherwise use `fsetattrlist`.
        let (attrbuf_size, times, attrs) = times_to_attrlist(times);

        ret(fsetattrlist(
            borrowed_fd(fd),
            &attrs,
            as_ptr(&times).cast(),
            attrbuf_size,
            0,
        ))
    }
}

#[cfg(not(any(
    target_os = "dragonfly",
    target_os = "illumos",
    target_os = "ios",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "redox"
)))]
pub(crate) fn fallocate(
    fd: BorrowedFd<'_>,
    mode: FallocateFlags,
    offset: u64,
    len: u64,
) -> io::Result<()> {
    // Silently cast; we'll get `EINVAL` if the value is negative.
    let offset = offset as i64;
    let len = len as i64;

    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
    unsafe {
        ret(libc_fallocate(borrowed_fd(fd), mode.bits(), offset, len))
    }

    #[cfg(not(any(target_os = "android", target_os = "fuchsia", target_os = "linux")))]
    {
        assert!(mode.is_empty());
        let err = unsafe { libc_posix_fallocate(borrowed_fd(fd), offset, len) };

        // `posix_fallocate` returns its error status rather than using `errno`.
        if err == 0 {
            Ok(())
        } else {
            Err(io::Error(err))
        }
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn fallocate(
    fd: BorrowedFd<'_>,
    mode: FallocateFlags,
    offset: u64,
    len: u64,
) -> io::Result<()> {
    // Silently cast; we'll get `EINVAL` if the value is negative.
    let offset = offset as i64;
    let len = len as i64;

    assert!(mode.is_empty());

    let new_len = offset.checked_add(len).ok_or_else(|| io::Error::FBIG)?;
    let mut store = c::fstore_t {
        fst_flags: c::F_ALLOCATECONTIG,
        fst_posmode: c::F_PEOFPOSMODE,
        fst_offset: 0,
        fst_length: new_len,
        fst_bytesalloc: 0,
    };
    unsafe {
        if c::fcntl(borrowed_fd(fd), c::F_PREALLOCATE, &store) == -1 {
            store.fst_flags = c::F_ALLOCATEALL;
            let _ = ret_c_int(c::fcntl(borrowed_fd(fd), c::F_PREALLOCATE, &store))?;
        }
        ret(c::ftruncate(borrowed_fd(fd), new_len))
    }
}

pub(crate) fn fsync(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::fsync(borrowed_fd(fd))) }
}

#[cfg(not(any(
    target_os = "dragonfly",
    target_os = "ios",
    target_os = "macos",
    target_os = "redox"
)))]
pub(crate) fn fdatasync(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::fdatasync(borrowed_fd(fd))) }
}

pub(crate) fn ftruncate(fd: BorrowedFd<'_>, length: u64) -> io::Result<()> {
    let length = length.try_into().map_err(|_overflow_err| io::Error::FBIG)?;
    unsafe { ret(libc_ftruncate(borrowed_fd(fd), length)) }
}

#[cfg(any(target_os = "android", target_os = "freebsd", target_os = "linux"))]
pub(crate) fn memfd_create(path: &ZStr, flags: MemfdFlags) -> io::Result<OwnedFd> {
    #[cfg(target_os = "freebsd")]
    weakcall! {
        fn memfd_create(
            name: *const c::c_char,
            flags: c::c_uint
        ) -> c::c_int
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    weak_or_syscall! {
        fn memfd_create(
            name: *const c::c_char,
            flags: c::c_uint
        ) via SYS_memfd_create -> c::c_int
    }

    unsafe { ret_owned_fd(memfd_create(c_str(path), flags.bits())) }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub(crate) fn openat2(
    dirfd: BorrowedFd<'_>,
    path: &ZStr,
    oflags: OFlags,
    mode: Mode,
    resolve: ResolveFlags,
) -> io::Result<OwnedFd> {
    let oflags: i32 = oflags.bits();
    let open_how = OpenHow {
        oflag: u64::from(oflags as u32),
        mode: u64::from(mode.bits()),
        resolve: resolve.bits(),
    };

    unsafe {
        syscall_ret_owned_fd(c::syscall(
            SYS_OPENAT2,
            borrowed_fd(dirfd),
            c_str(path),
            &open_how,
            SIZEOF_OPEN_HOW,
        ))
    }
}
#[cfg(all(
    target_pointer_width = "32",
    any(target_os = "android", target_os = "linux")
))]
const SYS_OPENAT2: i32 = 437;
#[cfg(all(
    target_pointer_width = "64",
    any(target_os = "android", target_os = "linux")
))]
const SYS_OPENAT2: i64 = 437;

#[cfg(any(target_os = "android", target_os = "linux"))]
#[repr(C)]
#[derive(Debug)]
struct OpenHow {
    oflag: u64,
    mode: u64,
    resolve: u64,
}
#[cfg(any(target_os = "android", target_os = "linux"))]
const SIZEOF_OPEN_HOW: usize = size_of::<OpenHow>();

#[cfg(target_os = "linux")]
pub(crate) fn sendfile(
    out_fd: BorrowedFd<'_>,
    in_fd: BorrowedFd<'_>,
    offset: Option<&mut u64>,
    count: usize,
) -> io::Result<usize> {
    unsafe {
        let nsent = ret_ssize_t(c::sendfile64(
            borrowed_fd(out_fd),
            borrowed_fd(in_fd),
            transmute(offset),
            count,
        ))?;
        Ok(nsent as usize)
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[allow(non_upper_case_globals)]
pub(crate) fn statx(
    dirfd: BorrowedFd<'_>,
    path: &ZStr,
    flags: AtFlags,
    mask: StatxFlags,
) -> io::Result<Statx> {
    #[cfg(all(target_os = "android", target_arch = "arm"))]
    const SYS_statx: c::c_long = 397;
    #[cfg(all(target_os = "android", target_arch = "x86"))]
    const SYS_statx: c::c_long = 383;
    #[cfg(all(target_os = "android", target_arch = "aarch64"))]
    const SYS_statx: c::c_long = 291;
    #[cfg(all(target_os = "android", target_arch = "x86_64"))]
    const SYS_statx: c::c_long = 332;

    weak_or_syscall! {
        fn statx(
            pirfd: BorrowedFd<'_>,
            path: *const c::c_char,
            flags: c::c_int,
            mask: c::c_uint,
            buf: *mut Statx
        ) via SYS_statx -> c::c_int
    }

    let mut statx_buf = MaybeUninit::<Statx>::uninit();
    unsafe {
        ret(statx(
            dirfd,
            c_str(path),
            flags.bits(),
            mask.bits(),
            statx_buf.as_mut_ptr(),
        ))?;
        Ok(statx_buf.assume_init())
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) unsafe fn fcopyfile(
    from: BorrowedFd<'_>,
    to: BorrowedFd<'_>,
    state: copyfile_state_t,
    flags: CopyfileFlags,
) -> io::Result<()> {
    extern "C" {
        fn fcopyfile(
            from: c::c_int,
            to: c::c_int,
            state: copyfile_state_t,
            flags: c::c_uint,
        ) -> c::c_int;
    }

    nonnegative_ret(fcopyfile(
        borrowed_fd(from),
        borrowed_fd(to),
        state,
        flags.bits(),
    ))
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn copyfile_state_alloc() -> io::Result<copyfile_state_t> {
    extern "C" {
        fn copyfile_state_alloc() -> copyfile_state_t;
    }

    let result = unsafe { copyfile_state_alloc() };
    if result.0.is_null() {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) unsafe fn copyfile_state_free(state: copyfile_state_t) -> io::Result<()> {
    extern "C" {
        fn copyfile_state_free(state: copyfile_state_t) -> c::c_int;
    }

    nonnegative_ret(copyfile_state_free(state))
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
const COPYFILE_STATE_COPIED: u32 = 8;

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) unsafe fn copyfile_state_get_copied(state: copyfile_state_t) -> io::Result<u64> {
    let mut copied = MaybeUninit::<u64>::uninit();
    copyfile_state_get(state, COPYFILE_STATE_COPIED, copied.as_mut_ptr().cast())?;
    Ok(copied.assume_init())
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) unsafe fn copyfile_state_get(
    state: copyfile_state_t,
    flag: u32,
    dst: *mut c::c_void,
) -> io::Result<()> {
    extern "C" {
        fn copyfile_state_get(state: copyfile_state_t, flag: u32, dst: *mut c::c_void) -> c::c_int;
    }

    nonnegative_ret(copyfile_state_get(state, flag, dst))
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn getpath(fd: BorrowedFd<'_>) -> io::Result<ZString> {
    // The use of PATH_MAX is generally not encouraged, but it
    // is inevitable in this case because macOS defines `fcntl` with
    // `F_GETPATH` in terms of `MAXPATHLEN`, and there are no
    // alternatives. If a better method is invented, it should be used
    // instead.
    let mut buf = vec![0; c::PATH_MAX as usize];

    // From the [macOS `fcntl` man page]:
    // `F_GETPATH` - Get the path of the file descriptor `Fildes`. The argument
    //               must be a buffer of size `MAXPATHLEN` or greater.
    //
    // [macOS `fcntl` man page]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
    unsafe {
        ret(c::fcntl(borrowed_fd(fd), c::F_GETPATH, buf.as_mut_ptr()))?;
    }

    let l = buf.iter().position(|&c| c == 0).unwrap();
    buf.truncate(l);

    // TODO: On Rust 1.56, we can use `shrink_to` here.
    //buf.shrink_to(l + 1);
    buf.shrink_to_fit();

    Ok(ZString::new(buf).unwrap())
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn fcntl_rdadvise(fd: BorrowedFd<'_>, offset: u64, len: u64) -> io::Result<()> {
    // From the [macOS `fcntl` man page]:
    // `F_RDADVISE` - Issue an advisory read async with no copy to user.
    //
    // The `F_RDADVISE` command operates on the following structure which holds
    // information passed from the user to the system:
    //
    // ```
    // struct radvisory {
    //      off_t   ra_offset;  /* offset into the file */
    //      int     ra_count;   /* size of the read     */
    // };
    // ```
    //
    // [macOS `fcntl` man page]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/fcntl.2.html
    let ra_offset = match offset.try_into() {
        Ok(len) => len,
        // If this conversion fails, the user is providing an offset outside
        // any possible file extent, so just ignore it.
        Err(_) => return Ok(()),
    };
    let ra_count = match len.try_into() {
        Ok(len) => len,
        // If this conversion fails, the user is providing a dubiously large
        // hint which is unlikely to improve performance.
        Err(_) => return Ok(()),
    };
    unsafe {
        let radvisory = c::radvisory {
            ra_offset,
            ra_count,
        };
        ret(c::fcntl(borrowed_fd(fd), c::F_RDADVISE, &radvisory))
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub(crate) fn fcntl_fullfsync(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(c::fcntl(borrowed_fd(fd), c::F_FULLFSYNC)) }
}

/// Convert `times` from a `futimens`/`utimensat` argument into `setattrlist`
/// arguments.
#[cfg(any(target_os = "ios", target_os = "macos"))]
fn times_to_attrlist(times: &Timestamps) -> (c::size_t, [c::timespec; 2], Attrlist) {
    // ABI details.
    const ATTR_CMN_MODTIME: u32 = 0x00000400;
    const ATTR_CMN_ACCTIME: u32 = 0x00001000;
    const ATTR_BIT_MAP_COUNT: u16 = 5;

    let mut times = times.clone();

    // If we have any `UTIME_NOW` elements, replace them with the current time.
    if times.last_access.tv_nsec == c::UTIME_NOW || times.last_modification.tv_nsec == c::UTIME_NOW
    {
        let now = {
            let mut tv = c::timeval {
                tv_sec: 0,
                tv_usec: 0,
            };
            unsafe {
                let r = c::gettimeofday(&mut tv, null_mut());
                assert_eq!(r, 0);
            }
            c::timespec {
                tv_sec: tv.tv_sec,
                tv_nsec: (tv.tv_usec * 1000) as _,
            }
        };
        if times.last_access.tv_nsec == c::UTIME_NOW {
            times.last_access = now;
        }
        if times.last_modification.tv_nsec == c::UTIME_NOW {
            times.last_modification = now;
        }
    }

    // Pack the return values following the rules for [`getattrlist`].
    //
    // [`getattrlist`]: https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/getattrlist.2.html
    let mut times_size = 0;
    let mut attrs = Attrlist {
        bitmapcount: ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: 0,
        volattr: 0,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    let mut return_times = [c::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    }; 2];
    let mut times_index = 0;
    if times.last_modification.tv_nsec != c::UTIME_OMIT {
        attrs.commonattr |= ATTR_CMN_MODTIME;
        return_times[times_index] = times.last_modification;
        times_index += 1;
        times_size += size_of::<c::timespec>();
    }
    if times.last_access.tv_nsec != c::UTIME_OMIT {
        attrs.commonattr |= ATTR_CMN_ACCTIME;
        return_times[times_index] = times.last_access;
        times_size += size_of::<c::timespec>();
    }

    (times_size, return_times, attrs)
}

/// Support type for `Attrlist`.
#[cfg(any(target_os = "ios", target_os = "macos"))]
type Attrgroup = u32;

/// Attribute list for use with `setattrlist`.
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[repr(C)]
struct Attrlist {
    bitmapcount: u16,
    reserved: u16,
    commonattr: Attrgroup,
    volattr: Attrgroup,
    dirattr: Attrgroup,
    fileattr: Attrgroup,
    forkattr: Attrgroup,
}
