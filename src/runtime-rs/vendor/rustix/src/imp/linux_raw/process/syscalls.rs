//! linux_raw syscalls supporting `rustix::process`.
//!
//! # Safety
//!
//! See the `rustix::imp::syscalls` module documentation for details.

#![allow(unsafe_code)]

use super::super::arch::choose::{
    syscall0_readonly, syscall1, syscall1_noreturn, syscall1_readonly, syscall2, syscall2_readonly,
    syscall3, syscall3_readonly, syscall4,
};
use super::super::c;
use super::super::conv::{
    borrowed_fd, by_mut, by_ref, c_int, c_str, c_uint, const_void_star, negative_pid, out,
    pass_usize, regular_pid, resource, ret, ret_c_int, ret_c_uint, ret_infallible, ret_usize,
    ret_usize_infallible, signal, size_of, slice_just_addr, slice_mut, void_star, zero,
};
use super::super::reg::nr;
use super::{RawCpuSet, RawUname};
use crate::fd::BorrowedFd;
use crate::ffi::ZStr;
use crate::io;
use crate::process::{
    Cpuid, Gid, MembarrierCommand, MembarrierQuery, Pid, RawNonZeroPid, RawPid, Resource, Rlimit,
    Signal, Uid, WaitOptions, WaitStatus,
};
use core::convert::TryInto;
use core::mem::MaybeUninit;
#[cfg(not(any(
    target_arch = "arm",
    target_arch = "powerpc",
    target_arch = "powerpc64",
    target_arch = "x86"
)))]
use linux_raw_sys::general::__NR_getrlimit;
#[cfg(any(
    target_arch = "arm",
    target_arch = "powerpc",
    target_arch = "powerpc64",
    target_arch = "x86"
))]
use linux_raw_sys::general::__NR_ugetrlimit as __NR_getrlimit;
use linux_raw_sys::general::{
    __NR_chdir, __NR_exit_group, __NR_fchdir, __NR_getcwd, __NR_getpid, __NR_getppid,
    __NR_getpriority, __NR_kill, __NR_membarrier, __NR_prlimit64, __NR_sched_getaffinity,
    __NR_sched_setaffinity, __NR_sched_yield, __NR_setpriority, __NR_setrlimit, __NR_setsid,
    __NR_uname, __NR_wait4, __kernel_gid_t, __kernel_pid_t, __kernel_uid_t,
};
#[cfg(not(any(target_arch = "x86", target_arch = "sparc", target_arch = "arm")))]
use linux_raw_sys::general::{__NR_getegid, __NR_geteuid, __NR_getgid, __NR_getuid};
#[cfg(any(target_arch = "x86", target_arch = "sparc", target_arch = "arm"))]
use linux_raw_sys::general::{__NR_getegid32, __NR_geteuid32, __NR_getgid32, __NR_getuid32};

#[inline]
pub(crate) fn chdir(filename: &ZStr) -> io::Result<()> {
    unsafe { ret(syscall1_readonly(nr(__NR_chdir), c_str(filename))) }
}

#[inline]
pub(crate) fn fchdir(fd: BorrowedFd<'_>) -> io::Result<()> {
    unsafe { ret(syscall1_readonly(nr(__NR_fchdir), borrowed_fd(fd))) }
}

#[inline]
pub(crate) fn getcwd(buf: &mut [u8]) -> io::Result<usize> {
    let (buf_addr_mut, buf_len) = slice_mut(buf);
    unsafe { ret_usize(syscall2(nr(__NR_getcwd), buf_addr_mut, buf_len)) }
}

#[inline]
pub(crate) fn membarrier_query() -> MembarrierQuery {
    unsafe {
        match ret_c_uint(syscall2(
            nr(__NR_membarrier),
            c_int(linux_raw_sys::general::membarrier_cmd::MEMBARRIER_CMD_QUERY as _),
            c_uint(0),
        )) {
            Ok(query) => {
                // Safety: The safety of `from_bits_unchecked` is discussed
                // [here]. Our "source of truth" is Linux, and here, the
                // `query` value is coming from Linux, so we know it only
                // contains "source of truth" valid bits.
                //
                // [here]: https://github.com/bitflags/bitflags/pull/207#issuecomment-671668662
                MembarrierQuery::from_bits_unchecked(query)
            }
            Err(_) => MembarrierQuery::empty(),
        }
    }
}

#[inline]
pub(crate) fn membarrier(cmd: MembarrierCommand) -> io::Result<()> {
    unsafe {
        ret(syscall2(
            nr(__NR_membarrier),
            c_int(cmd as c::c_int),
            c_uint(0),
        ))
    }
}

#[inline]
pub(crate) fn membarrier_cpu(cmd: MembarrierCommand, cpu: Cpuid) -> io::Result<()> {
    unsafe {
        ret(syscall3(
            nr(__NR_membarrier),
            c_int(cmd as c::c_int),
            c_uint(linux_raw_sys::general::membarrier_cmd_flag::MEMBARRIER_CMD_FLAG_CPU as _),
            c_uint(cpu.as_raw()),
        ))
    }
}

#[inline]
pub(crate) fn getpid() -> Pid {
    unsafe {
        let pid: i32 = ret_usize_infallible(syscall0_readonly(nr(__NR_getpid))) as __kernel_pid_t;
        debug_assert_ne!(pid, 0);
        Pid::from_raw_nonzero(RawNonZeroPid::new_unchecked(pid as u32))
    }
}

#[inline]
pub(crate) fn getppid() -> Option<Pid> {
    unsafe {
        let ppid: i32 = ret_usize_infallible(syscall0_readonly(nr(__NR_getppid))) as __kernel_pid_t;
        Pid::from_raw(ppid as u32)
    }
}

#[inline]
pub(crate) fn getgid() -> Gid {
    #[cfg(any(target_arch = "x86", target_arch = "sparc", target_arch = "arm"))]
    unsafe {
        let gid: i32 =
            (ret_usize_infallible(syscall0_readonly(nr(__NR_getgid32))) as __kernel_gid_t).into();
        Gid::from_raw(gid as u32)
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "sparc", target_arch = "arm")))]
    unsafe {
        let gid = ret_usize_infallible(syscall0_readonly(nr(__NR_getgid))) as __kernel_gid_t;
        Gid::from_raw(gid)
    }
}

#[inline]
pub(crate) fn getegid() -> Gid {
    #[cfg(any(target_arch = "x86", target_arch = "sparc", target_arch = "arm"))]
    unsafe {
        let gid: i32 =
            (ret_usize_infallible(syscall0_readonly(nr(__NR_getegid32))) as __kernel_gid_t).into();
        Gid::from_raw(gid as u32)
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "sparc", target_arch = "arm")))]
    unsafe {
        let gid = ret_usize_infallible(syscall0_readonly(nr(__NR_getegid))) as __kernel_gid_t;
        Gid::from_raw(gid)
    }
}

#[inline]
pub(crate) fn getuid() -> Uid {
    #[cfg(any(target_arch = "x86", target_arch = "sparc", target_arch = "arm"))]
    unsafe {
        let uid =
            (ret_usize_infallible(syscall0_readonly(nr(__NR_getuid32))) as __kernel_uid_t).into();
        Uid::from_raw(uid)
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "sparc", target_arch = "arm")))]
    unsafe {
        let uid = ret_usize_infallible(syscall0_readonly(nr(__NR_getuid))) as __kernel_uid_t;
        Uid::from_raw(uid)
    }
}

#[inline]
pub(crate) fn geteuid() -> Uid {
    #[cfg(any(target_arch = "x86", target_arch = "sparc", target_arch = "arm"))]
    unsafe {
        let uid: i32 =
            (ret_usize_infallible(syscall0_readonly(nr(__NR_geteuid32))) as __kernel_uid_t).into();
        Uid::from_raw(uid as u32)
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "sparc", target_arch = "arm")))]
    unsafe {
        let uid = ret_usize_infallible(syscall0_readonly(nr(__NR_geteuid))) as __kernel_uid_t;
        Uid::from_raw(uid)
    }
}

#[inline]
pub(crate) fn sched_getaffinity(pid: Option<Pid>, cpuset: &mut RawCpuSet) -> io::Result<()> {
    unsafe {
        // The raw linux syscall returns the size (in bytes) of the `cpumask_t`
        // data type that is used internally by the kernel to represent the CPU
        // set bit mask.
        let size = ret_usize(syscall3(
            nr(__NR_sched_getaffinity),
            c_uint(Pid::as_raw(pid)),
            size_of::<RawCpuSet, _>(),
            by_mut(&mut cpuset.bits),
        ))?;
        let bytes = (cpuset as *mut RawCpuSet).cast::<u8>();
        let rest = bytes.wrapping_add(size);
        // Zero every byte in the cpuset not set by the kernel.
        rest.write_bytes(0, core::mem::size_of::<RawCpuSet>() - size);
        Ok(())
    }
}

#[inline]
pub(crate) fn sched_setaffinity(pid: Option<Pid>, cpuset: &RawCpuSet) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_sched_setaffinity),
            c_uint(Pid::as_raw(pid)),
            size_of::<RawCpuSet, _>(),
            slice_just_addr(&cpuset.bits),
        ))
    }
}

#[inline]
pub(crate) fn sched_yield() {
    unsafe {
        // See the docunentation for [`crate::process::sched_yield`] for why
        // errors are ignored.
        syscall0_readonly(nr(__NR_sched_yield)).decode_void();
    }
}

#[inline]
pub(crate) fn uname() -> RawUname {
    let mut uname = MaybeUninit::<RawUname>::uninit();
    unsafe {
        ret(syscall1(nr(__NR_uname), out(&mut uname))).unwrap();
        uname.assume_init()
    }
}

#[inline]
pub(crate) fn nice(inc: i32) -> io::Result<i32> {
    let priority = if inc > -40 && inc < 40 {
        inc + getpriority_process(None)?
    } else {
        inc
    }
    // TODO: With Rust 1.50, use `.clamp` instead of `.min` and `.max`.
    //.clamp(-20, 19);
    .min(19)
    .max(-20);
    setpriority_process(None, priority)?;
    Ok(priority)
}

#[inline]
pub(crate) fn getpriority_user(uid: Uid) -> io::Result<i32> {
    unsafe {
        Ok(20
            - ret_c_int(syscall2_readonly(
                nr(__NR_getpriority),
                c_uint(linux_raw_sys::general::PRIO_USER),
                c_uint(uid.as_raw()),
            ))?)
    }
}

#[inline]
pub(crate) fn getpriority_pgrp(pgid: Option<Pid>) -> io::Result<i32> {
    unsafe {
        Ok(20
            - ret_c_int(syscall2_readonly(
                nr(__NR_getpriority),
                c_uint(linux_raw_sys::general::PRIO_PGRP),
                c_uint(Pid::as_raw(pgid)),
            ))?)
    }
}

#[inline]
pub(crate) fn getpriority_process(pid: Option<Pid>) -> io::Result<i32> {
    unsafe {
        Ok(20
            - ret_c_int(syscall2_readonly(
                nr(__NR_getpriority),
                c_uint(linux_raw_sys::general::PRIO_PROCESS),
                c_uint(Pid::as_raw(pid)),
            ))?)
    }
}

#[inline]
pub(crate) fn setpriority_user(uid: Uid, priority: i32) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_setpriority),
            c_uint(linux_raw_sys::general::PRIO_USER),
            c_uint(uid.as_raw()),
            c_int(priority),
        ))
    }
}

#[inline]
pub(crate) fn setpriority_pgrp(pgid: Option<Pid>, priority: i32) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_setpriority),
            c_uint(linux_raw_sys::general::PRIO_PGRP),
            c_uint(Pid::as_raw(pgid)),
            c_int(priority),
        ))
    }
}

#[inline]
pub(crate) fn setpriority_process(pid: Option<Pid>, priority: i32) -> io::Result<()> {
    unsafe {
        ret(syscall3_readonly(
            nr(__NR_setpriority),
            c_uint(linux_raw_sys::general::PRIO_PROCESS),
            c_uint(Pid::as_raw(pid)),
            c_int(priority),
        ))
    }
}

#[inline]
pub(crate) fn getrlimit(limit: Resource) -> Rlimit {
    let mut result = MaybeUninit::<linux_raw_sys::general::rlimit64>::uninit();
    unsafe {
        match ret(syscall4(
            nr(__NR_prlimit64),
            c_uint(0),
            resource(limit),
            const_void_star(core::ptr::null()),
            out(&mut result),
        )) {
            Ok(()) => rlimit_from_linux(result.assume_init()),
            Err(e) => {
                debug_assert_eq!(e, io::Error::NOSYS);
                getrlimit_old(limit)
            }
        }
    }
}

/// The old 32-bit-only `getrlimit` syscall, for when we lack the new
/// `prlimit64`.
unsafe fn getrlimit_old(limit: Resource) -> Rlimit {
    let mut result = MaybeUninit::<linux_raw_sys::general::rlimit>::uninit();
    ret_infallible(syscall2(
        nr(__NR_getrlimit),
        resource(limit),
        out(&mut result),
    ));
    rlimit_from_linux_old(result.assume_init())
}

#[inline]
pub(crate) fn setrlimit(limit: Resource, new: Rlimit) -> io::Result<()> {
    unsafe {
        let lim = rlimit_to_linux(new.clone())?;
        match ret(syscall4(
            nr(__NR_prlimit64),
            c_uint(0),
            resource(limit),
            by_ref(&lim),
            void_star(core::ptr::null_mut()),
        )) {
            Ok(()) => Ok(()),
            Err(io::Error::NOSYS) => setrlimit_old(limit, new),
            Err(e) => Err(e),
        }
    }
}

/// The old 32-bit-only `setrlimit` syscall, for when we lack the new
/// `prlimit64`.
unsafe fn setrlimit_old(limit: Resource, new: Rlimit) -> io::Result<()> {
    let lim = rlimit_to_linux_old(new)?;
    ret(syscall2(nr(__NR_setrlimit), resource(limit), by_ref(&lim)))
}

#[inline]
pub(crate) fn prlimit(pid: Option<Pid>, limit: Resource, new: Rlimit) -> io::Result<Rlimit> {
    let lim = rlimit_to_linux(new)?;
    let mut result = MaybeUninit::<linux_raw_sys::general::rlimit64>::uninit();
    unsafe {
        match ret(syscall4(
            nr(__NR_prlimit64),
            c_uint(Pid::as_raw(pid)),
            resource(limit),
            by_ref(&lim),
            out(&mut result),
        )) {
            Ok(()) => Ok(rlimit_from_linux(result.assume_init())),
            Err(e) => Err(e),
        }
    }
}

/// Convert a Rust [`Rlimit`] to a C `rlimit64`.
#[inline]
fn rlimit_from_linux(lim: linux_raw_sys::general::rlimit64) -> Rlimit {
    let current = if lim.rlim_cur == linux_raw_sys::general::RLIM64_INFINITY as _ {
        None
    } else {
        Some(lim.rlim_cur)
    };
    let maximum = if lim.rlim_max == linux_raw_sys::general::RLIM64_INFINITY as _ {
        None
    } else {
        Some(lim.rlim_max)
    };
    Rlimit { current, maximum }
}

/// Convert a C `rlimit64` to a Rust `Rlimit`.
#[inline]
fn rlimit_to_linux(lim: Rlimit) -> io::Result<linux_raw_sys::general::rlimit64> {
    let rlim_cur = match lim.current {
        Some(r) => r,
        None => linux_raw_sys::general::RLIM64_INFINITY as _,
    };
    let rlim_max = match lim.maximum {
        Some(r) => r,
        None => linux_raw_sys::general::RLIM64_INFINITY as _,
    };
    Ok(linux_raw_sys::general::rlimit64 { rlim_cur, rlim_max })
}

/// Like `rlimit_from_linux` but uses Linux's old 32-bit `rlimit`.
fn rlimit_from_linux_old(lim: linux_raw_sys::general::rlimit) -> Rlimit {
    let current = if lim.rlim_cur == linux_raw_sys::general::RLIM_INFINITY as _ {
        None
    } else {
        Some(lim.rlim_cur.into())
    };
    let maximum = if lim.rlim_max == linux_raw_sys::general::RLIM_INFINITY as _ {
        None
    } else {
        Some(lim.rlim_max.into())
    };
    Rlimit { current, maximum }
}

/// Like `rlimit_to_linux` but uses Linux's old 32-bit `rlimit`.
fn rlimit_to_linux_old(lim: Rlimit) -> io::Result<linux_raw_sys::general::rlimit> {
    let rlim_cur = match lim.current {
        Some(r) => r.try_into().map_err(|_| io::Error::INVAL)?,
        None => linux_raw_sys::general::RLIM_INFINITY as _,
    };
    let rlim_max = match lim.maximum {
        Some(r) => r.try_into().map_err(|_| io::Error::INVAL)?,
        None => linux_raw_sys::general::RLIM_INFINITY as _,
    };
    Ok(linux_raw_sys::general::rlimit { rlim_cur, rlim_max })
}

#[inline]
pub(crate) fn wait(waitopts: WaitOptions) -> io::Result<Option<(Pid, WaitStatus)>> {
    _waitpid(!0, waitopts)
}

#[inline]
pub(crate) fn waitpid(
    pid: Option<Pid>,
    waitopts: WaitOptions,
) -> io::Result<Option<(Pid, WaitStatus)>> {
    _waitpid(Pid::as_raw(pid), waitopts)
}

#[inline]
pub(crate) fn _waitpid(
    pid: RawPid,
    waitopts: WaitOptions,
) -> io::Result<Option<(Pid, WaitStatus)>> {
    unsafe {
        let mut status: u32 = 0;
        let pid = ret_c_uint(syscall4(
            nr(__NR_wait4),
            c_int(pid as _),
            by_mut(&mut status),
            c_int(waitopts.bits() as _),
            zero(),
        ))?;
        Ok(RawNonZeroPid::new(pid)
            .map(|non_zero| (Pid::from_raw_nonzero(non_zero), WaitStatus::new(status))))
    }
}

#[inline]
pub(crate) fn exit_group(code: c::c_int) -> ! {
    unsafe { syscall1_noreturn(nr(__NR_exit_group), c_int(code)) }
}

#[inline]
pub(crate) fn setsid() -> io::Result<Pid> {
    unsafe {
        let pid = ret_usize(syscall0_readonly(nr(__NR_setsid)))?;
        debug_assert_ne!(pid, 0);
        Ok(Pid::from_raw_nonzero(RawNonZeroPid::new_unchecked(
            pid as u32,
        )))
    }
}

#[inline]
pub(crate) fn kill_process(pid: Pid, sig: Signal) -> io::Result<()> {
    unsafe {
        ret(syscall2_readonly(
            nr(__NR_kill),
            regular_pid(pid),
            signal(sig),
        ))
    }
}

#[inline]
pub(crate) fn kill_process_group(pid: Pid, sig: Signal) -> io::Result<()> {
    unsafe {
        ret(syscall2_readonly(
            nr(__NR_kill),
            negative_pid(pid),
            signal(sig),
        ))
    }
}

#[inline]
pub(crate) fn kill_current_process_group(sig: Signal) -> io::Result<()> {
    unsafe { ret(syscall2_readonly(nr(__NR_kill), pass_usize(0), signal(sig))) }
}
