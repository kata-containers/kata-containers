// Copyright 2019 Intel Corporation. All Rights Reserved.
//
// Copyright 2017 The Chromium OS Authors. All rights reserved.
//
// SPDX-License-Identifier: (Apache-2.0 AND BSD-3-Clause)

//! Structure and wrapper functions for working with
//! [`eventfd`](http://man7.org/linux/man-pages/man2/eventfd.2.html).

use std::fs::File;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::{io, mem, result};

use libc::{c_void, dup, eventfd, read, write};

// Reexport commonly used flags from libc.
pub use libc::{EFD_CLOEXEC, EFD_NONBLOCK, EFD_SEMAPHORE};

/// A safe wrapper around Linux
/// [`eventfd`](http://man7.org/linux/man-pages/man2/eventfd.2.html).
#[derive(Debug)]
pub struct EventFd {
    eventfd: File,
}

impl EventFd {
    /// Create a new EventFd with an initial value.
    ///
    /// # Arguments
    ///
    /// * `flag`: The initial value used for creating the `EventFd`.
    /// Refer to Linux [`eventfd`](http://man7.org/linux/man-pages/man2/eventfd.2.html).
    /// # Examples
    ///
    /// ```
    /// extern crate vmm_sys_util;
    /// use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};
    ///
    /// EventFd::new(EFD_NONBLOCK).unwrap();
    /// ```
    pub fn new(flag: i32) -> result::Result<EventFd, io::Error> {
        // This is safe because eventfd merely allocated an eventfd for
        // our process and we handle the error case.
        let ret = unsafe { eventfd(0, flag) };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            // This is safe because we checked ret for success and know
            // the kernel gave us an fd that we own.
            Ok(EventFd {
                eventfd: unsafe { File::from_raw_fd(ret) },
            })
        }
    }

    /// Add a value to the eventfd's counter.
    ///
    /// When the addition causes the counter overflow, this would either block
    /// until a [`read`](http://man7.org/linux/man-pages/man2/read.2.html) is
    /// performed on the file descriptor, or fail with the
    /// error EAGAIN if the file descriptor has been made nonblocking.
    ///
    /// # Arguments
    ///
    /// * `v`: the value to be added to the eventfd's counter.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate vmm_sys_util;
    /// use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};
    ///
    /// let evt = EventFd::new(EFD_NONBLOCK).unwrap();
    /// evt.write(55).unwrap();
    /// ```
    pub fn write(&self, v: u64) -> result::Result<(), io::Error> {
        // This is safe because we made this fd and the pointer we pass
        // can not overflow because we give the syscall's size parameter properly.
        let ret = unsafe {
            write(
                self.as_raw_fd(),
                &v as *const u64 as *const c_void,
                mem::size_of::<u64>(),
            )
        };
        if ret <= 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Read a value from the eventfd.
    ///
    /// If the counter is zero, this would either block
    /// until the counter becomes nonzero, or fail with the
    /// error EAGAIN if the file descriptor has been made nonblocking.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate vmm_sys_util;
    /// use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};
    ///
    /// let evt = EventFd::new(EFD_NONBLOCK).unwrap();
    /// evt.write(55).unwrap();
    /// assert_eq!(evt.read().unwrap(), 55);
    /// ```
    pub fn read(&self) -> result::Result<u64, io::Error> {
        let mut buf: u64 = 0;
        let ret = unsafe {
            // This is safe because we made this fd and the pointer we
            // pass can not overflow because we give the syscall's size parameter properly.
            read(
                self.as_raw_fd(),
                &mut buf as *mut u64 as *mut c_void,
                mem::size_of::<u64>(),
            )
        };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(buf)
        }
    }

    /// Clone this EventFd.
    ///
    /// This internally creates a new file descriptor and it will share the same
    /// underlying count within the kernel.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate vmm_sys_util;
    /// use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};
    ///
    /// let evt = EventFd::new(EFD_NONBLOCK).unwrap();
    /// let evt_clone = evt.try_clone().unwrap();
    /// evt.write(923).unwrap();
    /// assert_eq!(evt_clone.read().unwrap(), 923);
    /// ```
    pub fn try_clone(&self) -> result::Result<EventFd, io::Error> {
        // This is safe because we made this fd and properly check that it returns without error.
        let ret = unsafe { dup(self.as_raw_fd()) };
        if ret < 0 {
            Err(io::Error::last_os_error())
        } else {
            // This is safe because we checked ret for success and know the kernel gave us an fd that we
            // own.
            Ok(EventFd {
                eventfd: unsafe { File::from_raw_fd(ret) },
            })
        }
    }
}

impl AsRawFd for EventFd {
    fn as_raw_fd(&self) -> RawFd {
        self.eventfd.as_raw_fd()
    }
}

impl FromRawFd for EventFd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        EventFd {
            eventfd: File::from_raw_fd(fd),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        EventFd::new(EFD_NONBLOCK).unwrap();
        EventFd::new(0).unwrap();
    }

    #[test]
    fn test_read_write() {
        let evt = EventFd::new(EFD_NONBLOCK).unwrap();
        evt.write(55).unwrap();
        assert_eq!(evt.read().unwrap(), 55);
    }

    #[test]
    fn test_write_overflow() {
        let evt = EventFd::new(EFD_NONBLOCK).unwrap();
        evt.write(std::u64::MAX - 1).unwrap();
        let r = evt.write(1);
        match r {
            Err(ref inner) if inner.kind() == io::ErrorKind::WouldBlock => (),
            _ => panic!("Unexpected"),
        }
    }
    #[test]
    fn test_read_nothing() {
        let evt = EventFd::new(EFD_NONBLOCK).unwrap();
        let r = evt.read();
        match r {
            Err(ref inner) if inner.kind() == io::ErrorKind::WouldBlock => (),
            _ => panic!("Unexpected"),
        }
    }
    #[test]
    fn test_clone() {
        let evt = EventFd::new(EFD_NONBLOCK).unwrap();
        let evt_clone = evt.try_clone().unwrap();
        evt.write(923).unwrap();
        assert_eq!(evt_clone.read().unwrap(), 923);
    }
}
