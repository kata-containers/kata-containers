use {io, poll, Evented, Ready, Poll, PollOpt, Token};
use libc;
use zircon;
use zircon::AsHandleRef;
use sys::fuchsia::{DontDrop, poll_opts_to_wait_async, sys};
use std::mem;
use std::os::unix::io::RawFd;
use std::sync::{Arc, Mutex};

/// Properties of an `EventedFd`'s current registration
#[derive(Debug)]
pub struct EventedFdRegistration {
    token: Token,
    handle: DontDrop<zircon::Handle>,
    rereg_signals: Option<(zircon::Signals, zircon::WaitAsyncOpts)>,
}

impl EventedFdRegistration {
    unsafe fn new(token: Token,
                  raw_handle: sys::zx_handle_t,
                  rereg_signals: Option<(zircon::Signals, zircon::WaitAsyncOpts)>,
                  ) -> Self
    {
        EventedFdRegistration {
            token: token,
            handle: DontDrop::new(zircon::Handle::from_raw(raw_handle)),
            rereg_signals: rereg_signals
        }
    }

    pub fn rereg_signals(&self) -> Option<(zircon::Signals, zircon::WaitAsyncOpts)> {
        self.rereg_signals
    }
}

/// An event-ed file descriptor. The file descriptor is owned by this structure.
#[derive(Debug)]
pub struct EventedFdInner {
    /// Properties of the current registration.
    registration: Mutex<Option<EventedFdRegistration>>,

    /// Owned file descriptor.
    ///
    /// `fd` is closed on `Drop`, so modifying `fd` is a memory-unsafe operation.
    fd: RawFd,

    /// Owned `fdio_t` pointer.
    fdio: *const sys::fdio_t,
}

impl EventedFdInner {
    pub fn rereg_for_level(&self, port: &zircon::Port) {
        let registration_opt = self.registration.lock().unwrap();
        if let Some(ref registration) = *registration_opt {
            if let Some((rereg_signals, rereg_opts)) = registration.rereg_signals {
                let _res =
                    registration
                        .handle.inner_ref()
                        .wait_async_handle(
                            port,
                            registration.token.0 as u64,
                            rereg_signals,
                            rereg_opts);
            }
        }
    }

    pub fn registration(&self) -> &Mutex<Option<EventedFdRegistration>> {
        &self.registration
    }

    pub fn fdio(&self) -> &sys::fdio_t {
        unsafe { &*self.fdio }
    }
}

impl Drop for EventedFdInner {
    fn drop(&mut self) {
        unsafe {
            sys::__fdio_release(self.fdio);
            let _ = libc::close(self.fd);
        }
    }
}

// `EventedInner` must be manually declared `Send + Sync` because it contains a `RawFd` and a
// `*const sys::fdio_t`. These are only used to make thread-safe system calls, so accessing
// them is entirely thread-safe.
//
// Note: one minor exception to this are the calls to `libc::close` and `__fdio_release`, which
// happen on `Drop`. These accesses are safe because `drop` can only be called at most once from
// a single thread, and after it is called no other functions can be called on the `EventedFdInner`.
unsafe impl Sync for EventedFdInner {}
unsafe impl Send for EventedFdInner {}

#[derive(Clone, Debug)]
pub struct EventedFd {
    pub inner: Arc<EventedFdInner>
}

impl EventedFd {
    pub unsafe fn new(fd: RawFd) -> Self {
        let fdio = sys::__fdio_fd_to_io(fd);
        assert!(fdio != ::std::ptr::null(), "FileDescriptor given to EventedFd must be valid.");

        EventedFd {
            inner: Arc::new(EventedFdInner {
                registration: Mutex::new(None),
                fd: fd,
                fdio: fdio,
            })
        }
    }

    fn handle_and_signals_for_events(&self, interest: Ready, opts: PollOpt)
                -> (sys::zx_handle_t, zircon::Signals)
    {
        let epoll_events = ioevent_to_epoll(interest, opts);

        unsafe {
            let mut raw_handle: sys::zx_handle_t = mem::uninitialized();
            let mut signals: sys::zx_signals_t = mem::uninitialized();
            sys::__fdio_wait_begin(self.inner.fdio, epoll_events, &mut raw_handle, &mut signals);

            (raw_handle, signals)
        }
    }

    fn register_with_lock(
        &self,
        registration: &mut Option<EventedFdRegistration>,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt) -> io::Result<()>
    {
        if registration.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Called register on an already registered file descriptor."));
        }

        let (raw_handle, signals) = self.handle_and_signals_for_events(interest, opts);

        let needs_rereg = opts.is_level() && !opts.is_oneshot();

        // If we need to reregister, then each registration should be `oneshot`
        let opts = opts | if needs_rereg { PollOpt::oneshot() } else { PollOpt::empty() };

        let rereg_signals = if needs_rereg {
            Some((signals, poll_opts_to_wait_async(opts)))
        } else {
            None
        };

        *registration = Some(
            unsafe { EventedFdRegistration::new(token, raw_handle, rereg_signals) }
        );

        // We don't have ownership of the handle, so we can't drop it
        let handle = DontDrop::new(unsafe { zircon::Handle::from_raw(raw_handle) });

        let registered = poll::selector(poll)
            .register_fd(handle.inner_ref(), self, token, signals, opts);

        if registered.is_err() {
            *registration = None;
        }

        registered
    }

    fn deregister_with_lock(
        &self,
        registration: &mut Option<EventedFdRegistration>,
        poll: &Poll) -> io::Result<()>
    {
        let old_registration = if let Some(old_reg) = registration.take() {
            old_reg
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "Called rereregister on an unregistered file descriptor."))
        };

        poll::selector(poll)
            .deregister_fd(old_registration.handle.inner_ref(), old_registration.token)
    }
}

impl Evented for EventedFd {
    fn register(&self,
                poll: &Poll,
                token: Token,
                interest: Ready,
                opts: PollOpt) -> io::Result<()>
    {
        self.register_with_lock(
            &mut *self.inner.registration.lock().unwrap(),
            poll,
            token,
            interest,
            opts)
    }

    fn reregister(&self,
                  poll: &Poll,
                  token: Token,
                  interest: Ready,
                  opts: PollOpt) -> io::Result<()>
    {
        // Take out the registration lock
        let mut registration_lock = self.inner.registration.lock().unwrap();

        // Deregister
        self.deregister_with_lock(&mut *registration_lock, poll)?;

        self.register_with_lock(
            &mut *registration_lock,
            poll,
            token,
            interest,
            opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        let mut registration_lock = self.inner.registration.lock().unwrap();
        self.deregister_with_lock(&mut *registration_lock, poll)
    }
}

fn ioevent_to_epoll(interest: Ready, opts: PollOpt) -> u32 {
    use event_imp::ready_from_usize;
    const HUP: usize   = 0b01000;

    let mut kind = 0;

    if interest.is_readable() {
        kind |= libc::EPOLLIN;
    }

    if interest.is_writable() {
        kind |= libc::EPOLLOUT;
    }

    if interest.contains(ready_from_usize(HUP)) {
        kind |= libc::EPOLLRDHUP;
    }

    if opts.is_edge() {
        kind |= libc::EPOLLET;
    }

    if opts.is_oneshot() {
        kind |= libc::EPOLLONESHOT;
    }

    if opts.is_level() {
        kind &= !libc::EPOLLET;
    }

    kind as u32
}
