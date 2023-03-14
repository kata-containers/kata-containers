// Copyright 2015 Nathan Sizemore <nathanrsizemore@gmail.com>
//
// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
// If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.

#[macro_use]
extern crate bitflags;
extern crate libc;

use std::fmt::{Formatter, Debug, Result};
use std::io::{self, Error};
use std::os::unix::io::RawFd;

#[repr(i32)]
#[allow(non_camel_case_types)]
pub enum ControlOptions {
    /// Indicates an addition to the interest list.
    EPOLL_CTL_ADD = libc::EPOLL_CTL_ADD,
    /// Indicates a modification of flags for an interest already in list.
    EPOLL_CTL_MOD = libc::EPOLL_CTL_MOD,
    /// Indicates a removal of an interest from the list.
    EPOLL_CTL_DEL = libc::EPOLL_CTL_DEL,
}

bitflags! {
    pub struct Events: u32 {
        /// Sets the Edge Triggered behavior for the associated file descriptor.
        ///
        /// The default behavior for epoll is Level Triggered.
        const EPOLLET      = libc::EPOLLET as u32;
        /// The associated file is available for read operations.
        const EPOLLIN      = libc::EPOLLIN as u32;
        /// Error condition happened on the associated file descriptor.
        ///
        /// `wait` will always wait for this event; is not necessary to set it in events.
        const EPOLLERR     = libc::EPOLLERR as u32;
        /// Hang up happened on the associated file descriptor.
        ///
        /// `wait` will always wait for this event; it is not necessary to set it in events.
        /// Note that when reading from a channel such as a pipe or a stream socket, this event
        /// merely indicates that the peer closed its end of the channel. Subsequent reads from
        /// the channel will return 0 (end of file) only after all outstanding data in the
        /// channel has been consumed.
        const EPOLLHUP     = libc::EPOLLHUP as u32;
        /// The associated file is available for write operations.
        const EPOLLOUT     = libc::EPOLLOUT as u32;
        /// There is urgent data available for read operations.
        const EPOLLPRI     = libc::EPOLLPRI as u32;
        /// Stream socket peer closed connection, or shut down writing half of connection.
        ///
        /// This flag is especially useful for writing simple code to detect peer shutdown when
        /// using Edge Triggered monitoring.
        const EPOLLRDHUP   = libc::EPOLLRDHUP as u32;
        /// If `EPOLLONESHOT` and `EPOLLET` are clear and the process has the `CAP_BLOCK_SUSPEND`
        /// capability, ensure that the system does not enter "suspend" or "hibernate" while this
        /// event is pending or being processed.
        ///
        /// The event is considered as being "processed" from the time when it is returned by
        /// a call to `wait` until the next call to `wait` on the same `EpollInstance`
        /// descriptor, the closure of that file descriptor, the removal of the event file
        /// descriptor with `EPOLL_CTL_DEL`, or the clearing of `EPOLLWAKEUP` for the event file
        /// descriptor with `EPOLL_CTL_MOD`.
        const EPOLLWAKEUP  = libc::EPOLLWAKEUP as u32;
        /// Sets the one-shot behavior for the associated file descriptor.
        ///
        /// This means that after an event is pulled out with `wait` the associated file
        /// descriptor is internally disabled and no other events will be reported by the epoll
        /// interface.  The user must call `ctl` with `EPOLL_CTL_MOD` to rearm the file
        /// descriptor with a new event mask.
        const EPOLLONESHOT = libc::EPOLLONESHOT as u32;
        /// Sets an exclusive wakeup mode for the epoll file descriptor that is being attached to
        /// the target file descriptor, `fd`. When a wakeup event occurs and multiple epoll file
        /// descriptors are attached to the same target file using `EPOLLEXCLUSIVE`, one or more of
        /// the epoll file descriptors will receive an event with `wait`. The default in this
        /// scenario (when `EPOLLEXCLUSIVE` is not set) is for all epoll file descriptors to
        /// receive an event. `EPOLLEXCLUSIVE` is thus useful for avoiding thundering herd problems
        /// in certain scenarios.
        ///
        /// If the same file descriptor is in multiple epoll instances, some with the
        /// `EPOLLEXCLUSIVE` flag, and others without, then events will be provided to all epoll
        /// instances that did not specify `EPOLLEXCLUSIVE`, and at least one of the epoll
        /// instances that did specify `EPOLLEXCLUSIVE`.
        ///
        /// The following values may be specified in conjunction with `EPOLLEXCLUSIVE`: `EPOLLIN`,
        /// `EPOLLOUT`, `EPOLLWAKEUP`, and `EPOLLET`. `EPOLLHUP` and `EPOLLERR` can also be
        /// specified, but this is not required: as usual, these events are always reported if they
        /// occur, regardless of whether they are specified in `Events`. Attempts to specify other
        /// values in `Events` yield the error `EINVAL`.
        ///
        /// `EPOLLEXCLUSIVE` may be used only in an `EPOLL_CTL_ADD` operation; attempts to employ
        /// it with `EPOLL_CTL_MOD` yield an error. If `EPOLLEXCLUSIVE` has been set using `ctl`,
        /// then a subsequent `EPOLL_CTL_MOD` on the same `epfd`, `fd` pair yields an error. A call
        /// to `ctl` that specifies `EPOLLEXCLUSIVE` in `Events` and specifies the target file
        /// descriptor `fd` as an epoll instance will likewise fail. The error in all of these
        /// cases is `EINVAL`.
        ///
        /// The `EPOLLEXCLUSIVE` flag is an input flag for the `Event.events` field when calling
        /// `ctl`; it is never returned by `wait`.
        const EPOLLEXCLUSIVE = libc::EPOLLEXCLUSIVE as u32;
    }
}

/// 'libc::epoll_event' equivalent.
#[repr(C)]
#[cfg_attr(target_arch = "x86_64", repr(packed))]
#[derive(Clone, Copy)]
pub struct Event {
    pub events: u32,
    pub data: u64,
}

impl Event {
    pub fn new(events: Events, data: u64) -> Event {
        Event {
            events: events.bits(),
            data: data,
        }
    }
}

impl Debug for Event {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let data = self.data;
        f.debug_struct("Event")
            .field("events", unsafe {
                //Unsafe since we'd like to show which bits were wrongly set
                &Events::from_bits_unchecked(self.events)
            })
            .field("data", &data) 
            .finish()
    }
}

/// Creates a new epoll file descriptor.
///
/// If `cloexec` is true, `FD_CLOEXEC` will be set on the returned file descriptor.
///
/// ## Notes
///
/// * `epoll_create1()` is the underlying syscall.
pub fn create(cloexec: bool) -> io::Result<RawFd> {
    let flags = if cloexec { libc::EPOLL_CLOEXEC } else { 0 };
    unsafe { cvt(libc::epoll_create1(flags)) }
}

/// Safe wrapper for `libc::epoll_ctl`
pub fn ctl(
    epfd: RawFd,
    op: ControlOptions,
    fd: RawFd,
    mut event: Event,
) -> io::Result<()> {
    let e = &mut event as *mut _ as *mut libc::epoll_event;
    unsafe { cvt(libc::epoll_ctl(epfd, op as i32, fd, e))? };
    Ok(())
}

/// Safe wrapper for `libc::epoll_wait`
///
/// ## Notes
///
/// * If `timeout` is negative, it will block until an event is received.
pub fn wait(epfd: RawFd, timeout: i32, buf: &mut [Event]) -> io::Result<usize> {
    let timeout = if timeout < -1 { -1 } else { timeout };
    let num_events = unsafe {
        cvt(libc::epoll_wait(
            epfd,
            buf.as_mut_ptr() as *mut libc::epoll_event,
            buf.len() as i32,
            timeout,
        ))? as usize
    };
    Ok(num_events)
}

/// Safe wrapper for `libc::close`
pub fn close(epfd: RawFd) -> io::Result<()> {
    cvt(unsafe { libc::close(epfd) })?;
    Ok(())
}

fn cvt(result: libc::c_int) -> io::Result<libc::c_int> {
    if result < 0 {
        Err(Error::last_os_error())
    } else {
        Ok(result)
    }
}
