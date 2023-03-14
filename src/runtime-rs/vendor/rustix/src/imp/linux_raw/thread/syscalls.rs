//! linux_raw syscalls supporting `rustix::thread`.
//!
//! # Safety
//!
//! See the `rustix::imp::syscalls` module documentation for details.

#![allow(unsafe_code)]

use super::super::arch::choose::{
    syscall0_readonly, syscall2, syscall4, syscall4_readonly, syscall6,
};
use super::super::c;
use super::super::conv::{
    by_ref, c_int, c_uint, clockid_t, const_void_star, out, ret, ret_usize, ret_usize_infallible,
    void_star, zero,
};
use super::super::reg::nr;
use crate::io;
use crate::process::{Pid, RawNonZeroPid};
use crate::thread::{FutexFlags, FutexOperation, NanosleepRelativeResult};
use crate::time::{ClockId, Timespec};
use core::mem::MaybeUninit;
use linux_raw_sys::general::{
    __NR_clock_nanosleep, __NR_futex, __NR_gettid, __NR_nanosleep, __kernel_pid_t,
    __kernel_timespec, TIMER_ABSTIME,
};
#[cfg(target_pointer_width = "32")]
use {
    core::convert::TryInto,
    core::ptr,
    linux_raw_sys::general::timespec as __kernel_old_timespec,
    linux_raw_sys::general::{__NR_clock_nanosleep_time64, __NR_futex_time64},
};

#[inline]
pub(crate) fn clock_nanosleep_relative(
    id: ClockId,
    req: &__kernel_timespec,
) -> NanosleepRelativeResult {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut rem = MaybeUninit::<__kernel_timespec>::uninit();
        match ret(syscall4(
            nr(__NR_clock_nanosleep_time64),
            clockid_t(id),
            c_int(0),
            by_ref(req),
            out(&mut rem),
        ))
        .or_else(|err| {
            // See the comments in `rustix_clock_gettime_via_syscall` about
            // emulation.
            if err == io::Error::NOSYS {
                clock_nanosleep_relative_old(id, req, &mut rem)
            } else {
                Err(err)
            }
        }) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Error::INTR) => NanosleepRelativeResult::Interrupted(rem.assume_init()),
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut rem = MaybeUninit::<__kernel_timespec>::uninit();
        match ret(syscall4(
            nr(__NR_clock_nanosleep),
            clockid_t(id),
            c_int(0),
            by_ref(req),
            out(&mut rem),
        )) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Error::INTR) => NanosleepRelativeResult::Interrupted(rem.assume_init()),
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn clock_nanosleep_relative_old(
    id: ClockId,
    req: &__kernel_timespec,
    rem: &mut MaybeUninit<__kernel_timespec>,
) -> io::Result<()> {
    let old_req = __kernel_old_timespec {
        tv_sec: req.tv_sec.try_into().map_err(|_| io::Error::INVAL)?,
        tv_nsec: req.tv_nsec.try_into().map_err(|_| io::Error::INVAL)?,
    };
    let mut old_rem = MaybeUninit::<__kernel_old_timespec>::uninit();
    ret(syscall4(
        nr(__NR_clock_nanosleep),
        clockid_t(id),
        c_int(0),
        by_ref(&old_req),
        out(&mut old_rem),
    ))?;
    let old_rem = old_rem.assume_init();
    // TODO: With Rust 1.55, we can use MaybeUninit::write here.
    ptr::write(
        rem.as_mut_ptr(),
        __kernel_timespec {
            tv_sec: old_rem.tv_sec.into(),
            tv_nsec: old_rem.tv_nsec.into(),
        },
    );
    Ok(())
}

#[inline]
pub(crate) fn clock_nanosleep_absolute(id: ClockId, req: &__kernel_timespec) -> io::Result<()> {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall4_readonly(
            nr(__NR_clock_nanosleep_time64),
            clockid_t(id),
            c_uint(TIMER_ABSTIME),
            by_ref(req),
            zero(),
        ))
        .or_else(|err| {
            // See the comments in `rustix_clock_gettime_via_syscall` about
            // emulation.
            if err == io::Error::NOSYS {
                clock_nanosleep_absolute_old(id, req)
            } else {
                Err(err)
            }
        })
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall4_readonly(
            nr(__NR_clock_nanosleep),
            clockid_t(id),
            c_uint(TIMER_ABSTIME),
            by_ref(req),
            zero(),
        ))
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn clock_nanosleep_absolute_old(id: ClockId, req: &__kernel_timespec) -> io::Result<()> {
    let old_req = __kernel_old_timespec {
        tv_sec: req.tv_sec.try_into().map_err(|_| io::Error::INVAL)?,
        tv_nsec: req.tv_nsec.try_into().map_err(|_| io::Error::INVAL)?,
    };
    ret(syscall4_readonly(
        nr(__NR_clock_nanosleep),
        clockid_t(id),
        c_int(0),
        by_ref(&old_req),
        zero(),
    ))
}

#[inline]
pub(crate) fn nanosleep(req: &__kernel_timespec) -> NanosleepRelativeResult {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut rem = MaybeUninit::<__kernel_timespec>::uninit();
        match ret(syscall4(
            nr(__NR_clock_nanosleep_time64),
            clockid_t(ClockId::Realtime),
            c_int(0),
            by_ref(req),
            out(&mut rem),
        ))
        .or_else(|err| {
            // See the comments in `rustix_clock_gettime_via_syscall` about
            // emulation.
            if err == io::Error::NOSYS {
                nanosleep_old(req, &mut rem)
            } else {
                Err(err)
            }
        }) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Error::INTR) => NanosleepRelativeResult::Interrupted(rem.assume_init()),
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut rem = MaybeUninit::<__kernel_timespec>::uninit();
        match ret(syscall2(nr(__NR_nanosleep), by_ref(req), out(&mut rem))) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Error::INTR) => NanosleepRelativeResult::Interrupted(rem.assume_init()),
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn nanosleep_old(
    req: &__kernel_timespec,
    rem: &mut MaybeUninit<__kernel_timespec>,
) -> io::Result<()> {
    let old_req = __kernel_old_timespec {
        tv_sec: req.tv_sec.try_into().map_err(|_| io::Error::INVAL)?,
        tv_nsec: req.tv_nsec.try_into().map_err(|_| io::Error::INVAL)?,
    };
    let mut old_rem = MaybeUninit::<__kernel_old_timespec>::uninit();
    ret(syscall2(
        nr(__NR_nanosleep),
        by_ref(&old_req),
        out(&mut old_rem),
    ))?;
    let old_rem = old_rem.assume_init();
    // TODO: With Rust 1.55, we can use MaybeUninit::write here.
    ptr::write(
        rem.as_mut_ptr(),
        __kernel_timespec {
            tv_sec: old_rem.tv_sec.into(),
            tv_nsec: old_rem.tv_nsec.into(),
        },
    );
    Ok(())
}

#[inline]
pub(crate) fn gettid() -> Pid {
    unsafe {
        let tid: i32 = ret_usize_infallible(syscall0_readonly(nr(__NR_gettid))) as __kernel_pid_t;
        debug_assert_ne!(tid, 0);
        Pid::from_raw_nonzero(RawNonZeroPid::new_unchecked(tid as u32))
    }
}

// TODO: This could be de-multiplexed.
#[inline]
pub(crate) unsafe fn futex(
    uaddr: *mut u32,
    op: FutexOperation,
    flags: FutexFlags,
    val: u32,
    utime: *const Timespec,
    uaddr2: *mut u32,
    val3: u32,
) -> io::Result<usize> {
    #[cfg(target_pointer_width = "32")]
    {
        ret_usize(syscall6(
            nr(__NR_futex_time64),
            void_star(uaddr.cast()),
            c_uint(op as c::c_uint | flags.bits()),
            c_uint(val),
            const_void_star(utime.cast()),
            void_star(uaddr2.cast()),
            c_uint(val3),
        ))
        .or_else(|err| {
            // See the comments in `rustix_clock_gettime_via_syscall` about
            // emulation.
            if err == io::Error::NOSYS {
                futex_old(uaddr, op, flags, val, utime, uaddr2, val3)
            } else {
                Err(err)
            }
        })
    }
    #[cfg(target_pointer_width = "64")]
    ret_usize(syscall6(
        nr(__NR_futex),
        void_star(uaddr.cast()),
        c_uint(op as c::c_uint | flags.bits()),
        c_uint(val),
        const_void_star(utime.cast()),
        void_star(uaddr2.cast()),
        c_uint(val3),
    ))
}

#[cfg(target_pointer_width = "32")]
unsafe fn futex_old(
    uaddr: *mut u32,
    op: FutexOperation,
    flags: FutexFlags,
    val: u32,
    utime: *const Timespec,
    uaddr2: *mut u32,
    val3: u32,
) -> io::Result<usize> {
    let old_utime = __kernel_old_timespec {
        tv_sec: (*utime).tv_sec.try_into().map_err(|_| io::Error::INVAL)?,
        tv_nsec: (*utime).tv_nsec.try_into().map_err(|_| io::Error::INVAL)?,
    };
    ret_usize(syscall6(
        nr(__NR_futex),
        void_star(uaddr.cast()),
        c_uint(op as c::c_uint | flags.bits()),
        c_uint(val),
        by_ref(&old_utime),
        void_star(uaddr2.cast()),
        c_uint(val3),
    ))
}
