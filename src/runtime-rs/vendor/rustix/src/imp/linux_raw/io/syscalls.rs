//! linux_raw syscalls supporting `rustix::io`.
//!
//! # Safety
//!
//! See the `rustix::imp::syscalls` module documentation for details.

#![allow(unsafe_code)]

#[cfg(not(any(
    target_arch = "aarch64",
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "riscv64"
)))]
use super::super::arch::choose::syscall1;
use super::super::arch::choose::{
    syscall1_readonly, syscall2, syscall2_readonly, syscall3, syscall3_readonly, syscall4,
    syscall4_readonly, syscall5, syscall5_readonly, syscall6,
};
use super::super::c;
use super::super::conv::{
    borrowed_fd, by_ref, c_int, c_uint, no_fd, out, pass_usize, raw_fd, ret, ret_discarded_fd,
    ret_owned_fd, ret_usize, ret_void_star, slice, slice_mut, void_star, zero,
};
use super::super::reg::nr;
use crate::fd::{AsFd, BorrowedFd, RawFd};
use crate::io::{
    self, epoll, Advice, DupFlags, EventfdFlags, IoSlice, IoSliceMut, MapFlags, MlockFlags,
    MprotectFlags, MremapFlags, MsyncFlags, OwnedFd, PipeFlags, PollFd, ProtFlags, ReadWriteFlags,
    Termios, UserfaultfdFlags, Winsize,
};
use crate::net::{RecvFlags, SendFlags};
#[cfg(feature = "procfs")]
use crate::{ffi::ZStr, fs::FileType, path::DecInt};
use core::cmp;
use core::mem::MaybeUninit;
#[cfg(any(target_arch = "arm", target_arch = "mips", target_arch = "x86"))]
use linux_raw_sys::general::__NR_mmap2;
#[cfg(not(any(
    target_arch = "aarch64",
    target_arch = "mips",
    target_arch = "mips64",
    target_arch = "riscv64"
)))]
use linux_raw_sys::general::__NR_pipe;
use linux_raw_sys::general::{
    __NR_close, __NR_dup, __NR_dup3, __NR_epoll_create1, __NR_epoll_ctl, __NR_eventfd2, __NR_ioctl,
    __NR_madvise, __NR_mlock, __NR_mlock2, __NR_mprotect, __NR_mremap, __NR_msync, __NR_munlock,
    __NR_munmap, __NR_pipe2, __NR_pread64, __NR_preadv, __NR_preadv2, __NR_pwrite64, __NR_pwritev,
    __NR_pwritev2, __NR_read, __NR_readv, __NR_userfaultfd, __NR_write, __NR_writev, epoll_event,
    EPOLL_CTL_ADD, EPOLL_CTL_DEL, EPOLL_CTL_MOD,
};
use linux_raw_sys::ioctl::{
    BLKPBSZGET, BLKSSZGET, FIONBIO, FIONREAD, TCGETS, TIOCEXCL, TIOCGWINSZ, TIOCNXCL,
};
#[cfg(target_pointer_width = "32")]
use {
    super::super::arch::choose::syscall6_readonly,
    super::super::conv::{hi, lo},
    core::convert::TryInto,
};
#[cfg(target_pointer_width = "64")]
use {super::super::conv::loff_t_from_u64, linux_raw_sys::general::__NR_mmap};
#[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
use {
    super::super::conv::{opt_ref, size_of},
    linux_raw_sys::general::__NR_epoll_pwait,
    linux_raw_sys::general::{__NR_ppoll, __kernel_timespec, sigset_t},
};
#[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
use {
    linux_raw_sys::general::__NR_epoll_wait,
    linux_raw_sys::general::{__NR_dup2, __NR_poll},
};

#[inline]
pub(crate) fn read(fd: BorrowedFd<'_>, buf: &mut [u8]) -> io::Result<usize> {
    let (buf_addr_mut, buf_len) = slice_mut(buf);

    unsafe {
        ret_usize(syscall3(
            nr(__NR_read),
            borrowed_fd(fd),
            buf_addr_mut,
            buf_len,
        ))
    }
}

#[inline]
pub(crate) fn pread(fd: BorrowedFd<'_>, buf: &mut [u8], pos: u64) -> io::Result<usize> {
    let (buf_addr_mut, buf_len) = slice_mut(buf);

    // <https://github.com/torvalds/linux/blob/fcadab740480e0e0e9fa9bd272acd409884d431a/arch/arm64/kernel/sys32.c#L75>
    #[cfg(all(
        target_pointer_width = "32",
        any(target_arch = "arm", target_arch = "mips", target_arch = "power")
    ))]
    unsafe {
        ret_usize(syscall6(
            nr(__NR_pread64),
            borrowed_fd(fd),
            buf_addr_mut,
            buf_len,
            zero(),
            hi(pos),
            lo(pos),
        ))
    }
    #[cfg(all(
        target_pointer_width = "32",
        not(any(target_arch = "arm", target_arch = "mips", target_arch = "power"))
    ))]
    unsafe {
        ret_usize(syscall5(
            nr(__NR_pread64),
            borrowed_fd(fd),
            buf_addr_mut,
            buf_len,
            hi(pos),
            lo(pos),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall4(
            nr(__NR_pread64),
            borrowed_fd(fd),
            buf_addr_mut,
            buf_len,
            loff_t_from_u64(pos),
        ))
    }
}

#[inline]
pub(crate) fn readv(fd: BorrowedFd<'_>, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), max_iov())]);

    unsafe {
        ret_usize(syscall3(
            nr(__NR_readv),
            borrowed_fd(fd),
            bufs_addr,
            bufs_len,
        ))
    }
}

#[inline]
pub(crate) fn preadv(
    fd: BorrowedFd<'_>,
    bufs: &mut [IoSliceMut<'_>],
    pos: u64,
) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), max_iov())]);

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_usize(syscall5(
            nr(__NR_preadv),
            borrowed_fd(fd),
            bufs_addr,
            bufs_len,
            hi(pos),
            lo(pos),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall4(
            nr(__NR_preadv),
            borrowed_fd(fd),
            bufs_addr,
            bufs_len,
            loff_t_from_u64(pos),
        ))
    }
}

#[inline]
pub(crate) fn preadv2(
    fd: BorrowedFd<'_>,
    bufs: &mut [IoSliceMut<'_>],
    pos: u64,
    flags: ReadWriteFlags,
) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), max_iov())]);

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_usize(syscall6(
            nr(__NR_preadv2),
            borrowed_fd(fd),
            bufs_addr,
            bufs_len,
            hi(pos),
            lo(pos),
            c_uint(flags.bits()),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall5(
            nr(__NR_preadv2),
            borrowed_fd(fd),
            bufs_addr,
            bufs_len,
            loff_t_from_u64(pos),
            c_uint(flags.bits()),
        ))
    }
}

#[inline]
pub(crate) fn write(fd: BorrowedFd<'_>, buf: &[u8]) -> io::Result<usize> {
    let (buf_addr, buf_len) = slice(buf);

    unsafe {
        ret_usize(syscall3_readonly(
            nr(__NR_write),
            borrowed_fd(fd),
            buf_addr,
            buf_len,
        ))
    }
}

#[inline]
pub(crate) fn pwrite(fd: BorrowedFd<'_>, buf: &[u8], pos: u64) -> io::Result<usize> {
    let (buf_addr, buf_len) = slice(buf);

    // <https://github.com/torvalds/linux/blob/fcadab740480e0e0e9fa9bd272acd409884d431a/arch/arm64/kernel/sys32.c#L81-L83>
    #[cfg(all(
        target_pointer_width = "32",
        any(target_arch = "arm", target_arch = "mips", target_arch = "power")
    ))]
    unsafe {
        ret_usize(syscall6_readonly(
            nr(__NR_pwrite64),
            borrowed_fd(fd),
            buf_addr,
            buf_len,
            zero(),
            hi(pos),
            lo(pos),
        ))
    }
    #[cfg(all(
        target_pointer_width = "32",
        not(any(target_arch = "arm", target_arch = "mips", target_arch = "power"))
    ))]
    unsafe {
        ret_usize(syscall5_readonly(
            nr(__NR_pwrite64),
            borrowed_fd(fd),
            buf_addr,
            buf_len,
            hi(pos),
            lo(pos),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall4_readonly(
            nr(__NR_pwrite64),
            borrowed_fd(fd),
            buf_addr,
            buf_len,
            loff_t_from_u64(pos),
        ))
    }
}

#[inline]
pub(crate) fn writev(fd: BorrowedFd<'_>, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), max_iov())]);

    unsafe {
        ret_usize(syscall3_readonly(
            nr(__NR_writev),
            borrowed_fd(fd),
            bufs_addr,
            bufs_len,
        ))
    }
}

#[inline]
pub(crate) fn pwritev(fd: BorrowedFd<'_>, bufs: &[IoSlice<'_>], pos: u64) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), max_iov())]);

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_usize(syscall5_readonly(
            nr(__NR_pwritev),
            borrowed_fd(fd),
            bufs_addr,
            bufs_len,
            hi(pos),
            lo(pos),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall4_readonly(
            nr(__NR_pwritev),
            borrowed_fd(fd),
            bufs_addr,
            bufs_len,
            loff_t_from_u64(pos),
        ))
    }
}

#[inline]
pub(crate) fn pwritev2(
    fd: BorrowedFd<'_>,
    bufs: &[IoSlice<'_>],
    pos: u64,
    flags: ReadWriteFlags,
) -> io::Result<usize> {
    let (bufs_addr, bufs_len) = slice(&bufs[..cmp::min(bufs.len(), max_iov())]);

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret_usize(syscall6_readonly(
            nr(__NR_pwritev2),
            borrowed_fd(fd),
            bufs_addr,
            bufs_len,
            hi(pos),
            lo(pos),
            c_uint(flags.bits()),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret_usize(syscall5_readonly(
            nr(__NR_pwritev2),
            borrowed_fd(fd),
            bufs_addr,
            bufs_len,
            loff_t_from_u64(pos),
            c_uint(flags.bits()),
        ))
    }
}

const fn max_iov() -> usize {
    linux_raw_sys::general::UIO_MAXIOV as usize
}

#[inline]
pub(crate) unsafe fn close(fd: RawFd) {
    // See the docunentation for [`io::close`] for why errors are ignored.
    syscall1_readonly(nr(__NR_close), raw_fd(fd)).decode_void();
}

#[inline]
pub(crate) fn madvise(addr: *mut c::c_void, len: usize, advice: Advice) -> io::Result<()> {
    unsafe {
        ret(syscall3(
            nr(__NR_madvise),
            void_star(addr),
            pass_usize(len),
            c_uint(advice as c::c_uint),
        ))
    }
}

#[inline]
pub(crate) unsafe fn msync(addr: *mut c::c_void, len: usize, flags: MsyncFlags) -> io::Result<()> {
    ret(syscall3(
        nr(__NR_msync),
        void_star(addr),
        pass_usize(len),
        c_uint(flags.bits()),
    ))
}

#[inline]
pub(crate) fn eventfd(initval: u32, flags: EventfdFlags) -> io::Result<OwnedFd> {
    unsafe {
        ret_owned_fd(syscall2_readonly(
            nr(__NR_eventfd2),
            c_uint(initval),
            c_uint(flags.bits()),
        ))
    }
}

#[inline]
pub(crate) fn ioctl_fionread(fd: BorrowedFd<'_>) -> io::Result<u64> {
    unsafe {
        let mut result = MaybeUninit::<c::c_int>::uninit();
        ret(syscall3(
            nr(__NR_ioctl),
            borrowed_fd(fd),
            c_uint(FIONREAD),
            out(&mut result),
        ))
        .map(|()| result.assume_init() as u64)
    }
}

#[inline]
pub(crate) fn ioctl_fionbio(fd: BorrowedFd<'_>, value: bool) -> io::Result<()> {
    unsafe {
        let data = value as c::c_int;
        ret(syscall3_readonly(
            nr(__NR_ioctl),
            borrowed_fd(fd),
            c_uint(FIONBIO),
            by_ref(&data),
        ))
    }
}

#[inline]
pub(crate) fn ioctl_tiocgwinsz(fd: BorrowedFd<'_>) -> io::Result<Winsize> {
    unsafe {
        let mut result = MaybeUninit::<Winsize>::uninit();
        ret(syscall3(
            nr(__NR_ioctl),
            borrowed_fd(fd),
            c_uint(TIOCGWINSZ),
            out(&mut result),
        ))
        .map(|()| result.assume_init())
    }
}

#[inline]
pub(crate) fn ioctl_tiocexcl(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(syscall2_readonly(
            nr(__NR_ioctl),
            borrowed_fd(fd),
            c_uint(TIOCEXCL),
        ))
    }
}

#[inline]
pub(crate) fn ioctl_tiocnxcl(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe {
        ret(syscall2_readonly(
            nr(__NR_ioctl),
            borrowed_fd(fd),
            c_uint(TIOCNXCL),
        ))
    }
}

#[inline]
pub(crate) fn ioctl_tcgets(fd: BorrowedFd<'_>) -> io::Result<Termios> {
    unsafe {
        let mut result = MaybeUninit::<Termios>::uninit();
        ret(syscall3(
            nr(__NR_ioctl),
            borrowed_fd(fd),
            c_uint(TCGETS),
            out(&mut result),
        ))
        .map(|()| result.assume_init())
    }
}

#[inline]
pub(crate) fn ioctl_blksszget(fd: BorrowedFd) -> io::Result<u32> {
    let mut result = MaybeUninit::<c::c_uint>::uninit();
    unsafe {
        ret(syscall3(
            nr(__NR_ioctl),
            borrowed_fd(fd),
            c_uint(BLKSSZGET),
            out(&mut result),
        ))
        .map(|()| result.assume_init() as u32)
    }
}

#[inline]
pub(crate) fn ioctl_blkpbszget(fd: BorrowedFd) -> io::Result<u32> {
    let mut result = MaybeUninit::<c::c_uint>::uninit();
    unsafe {
        ret(syscall3(
            nr(__NR_ioctl),
            borrowed_fd(fd),
            c_uint(BLKPBSZGET),
            out(&mut result),
        ))
        .map(|()| result.assume_init() as u32)
    }
}

#[inline]
pub(crate) fn isatty(fd: BorrowedFd<'_>) -> bool {
    // On error, Linux will return either `EINVAL` (2.6.32) or `ENOTTY`
    // (otherwise), because we assume we're never passing an invalid
    // file descriptor (which would get `EBADF`). Either way, an error
    // means we don't have a tty.
    ioctl_tiocgwinsz(fd).is_ok()
}

pub(crate) fn is_read_write(fd: BorrowedFd<'_>) -> io::Result<(bool, bool)> {
    let (mut read, mut write) = crate::fs::fd::_is_file_read_write(fd)?;
    let mut not_socket = false;
    if read {
        // Do a `recv` with `PEEK` and `DONTWAIT` for 1 byte. A 0 indicates
        // the read side is shut down; an `EWOULDBLOCK` indicates the read
        // side is still open.
        //
        // TODO: This code would benefit from having a better way to read into
        // uninitialized memory.
        let mut buf = [0];
        match super::super::net::syscalls::recv(fd, &mut buf, RecvFlags::PEEK | RecvFlags::DONTWAIT)
        {
            Ok(0) => read = false,
            Err(err) => {
                #[allow(unreachable_patterns)] // `EAGAIN` may equal `EWOULDBLOCK`
                match err {
                    io::Error::AGAIN | io::Error::WOULDBLOCK => (),
                    io::Error::NOTSOCK => not_socket = true,
                    _ => return Err(err),
                }
            }
            Ok(_) => (),
        }
    }
    if write && !not_socket {
        // Do a `send` with `DONTWAIT` for 0 bytes. An `EPIPE` indicates
        // the write side is shut down.
        #[allow(unreachable_patterns)] // `EAGAIN` equals `EWOULDBLOCK`
        match super::super::net::syscalls::send(fd, &[], SendFlags::DONTWAIT) {
            // TODO or-patterns when we don't need 1.51
            Err(io::Error::AGAIN) => (),
            Err(io::Error::WOULDBLOCK) => (),
            Err(io::Error::NOTSOCK) => (),
            Err(io::Error::PIPE) => write = false,
            Err(err) => return Err(err),
            Ok(_) => (),
        }
    }
    Ok((read, write))
}

#[inline]
pub(crate) fn dup(fd: BorrowedFd<'_>) -> io::Result<OwnedFd> {
    unsafe { ret_owned_fd(syscall1_readonly(nr(__NR_dup), borrowed_fd(fd))) }
}

#[inline]
pub(crate) fn dup2(fd: BorrowedFd<'_>, new: &OwnedFd) -> io::Result<()> {
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    {
        // `dup3` fails if the old and new file descriptors have the same
        // value, so emulate the `dup2` behvior.
        use crate::fd::AsRawFd;
        if fd.as_raw_fd() == new.as_raw_fd() {
            return Ok(());
        }
        dup3(fd, new, DupFlags::empty())
    }

    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
    unsafe {
        ret_discarded_fd(syscall2_readonly(
            nr(__NR_dup2),
            borrowed_fd(fd),
            borrowed_fd(new.as_fd()),
        ))
    }
}

#[inline]
pub(crate) fn dup3(fd: BorrowedFd<'_>, new: &OwnedFd, flags: DupFlags) -> io::Result<()> {
    unsafe {
        ret_discarded_fd(syscall3_readonly(
            nr(__NR_dup3),
            borrowed_fd(fd),
            borrowed_fd(new.as_fd()),
            c_uint(flags.bits()),
        ))
    }
}

#[cfg(feature = "procfs")]
#[inline]
pub(crate) fn ttyname(fd: BorrowedFd<'_>, buf: &mut [u8]) -> io::Result<usize> {
    let fd_stat = super::super::fs::syscalls::fstat(fd)?;

    // Quick check: if `fd` isn't a character device, it's not a tty.
    if FileType::from_raw_mode(fd_stat.st_mode) != FileType::CharacterDevice {
        return Err(crate::io::Error::NOTTY);
    }

    // Check that `fd` is really a tty.
    ioctl_tiocgwinsz(fd)?;

    // Get a fd to '/proc/self/fd'.
    let proc_self_fd = io::proc_self_fd()?;

    // Gather the ttyname by reading the 'fd' file inside 'proc_self_fd'.
    let r =
        super::super::fs::syscalls::readlinkat(proc_self_fd, DecInt::from_fd(&fd).as_c_str(), buf)?;

    // If the number of bytes is equal to the buffer length, truncation may
    // have occurred. This check also ensures that we have enough space for
    // adding a NUL terminator.
    if r == buf.len() {
        return Err(io::Error::RANGE);
    }
    buf[r] = b'\0';

    // Check that the path we read refers to the same file as `fd`.
    let path = ZStr::from_bytes_with_nul(&buf[..=r]).unwrap();

    let path_stat = super::super::fs::syscalls::stat(path)?;
    if path_stat.st_dev != fd_stat.st_dev || path_stat.st_ino != fd_stat.st_ino {
        return Err(crate::io::Error::NODEV);
    }

    Ok(r)
}

/// # Safety
///
/// `mmap` is primarily unsafe due to the `addr` parameter, as anything working
/// with memory pointed to by raw pointers is unsafe.
#[inline]
pub(crate) unsafe fn mmap(
    addr: *mut c::c_void,
    length: usize,
    prot: ProtFlags,
    flags: MapFlags,
    fd: BorrowedFd<'_>,
    offset: u64,
) -> io::Result<*mut c::c_void> {
    #[cfg(target_pointer_width = "32")]
    {
        ret_void_star(syscall6(
            nr(__NR_mmap2),
            void_star(addr),
            pass_usize(length),
            c_uint(prot.bits()),
            c_uint(flags.bits()),
            borrowed_fd(fd),
            (offset / 4096)
                .try_into()
                .map(|scaled_offset| pass_usize(scaled_offset))
                .map_err(|_| io::Error::INVAL)?,
        ))
    }
    #[cfg(target_pointer_width = "64")]
    {
        ret_void_star(syscall6(
            nr(__NR_mmap),
            void_star(addr),
            pass_usize(length),
            c_uint(prot.bits()),
            c_uint(flags.bits()),
            borrowed_fd(fd),
            loff_t_from_u64(offset),
        ))
    }
}

/// # Safety
///
/// `mmap` is primarily unsafe due to the `addr` parameter, as anything working
/// with memory pointed to by raw pointers is unsafe.
#[inline]
pub(crate) unsafe fn mmap_anonymous(
    addr: *mut c::c_void,
    length: usize,
    prot: ProtFlags,
    flags: MapFlags,
) -> io::Result<*mut c::c_void> {
    #[cfg(target_pointer_width = "32")]
    {
        ret_void_star(syscall6(
            nr(__NR_mmap2),
            void_star(addr),
            pass_usize(length),
            c_uint(prot.bits()),
            c_uint(flags.bits() | linux_raw_sys::general::MAP_ANONYMOUS),
            no_fd(),
            pass_usize(0),
        ))
    }
    #[cfg(target_pointer_width = "64")]
    {
        ret_void_star(syscall6(
            nr(__NR_mmap),
            void_star(addr),
            pass_usize(length),
            c_uint(prot.bits()),
            c_uint(flags.bits() | linux_raw_sys::general::MAP_ANONYMOUS),
            no_fd(),
            loff_t_from_u64(0),
        ))
    }
}

#[inline]
pub(crate) unsafe fn mprotect(
    ptr: *mut c::c_void,
    len: usize,
    flags: MprotectFlags,
) -> io::Result<()> {
    ret(syscall3(
        nr(__NR_mprotect),
        void_star(ptr),
        pass_usize(len),
        c_uint(flags.bits()),
    ))
}

/// # Safety
///
/// `munmap` is primarily unsafe due to the `addr` parameter, as anything
/// working with memory pointed to by raw pointers is unsafe.
#[inline]
pub(crate) unsafe fn munmap(addr: *mut c::c_void, length: usize) -> io::Result<()> {
    ret(syscall2(
        nr(__NR_munmap),
        void_star(addr),
        pass_usize(length),
    ))
}

/// # Safety
///
/// `mremap` is primarily unsafe due to the `old_address` parameter, as
/// anything working with memory pointed to by raw pointers is unsafe.
#[inline]
pub(crate) unsafe fn mremap(
    old_address: *mut c::c_void,
    old_size: usize,
    new_size: usize,
    flags: MremapFlags,
) -> io::Result<*mut c::c_void> {
    ret_void_star(syscall4(
        nr(__NR_mremap),
        void_star(old_address),
        pass_usize(old_size),
        pass_usize(new_size),
        c_uint(flags.bits()),
    ))
}

/// # Safety
///
/// `mremap_fixed` is primarily unsafe due to the `old_address` and
/// `new_address` parameters, as anything working with memory pointed to by raw
/// pointers is unsafe.
#[inline]
pub(crate) unsafe fn mremap_fixed(
    old_address: *mut c::c_void,
    old_size: usize,
    new_size: usize,
    flags: MremapFlags,
    new_address: *mut c::c_void,
) -> io::Result<*mut c::c_void> {
    ret_void_star(syscall5(
        nr(__NR_mremap),
        void_star(old_address),
        pass_usize(old_size),
        pass_usize(new_size),
        c_uint(flags.bits()),
        void_star(new_address),
    ))
}

/// # Safety
///
/// `mlock` operates on raw pointers and may round out to the nearest page
/// boundaries.
#[inline]
pub(crate) unsafe fn mlock(addr: *mut c::c_void, length: usize) -> io::Result<()> {
    ret(syscall2(
        nr(__NR_mlock),
        void_star(addr),
        pass_usize(length),
    ))
}

/// # Safety
///
/// `mlock_with` operates on raw pointers and may round out to the nearest page
/// boundaries.
#[inline]
pub(crate) unsafe fn mlock_with(
    addr: *mut c::c_void,
    length: usize,
    flags: MlockFlags,
) -> io::Result<()> {
    ret(syscall3(
        nr(__NR_mlock2),
        void_star(addr),
        pass_usize(length),
        c_uint(flags.bits()),
    ))
}

/// # Safety
///
/// `munlock` operates on raw pointers and may round out to the nearest page
/// boundaries.
#[inline]
pub(crate) unsafe fn munlock(addr: *mut c::c_void, length: usize) -> io::Result<()> {
    ret(syscall2(
        nr(__NR_munlock),
        void_star(addr),
        pass_usize(length),
    ))
}

#[inline]
pub(crate) fn pipe_with(flags: PipeFlags) -> io::Result<(OwnedFd, OwnedFd)> {
    unsafe {
        let mut result = MaybeUninit::<[OwnedFd; 2]>::uninit();
        ret(syscall2(
            nr(__NR_pipe2),
            out(&mut result),
            c_uint(flags.bits()),
        ))?;
        let [p0, p1] = result.assume_init();
        Ok((p0, p1))
    }
}

#[inline]
pub(crate) fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    // aarch64 and risc64 omit `__NR_pipe`. On mips, `__NR_pipe` uses a special
    // calling convention, but using it is not worth complicating our syscall
    // wrapping infrastructure at this time.
    #[cfg(any(
        target_arch = "aarch64",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "riscv64"
    ))]
    {
        pipe_with(PipeFlags::empty())
    }
    #[cfg(not(any(
        target_arch = "aarch64",
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "riscv64"
    )))]
    unsafe {
        let mut result = MaybeUninit::<[OwnedFd; 2]>::uninit();
        ret(syscall1(nr(__NR_pipe), out(&mut result)))?;
        let [p0, p1] = result.assume_init();
        Ok((p0, p1))
    }
}

#[inline]
pub(crate) fn poll(fds: &mut [PollFd<'_>], timeout: c::c_int) -> io::Result<usize> {
    let (fds_addr_mut, fds_len) = slice_mut(fds);

    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    unsafe {
        let timeout = if timeout >= 0 {
            Some(__kernel_timespec {
                tv_sec: (timeout as i64) / 1000,
                tv_nsec: (timeout as i64) % 1000 * 1000000,
            })
        } else {
            None
        };
        ret_usize(syscall5(
            nr(__NR_ppoll),
            fds_addr_mut,
            fds_len,
            opt_ref(timeout.as_ref()),
            zero(),
            size_of::<sigset_t, _>(),
        ))
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
    unsafe {
        ret_usize(syscall3(
            nr(__NR_poll),
            fds_addr_mut,
            fds_len,
            c_int(timeout),
        ))
    }
}

#[inline]
pub(crate) unsafe fn userfaultfd(flags: UserfaultfdFlags) -> io::Result<OwnedFd> {
    ret_owned_fd(syscall1_readonly(
        nr(__NR_userfaultfd),
        c_uint(flags.bits()),
    ))
}

#[inline]
pub(crate) fn epoll_create(flags: epoll::CreateFlags) -> io::Result<OwnedFd> {
    unsafe {
        ret_owned_fd(syscall1_readonly(
            nr(__NR_epoll_create1),
            c_uint(flags.bits()),
        ))
    }
}

#[inline]
pub(crate) unsafe fn epoll_add(
    epfd: BorrowedFd<'_>,
    fd: c::c_int,
    event: &epoll_event,
) -> io::Result<()> {
    ret(syscall4_readonly(
        nr(__NR_epoll_ctl),
        borrowed_fd(epfd),
        c_uint(EPOLL_CTL_ADD),
        raw_fd(fd),
        by_ref(event),
    ))
}

#[inline]
pub(crate) unsafe fn epoll_mod(
    epfd: BorrowedFd<'_>,
    fd: c::c_int,
    event: &epoll_event,
) -> io::Result<()> {
    ret(syscall4_readonly(
        nr(__NR_epoll_ctl),
        borrowed_fd(epfd),
        c_uint(EPOLL_CTL_MOD),
        raw_fd(fd),
        by_ref(event),
    ))
}

#[inline]
pub(crate) unsafe fn epoll_del(epfd: BorrowedFd<'_>, fd: c::c_int) -> io::Result<()> {
    ret(syscall4_readonly(
        nr(__NR_epoll_ctl),
        borrowed_fd(epfd),
        c_uint(EPOLL_CTL_DEL),
        raw_fd(fd),
        zero(),
    ))
}

#[inline]
pub(crate) fn epoll_wait(
    epfd: BorrowedFd<'_>,
    events: *mut epoll_event,
    num_events: usize,
    timeout: c::c_int,
) -> io::Result<usize> {
    #[cfg(not(any(target_arch = "aarch64", target_arch = "riscv64")))]
    unsafe {
        ret_usize(syscall4(
            nr(__NR_epoll_wait),
            borrowed_fd(epfd),
            void_star(events.cast::<c::c_void>()),
            pass_usize(num_events),
            c_int(timeout),
        ))
    }
    #[cfg(any(target_arch = "aarch64", target_arch = "riscv64"))]
    unsafe {
        ret_usize(syscall5(
            nr(__NR_epoll_pwait),
            borrowed_fd(epfd),
            void_star(events.cast::<c::c_void>()),
            pass_usize(num_events),
            c_int(timeout),
            zero(),
        ))
    }
}
