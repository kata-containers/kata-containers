#![allow(deprecated)]

use std::{fmt, io};
use std::cell::UnsafeCell;
use std::os::windows::prelude::*;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::time::Duration;

use lazycell::AtomicLazyCell;

use winapi::*;
use miow;
use miow::iocp::{CompletionPort, CompletionStatus};

use event_imp::{Event, Evented, Ready};
use poll::{self, Poll};
use sys::windows::buffer_pool::BufferPool;
use {Token, PollOpt};

/// Each Selector has a globally unique(ish) ID associated with it. This ID
/// gets tracked by `TcpStream`, `TcpListener`, etc... when they are first
/// registered with the `Selector`. If a type that is previously associated with
/// a `Selector` attempts to register itself with a different `Selector`, the
/// operation will return with an error. This matches windows behavior.
static NEXT_ID: AtomicUsize = ATOMIC_USIZE_INIT;

/// The guts of the Windows event loop, this is the struct which actually owns
/// a completion port.
///
/// Internally this is just an `Arc`, and this allows handing out references to
/// the internals to I/O handles registered on this selector. This is
/// required to schedule I/O operations independently of being inside the event
/// loop (e.g. when a call to `write` is seen we're not "in the event loop").
pub struct Selector {
    inner: Arc<SelectorInner>,
}

struct SelectorInner {
    /// Unique identifier of the `Selector`
    id: usize,

    /// The actual completion port that's used to manage all I/O
    port: CompletionPort,

    /// A pool of buffers usable by this selector.
    ///
    /// Primitives will take buffers from this pool to perform I/O operations,
    /// and once complete they'll be put back in.
    buffers: Mutex<BufferPool>,
}

impl Selector {
    pub fn new() -> io::Result<Selector> {
        // offset by 1 to avoid choosing 0 as the id of a selector
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed) + 1;

        CompletionPort::new(0).map(|cp| {
            Selector {
                inner: Arc::new(SelectorInner {
                    id: id,
                    port: cp,
                    buffers: Mutex::new(BufferPool::new(256)),
                }),
            }
        })
    }

    pub fn select(&self,
                  events: &mut Events,
                  awakener: Token,
                  timeout: Option<Duration>) -> io::Result<bool> {
        trace!("select; timeout={:?}", timeout);

        // Clear out the previous list of I/O events and get some more!
        events.clear();

        trace!("polling IOCP");
        let n = match self.inner.port.get_many(&mut events.statuses, timeout) {
            Ok(statuses) => statuses.len(),
            Err(ref e) if e.raw_os_error() == Some(WAIT_TIMEOUT as i32) => 0,
            Err(e) => return Err(e),
        };

        let mut ret = false;
        for status in events.statuses[..n].iter() {
            // This should only ever happen from the awakener, and we should
            // only ever have one awakener right now, so assert as such.
            if status.overlapped() as usize == 0 {
                assert_eq!(status.token(), usize::from(awakener));
                ret = true;
                continue;
            }

            let callback = unsafe {
                (*(status.overlapped() as *mut Overlapped)).callback
            };

            trace!("select; -> got overlapped");
            callback(status.entry());
        }

        trace!("returning");
        Ok(ret)
    }

    /// Gets a reference to the underlying `CompletionPort` structure.
    pub fn port(&self) -> &CompletionPort {
        &self.inner.port
    }

    /// Gets a new reference to this selector, although all underlying data
    /// structures will refer to the same completion port.
    pub fn clone_ref(&self) -> Selector {
        Selector { inner: self.inner.clone() }
    }

    /// Return the `Selector`'s identifier
    pub fn id(&self) -> usize {
        self.inner.id
    }
}

impl SelectorInner {
    fn identical(&self, other: &SelectorInner) -> bool {
        (self as *const SelectorInner) == (other as *const SelectorInner)
    }
}

// A registration is stored in each I/O object which keeps track of how it is
// associated with a `Selector` above.
//
// Once associated with a `Selector`, a registration can never be un-associated
// (due to IOCP requirements). This is actually implemented through the
// `poll::Registration` and `poll::SetReadiness` APIs to keep track of all the
// level/edge/filtering business.
/// A `Binding` is embedded in all I/O objects associated with a `Poll`
/// object.
///
/// Each registration keeps track of which selector the I/O object is
/// associated with, ensuring that implementations of `Evented` can be
/// conformant for the various methods on Windows.
///
/// If you're working with custom IOCP-enabled objects then you'll want to
/// ensure that one of these instances is stored in your object and used in the
/// implementation of `Evented`.
///
/// For more information about how to use this see the `windows` module
/// documentation in this crate.
pub struct Binding {
    selector: AtomicLazyCell<Arc<SelectorInner>>,
}

impl Binding {
    /// Creates a new blank binding ready to be inserted into an I/O
    /// object.
    ///
    /// Won't actually do anything until associated with a `Poll` loop.
    pub fn new() -> Binding {
        Binding { selector: AtomicLazyCell::new() }
    }

    /// Registers a new handle with the `Poll` specified, also assigning the
    /// `token` specified.
    ///
    /// This function is intended to be used as part of `Evented::register` for
    /// custom IOCP objects. It will add the specified handle to the internal
    /// IOCP object with the provided `token`. All future events generated by
    /// the handled provided will be received by the `Poll`'s internal IOCP
    /// object.
    ///
    /// # Unsafety
    ///
    /// This function is unsafe as the `Poll` instance has assumptions about
    /// what the `OVERLAPPED` pointer used for each I/O operation looks like.
    /// Specifically they must all be instances of the `Overlapped` type in
    /// this crate. More information about this can be found on the
    /// `windows` module in this crate.
    pub unsafe fn register_handle(&self,
                                  handle: &AsRawHandle,
                                  token: Token,
                                  poll: &Poll) -> io::Result<()> {
        let selector = poll::selector(poll);

        // Ignore errors, we'll see them on the next line.
        drop(self.selector.fill(selector.inner.clone()));
        self.check_same_selector(poll)?;

        selector.inner.port.add_handle(usize::from(token), handle)
    }

    /// Same as `register_handle` but for sockets.
    pub unsafe fn register_socket(&self,
                                  handle: &AsRawSocket,
                                  token: Token,
                                  poll: &Poll) -> io::Result<()> {
        let selector = poll::selector(poll);
        drop(self.selector.fill(selector.inner.clone()));
        self.check_same_selector(poll)?;
        selector.inner.port.add_socket(usize::from(token), handle)
    }

    /// Reregisters the handle provided from the `Poll` provided.
    ///
    /// This is intended to be used as part of `Evented::reregister` but note
    /// that this function does not currently reregister the provided handle
    /// with the `poll` specified. IOCP has a special binding for changing the
    /// token which has not yet been implemented. Instead this function should
    /// be used to assert that the call to `reregister` happened on the same
    /// `Poll` that was passed into to `register`.
    ///
    /// Eventually, though, the provided `handle` will be re-assigned to have
    /// the token `token` on the given `poll` assuming that it's been
    /// previously registered with it.
    ///
    /// # Unsafety
    ///
    /// This function is unsafe for similar reasons to `register`. That is,
    /// there may be pending I/O events and such which aren't handled correctly.
    pub unsafe fn reregister_handle(&self,
                                    _handle: &AsRawHandle,
                                    _token: Token,
                                    poll: &Poll) -> io::Result<()> {
        self.check_same_selector(poll)
    }

    /// Same as `reregister_handle`, but for sockets.
    pub unsafe fn reregister_socket(&self,
                                    _socket: &AsRawSocket,
                                    _token: Token,
                                    poll: &Poll) -> io::Result<()> {
        self.check_same_selector(poll)
    }

    /// Deregisters the handle provided from the `Poll` provided.
    ///
    /// This is intended to be used as part of `Evented::deregister` but note
    /// that this function does not currently deregister the provided handle
    /// from the `poll` specified. IOCP has a special binding for that which has
    /// not yet been implemented. Instead this function should be used to assert
    /// that the call to `deregister` happened on the same `Poll` that was
    /// passed into to `register`.
    ///
    /// # Unsafety
    ///
    /// This function is unsafe for similar reasons to `register`. That is,
    /// there may be pending I/O events and such which aren't handled correctly.
    pub unsafe fn deregister_handle(&self,
                                    _handle: &AsRawHandle,
                                    poll: &Poll) -> io::Result<()> {
        self.check_same_selector(poll)
    }

    /// Same as `deregister_handle`, but for sockets.
    pub unsafe fn deregister_socket(&self,
                                    _socket: &AsRawSocket,
                                    poll: &Poll) -> io::Result<()> {
        self.check_same_selector(poll)
    }

    fn check_same_selector(&self, poll: &Poll) -> io::Result<()> {
        let selector = poll::selector(poll);
        match self.selector.borrow() {
            Some(prev) if prev.identical(&selector.inner) => Ok(()),
            Some(_) |
            None => Err(other("socket already registered")),
        }
    }
}

impl fmt::Debug for Binding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Binding")
            .finish()
    }
}

/// Helper struct used for TCP and UDP which bundles a `binding` with a
/// `SetReadiness` handle.
pub struct ReadyBinding {
    binding: Binding,
    readiness: Option<poll::SetReadiness>,
}

impl ReadyBinding {
    /// Creates a new blank binding ready to be inserted into an I/O object.
    ///
    /// Won't actually do anything until associated with an `Selector` loop.
    pub fn new() -> ReadyBinding {
        ReadyBinding {
            binding: Binding::new(),
            readiness: None,
        }
    }

    /// Returns whether this binding has been associated with a selector
    /// yet.
    pub fn registered(&self) -> bool {
        self.readiness.is_some()
    }

    /// Acquires a buffer with at least `size` capacity.
    ///
    /// If associated with a selector, this will attempt to pull a buffer from
    /// that buffer pool. If not associated with a selector, this will allocate
    /// a fresh buffer.
    pub fn get_buffer(&self, size: usize) -> Vec<u8> {
        match self.binding.selector.borrow() {
            Some(i) => i.buffers.lock().unwrap().get(size),
            None => Vec::with_capacity(size),
        }
    }

    /// Returns a buffer to this binding.
    ///
    /// If associated with a selector, this will push the buffer back into the
    /// selector's pool of buffers. Otherwise this will just drop the buffer.
    pub fn put_buffer(&self, buf: Vec<u8>) {
        if let Some(i) = self.binding.selector.borrow() {
            i.buffers.lock().unwrap().put(buf);
        }
    }

    /// Sets the readiness of this I/O object to a particular `set`.
    ///
    /// This is later used to fill out and respond to requests to `poll`. Note
    /// that this is all implemented through the `SetReadiness` structure in the
    /// `poll` module.
    pub fn set_readiness(&self, set: Ready) {
        if let Some(ref i) = self.readiness {
            trace!("set readiness to {:?}", set);
            i.set_readiness(set).expect("event loop disappeared?");
        }
    }

    /// Queries what the current readiness of this I/O object is.
    ///
    /// This is what's being used to generate events returned by `poll`.
    pub fn readiness(&self) -> Ready {
        match self.readiness {
            Some(ref i) => i.readiness(),
            None => Ready::empty(),
        }
    }

    /// Implementation of the `Evented::register` function essentially.
    ///
    /// Returns an error if we're already registered with another event loop,
    /// and otherwise just reassociates ourselves with the event loop to
    /// possible change tokens.
    pub fn register_socket(&mut self,
                           socket: &AsRawSocket,
                           poll: &Poll,
                           token: Token,
                           events: Ready,
                           opts: PollOpt,
                           registration: &Mutex<Option<poll::Registration>>)
                           -> io::Result<()> {
        trace!("register {:?} {:?}", token, events);
        unsafe {
            self.binding.register_socket(socket, token, poll)?;
        }

        let (r, s) = poll::new_registration(poll, token, events, opts);
        self.readiness = Some(s);
        *registration.lock().unwrap() = Some(r);
        Ok(())
    }

    /// Implementation of `Evented::reregister` function.
    pub fn reregister_socket(&mut self,
                             socket: &AsRawSocket,
                             poll: &Poll,
                             token: Token,
                             events: Ready,
                             opts: PollOpt,
                             registration: &Mutex<Option<poll::Registration>>)
                             -> io::Result<()> {
        trace!("reregister {:?} {:?}", token, events);
        unsafe {
            self.binding.reregister_socket(socket, token, poll)?;
        }

        registration.lock().unwrap()
                    .as_mut().unwrap()
                    .reregister(poll, token, events, opts)
    }

    /// Implementation of the `Evented::deregister` function.
    ///
    /// Doesn't allow registration with another event loop, just shuts down
    /// readiness notifications and such.
    pub fn deregister(&mut self,
                      socket: &AsRawSocket,
                      poll: &Poll,
                      registration: &Mutex<Option<poll::Registration>>)
                      -> io::Result<()> {
        trace!("deregistering");
        unsafe {
            self.binding.deregister_socket(socket, poll)?;
        }

        registration.lock().unwrap()
                    .as_ref().unwrap()
                    .deregister(poll)
    }
}

fn other(s: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, s)
}

#[derive(Debug)]
pub struct Events {
    /// Raw I/O event completions are filled in here by the call to `get_many`
    /// on the completion port above. These are then processed to run callbacks
    /// which figure out what to do after the event is done.
    statuses: Box<[CompletionStatus]>,

    /// Literal events returned by `get` to the upwards `EventLoop`. This file
    /// doesn't really modify this (except for the awakener), instead almost all
    /// events are filled in by the `ReadinessQueue` from the `poll` module.
    events: Vec<Event>,
}

impl Events {
    pub fn with_capacity(cap: usize) -> Events {
        // Note that it's possible for the output `events` to grow beyond the
        // capacity as it can also include deferred events, but that's certainly
        // not the end of the world!
        Events {
            statuses: vec![CompletionStatus::zero(); cap].into_boxed_slice(),
            events: Vec::with_capacity(cap),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn capacity(&self) -> usize {
        self.events.capacity()
    }

    pub fn get(&self, idx: usize) -> Option<Event> {
        self.events.get(idx).map(|e| *e)
    }

    pub fn push_event(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn clear(&mut self) {
        self.events.truncate(0);
    }
}

macro_rules! overlapped2arc {
    ($e:expr, $t:ty, $($field:ident).+) => ({
        let offset = offset_of!($t, $($field).+);
        debug_assert!(offset < mem::size_of::<$t>());
        FromRawArc::from_raw(($e as usize - offset) as *mut $t)
    })
}

macro_rules! offset_of {
    ($t:ty, $($field:ident).+) => (
        &(*(0 as *const $t)).$($field).+ as *const _ as usize
    )
}

// See sys::windows module docs for why this exists.
//
// The gist of it is that `Selector` assumes that all `OVERLAPPED` pointers are
// actually inside one of these structures so it can use the `Callback` stored
// right after it.
//
// We use repr(C) here to ensure that we can assume the overlapped pointer is
// at the start of the structure so we can just do a cast.
/// A wrapper around an internal instance over `miow::Overlapped` which is in
/// turn a wrapper around the Windows type `OVERLAPPED`.
///
/// This type is required to be used for all IOCP operations on handles that are
/// registered with an event loop. The event loop will receive notifications
/// over `OVERLAPPED` pointers that have completed, and it will cast that
/// pointer to a pointer to this structure and invoke the associated callback.
#[repr(C)]
pub struct Overlapped {
    inner: UnsafeCell<miow::Overlapped>,
    callback: fn(&OVERLAPPED_ENTRY),
}

impl Overlapped {
    /// Creates a new `Overlapped` which will invoke the provided `cb` callback
    /// whenever it's triggered.
    ///
    /// The returned `Overlapped` must be used as the `OVERLAPPED` passed to all
    /// I/O operations that are registered with mio's event loop. When the I/O
    /// operation associated with an `OVERLAPPED` pointer completes the event
    /// loop will invoke the function pointer provided by `cb`.
    pub fn new(cb: fn(&OVERLAPPED_ENTRY)) -> Overlapped {
        Overlapped {
            inner: UnsafeCell::new(miow::Overlapped::zero()),
            callback: cb,
        }
    }

    /// Get the underlying `Overlapped` instance as a raw pointer.
    ///
    /// This can be useful when only a shared borrow is held and the overlapped
    /// pointer needs to be passed down to winapi.
    pub fn as_mut_ptr(&self) -> *mut OVERLAPPED {
        unsafe {
            (*self.inner.get()).raw()
        }
    }
}

impl fmt::Debug for Overlapped {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Overlapped")
            .finish()
    }
}

// Overlapped's APIs are marked as unsafe Overlapped's APIs are marked as
// unsafe as they must be used with caution to ensure thread safety. The
// structure itself is safe to send across threads.
unsafe impl Send for Overlapped {}
unsafe impl Sync for Overlapped {}
