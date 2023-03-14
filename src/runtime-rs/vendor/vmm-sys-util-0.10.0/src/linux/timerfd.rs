// Copyright 2019 Intel Corporation. All Rights Reserved.
//
// Copyright 2018 The Chromium OS Authors. All rights reserved.
//
// SPDX-License-Identifier: (Apache-2.0 AND BSD-3-Clause)

//! Structure and functions for working with
//! [`timerfd`](http://man7.org/linux/man-pages/man2/timerfd_create.2.html).

use std::fs::File;
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
use std::ptr;
use std::time::Duration;

use libc::{self, timerfd_create, timerfd_gettime, timerfd_settime, CLOCK_MONOTONIC, TFD_CLOEXEC};

use crate::errno::{errno_result, Result};

/// A safe wrapper around a Linux
/// [`timerfd`](http://man7.org/linux/man-pages/man2/timerfd_create.2.html).
pub struct TimerFd(File);

impl TimerFd {
    /// Create a new [`TimerFd`](struct.TimerFd.html).
    ///
    /// This creates a nonsettable monotonically increasing clock that does not
    /// change after system startup. The timer is initally disarmed and must be
    /// armed by calling [`reset`](fn.reset.html).
    pub fn new() -> Result<TimerFd> {
        // Safe because this doesn't modify any memory and we check the return value.
        let ret = unsafe { timerfd_create(CLOCK_MONOTONIC, TFD_CLOEXEC) };
        if ret < 0 {
            return errno_result();
        }

        // Safe because we uniquely own the file descriptor.
        Ok(TimerFd(unsafe { File::from_raw_fd(ret) }))
    }

    /// Arm the [`TimerFd`](struct.TimerFd.html).
    ///
    /// Set the timer to expire after `dur`.
    ///
    /// # Arguments
    ///
    /// * `dur`: Specify the initial expiration of the timer.
    /// * `interval`: Specify the period for repeated expirations, depending on the
    /// value passed. If `interval` is not `None`, it represents the period after
    /// the initial expiration. Otherwise the timer will expire just once. Cancels
    /// any existing duration and repeating interval.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate vmm_sys_util;
    /// # use std::time::Duration;
    /// use vmm_sys_util::timerfd::TimerFd;
    ///
    /// let mut timer = TimerFd::new().unwrap();
    /// let dur = Duration::from_millis(100);
    /// let interval = Duration::from_millis(100);
    ///
    /// timer.reset(dur, Some(interval)).unwrap();
    /// ```
    pub fn reset(&mut self, dur: Duration, interval: Option<Duration>) -> Result<()> {
        // Safe because we are zero-initializing a struct with only primitive member fields.
        let mut spec: libc::itimerspec = unsafe { mem::zeroed() };
        // https://github.com/rust-lang/libc/issues/1848
        #[cfg_attr(target_env = "musl", allow(deprecated))]
        {
            spec.it_value.tv_sec = dur.as_secs() as libc::time_t;
        }
        // nsec always fits in i32 because subsec_nanos is defined to be less than one billion.
        let nsec = dur.subsec_nanos() as i32;
        spec.it_value.tv_nsec = libc::c_long::from(nsec);

        if let Some(int) = interval {
            // https://github.com/rust-lang/libc/issues/1848
            #[cfg_attr(target_env = "musl", allow(deprecated))]
            {
                spec.it_interval.tv_sec = int.as_secs() as libc::time_t;
            }
            // nsec always fits in i32 because subsec_nanos is defined to be less than one billion.
            let nsec = int.subsec_nanos() as i32;
            spec.it_interval.tv_nsec = libc::c_long::from(nsec);
        }

        // Safe because this doesn't modify any memory and we check the return value.
        let ret = unsafe { timerfd_settime(self.as_raw_fd(), 0, &spec, ptr::null_mut()) };
        if ret < 0 {
            return errno_result();
        }

        Ok(())
    }

    /// Wait until the timer expires.
    ///
    /// The return value represents the number of times the timer has expired since
    /// the last time `wait` was called. If the timer has not yet expired once,
    /// this call will block until it does.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate vmm_sys_util;
    /// # use std::time::Duration;
    /// # use std::thread::sleep;
    /// use vmm_sys_util::timerfd::TimerFd;
    ///
    /// let mut timer = TimerFd::new().unwrap();
    /// let dur = Duration::from_millis(100);
    /// let interval = Duration::from_millis(100);
    /// timer.reset(dur, Some(interval)).unwrap();
    ///
    /// sleep(dur * 3);
    /// let count = timer.wait().unwrap();
    /// assert!(count >= 3);
    /// ```
    pub fn wait(&mut self) -> Result<u64> {
        let mut count = 0u64;

        // Safe because this will only modify |buf| and we check the return value.
        let ret = unsafe {
            libc::read(
                self.as_raw_fd(),
                &mut count as *mut _ as *mut libc::c_void,
                mem::size_of_val(&count),
            )
        };
        if ret < 0 {
            return errno_result();
        }

        // The bytes in the buffer are guaranteed to be in native byte-order so we don't need to
        // use from_le or from_be.
        Ok(count)
    }

    /// Tell if the timer is armed.
    ///
    /// Returns `Ok(true)` if the timer is currently armed, otherwise the errno set by
    /// [`timerfd_gettime`](http://man7.org/linux/man-pages/man2/timerfd_create.2.html).
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate vmm_sys_util;
    /// # use std::time::Duration;
    /// use vmm_sys_util::timerfd::TimerFd;
    ///
    /// let mut timer = TimerFd::new().unwrap();
    /// let dur = Duration::from_millis(100);
    ///
    /// timer.reset(dur, None).unwrap();
    /// assert!(timer.is_armed().unwrap());
    /// ```
    pub fn is_armed(&self) -> Result<bool> {
        // Safe because we are zero-initializing a struct with only primitive member fields.
        let mut spec: libc::itimerspec = unsafe { mem::zeroed() };

        // Safe because timerfd_gettime is trusted to only modify `spec`.
        let ret = unsafe { timerfd_gettime(self.as_raw_fd(), &mut spec) };
        if ret < 0 {
            return errno_result();
        }

        Ok(spec.it_value.tv_sec != 0 || spec.it_value.tv_nsec != 0)
    }

    /// Disarm the timer.
    ///
    /// Set zero to disarm the timer, referring to
    /// [`timerfd_settime`](http://man7.org/linux/man-pages/man2/timerfd_create.2.html).
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate vmm_sys_util;
    /// # use std::time::Duration;
    /// use vmm_sys_util::timerfd::TimerFd;
    ///
    /// let mut timer = TimerFd::new().unwrap();
    /// let dur = Duration::from_millis(100);
    ///
    /// timer.reset(dur, None).unwrap();
    /// timer.clear().unwrap();
    /// ```
    pub fn clear(&mut self) -> Result<()> {
        // Safe because we are zero-initializing a struct with only primitive member fields.
        let spec: libc::itimerspec = unsafe { mem::zeroed() };

        // Safe because this doesn't modify any memory and we check the return value.
        let ret = unsafe { timerfd_settime(self.as_raw_fd(), 0, &spec, ptr::null_mut()) };
        if ret < 0 {
            return errno_result();
        }

        Ok(())
    }
}

impl AsRawFd for TimerFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl FromRawFd for TimerFd {
    /// This function is unsafe as the primitives currently returned
    /// have the contract that they are the sole owner of the file
    /// descriptor they are wrapping. Usage of this function could
    /// accidentally allow violating this contract which can cause memory
    /// unsafety in code that relies on it being true.
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        TimerFd(File::from_raw_fd(fd))
    }
}

impl IntoRawFd for TimerFd {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    #[test]
    fn test_from_raw_fd() {
        let ret = unsafe { timerfd_create(CLOCK_MONOTONIC, TFD_CLOEXEC) };
        let tfd = unsafe { TimerFd::from_raw_fd(ret) };
        assert!(!tfd.is_armed().unwrap());
    }

    #[test]
    fn test_into_raw_fd() {
        let tfd = TimerFd::new().expect("failed to create timerfd");
        let fd = tfd.into_raw_fd();
        assert!(fd > 0);
    }
    #[test]
    fn test_one_shot() {
        let mut tfd = TimerFd::new().expect("failed to create timerfd");
        assert!(!tfd.is_armed().unwrap());

        let dur = Duration::from_millis(200);
        let now = Instant::now();
        tfd.reset(dur, None).expect("failed to arm timer");

        assert!(tfd.is_armed().unwrap());

        let count = tfd.wait().expect("unable to wait for timer");

        assert_eq!(count, 1);
        assert!(now.elapsed() >= dur);
        tfd.clear().expect("unable to clear the timer");
        assert!(!tfd.is_armed().unwrap());
    }

    #[test]
    fn test_repeating() {
        let mut tfd = TimerFd::new().expect("failed to create timerfd");

        let dur = Duration::from_millis(200);
        let interval = Duration::from_millis(100);
        tfd.reset(dur, Some(interval)).expect("failed to arm timer");

        sleep(dur * 3);

        let count = tfd.wait().expect("unable to wait for timer");
        assert!(count >= 5, "count = {}", count);
        tfd.clear().expect("unable to clear the timer");
        assert!(!tfd.is_armed().unwrap());
    }
}
