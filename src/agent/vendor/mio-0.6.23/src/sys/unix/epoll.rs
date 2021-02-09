#![allow(deprecated)]
use std::os::unix::io::AsRawFd;
use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::time::Duration;
use std::{cmp, i32};

use libc::{self, c_int};
use libc::{EPOLLERR, EPOLLHUP, EPOLLONESHOT};
use libc::{EPOLLET, EPOLLOUT, EPOLLIN, EPOLLPRI};

use {io, Ready, PollOpt, Token};
use event_imp::Event;
use sys::unix::{cvt, UnixReady};
use sys::unix::io::set_cloexec;

/// Each Selector has a globally unique(ish) ID associated with it. This ID
/// gets tracked by `TcpStream`, `TcpListener`, etc... when they are first
/// registered with the `Selector`. If a type that is previously associated with
/// a `Selector` attempts to register itself with a different `Selector`, the
/// operation will return with an error. This matches windows behavior.
static NEXT_ID: AtomicUsize = ATOMIC_USIZE_INIT;

#[derive(Debug)]
pub struct Selector {
    id: usize,
    epfd: RawFd,
}

impl Selector {
    pub fn new() -> io::Result<Selector> {
        let epfd = unsafe {
            // Emulate `epoll_create` by using `epoll_create1` if it's available
            // and otherwise falling back to `epoll_create` followed by a call to
            // set the CLOEXEC flag.
            dlsym!(fn epoll_create1(c_int) -> c_int);

            match epoll_create1.get() {
                Some(epoll_create1_fn) => {
                    cvt(epoll_create1_fn(libc::EPOLL_CLOEXEC))?
                }
                None => {
                    let fd = cvt(libc::epoll_create(1024))?;
                    drop(set_cloexec(fd));
                    fd
                }
            }
        };

        // offset by 1 to avoid choosing 0 as the id of a selector
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed) + 1;

        Ok(Selector {
            id: id,
            epfd: epfd,
        })
    }

    pub fn id(&self) -> usize {
        self.id
    }

    /// Wait for events from the OS
    pub fn select(&self, evts: &mut Events, awakener: Token, timeout: Option<Duration>) -> io::Result<bool> {
        // A bug in kernels < 2.6.37 makes timeouts larger than LONG_MAX / CONFIG_HZ
        // (approx. 30 minutes with CONFIG_HZ=1200) effectively infinite on 32 bits
        // architectures. The magic number is the same constant used by libuv.
        #[cfg(target_pointer_width = "32")]
        const MAX_SAFE_TIMEOUT: u64 = 1789569;
        #[cfg(not(target_pointer_width = "32"))]
        const MAX_SAFE_TIMEOUT: u64 = c_int::max_value() as u64;

        let timeout_ms = timeout
            .map(|to| cmp::min(millis(to), MAX_SAFE_TIMEOUT) as c_int)
            .unwrap_or(-1);

        // Wait for epoll events for at most timeout_ms milliseconds
        evts.clear();
        unsafe {
            let cnt = cvt(libc::epoll_wait(self.epfd,
                                           evts.events.as_mut_ptr(),
                                           evts.events.capacity() as i32,
                                           timeout_ms))?;
            let cnt = cnt as usize;
            evts.events.set_len(cnt);

            for i in 0..cnt {
                if evts.events[i].u64 as usize == awakener.into() {
                    evts.events.remove(i);
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Register event interests for the given IO handle with the OS
    pub fn register(&self, fd: RawFd, token: Token, interests: Ready, opts: PollOpt) -> io::Result<()> {
        let mut info = libc::epoll_event {
            events: ioevent_to_epoll(interests, opts),
            u64: usize::from(token) as u64
        };

        unsafe {
            cvt(libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_ADD, fd, &mut info))?;
            Ok(())
        }
    }

    /// Register event interests for the given IO handle with the OS
    pub fn reregister(&self, fd: RawFd, token: Token, interests: Ready, opts: PollOpt) -> io::Result<()> {
        let mut info = libc::epoll_event {
            events: ioevent_to_epoll(interests, opts),
            u64: usize::from(token) as u64
        };

        unsafe {
            cvt(libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_MOD, fd, &mut info))?;
            Ok(())
        }
    }

    /// Deregister event interests for the given IO handle with the OS
    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        // The &info argument should be ignored by the system,
        // but linux < 2.6.9 required it to be not null.
        // For compatibility, we provide a dummy EpollEvent.
        let mut info = libc::epoll_event {
            events: 0,
            u64: 0,
        };

        unsafe {
            cvt(libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_DEL, fd, &mut info))?;
            Ok(())
        }
    }
}

fn ioevent_to_epoll(interest: Ready, opts: PollOpt) -> u32 {
    let mut kind = 0;

    if interest.is_readable() {
        kind |= EPOLLIN;
    }

    if interest.is_writable() {
        kind |= EPOLLOUT;
    }

    if UnixReady::from(interest).is_priority() {
        kind |= EPOLLPRI;
    }

    if opts.is_edge() {
        kind |= EPOLLET;
    }

    if opts.is_oneshot() {
        kind |= EPOLLONESHOT;
    }

    if opts.is_level() {
        kind &= !EPOLLET;
    }

    kind as u32
}

impl AsRawFd for Selector {
    fn as_raw_fd(&self) -> RawFd {
        self.epfd
    }
}

impl Drop for Selector {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::close(self.epfd);
        }
    }
}

pub struct Events {
    events: Vec<libc::epoll_event>,
}

impl Events {
    pub fn with_capacity(u: usize) -> Events {
        Events {
            events: Vec::with_capacity(u)
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.events.capacity()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    #[inline]
    pub fn get(&self, idx: usize) -> Option<Event> {
        self.events.get(idx).map(|event| {
            let epoll = event.events as c_int;
            let mut kind = Ready::empty();

            if (epoll & EPOLLIN) != 0 {
                kind = kind | Ready::readable();
            }

            if (epoll & EPOLLPRI) != 0 {
                kind = kind | Ready::readable() | UnixReady::priority();
            }

            if (epoll & EPOLLOUT) != 0 {
                kind = kind | Ready::writable();
            }

            // EPOLLHUP - Usually means a socket error happened
            if (epoll & EPOLLERR) != 0 {
                kind = kind | UnixReady::error();
            }

            if (epoll & EPOLLHUP) != 0 {
                kind = kind | UnixReady::hup();
            }

            let token = self.events[idx].u64;

            Event::new(kind, Token(token as usize))
        })
    }

    pub fn push_event(&mut self, event: Event) {
        self.events.push(libc::epoll_event {
            events: ioevent_to_epoll(event.readiness(), PollOpt::empty()),
            u64: usize::from(event.token()) as u64
        });
    }

    pub fn clear(&mut self) {
        unsafe { self.events.set_len(0); }
    }
}

const NANOS_PER_MILLI: u32 = 1_000_000;
const MILLIS_PER_SEC: u64 = 1_000;

/// Convert a `Duration` to milliseconds, rounding up and saturating at
/// `u64::MAX`.
///
/// The saturating is fine because `u64::MAX` milliseconds are still many
/// million years.
pub fn millis(duration: Duration) -> u64 {
    // Round up.
    let millis = (duration.subsec_nanos() + NANOS_PER_MILLI - 1) / NANOS_PER_MILLI;
    duration.as_secs().saturating_mul(MILLIS_PER_SEC).saturating_add(millis as u64)
}
