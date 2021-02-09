// Copyright 2016 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Type-safe bindings for Zircon timer objects.

use {AsHandleRef, ClockId, HandleBased, Handle, HandleRef, Status};
use {sys, ok};
use std::ops;
use std::time as stdtime;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Duration(sys::zx_duration_t);

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Time(sys::zx_time_t);

impl From<stdtime::Duration> for Duration {
    fn from(dur: stdtime::Duration) -> Self {
        Duration::from_seconds(dur.as_secs()) +
        Duration::from_nanos(dur.subsec_nanos() as u64)
    }
}

impl From<Duration> for stdtime::Duration {
    fn from(dur: Duration) -> Self {
        let secs = dur.seconds();
        let nanos = (dur.nanos() - (secs * 1_000_000_000)) as u32;
        stdtime::Duration::new(secs, nanos)
    }
}

impl ops::Add<Duration> for Time {
    type Output = Time;
    fn add(self, dur: Duration) -> Time {
        Time::from_nanos(dur.nanos() + self.nanos())
    }
}

impl ops::Add<Time> for Duration {
    type Output = Time;
    fn add(self, time: Time) -> Time {
        Time::from_nanos(self.nanos() + time.nanos())
    }
}

impl ops::Add for Duration {
    type Output = Duration;
    fn add(self, dur: Duration) -> Duration {
        Duration::from_nanos(self.nanos() + dur.nanos())
    }
}

impl ops::Sub for Duration {
    type Output = Duration;
    fn sub(self, dur: Duration) -> Duration {
        Duration::from_nanos(self.nanos() - dur.nanos())
    }
}

impl ops::Sub<Duration> for Time {
    type Output = Time;
    fn sub(self, dur: Duration) -> Time {
        Time::from_nanos(self.nanos() - dur.nanos())
    }
}

impl ops::AddAssign for Duration {
    fn add_assign(&mut self, dur: Duration) {
        self.0 += dur.nanos()
    }
}

impl ops::SubAssign for Duration {
    fn sub_assign(&mut self, dur: Duration) {
        self.0 -= dur.nanos()
    }
}

impl ops::AddAssign<Duration> for Time {
    fn add_assign(&mut self, dur: Duration) {
        self.0 += dur.nanos()
    }
}

impl ops::SubAssign<Duration> for Time {
    fn sub_assign(&mut self, dur: Duration) {
        self.0 -= dur.nanos()
    }
}

impl<T> ops::Mul<T> for Duration
    where T: Into<u64>
{
    type Output = Self;
    fn mul(self, mul: T) -> Self {
        Duration::from_nanos(self.0 * mul.into())
    }
}

impl<T> ops::Div<T> for Duration
    where T: Into<u64>
{
    type Output = Self;
    fn div(self, div: T) -> Self {
        Duration::from_nanos(self.0 / div.into())
    }
}

impl Duration {
    /// Sleep for the given amount of time.
    pub fn sleep(self) {
        Time::after(self).sleep()
    }

    pub fn nanos(self) -> u64 {
        self.0
    }

    pub fn millis(self) -> u64 {
        self.0 / 1_000_000
    }

    pub fn seconds(self) -> u64 {
        self.millis() / 1_000
    }

    pub fn minutes(self) -> u64 {
        self.seconds() / 60
    }

    pub fn hours(self) -> u64 {
        self.minutes() / 60
    }

    pub fn from_nanos(nanos: u64) -> Self {
        Duration(nanos)
    }

    pub fn from_millis(millis: u64) -> Self {
        Duration(millis * 1_000_000)
    }

    pub fn from_seconds(secs: u64) -> Self {
        Duration::from_millis(secs * 1_000)
    }

    pub fn from_minutes(min: u64) -> Self {
        Duration::from_seconds(min * 60)
    }

    pub fn from_hours(hours: u64) -> Self {
        Duration::from_minutes(hours * 60)
    }

    /// Returns a `Time` which is a `Duration` after the current time.
    /// `duration.after_now()` is equivalent to `Time::after(duration)`.
    pub fn after_now(self) -> Time {
        Time::after(self)
    }
}

pub trait DurationNum: Sized {
    fn nanos(self) -> Duration;
    fn millis(self) -> Duration;
    fn seconds(self) -> Duration;
    fn minutes(self) -> Duration;
    fn hours(self) -> Duration;

    // Singular versions to allow for `1.milli()` and `1.second()`, etc.
    fn milli(self) -> Duration { self.millis() }
    fn second(self) -> Duration { self.seconds() }
    fn minute(self) -> Duration { self.minutes() }
    fn hour(self) -> Duration { self.hours() }
}

// Note: this could be implemented for other unsized integer types, but it doesn't seem
// necessary to support the usual case.
impl DurationNum for u64 {
    fn nanos(self) -> Duration {
        Duration::from_nanos(self)
    }

    fn millis(self) -> Duration {
        Duration::from_millis(self)
    }

    fn seconds(self) -> Duration {
        Duration::from_seconds(self)
    }

    fn minutes(self) -> Duration {
        Duration::from_minutes(self)
    }

    fn hours(self) -> Duration {
        Duration::from_hours(self)
    }
}

impl Time {
    pub const INFINITE: Time = Time(sys::ZX_TIME_INFINITE);

    /// Get the current time, from the specific clock id.
    ///
    /// Wraps the
    /// [zx_time_get](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/time_get.md)
    /// syscall.
    pub fn get(clock_id: ClockId) -> Time {
        unsafe { Time(sys::zx_time_get(clock_id as u32)) }
    }

    /// Compute a deadline for the time in the future that is the given `Duration` away.
    ///
    /// Wraps the
    /// [zx_deadline_after](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/deadline_after.md)
    /// syscall.
    pub fn after(duration: Duration) -> Time {
        unsafe { Time(sys::zx_deadline_after(duration.0)) }
    }

    /// Sleep until the given time.
    ///
    /// Wraps the
    /// [zx_nanosleep](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/nanosleep.md)
    /// syscall.
    pub fn sleep(self) {
        unsafe { sys::zx_nanosleep(self.0); }
    }

    pub fn nanos(self) -> u64 {
        self.0
    }

    pub fn from_nanos(nanos: u64) -> Self {
        Time(nanos)
    }
}

/// Read the number of high-precision timer ticks since boot. These ticks may be processor cycles,
/// high speed timer, profiling timer, etc. They are not guaranteed to continue advancing when the
/// system is asleep.
///
/// Wraps the
/// [zx_ticks_get](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/ticks_get.md)
/// syscall.
pub fn ticks_get() -> u64 {
    unsafe { sys::zx_ticks_get() }
}

/// Return the number of high-precision timer ticks in a second.
///
/// Wraps the
/// [zx_ticks_per_second](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/ticks_per_second.md)
/// syscall.
pub fn ticks_per_second() -> u64 {
    unsafe { sys::zx_ticks_per_second() }
}

/// An object representing a Zircon timer, such as the one returned by
/// [zx_timer_create](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/timer_create.md).
///
/// As essentially a subtype of `Handle`, it can be freely interconverted.
#[derive(Debug, Eq, PartialEq)]
pub struct Timer(Handle);
impl_handle_based!(Timer);

impl Timer {
    /// Create a timer, an object that can signal when a specified point in time has been reached.
    /// Wraps the
    /// [zx_timer_create](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/timer_create.md)
    /// syscall.
    pub fn create(clock_id: ClockId) -> Result<Timer, Status> {
        let mut out = 0;
        let opts = 0;
        let status = unsafe { sys::zx_timer_create(opts, clock_id as u32, &mut out) };
        ok(status)?;
        unsafe {
            Ok(Self::from(Handle::from_raw(out)))
        }
    }

    /// Start a one-shot timer that will fire when `deadline` passes. Wraps the
    /// [zx_timer_set](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/timer_set.md)
    /// syscall.
    pub fn set(&self, deadline: Time, slack: Duration) -> Result<(), Status> {
        let status = unsafe {
            sys::zx_timer_set(self.raw_handle(), deadline.nanos(), slack.nanos())
        };
        ok(status)
    }

    /// Cancels a pending timer that was started with start(). Wraps the
    /// [zx_timer_cancel](https://fuchsia.googlesource.com/zircon/+/master/docs/syscalls/timer_cancel.md)
    /// syscall.
    pub fn cancel(&self) -> Result<(), Status> {
        let status = unsafe { sys::zx_timer_cancel(self.raw_handle()) };
        ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Signals;

    #[test]
    fn create_timer_invalid_clock() {
        assert_eq!(Timer::create(ClockId::UTC).unwrap_err(), Status::INVALID_ARGS);
        assert_eq!(Timer::create(ClockId::Thread), Err(Status::INVALID_ARGS));
    }

    #[test]
    fn into_from_std() {
        let std_dur = stdtime::Duration::new(25, 25);
        assert_eq!(std_dur, stdtime::Duration::from(Duration::from(std_dur)));
    }

    #[test]
    fn timer_basic() {
        let slack = 0.millis();
        let ten_ms = 10.millis();
        let five_secs = 5.seconds();
        let six_secs = 6.seconds();

        // Create a timer
        let timer = Timer::create(ClockId::Monotonic).unwrap();

        // Should not signal yet.
        assert_eq!(
            timer.wait_handle(Signals::TIMER_SIGNALED, ten_ms.after_now()),
            Err(Status::TIMED_OUT));

        // Set it, and soon it should signal.
        assert_eq!(timer.set(five_secs.after_now(), slack), Ok(()));
        assert_eq!(
            timer.wait_handle(Signals::TIMER_SIGNALED, six_secs.after_now()),
            Ok(Signals::TIMER_SIGNALED));

        // Cancel it, and it should stop signalling.
        assert_eq!(timer.cancel(), Ok(()));
        assert_eq!(
            timer.wait_handle(Signals::TIMER_SIGNALED, ten_ms.after_now()),
            Err(Status::TIMED_OUT));
    }
}
