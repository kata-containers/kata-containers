//! linux_raw syscalls supporting `rustix::time`.
//!
//! # Safety
//!
//! See the `rustix::imp::syscalls` module documentation for details.

#![allow(unsafe_code)]

use super::super::arch::choose::{syscall2, syscall4};
use super::super::conv::{
    borrowed_fd, by_ref, c_uint, clockid_t, out, ret, ret_owned_fd, timerfd_clockid_t,
};
use super::super::reg::nr;
use crate::fd::BorrowedFd;
use crate::io::{self, OwnedFd};
use crate::time::{ClockId, Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags};
use core::mem::MaybeUninit;
use linux_raw_sys::general::{
    __NR_clock_getres, __NR_timerfd_create, __NR_timerfd_gettime, __NR_timerfd_settime,
    __kernel_timespec,
};
#[cfg(target_pointer_width = "32")]
use {
    core::convert::TryInto,
    core::ptr,
    linux_raw_sys::general::__NR_clock_getres_time64,
    linux_raw_sys::general::itimerspec as __kernel_old_itimerspec,
    linux_raw_sys::general::timespec as __kernel_old_timespec,
    linux_raw_sys::general::{__NR_timerfd_gettime64, __NR_timerfd_settime64},
};

// `clock_gettime` has special optimizations via the vDSO.
pub(crate) use super::super::vdso_wrappers::{clock_gettime, clock_gettime_dynamic};

#[inline]
pub(crate) fn clock_getres(which_clock: ClockId) -> __kernel_timespec {
    #[cfg(target_pointer_width = "32")]
    unsafe {
        let mut result = MaybeUninit::<__kernel_timespec>::uninit();
        let _ = ret(syscall2(
            nr(__NR_clock_getres_time64),
            clockid_t(which_clock),
            out(&mut result),
        ))
        .or_else(|err| {
            // See the comments in `rustix_clock_gettime_via_syscall` about
            // emulation.
            if err == io::Error::NOSYS {
                clock_getres_old(which_clock, &mut result)
            } else {
                Err(err)
            }
        });
        result.assume_init()
    }
    #[cfg(target_pointer_width = "64")]
    unsafe {
        let mut result = MaybeUninit::<__kernel_timespec>::uninit();
        let _ = syscall2(
            nr(__NR_clock_getres),
            clockid_t(which_clock),
            out(&mut result),
        );
        result.assume_init()
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn clock_getres_old(
    which_clock: ClockId,
    result: &mut MaybeUninit<__kernel_timespec>,
) -> io::Result<()> {
    let mut old_result = MaybeUninit::<__kernel_old_timespec>::uninit();
    ret(syscall2(
        nr(__NR_clock_getres),
        clockid_t(which_clock),
        out(&mut old_result),
    ))?;
    let old_result = old_result.assume_init();
    // TODO: With Rust 1.55, we can use MaybeUninit::write here.
    ptr::write(
        result.as_mut_ptr(),
        __kernel_timespec {
            tv_sec: old_result.tv_sec.into(),
            tv_nsec: old_result.tv_nsec.into(),
        },
    );
    Ok(())
}

#[inline]
pub(crate) fn timerfd_create(clockid: TimerfdClockId, flags: TimerfdFlags) -> io::Result<OwnedFd> {
    unsafe {
        ret_owned_fd(syscall2(
            nr(__NR_timerfd_create),
            timerfd_clockid_t(clockid),
            c_uint(flags.bits()),
        ))
    }
}

#[inline]
pub(crate) fn timerfd_settime(
    fd: BorrowedFd<'_>,
    flags: TimerfdTimerFlags,
    new_value: &Itimerspec,
) -> io::Result<Itimerspec> {
    let mut result = MaybeUninit::<Itimerspec>::uninit();

    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall4(
            nr(__NR_timerfd_settime),
            borrowed_fd(fd),
            c_uint(flags.bits()),
            by_ref(new_value),
            out(&mut result),
        ))
        .map(|()| result.assume_init())
    }

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall4(
            nr(__NR_timerfd_settime64),
            borrowed_fd(fd),
            c_uint(flags.bits()),
            by_ref(new_value),
            out(&mut result),
        ))
        .or_else(|err| {
            // See the comments in `rustix_clock_gettime_via_syscall` about
            // emulation.
            if err == io::Error::NOSYS {
                timerfd_settime_old(fd, flags, new_value, &mut result)
            } else {
                Err(err)
            }
        })
        .map(|()| result.assume_init())
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn timerfd_settime_old(
    fd: BorrowedFd<'_>,
    flags: TimerfdTimerFlags,
    new_value: &Itimerspec,
    result: &mut MaybeUninit<Itimerspec>,
) -> io::Result<()> {
    let mut old_result = MaybeUninit::<__kernel_old_itimerspec>::uninit();
    let old_new_value = __kernel_old_itimerspec {
        it_interval: __kernel_old_timespec {
            tv_sec: new_value
                .it_interval
                .tv_sec
                .try_into()
                .map_err(|_| io::Error::INVAL)?,
            tv_nsec: new_value
                .it_interval
                .tv_nsec
                .try_into()
                .map_err(|_| io::Error::INVAL)?,
        },
        it_value: __kernel_old_timespec {
            tv_sec: new_value
                .it_value
                .tv_sec
                .try_into()
                .map_err(|_| io::Error::INVAL)?,
            tv_nsec: new_value
                .it_value
                .tv_nsec
                .try_into()
                .map_err(|_| io::Error::INVAL)?,
        },
    };
    ret(syscall4(
        nr(__NR_timerfd_settime),
        borrowed_fd(fd),
        c_uint(flags.bits()),
        by_ref(&old_new_value),
        out(&mut old_result),
    ))?;
    let old_result = old_result.assume_init();
    // TODO: With Rust 1.55, we can use MaybeUninit::write here.
    ptr::write(
        result.as_mut_ptr(),
        Itimerspec {
            it_interval: __kernel_timespec {
                tv_sec: old_result.it_interval.tv_sec.into(),
                tv_nsec: old_result.it_interval.tv_nsec.into(),
            },
            it_value: __kernel_timespec {
                tv_sec: old_result.it_value.tv_sec.into(),
                tv_nsec: old_result.it_value.tv_nsec.into(),
            },
        },
    );
    Ok(())
}

#[inline]
pub(crate) fn timerfd_gettime(fd: BorrowedFd<'_>) -> io::Result<Itimerspec> {
    let mut result = MaybeUninit::<Itimerspec>::uninit();

    #[cfg(target_pointer_width = "64")]
    unsafe {
        ret(syscall2(
            nr(__NR_timerfd_gettime),
            borrowed_fd(fd),
            out(&mut result),
        ))
        .map(|()| result.assume_init())
    }

    #[cfg(target_pointer_width = "32")]
    unsafe {
        ret(syscall2(
            nr(__NR_timerfd_gettime64),
            borrowed_fd(fd),
            out(&mut result),
        ))
        .or_else(|err| {
            // See the comments in `rustix_clock_gettime_via_syscall` about
            // emulation.
            if err == io::Error::NOSYS {
                timerfd_gettime_old(fd, &mut result)
            } else {
                Err(err)
            }
        })
        .map(|()| result.assume_init())
    }
}

#[cfg(target_pointer_width = "32")]
unsafe fn timerfd_gettime_old(
    fd: BorrowedFd<'_>,
    result: &mut MaybeUninit<Itimerspec>,
) -> io::Result<()> {
    let mut old_result = MaybeUninit::<__kernel_old_itimerspec>::uninit();
    ret(syscall2(
        nr(__NR_timerfd_gettime),
        borrowed_fd(fd),
        out(&mut old_result),
    ))?;
    let old_result = old_result.assume_init();
    // TODO: With Rust 1.55, we can use MaybeUninit::write here.
    ptr::write(
        result.as_mut_ptr(),
        Itimerspec {
            it_interval: __kernel_timespec {
                tv_sec: old_result.it_interval.tv_sec.into(),
                tv_nsec: old_result.it_interval.tv_nsec.into(),
            },
            it_value: __kernel_timespec {
                tv_sec: old_result.it_value.tv_sec.into(),
                tv_nsec: old_result.it_value.tv_nsec.into(),
            },
        },
    );
    Ok(())
}
