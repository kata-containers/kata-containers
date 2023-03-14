//! A rust interface to the Linux kernel's timerfd API.
//!
//! # Example
//!
//! ```
//! use timerfd::{TimerFd, TimerState, SetTimeFlags};
//! use std::time::Duration;
//!
//! // Create a new timerfd
//! // (unwrap is actually fine here for most usecases)
//! let mut tfd = TimerFd::new().unwrap();
//!
//! // The timer is initially disarmed
//! assert_eq!(tfd.get_state(), TimerState::Disarmed);
//!
//! // Set the timer
//! tfd.set_state(TimerState::Oneshot(Duration::new(1, 0)), SetTimeFlags::Default);
//!
//! // Observe that the timer is now set
//! match tfd.get_state() {
//!     TimerState::Oneshot(d) => println!("Remaining: {:?}", d),
//!     _ => unreachable!(),
//! }
//!
//! // Wait for the remaining time
//! tfd.read();
//!
//! // It was a oneshot timer, so it's now disarmed
//! assert_eq!(tfd.get_state(), TimerState::Disarmed);
//! ```
//!
//! # Usage
//!
//! Unfortunately, this example can't show why you would use
//! timerfd in the first place: Because it creates a file descriptor
//! that you can monitor with `select(2)`, `poll(2)` and `epoll(2)`.
//!
//! In other words, the only advantage this offers over any other
//! timer implementation is that it implements the `AsRawFd` trait.
//!
//! The file descriptor becomes ready/readable whenever the timer expires.


extern crate rustix;

use std::os::unix::prelude::*;
use std::time::Duration;
use std::io::Result as IoResult;
use std::fmt;
use rustix::time::{Itimerspec, TimerfdClockId};

#[derive(Clone, PartialEq, Eq)]
pub enum ClockId {
    /// Available clocks:
    ///
    /// A settable system-wide real-time clock.
    Realtime       = TimerfdClockId::Realtime   as isize,

    /// This clock is like CLOCK_REALTIME, but will wake the system if it is suspended. The
    /// caller must have the CAP_WAKE_ALARM capability in order to set a timer against this
    /// clock.
    RealtimeAlarm  = TimerfdClockId::RealtimeAlarm as isize,

    /// A nonsettable monotonically increasing clock that measures time from some unspecified
    /// point in the past that does not change after system startup.
    Monotonic      = TimerfdClockId::Monotonic  as isize,

    /// Like CLOCK_MONOTONIC, this is a monotonically increasing clock. However, whereas the
    /// CLOCK_MONOTONIC clock does not measure the time while a system is suspended, the
    /// CLOCK_BOOTTIME clock does include the time during which the system is suspended. This
    /// is useful for applications that need to be suspend-aware. CLOCK_REALTIME is not
    /// suitable for such applications, since that clock is affected by discon‐ tinuous
    /// changes to the system clock.
    Boottime       = TimerfdClockId::Boottime   as isize,

    /// This clock is like CLOCK_BOOTTIME, but will wake the system if it is suspended. The
    /// caller must have the CAP_WAKE_ALARM capability in order to set a timer against this
    /// clock.
    BoottimeAlarm  = TimerfdClockId::BoottimeAlarm as isize,
}

fn clock_name (clock: &ClockId) -> &'static str {
    match *clock {
        ClockId::Realtime       => "CLOCK_REALTIME",
        ClockId::RealtimeAlarm  => "CLOCK_REALTIME_ALARM",
        ClockId::Monotonic      => "CLOCK_MONOTONIC",
        ClockId::Boottime       => "CLOCK_BOOTTIME",
        ClockId::BoottimeAlarm  => "CLOCK_BOOTTIME_ALARM",
    }
}

impl fmt::Display for ClockId {
    fn fmt (&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", clock_name(self))
    }
}

impl fmt::Debug for ClockId {
    fn fmt (&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({})", self.clone() as isize, clock_name(self))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetTimeFlags {
    /// Flags to `timerfd_settime(2)`.
    ///
    /// The default is zero, i. e. all bits unset.
    Default,

    /// Interpret new_value.it_value as an absolute value on the timer's clock. The timer will
    /// expire when the value of the timer's clock reaches the value specified in
    /// new_value.it_value.
    Abstime,

    /// If this flag is specified along with TFD_TIMER_ABSTIME and the clock for this timer is
    /// CLOCK_REALTIME or CLOCK_REALTIME_ALARM, then mark this timer as cancelable if the
    /// real-time clock undergoes a discontinuous change (settimeofday(2), clock_settime(2),
    /// or similar). When such changes occur, a current or future read(2) from the file
    /// descriptor will fail with the error ECANCELED.
    ///
    /// `TFD_TIMER_CANCEL_ON_SET` is useless without `TFD_TIMER_ABSTIME` set, cf. `fs/timerfd.c`.
    /// Thus `TimerCancelOnSet`` implies `Abstime`.
    TimerCancelOnSet,
}

use rustix::time::{TimerfdFlags, TimerfdTimerFlags};

mod structs;

/// Holds the state of a `TimerFd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerState {
    /// The timer is disarmed and will not fire.
    Disarmed,

    /// The timer will fire once after the specified duration
    /// and then disarm.
    Oneshot(Duration),

    /// The timer will fire once after `current` and then
    /// automatically rearm with `interval` as its duration.
    Periodic {
        current: Duration,
        interval: Duration,
    }
}

/// Represents a timerfd.
///
/// See also [`timerfd_create(2)`].
///
/// [`timerfd_create(2)`]: http://man7.org/linux/man-pages/man2/timerfd_create.2.html
pub struct TimerFd(rustix::io::OwnedFd);

impl TimerFd {
    /// Creates a new `TimerFd`.
    ///
    /// By default, it uses the monotonic clock, is blocking and does not close on exec.
    /// The parameters allow you to change that.
    ///
    /// # Errors
    ///
    /// On Linux 2.6.26 and earlier, nonblocking and cloexec are not supported and setting them
    /// will return an error of kind `ErrorKind::InvalidInput`.
    ///
    /// This can also fail in various cases of resource exhaustion. Please check
    /// `timerfd_create(2)` for details.
    pub fn new_custom(clock: ClockId, nonblocking: bool, cloexec: bool) -> IoResult<TimerFd> {
        let mut flags = TimerfdFlags::empty();
        if nonblocking {
            flags |= TimerfdFlags::NONBLOCK;
        }
        if cloexec {
            flags |= TimerfdFlags::CLOEXEC;
        }

        let clock = match clock {
            ClockId::Realtime => TimerfdClockId::Realtime,
            ClockId::RealtimeAlarm => TimerfdClockId::RealtimeAlarm,
            ClockId::Monotonic => TimerfdClockId::Monotonic,
            ClockId::Boottime => TimerfdClockId::Boottime,
            ClockId::BoottimeAlarm => TimerfdClockId::BoottimeAlarm,
        };
        let fd = rustix::time::timerfd_create(clock, flags)?;
        Ok(TimerFd(fd))
    }

    /// Creates a new `TimerFd` with default settings.
    ///
    /// Use `new_custom` to specify custom settings.
    pub fn new() -> IoResult<TimerFd> {
        TimerFd::new_custom(ClockId::Monotonic, false, false)
    }

    /// Sets this timerfd to a given `TimerState` and returns the old state.
    pub fn set_state(&mut self, state: TimerState, sflags: SetTimeFlags) -> TimerState {
        let flags = match sflags {
            SetTimeFlags::Default => TimerfdTimerFlags::empty(),
            SetTimeFlags::Abstime => TimerfdTimerFlags::ABSTIME,
            SetTimeFlags::TimerCancelOnSet => {
                TimerfdTimerFlags::ABSTIME | TimerfdTimerFlags::CANCEL_ON_SET
            }
        };
        let new: Itimerspec = state.into();
        let old = rustix::time::timerfd_settime(&self.0, flags, &new)
            .expect("Looks like timerfd_settime failed in some undocumented way");
        old.into()
    }

    /// Returns the current `TimerState`.
    pub fn get_state(&self) -> TimerState {
        let state = rustix::time::timerfd_gettime(&self.0)
            .expect("Looks like timerfd_gettime failed in some undocumented way");
        state.into()
    }

    /// Read from this timerfd.
    ///
    /// Returns the number of timer expirations since the last read.
    /// If this timerfd is operating in blocking mode (the default), it will
    /// not return zero but instead block until the timer has expired at least once.
    pub fn read(&self) -> u64 {
        let mut buffer = [0_u8; 8];
        loop {
            match rustix::io::read(&self.0, &mut buffer) {
                Ok(8) => {
                    let value = u64::from_ne_bytes(buffer);
                    assert_ne!(value, 0);
                    return value;
                }
                Err(rustix::io::Error::WOULDBLOCK) => return 0,
                Err(rustix::io::Error::INTR) => (),
                Err(e) => panic!("Unexpected read error: {}", e),
                _ => unreachable!(),
            }
        }
    }
}

impl AsRawFd for TimerFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl FromRawFd for TimerFd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        TimerFd(FromRawFd::from_raw_fd(fd))
    }
}

#[cfg(test)]
mod tests {
    extern crate rustix;
    use super::{ClockId, Duration, SetTimeFlags, TimerFd, TimerState};

    #[test]
    fn clockid_new_custom () {

        fn __test_clockid (clockid: ClockId) {
            let tfd = TimerFd::new_custom(clockid, true, false).unwrap();
            assert_eq!(tfd.get_state(), TimerState::Disarmed);
        }

        __test_clockid(ClockId::Realtime);
        __test_clockid(ClockId::Monotonic);
        __test_clockid(ClockId::Boottime);
        //__test_clockid(ClockId::RealtimeAlarm); // requires CAP_WAKE_ALARM
        //__test_clockid(ClockId::BoottimeAlarm); // requires CAP_WAKE_ALARM
    }

    const TEST_TIMER_OFFSET: u64 = 100; // seconds from now

    /// trivial monotonic timer some seconds into the future
    #[test]
    fn timerfd_settime_flags_default () {
        let mut tfd = TimerFd::new().unwrap();
        assert_eq!(tfd.get_state(), TimerState::Disarmed);

        tfd.set_state(TimerState::Oneshot(Duration::new(TEST_TIMER_OFFSET, 0)),
                      SetTimeFlags::Default);
        assert!(match tfd.get_state() { TimerState::Oneshot(_) => true, _ => false });
    }


    /// timer set from realtime clock
    #[test]
    fn timerfd_settime_flags_abstime () {
        let mut tfd = TimerFd::new_custom(ClockId::Realtime, true, true).unwrap();
        assert_eq!(tfd.get_state(), TimerState::Disarmed);

        let now = rustix::time::clock_gettime(rustix::time::ClockId::Realtime);
        tfd.set_state(TimerState::Oneshot(Duration::new(now.tv_sec as u64 + TEST_TIMER_OFFSET, 0)),
                      SetTimeFlags::Abstime);
        assert!(match tfd.get_state() { TimerState::Oneshot(_) => true, _ => false });
    }

    /// same as abstime, with `TimerCancelOnSet`
    #[test]
    fn timerfd_settime_flags_abstime_cancel () {
        let mut tfd = TimerFd::new_custom(ClockId::Realtime, true, true).unwrap();
        assert_eq!(tfd.get_state(), TimerState::Disarmed);

        let now = rustix::time::clock_gettime(rustix::time::ClockId::Realtime);
        tfd.set_state(TimerState::Oneshot(Duration::new(now.tv_sec as u64 + TEST_TIMER_OFFSET, 0)),
                      SetTimeFlags::TimerCancelOnSet);
        assert!(match tfd.get_state() { TimerState::Oneshot(_) => true, _ => false });
    }
}

