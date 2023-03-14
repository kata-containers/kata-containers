//! linux_raw syscalls supporting `rustix::fs`.
//!
//! # Safety
//!
//! See the `rustix::imp::syscalls` module documentation for details.

#![allow(unsafe_code)]
#![allow(dead_code)]

#[cfg(target_arch = "mips")]
use super::super::arch::choose::syscall7_readonly;
use super::super::arch::choose::{
    syscall1_readonly, syscall2, syscall2_readonly, syscall3, syscall3_readonly, syscall4,
    syscall4_readonly, syscall5, syscall5_readonly, syscall6,
};
use super::super::c;
#[cfg(all(
    target_pointer_width = "32",
    any(target_arch = "arm", target_arch = "mips", target_arch = "powerpc")
))]
use super::super::conv::zero;
use super::super::conv::{
    borrowed_fd, by_ref, c_int, c_str, c_uint, dev_t, fs_advice, mode_and_type_as, mode_as, oflags,
    oflags_for_open_how, opt_c_str, opt_mut, out, pass_usize, raw_fd, ret, ret_c_int, ret_c_uint,
    ret_owned_fd, ret_usize, size_of, slice_mut,
};
use super::super::reg::nr;
use super::{
    Access, Advice, AtFlags, FallocateFlags, FdFlags, FlockOperation, MemfdFlags, Mode, OFlags,
    RenameFlags, ResolveFlags, Stat, StatFs, StatxFlags,
};
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
use crate::fd::AsFd;
use crate::fd::{BorrowedFd, RawFd};
use crate::ffi::ZStr;
use crate::fs::{FileType, SealFlags, Timestamps};
use crate::io::{self, OwnedFd, SeekFrom};
use crate::process::{Gid, Uid};
use core::convert::TryInto;
use core::mem::MaybeUninit;
#[cfg(target_arch = "arm")]
use linux_raw_sys::general::__NR_arm_fadvise64_64 as __NR_fadvise64_64;
#[cfg(target_arch = "mips")]
use linux_raw_sys::general::__NR_fadvise64;
#[cfg(all(
    target_pointer_width = "32",
    not(any(target_arch = "arm", target_arch = "mips"))
))]
use linux_raw_sys::general::__NR_fadvise64_64;
#[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
use linux_raw_sys::general::__NR_open;
#[cfg(not(any(target_arch = "riscv64")))]
use linux_raw_sys::general::__NR_renameat;
use linux_raw_sys::general::{
    __NR_copy_file_range, __NR_faccessat, __NR_fallocate, __NR_fchmod, __NR_fchmodat, __NR_fchown,
    __NR_fchownat, __NR_fdatasync, __NR_flock, __NR_fsync, __NR_getdents64, __NR_linkat,
    __NR_memfd_create, __NR_mkdirat, __NR_mknodat, __NR_openat, __NR_openat2, __NR_readlinkat,
    __NR_renameat2, __NR_statx, __NR_symlinkat, __NR_unlinkat, __NR_utimensat, __kernel_timespec,
    open_how, statx, AT_FDCWD, AT_REMOVEDIR, AT_SYMLINK_NOFOLLOW, F_ADD_SEALS, F_DUPFD,
    F_DUPFD_CLOEXEC, F_GETFD, F_GETFL, F_GETLEASE, F_GETOWN, F_GETPIPE_SZ, F_GETSIG, F_GET_SEALS,
    F_SETFD, F_SETFL, F_SETPIPE_SZ,
};
#[cfg(target_pointer_width = "32")]
use {
    super::super::arch::choose::syscall6_readonly,
    super::super::conv::{hi, lo, slice_just_addr},
    linux_raw_sys::general::__NR_utimensat_time64,
    linux_raw_sys::general::{
        __NR__llseek, __NR_fcntl64, __NR_fstat64, __NR_fstatat64, __NR_fstatfs64, __NR_ftruncate64,
        __NR_sendfile64, __NR_statfs64, timespec as __kernel_old_timespec,
    },
};
#[cfg(target_pointer_width = "64")]
use {
    super::super::conv::{loff_t, loff_t_from_u64, ret_u64},
    linux_raw_sys::general::{
        __NR_fadvise64, __NR_fcntl, __NR_fstat, __NR_fstatfs, __NR_ftruncate, __NR_lseek,
        __NR_newfstatat, __NR_sendfile, __NR_statfs,
    },
};

#[inline]
pub(crate) fn open(filename: &ZStr, flags: OFlags, mode: Mode) -> io::Result<OwnedFd> {
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    {
        openat(crate::fs::cwd().as_fd(), filename, flags, mode)
    }
    #[cfg(all(
        target_pointer_width = "32",
        not(any(target_arch = "aarch64", target_arch = "riscv64"))
    ))]
    unsafe {
        ret_owned_fd(syscall3_readonly(
            nr(__NR_open),
            c_str(filename),
            oflags(flags),
            mode_as(mode),
        ))
    }
    #[cfg(all(
        target_pointer_width = "64",
        not(any(target_arch = "aarch64", target_arch = "riscv64"))
    ))]
    unsafe {
        ret_owned_fd(syscall3_readonly(
            nr(__NR_open),
            c_str(filename),
            oflags(flags),
            mode_as(mode),
        ))
    }
}

#[inline]
pub(crate) fn openat(
    dirfd: BorrowedFd<'_>,
    filename: &ZStr,
    flags: OFlags,
    mode: Mode,
) -> io::Result<OwnedFd> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_owned_fd(syscall4_readonly(
            nr(__NR_openat),
            borrowed_fd(dirfd),
            c_str(filename),
            oflags(flags),
            mode_as(mode),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_owned_fd(syscall4_readonly(
            nr(__NR_openat),
            borrowed_fd(dirfd),
            c_str(filename),
            oflags(flags),
            mode_as(mode),
        ))
    }
}

#[inline]
pub(crate) fn openat2(
    dirfd: BorrowedFd<'_>,
    pathname: &ZStr,
    flags: OFlags,
    mode: Mode,
    resolve: ResolveFlags,
) -> io::Result<OwnedFd> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_owned_fd(syscall4_readonly(
            nr(__NR_openat2),
            borrowed_fd(dirfd),
            c_str(pathname),
            by_ref(&open_how {
                flags: oflags_for_open_how(flags),
                mode: u64::from(mode.bits()),
                resolve: resolve.bits(),
            }),
            size_of::<open_how, _>(),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_owned_fd(syscall4_readonly(
            nr(__NR_openat2),
            borrowed_fd(dirfd),
            c_str(pathname),
            by_ref(&open_how {
                flags: oflags_for_open_how(flags),
                mode: u64::from(mode.bits()),
                resolve: resolve.bits(),
            }),
            size_of::<open_how, _>(),
        ))
    }
}

#[inline]
pub(crate) fn chmod(filename: &ZStr, mode: Mode) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_fchmodat),
            c_int(AT_FDCWD),
            c_str(filename),
            mode_as(mode),
        ))
    }
}

#[inline]
pub(crate) fn chmodat(dirfd: BorrowedFd<'_>, filename: &ZStr, mode: Mode) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_fchmodat),
            borrowed_fd(dirfd),
            c_str(filename),
            mode_as(mode),
        ))
    }
}

#[inline]
pub(crate) fn fchmod(fd: BorrowedFd<'_>, mode: Mode) -> io::Result<()> {
    unsafe {
        ret(syscall2_readonly(
            nr(__NR_fchmod),
            borrowed_fd(fd),
            mode_as(mode),
        ))
    }
}

#[inline]
pub(crate) fn chownat(
    dirfd: BorrowedFd<'_>,
    filename: &ZStr,
    owner: Uid,
    group: Gid,
    flags: AtFlags,
) -> io::Result<()> {
    unsafe {
        ret(syscall5_readonly(
            nr(__NR_fchownat),
            borrowed_fd(dirfd),
            c_str(filename),
            c_uint(owner.as_raw()),
            c_uint(group.as_raw()),
            c_uint(flags.bits()),
        ))
    }
}

#[inline]
pub(crate) fn fchown(fd: BorrowedFd<'_>, owner: Uid, group: Gid) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_fchown),
            borrowed_fd(fd),
            c_uint(owner.as_raw()),
            c_uint(group.as_raw()),
        ))
    }
}

#[inline]
pub(crate) fn mknodat(
    dirfd: BorrowedFd<'_>,
    filename: &ZStr,
    file_type: FileType,
    mode: Mode,
    dev: u64,
) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall4_readonly(
            nr(__NR_mknodat),
            borrowed_fd(dirfd),
            c_str(filename),
            mode_and_type_as(mode, file_type),
            dev_t(dev)?,
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall4_readonly(
            nr(__NR_mknodat),
            borrowed_fd(dirfd),
            c_str(filename),
            mode_and_type_as(mode, file_type),
            dev_t(dev),
        ))
    }
}

#[inline]
pub(crate) fn seek(fd: BorrowedFd<'_>, pos: SeekFrom) -> io::Result<u64> {
    let (whence, offset) = match pos {
        SeekFrom::Start(pos) => {
            let pos: u64 = pos;
            // Silently cast; we'll get `EINVAL` if the value is negative.
            (linux_raw_sys::general::SEEK_SET, pos as i64)
        }
        SeekFrom::End(offset) => (linux_raw_sys::general::SEEK_END, offset),
        SeekFrom::Current(offset) => (linux_raw_sys::general::SEEK_CUR, offset),
    };
    _seek(fd, offset, whence)
}

#[inline]
pub(crate) fn _seek(fd: BorrowedFd<'_>, offset: i64, whence: c::c_uint) -> io::Result<u64> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<u64>::uninit();
        ret(syscall5(
            nr(__NR__llseek),
            borrowed_fd(fd),
            // Don't use the hi/lo functions here because Linux's llseek
            // takes its 64-bit argument differently from everything else.
            pass_usize((offset >> 32) as usize),
            pass_usize(offset as usize),
            out(&mut result),
            c_uint(whence),
        ))
        .map(|()| result.assume_init())
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_u64(syscall3_readonly(
            nr(__NR_lseek),
            borrowed_fd(fd),
            loff_t(offset),
            c_uint(whence),
        ))
    }
}

#[inline]
pub(crate) fn tell(fd: BorrowedFd<'_>) -> io::Result<u64> {
    _seek(fd, 0, linux_raw_sys::general::SEEK_CUR).map(|x| x as u64)
}

#[inline]
pub(crate) fn ftruncate(fd: BorrowedFd<'_>, length: u64) -> io::Result<()> {
    // <https://github.com/torvalds/linux/blob/fcadab740480e0e0e9fa9bd272acd409884d431a/arch/arm64/kernel/sys32.c#L81-L83>
    #[cfg(all(
        target_pointer_width = "32",
        any(target_arch = "arm", target_arch = "mips", target_arch = "powerpc")
    ))]
    unsafe {
        ret(syscall4_readonly(
            nr(__NR_ftruncate64),
            borrowed_fd(fd),
            zero(),
            hi(length),
            lo(length),
        ))
    }
    #[cfg(all(
        target_pointer_width = "32",
        not(any(target_arch = "arm", target_arch = "mips", target_arch = "powerpc"))
    ))]
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_ftruncate64),
            borrowed_fd(fd),
            hi(length),
            lo(length),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall2_readonly(
            nr(__NR_ftruncate),
            borrowed_fd(fd),
            loff_t_from_u64(length),
        ))
    }
}

#[inline]
pub(crate) fn fallocate(
    fd: BorrowedFd<'_>,
    mode: FallocateFlags,
    offset: u64,
    len: u64,
) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall6_readonly(
            nr(__NR_fallocate),
            borrowed_fd(fd),
            c_uint(mode.bits()),
            hi(offset),
            lo(offset),
            hi(len),
            lo(len),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall4_readonly(
            nr(__NR_fallocate),
            borrowed_fd(fd),
            c_uint(mode.bits()),
            loff_t_from_u64(offset),
            loff_t_from_u64(len),
        ))
    }
}

#[inline]
pub(crate) fn fadvise(fd: BorrowedFd<'_>, pos: u64, len: u64, advice: Advice) -> io::Result<()> {
    // On arm and powerpc, the arguments are reordered so that the len and
    // pos argument pairs are aligned.
    #[cfg(any(target_arch = "arm", target_arch = "powerpc"))]
    unsafe {
        ret(syscall6_readonly(
            nr(__NR_fadvise64_64),
            borrowed_fd(fd),
            fs_advice(advice),
            hi(pos),
            lo(pos),
            hi(len),
            lo(len),
        ))
    }
    // On mips, the arguments are not reordered, and padding is inserted
    // instead to ensure alignment.
    #[cfg(target_arch = "mips")]
    unsafe {
        ret(syscall7_readonly(
            nr(__NR_fadvise64),
            borrowed_fd(fd),
            zero(),
            hi(pos),
            lo(pos),
            hi(len),
            lo(len),
            fs_advice(advice),
        ))
    }
    #[cfg(all(
        target_pointer_width = "32",
        not(any(target_arch = "arm", target_arch = "mips", target_arch = "powerpc"))
    ))]
    unsafe {
        ret(syscall6_readonly(
            nr(__NR_fadvise64_64),
            borrowed_fd(fd),
            hi(pos),
            lo(pos),
            hi(len),
            lo(len),
            fs_advice(advice),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall4_readonly(
            nr(__NR_fadvise64),
            borrowed_fd(fd),
            loff_t_from_u64(pos),
            loff_t_from_u64(len),
            fs_advice(advice),
        ))
    }
}

#[inline]
pub(crate) fn fsync(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(syscall1_readonly(nr(__NR_fsync), borrowed_fd(fd))) }
}

#[inline]
pub(crate) fn fdatasync(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(syscall1_readonly(nr(__NR_fdatasync), borrowed_fd(fd))) }
}

#[inline]
pub(crate) fn flock(fd: BorrowedFd<'_>, operation: FlockOperation) -> io::Result<()> {
    unsafe {
        ret(syscall2(
            nr(__NR_flock),
            borrowed_fd(fd),
            c_uint(operation as c::c_uint),
        ))
    }
}

#[inline]
pub(crate) fn fstat(fd: BorrowedFd<'_>) -> io::Result<Stat> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall2(
            nr(__NR_fstat64),
            borrowed_fd(fd),
            out(&mut result),
        ))
        .map(|()| result.assume_init())
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall2(nr(__NR_fstat), borrowed_fd(fd), out(&mut result)))
            .map(|()| result.assume_init())
    }
}

#[inline]
pub(crate) fn stat(filename: &ZStr) -> io::Result<Stat> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall4(
            nr(__NR_fstatat64),
            c_int(AT_FDCWD),
            c_str(filename),
            out(&mut result),
            c_uint(0),
        ))
        .map(|()| result.assume_init())
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall4(
            nr(__NR_newfstatat),
            c_int(AT_FDCWD),
            c_str(filename),
            out(&mut result),
            c_uint(0),
        ))
        .map(|()| result.assume_init())
    }
}

#[inline]
pub(crate) fn statat(dirfd: BorrowedFd<'_>, filename: &ZStr, flags: AtFlags) -> io::Result<Stat> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall4(
            nr(__NR_fstatat64),
            borrowed_fd(dirfd),
            c_str(filename),
            out(&mut result),
            c_uint(flags.bits()),
        ))
        .map(|()| result.assume_init())
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall4(
            nr(__NR_newfstatat),
            borrowed_fd(dirfd),
            c_str(filename),
            out(&mut result),
            c_uint(flags.bits()),
        ))
        .map(|()| result.assume_init())
    }
}

#[inline]
pub(crate) fn lstat(filename: &ZStr) -> io::Result<Stat> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall4(
            nr(__NR_fstatat64),
            c_int(AT_FDCWD),
            c_str(filename),
            out(&mut result),
            c_uint(AT_SYMLINK_NOFOLLOW),
        ))
        .map(|()| result.assume_init())
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut result = MaybeUninit::<Stat>::uninit();
        ret(syscall4(
            nr(__NR_newfstatat),
            c_int(AT_FDCWD),
            c_str(filename),
            out(&mut result),
            c_uint(AT_SYMLINK_NOFOLLOW),
        ))
        .map(|()| result.assume_init())
    }
}

#[inline]
pub(crate) fn statx(
    dirfd: BorrowedFd<'_>,
    pathname: &ZStr,
    flags: AtFlags,
    mask: StatxFlags,
) -> io::Result<statx> {
    unsafe {
        let mut statx_buf = MaybeUninit::<statx>::uninit();
        ret(syscall5(
            nr(__NR_statx),
            borrowed_fd(dirfd),
            c_str(pathname),
            c_uint(flags.bits()),
            c_uint(mask.bits()),
            out(&mut statx_buf),
        ))
        .map(|()| statx_buf.assume_init())
    }
}

#[inline]
pub(crate) fn fstatfs(fd: BorrowedFd<'_>) -> io::Result<StatFs> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<StatFs>::uninit();
        ret(syscall3(
            nr(__NR_fstatfs64),
            borrowed_fd(fd),
            size_of::<StatFs, _>(),
            out(&mut result),
        ))
        .map(|()| result.assume_init())
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut result = MaybeUninit::<StatFs>::uninit();
        ret(syscall2(
            nr(__NR_fstatfs),
            borrowed_fd(fd),
            out(&mut result),
        ))
        .map(|()| result.assume_init())
    }
}

#[inline]
pub(crate) fn statfs(filename: &ZStr) -> io::Result<StatFs> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<StatFs>::uninit();
        ret(syscall3(
            nr(__NR_statfs64),
            c_str(filename),
            size_of::<StatFs, _>(),
            out(&mut result),
        ))
        .map(|()| result.assume_init())
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut result = MaybeUninit::<StatFs>::uninit();
        ret(syscall2(nr(__NR_statfs), c_str(filename), out(&mut result)))
            .map(|()| result.assume_init())
    }
}

#[inline]
pub(crate) fn readlink(path: &ZStr, buf: &mut [u8]) -> io::Result<usize> {
    let (buf_addr_mut, buf_len) = slice_mut(buf);
    unsafe {
        ret_usize(syscall4(
            nr(__NR_readlinkat),
            c_int(AT_FDCWD),
            c_str(path),
            buf_addr_mut,
            buf_len,
        ))
    }
}

#[inline]
pub(crate) fn readlinkat(dirfd: BorrowedFd<'_>, path: &ZStr, buf: &mut [u8]) -> io::Result<usize> {
    let (buf_addr_mut, buf_len) = slice_mut(buf);
    unsafe {
        ret_usize(syscall4(
            nr(__NR_readlinkat),
            borrowed_fd(dirfd),
            c_str(path),
            buf_addr_mut,
            buf_len,
        ))
    }
}

#[inline]
pub(crate) fn fcntl_dupfd(fd: BorrowedFd<'_>, min: RawFd) -> io::Result<OwnedFd> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_owned_fd(syscall3_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_DUPFD),
            raw_fd(min),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_owned_fd(syscall3_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_DUPFD),
            raw_fd(min),
        ))
    }
}

#[inline]
pub(crate) fn fcntl_dupfd_cloexec(fd: BorrowedFd<'_>, min: RawFd) -> io::Result<OwnedFd> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_owned_fd(syscall3_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_DUPFD_CLOEXEC),
            raw_fd(min),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_owned_fd(syscall3_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_DUPFD_CLOEXEC),
            raw_fd(min),
        ))
    }
}

#[inline]
pub(crate) fn fcntl_getfd(fd: BorrowedFd<'_>) -> io::Result<FdFlags> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_c_uint(syscall2_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_GETFD),
        ))
        .map(FdFlags::from_bits_truncate)
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_c_uint(syscall2_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_GETFD),
        ))
        .map(FdFlags::from_bits_truncate)
    }
}

#[inline]
pub(crate) fn fcntl_setfd(fd: BorrowedFd<'_>, flags: FdFlags) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_SETFD),
            c_uint(flags.bits()),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_SETFD),
            c_uint(flags.bits()),
        ))
    }
}

#[inline]
pub(crate) fn fcntl_getfl(fd: BorrowedFd<'_>) -> io::Result<OFlags> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_c_uint(syscall2_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_GETFL),
        ))
        .map(OFlags::from_bits_truncate)
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_c_uint(syscall2_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_GETFL),
        ))
        .map(OFlags::from_bits_truncate)
    }
}

#[inline]
pub(crate) fn fcntl_setfl(fd: BorrowedFd<'_>, flags: OFlags) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_SETFL),
            oflags(flags),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_SETFL),
            oflags(flags),
        ))
    }
}

#[inline]
pub(crate) fn fcntl_getlease(fd: BorrowedFd<'_>) -> io::Result<c::c_int> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_c_int(syscall2_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_GETLEASE),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_c_int(syscall2_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_GETLEASE),
        ))
    }
}

#[inline]
pub(crate) fn fcntl_getown(fd: BorrowedFd<'_>) -> io::Result<c::c_int> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_c_int(syscall2_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_GETOWN),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_c_int(syscall2_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_GETOWN),
        ))
    }
}

#[inline]
pub(crate) fn fcntl_getsig(fd: BorrowedFd<'_>) -> io::Result<c::c_int> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_c_int(syscall2_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_GETSIG),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_c_int(syscall2_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_GETSIG),
        ))
    }
}

#[inline]
pub(crate) fn fcntl_getpipe_sz(fd: BorrowedFd<'_>) -> io::Result<usize> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_usize(syscall2_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_GETPIPE_SZ),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall2_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_GETPIPE_SZ),
        ))
    }
}

#[inline]
pub(crate) fn fcntl_setpipe_sz(fd: BorrowedFd<'_>, size: c::c_int) -> io::Result<usize> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_usize(syscall3_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_SETPIPE_SZ),
            c_int(size),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall3_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_SETPIPE_SZ),
            c_int(size),
        ))
    }
}

#[inline]
pub(crate) fn fcntl_get_seals(fd: BorrowedFd<'_>) -> io::Result<SealFlags> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_c_int(syscall2_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_GET_SEALS),
        ))
        .map(|seals| SealFlags::from_bits_unchecked(seals as u32))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_c_int(syscall2_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_GET_SEALS),
        ))
        .map(|seals| SealFlags::from_bits_unchecked(seals as u32))
    }
}

#[inline]
pub(crate) fn fcntl_add_seals(fd: BorrowedFd<'_>, seals: SealFlags) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_fcntl64),
            borrowed_fd(fd),
            c_uint(F_ADD_SEALS),
            c_uint(seals.bits()),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_fcntl),
            borrowed_fd(fd),
            c_uint(F_ADD_SEALS),
            c_uint(seals.bits()),
        ))
    }
}

#[inline]
pub(crate) fn rename(oldname: &ZStr, newname: &ZStr) -> io::Result<()> {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        ret(syscall5_readonly(
            nr(__NR_renameat2),
            c_int(AT_FDCWD),
            c_str(oldname),
            c_int(AT_FDCWD),
            c_str(newname),
            c_uint(0),
        ))
    }
    #[cfg(not(target_arch = "riscv64"))]
    unsafe {
        ret(syscall4_readonly(
            nr(__NR_renameat),
            c_int(AT_FDCWD),
            c_str(oldname),
            c_int(AT_FDCWD),
            c_str(newname),
        ))
    }
}

#[inline]
pub(crate) fn renameat(
    old_dirfd: BorrowedFd<'_>,
    oldname: &ZStr,
    new_dirfd: BorrowedFd<'_>,
    newname: &ZStr,
) -> io::Result<()> {
    #[cfg(target_arch = "riscv64")]
    unsafe {
        ret(syscall5_readonly(
            nr(__NR_renameat2),
            borrowed_fd(old_dirfd),
            c_str(oldname),
            borrowed_fd(new_dirfd),
            c_str(newname),
            c_uint(0),
        ))
    }
    #[cfg(not(target_arch = "riscv64"))]
    unsafe {
        ret(syscall4_readonly(
            nr(__NR_renameat),
            borrowed_fd(old_dirfd),
            c_str(oldname),
            borrowed_fd(new_dirfd),
            c_str(newname),
        ))
    }
}

#[inline]
pub(crate) fn renameat2(
    old_dirfd: BorrowedFd<'_>,
    oldname: &ZStr,
    new_dirfd: BorrowedFd<'_>,
    newname: &ZStr,
    flags: RenameFlags,
) -> io::Result<()> {
    unsafe {
        ret(syscall5_readonly(
            nr(__NR_renameat2),
            borrowed_fd(old_dirfd),
            c_str(oldname),
            borrowed_fd(new_dirfd),
            c_str(newname),
            c_uint(flags.bits()),
        ))
    }
}

#[inline]
pub(crate) fn unlink(pathname: &ZStr) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_unlinkat),
            c_int(AT_FDCWD),
            c_str(pathname),
            c_uint(0),
        ))
    }
}

#[inline]
pub(crate) fn unlinkat(dirfd: BorrowedFd<'_>, pathname: &ZStr, flags: AtFlags) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_unlinkat),
            borrowed_fd(dirfd),
            c_str(pathname),
            c_uint(flags.bits()),
        ))
    }
}

#[inline]
pub(crate) fn rmdir(pathname: &ZStr) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_unlinkat),
            c_int(AT_FDCWD),
            c_str(pathname),
            c_uint(AT_REMOVEDIR),
        ))
    }
}

#[inline]
pub(crate) fn link(oldname: &ZStr, newname: &ZStr) -> io::Result<()> {
    unsafe {
        ret(syscall5_readonly(
            nr(__NR_linkat),
            c_int(AT_FDCWD),
            c_str(oldname),
            c_int(AT_FDCWD),
            c_str(newname),
            c_uint(0),
        ))
    }
}

#[inline]
pub(crate) fn linkat(
    old_dirfd: BorrowedFd<'_>,
    oldname: &ZStr,
    new_dirfd: BorrowedFd<'_>,
    newname: &ZStr,
    flags: AtFlags,
) -> io::Result<()> {
    unsafe {
        ret(syscall5_readonly(
            nr(__NR_linkat),
            borrowed_fd(old_dirfd),
            c_str(oldname),
            borrowed_fd(new_dirfd),
            c_str(newname),
            c_uint(flags.bits()),
        ))
    }
}

#[inline]
pub(crate) fn symlink(oldname: &ZStr, newname: &ZStr) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_symlinkat),
            c_str(oldname),
            c_int(AT_FDCWD),
            c_str(newname),
        ))
    }
}

#[inline]
pub(crate) fn symlinkat(oldname: &ZStr, dirfd: BorrowedFd<'_>, newname: &ZStr) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_symlinkat),
            c_str(oldname),
            borrowed_fd(dirfd),
            c_str(newname),
        ))
    }
}

#[inline]
pub(crate) fn mkdir(pathname: &ZStr, mode: Mode) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_mkdirat),
            c_int(AT_FDCWD),
            c_str(pathname),
            mode_as(mode),
        ))
    }
}

#[inline]
pub(crate) fn mkdirat(dirfd: BorrowedFd<'_>, pathname: &ZStr, mode: Mode) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_mkdirat),
            borrowed_fd(dirfd),
            c_str(pathname),
            mode_as(mode),
        ))
    }
}

#[inline]
pub(crate) fn getdents(fd: BorrowedFd<'_>, dirent: &mut [u8]) -> io::Result<usize> {
    let (dirent_addr_mut, dirent_len) = slice_mut(dirent);

    unsafe {
        ret_usize(syscall3(
            nr(__NR_getdents64),
            borrowed_fd(fd),
            dirent_addr_mut,
            dirent_len,
        ))
    }
}

#[inline]
pub(crate) fn utimensat(
    dirfd: BorrowedFd<'_>,
    pathname: &ZStr,
    times: &Timestamps,
    flags: AtFlags,
) -> io::Result<()> {
    _utimensat(dirfd, Some(pathname), times, flags)
}

#[inline]
fn _utimensat(
    dirfd: BorrowedFd<'_>,
    pathname: Option<&ZStr>,
    times: &Timestamps,
    flags: AtFlags,
) -> io::Result<()> {
    // Assert that `Timestamps` has the expected layout.
    let _ = unsafe { core::mem::transmute::<Timestamps, [__kernel_timespec; 2]>(times.clone()) };

    #[cfg(target_pointer_width = "32")]
    unsafe {
        match ret(syscall4_readonly(
            nr(__NR_utimensat_time64),
            borrowed_fd(dirfd),
            opt_c_str(pathname),
            by_ref(times),
            c_uint(flags.bits()),
        )) {
            Err(io::Error::NOSYS) => _utimensat_old(dirfd, pathname, times, flags),
            otherwise => otherwise,
        }
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall4_readonly(
            nr(__NR_utimensat),
            borrowed_fd(dirfd),
            opt_c_str(pathname),
            by_ref(times),
            c_uint(flags.bits()),
        ))
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn _utimensat_old(
    dirfd: BorrowedFd<'_>,
    pathname: Option<&ZStr>,
    times: &Timestamps,
    flags: AtFlags,
) -> io::Result<()> {
    // See the comments in `rustix_clock_gettime_via_syscall` about
    // emulation.
    let old_times = [
        __kernel_old_timespec {
            tv_sec: times
                .last_access
                .tv_sec
                .try_into()
                .map_err(|_| io::Error::INVAL)?,
            tv_nsec: times
                .last_access
                .tv_nsec
                .try_into()
                .map_err(|_| io::Error::INVAL)?,
        },
        __kernel_old_timespec {
            tv_sec: times
                .last_modification
                .tv_sec
                .try_into()
                .map_err(|_| io::Error::INVAL)?,
            tv_nsec: times
                .last_modification
                .tv_nsec
                .try_into()
                .map_err(|_| io::Error::INVAL)?,
        },
    ];
    // The length of the array is fixed and not passed into the syscall.
    let old_times_addr = slice_just_addr(&old_times);
    ret(syscall4_readonly(
        nr(__NR_utimensat),
        borrowed_fd(dirfd),
        opt_c_str(pathname),
        old_times_addr,
        c_uint(flags.bits()),
    ))
}

#[inline]
pub(crate) fn futimens(fd: BorrowedFd<'_>, times: &Timestamps) -> io::Result<()> {
    _utimensat(fd, None, times, AtFlags::empty())
}

pub(crate) fn accessat(
    dirfd: BorrowedFd<'_>,
    path: &ZStr,
    access: Access,
    flags: AtFlags,
) -> io::Result<()> {
    if flags.is_empty()
        || (flags.bits() == linux_raw_sys::general::AT_EACCESS
            && crate::process::getuid() == crate::process::geteuid()
            && crate::process::getgid() == crate::process::getegid())
    {
        return _accessat(dirfd, path, access.bits());
    }

    if flags.bits() != linux_raw_sys::general::AT_EACCESS {
        return Err(io::Error::INVAL);
    }

    // TODO: Use faccessat2 in newer Linux versions.
    Err(io::Error::NOSYS)
}

#[inline]
fn _accessat(dirfd: BorrowedFd<'_>, pathname: &ZStr, mode: c::c_uint) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_faccessat),
            borrowed_fd(dirfd),
            c_str(pathname),
            c_uint(mode),
        ))
    }
}

#[inline]
pub(crate) fn copy_file_range(
    fd_in: BorrowedFd<'_>,
    off_in: Option<&mut u64>,
    fd_out: BorrowedFd<'_>,
    off_out: Option<&mut u64>,
    len: u64,
) -> io::Result<u64> {
    let len: usize = len.try_into().unwrap_or(usize::MAX);
    _copy_file_range(fd_in, off_in, fd_out, off_out, len, 0).map(|result| result as u64)
}

#[inline]
fn _copy_file_range(
    fd_in: BorrowedFd<'_>,
    off_in: Option<&mut u64>,
    fd_out: BorrowedFd<'_>,
    off_out: Option<&mut u64>,
    len: usize,
    flags: c::c_uint,
) -> io::Result<usize> {
    unsafe {
        ret_usize(syscall6(
            nr(__NR_copy_file_range),
            borrowed_fd(fd_in),
            opt_mut(off_in),
            borrowed_fd(fd_out),
            opt_mut(off_out),
            pass_usize(len),
            c_uint(flags),
        ))
    }
}

#[inline]
pub(crate) fn memfd_create(name: &ZStr, flags: MemfdFlags) -> io::Result<OwnedFd> {
    unsafe {
        ret_owned_fd(syscall2_readonly(
            nr(__NR_memfd_create),
            c_str(name),
            c_uint(flags.bits()),
        ))
    }
}

#[inline]
pub(crate) fn sendfile(
    out_fd: BorrowedFd<'_>,
    in_fd: BorrowedFd<'_>,
    offset: Option<&mut u64>,
    count: usize,
) -> io::Result<usize> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_usize(syscall4(
            nr(__NR_sendfile64),
            borrowed_fd(out_fd),
            borrowed_fd(in_fd),
            opt_mut(offset),
            pass_usize(count),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall4(
            nr(__NR_sendfile),
            borrowed_fd(out_fd),
            borrowed_fd(in_fd),
            opt_mut(offset),
            pass_usize(count),
        ))
    }
}
