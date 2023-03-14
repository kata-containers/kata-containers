//! libc syscalls supporting `rustix::thread`.

use super::super::c;
use super::super::conv::ret;
use crate::io;
#[cfg(any(target_os = "android", target_os = "linux"))]
use crate::process::{Pid, RawNonZeroPid};
#[cfg(not(target_os = "redox"))]
use crate::thread::NanosleepRelativeResult;
use crate::time::Timespec;
use core::mem::MaybeUninit;
#[cfg(not(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "emscripten",
    target_os = "ios",
    target_os = "macos",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "wasi",
)))]
use {crate::time::ClockId, core::ptr::null_mut};

#[cfg(not(any(
    target_os = "dragonfly",
    target_os = "emscripten",
    target_os = "freebsd", // FreeBSD 12 has clock_nanosleep, but libc targets FreeBSD 11.
    target_os = "ios",
    target_os = "macos",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "wasi",
)))]
#[inline]
pub(crate) fn clock_nanosleep_relative(id: ClockId, request: &Timespec) -> NanosleepRelativeResult {
    let mut remain = MaybeUninit::<Timespec>::uninit();
    let flags = 0;
    unsafe {
        match c::clock_nanosleep(id as c::clockid_t, flags, request, remain.as_mut_ptr()) {
            0 => NanosleepRelativeResult::Ok,
            err if err == io::Error::INTR.0 => {
                NanosleepRelativeResult::Interrupted(remain.assume_init())
            }
            err => NanosleepRelativeResult::Err(io::Error(err)),
        }
    }
}

#[cfg(not(any(
    target_os = "dragonfly",
    target_os = "freebsd", // FreeBSD 12 has clock_nanosleep, but libc targets FreeBSD 11.
    target_os = "emscripten",
    target_os = "ios",
    target_os = "macos",
    target_os = "openbsd",
    target_os = "redox",
    target_os = "wasi",
)))]
#[inline]
pub(crate) fn clock_nanosleep_absolute(id: ClockId, request: &Timespec) -> io::Result<()> {
    let flags = c::TIMER_ABSTIME;
    match unsafe { c::clock_nanosleep(id as c::clockid_t, flags, request, null_mut()) } {
        0 => Ok(()),
        err => Err(io::Error(err)),
    }
}

#[cfg(not(target_os = "redox"))]
#[inline]
pub(crate) fn nanosleep(request: &Timespec) -> NanosleepRelativeResult {
    let mut remain = MaybeUninit::<Timespec>::uninit();
    unsafe {
        match ret(c::nanosleep(request, remain.as_mut_ptr())) {
            Ok(()) => NanosleepRelativeResult::Ok,
            Err(io::Error::INTR) => NanosleepRelativeResult::Interrupted(remain.assume_init()),
            Err(err) => NanosleepRelativeResult::Err(err),
        }
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[inline]
#[must_use]
pub(crate) fn gettid() -> Pid {
    // `gettid` wasn't supported in glibc until 2.30, and musl until 1.2.2,
    // so use `syscall`.
    // <https://sourceware.org/bugzilla/show_bug.cgi?id=6399#c62>
    weak_or_syscall! {
        fn gettid() via SYS_gettid -> c::pid_t
    }

    unsafe {
        let tid = gettid();
        debug_assert_ne!(tid, 0);
        Pid::from_raw_nonzero(RawNonZeroPid::new_unchecked(tid))
    }
}
