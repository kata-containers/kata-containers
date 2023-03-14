use super::super::c;
#[cfg(not(target_os = "wasi"))]
use crate::fd::BorrowedFd;
#[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
use bitflags::bitflags;

/// `struct timespec`
pub type Timespec = c::timespec;

/// A type for the `tv_sec` field of [`Timespec`].
#[allow(deprecated)]
pub type Secs = c::time_t;

/// A type for the `tv_nsec` field of [`Timespec`].
#[cfg(all(target_arch = "x86_64", target_pointer_width = "32"))]
pub type Nsecs = i64;

/// A type for the `tv_nsec` field of [`Timespec`].
#[cfg(not(all(target_arch = "x86_64", target_pointer_width = "32")))]
pub type Nsecs = c::c_long;

/// `CLOCK_*` constants for use with [`clock_gettime`].
///
/// These constants are always supported at runtime so `clock_gettime` never
/// has to fail with `INVAL` due to an unsupported clock. See
/// [`DynamicClockId`] for a greater set of clocks, with the caveat that not
/// all of them are always supported.
///
/// [`clock_gettime`]: crate::time::clock_gettime
#[cfg(not(any(target_os = "ios", target_os = "macos", target_os = "wasi")))]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(not(target_os = "dragonfly"), repr(i32))]
#[cfg_attr(target_os = "dragonfly", repr(u64))]
#[non_exhaustive]
pub enum ClockId {
    /// `CLOCK_REALTIME`
    Realtime = c::CLOCK_REALTIME,

    /// `CLOCK_MONOTONIC`
    Monotonic = c::CLOCK_MONOTONIC,

    /// `CLOCK_PROCESS_CPUTIME_ID`
    #[cfg(not(any(
        target_os = "illumos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "redox"
    )))]
    ProcessCPUTime = c::CLOCK_PROCESS_CPUTIME_ID,

    /// `CLOCK_THREAD_CPUTIME_ID`
    #[cfg(not(any(
        target_os = "illumos",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "redox"
    )))]
    ThreadCPUTime = c::CLOCK_THREAD_CPUTIME_ID,

    /// `CLOCK_REALTIME_COARSE`
    #[cfg(any(target_os = "android", target_os = "linux"))]
    RealtimeCoarse = c::CLOCK_REALTIME_COARSE,

    /// `CLOCK_MONOTONIC_COARSE`
    #[cfg(any(target_os = "android", target_os = "linux"))]
    MonotonicCoarse = c::CLOCK_MONOTONIC_COARSE,

    /// `CLOCK_MONOTONIC_RAW`
    #[cfg(any(target_os = "android", target_os = "linux"))]
    MonotonicRaw = c::CLOCK_MONOTONIC_RAW,
}

/// `CLOCK_*` constants for use with [`clock_gettime`].
///
/// These constants are always supported at runtime so `clock_gettime` never
/// has to fail with `INVAL` due to an unsupported clock. See
/// [`DynamicClockId`] for a greater set of clocks, with the caveat that not
/// all of them are always supported.
#[cfg(any(target_os = "ios", target_os = "macos"))]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u32)]
#[non_exhaustive]
pub enum ClockId {
    /// `CLOCK_REALTIME`
    Realtime = c::CLOCK_REALTIME,

    /// `CLOCK_MONOTONIC`
    Monotonic = c::CLOCK_MONOTONIC,

    /// `CLOCK_PROCESS_CPUTIME_ID`
    ProcessCPUTime = c::CLOCK_PROCESS_CPUTIME_ID,

    /// `CLOCK_THREAD_CPUTIME_ID`
    ThreadCPUTime = c::CLOCK_THREAD_CPUTIME_ID,
}

/// `CLOCK_*` constants for use with [`clock_gettime_dynamic`].
///
/// These constants may be unsupported at runtime, depending on the OS version,
/// and `clock_gettime_dynamic` may fail with `INVAL`. See [`ClockId`] for
/// clocks which are always supported at runtime.
///
/// [`clock_gettime_dynamic`]: crate::time::clock_gettime_dynamic
#[cfg(not(target_os = "wasi"))]
#[derive(Debug, Copy, Clone)]
#[non_exhaustive]
pub enum DynamicClockId<'a> {
    /// `ClockId` values that are always supported at runtime.
    Known(ClockId),

    /// Linux dynamic clocks.
    Dynamic(BorrowedFd<'a>),

    /// `CLOCK_REALTIME_ALARM`, available on Linux >= 3.0
    #[cfg(any(target_os = "android", target_os = "linux"))]
    RealtimeAlarm,

    /// `CLOCK_TAI`, available on Linux >= 3.10
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Tai,

    /// `CLOCK_BOOTTIME`, available on Linux >= 2.6.39
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Boottime,

    /// `CLOCK_BOOTTIME_ALARM`, available on Linux >= 2.6.39
    #[cfg(any(target_os = "android", target_os = "linux"))]
    BoottimeAlarm,
}

/// `struct itimerspec`
#[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
pub type Itimerspec = libc::itimerspec;

#[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
bitflags! {
    /// `TFD_*` flags for use with [`timerfd_create`].
    pub struct TimerfdFlags: libc::c_int {
        /// `TFD_NONBLOCK`
        const NONBLOCK = libc::TFD_NONBLOCK;

        /// `TFD_CLOEXEC`
        const CLOEXEC = libc::TFD_CLOEXEC;
    }
}

#[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
bitflags! {
    /// `TFD_TIMER_*` flags for use with [`timerfd_settime`].
    pub struct TimerfdTimerFlags: libc::c_int {
        /// `TFD_TIMER_ABSTIME`
        const ABSTIME = libc::TFD_TIMER_ABSTIME;

        /// `TFD_TIMER_CANCEL_ON_SET`
        #[cfg(any(target_os = "android", target_os = "linux"))]
        const CANCEL_ON_SET = 2; // TODO: upstream TFD_TIMER_CANCEL_ON_SET
    }
}

/// `CLOCK_*` constants for use with [`timerfd_create`].
///
/// [`timerfd_create`]: crate::time::timerfd_create
#[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(i32)]
#[non_exhaustive]
pub enum TimerfdClockId {
    /// `CLOCK_REALTIME`—A clock that tells the "real" time.
    ///
    /// This is a clock that tells the amount of time elapsed since the
    /// Unix epoch, 1970-01-01T00:00:00Z. The clock is externally settable, so
    /// it is not monotonic. Successive reads may see decreasing times, so it
    /// isn't reliable for measuring durations.
    Realtime = c::CLOCK_REALTIME,

    /// `CLOCK_MONOTONIC`—A clock that tells an abstract time.
    ///
    /// Unlike `Realtime`, this clock is not based on a fixed known epoch, so
    /// individual times aren't meaningful. However, since it isn't settable,
    /// it is reliable for measuring durations.
    ///
    /// This clock does not advance while the system is suspended; see
    /// `Boottime` for a clock that does.
    Monotonic = c::CLOCK_MONOTONIC,

    /// `CLOCK_BOOTTIME`—Like `Monotonic`, but advances while suspended.
    ///
    /// This clock is similar to `Monotonic`, but does advance while the system
    /// is suspended.
    Boottime = c::CLOCK_BOOTTIME,

    /// `CLOCK_REALTIME_ALARM`—Like `Realtime`, but wakes a suspended system.
    ///
    /// This clock is like `Realtime`, but can wake up a suspended system.
    ///
    /// Use of this clock requires the `CAP_WAKE_ALARM` Linux capability.
    RealtimeAlarm = c::CLOCK_REALTIME_ALARM,

    /// `CLOCK_BOOTTIME_ALARM`—Like `Boottime`, but wakes a suspended system.
    ///
    /// This clock is like `Boottime`, but can wake up a suspended system.
    ///
    /// Use of this clock requires the `CAP_WAKE_ALARM` Linux capability.
    BoottimeAlarm = c::CLOCK_BOOTTIME_ALARM,
}
