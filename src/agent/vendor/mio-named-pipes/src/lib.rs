//! Windows named pipes bindings for mio.
//!
//! This crate implements bindings for named pipes for the mio crate. This
//! crate compiles on all platforms but only contains anything on Windows.
//! Currently this crate requires mio 0.6.2.
//!
//! On Windows, mio is implemented with an IOCP object at the heart of its
//! `Poll` implementation. For named pipes, this means that all I/O is done in
//! an overlapped fashion and the named pipes themselves are registered with
//! mio's internal IOCP object. Essentially, this crate is using IOCP for
//! bindings with named pipes.
//!
//! Note, though, that IOCP is a *completion* based model whereas mio expects a
//! *readiness* based model. As a result this crate, like with TCP objects in
//! mio, has internal buffering to translate the completion model to a readiness
//! model. This means that this crate is not a zero-cost binding over named
//! pipes on Windows, but rather approximates the performance of mio's TCP
//! implementation on Windows.
//!
//! # Trait implementations
//!
//! The `Read` and `Write` traits are implemented for `NamedPipe` and for
//! `&NamedPipe`. This represents that a named pipe can be concurrently read and
//! written to and also can be read and written to at all. Typically a named
//! pipe needs to be connected to a client before it can be read or written,
//! however.
//!
//! Note that for I/O operations on a named pipe to succeed then the named pipe
//! needs to be associated with an event loop. Until this happens all I/O
//! operations will return a "would block" error.
//!
//! # Managing connections
//!
//! The `NamedPipe` type supports a `connect` method to connect to a client and
//! a `disconnect` method to disconnect from that client. These two methods only
//! work once a named pipe is associated with an event loop.
//!
//! The `connect` method will succeed asynchronously and a completion can be
//! detected once the object receives a writable notification.
//!
//! # Named pipe clients
//!
//! Currently to create a client of a named pipe server then you can use the
//! `OpenOptions` type in the standard library to create a `File` that connects
//! to a named pipe. Afterwards you can use the `into_raw_handle` method coupled
//! with the `NamedPipe::from_raw_handle` method to convert that to a named pipe
//! that can operate asynchronously. Don't forget to pass the
//! `FILE_FLAG_OVERLAPPED` flag when opening the `File`.

#![cfg(windows)]
#![deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate mio;
extern crate miow;
extern crate winapi;

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::os::windows::io::*;
use std::slice;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Mutex;

use mio::windows;
use mio::{Evented, Poll, PollOpt, Ready, Registration, SetReadiness, Token};
use miow::iocp::CompletionStatus;
use miow::pipe;
use winapi::shared::winerror::*;
use winapi::um::ioapiset::*;
use winapi::um::minwinbase::*;

mod from_raw_arc;
use crate::from_raw_arc::FromRawArc;

macro_rules! offset_of {
    ($t:ty, $($field:ident).+) => (
        &(*(0 as *const $t)).$($field).+ as *const _ as usize
    )
}

macro_rules! overlapped2arc {
    ($e:expr, $t:ty, $($field:ident).+) => ({
        let offset = offset_of!($t, $($field).+);
        debug_assert!(offset < mem::size_of::<$t>());
        FromRawArc::from_raw(($e as usize - offset) as *mut $t)
    })
}

fn would_block() -> io::Error {
    io::ErrorKind::WouldBlock.into()
}

/// Representation of a named pipe on Windows.
///
/// This structure internally contains a `HANDLE` which represents the named
/// pipe, and also maintains state associated with the mio event loop and active
/// I/O operations that have been scheduled to translate IOCP to a readiness
/// model.
pub struct NamedPipe {
    registered: AtomicBool,
    ready_registration: Registration,
    poll_registration: windows::Binding,
    inner: FromRawArc<Inner>,
}

struct Inner {
    handle: pipe::NamedPipe,
    readiness: SetReadiness,

    connect: windows::Overlapped,
    connecting: AtomicBool,

    read: windows::Overlapped,
    write: windows::Overlapped,

    io: Mutex<Io>,

    pool: Mutex<BufferPool>,
}

struct Io {
    read: State,
    write: State,
    connect_error: Option<io::Error>,
}

enum State {
    None,
    Pending(Vec<u8>, usize),
    Ok(Vec<u8>, usize),
    Err(io::Error),
}

fn _assert_kinds() {
    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}
    _assert_send::<NamedPipe>();
    _assert_sync::<NamedPipe>();
}

impl NamedPipe {
    /// Creates a new named pipe at the specified `addr` given a "reasonable
    /// set" of initial configuration options.
    ///
    /// Currently the configuration options are the [same as miow]. To change
    /// these options, you can create a custom named pipe yourself and then use
    /// the `FromRawHandle` constructor to convert that type to an instance of a
    /// `NamedPipe` in this crate.
    ///
    /// [same as miow]: https://docs.rs/miow/0.1.4/x86_64-pc-windows-msvc/miow/pipe/struct.NamedPipe.html#method.new
    pub fn new<A: AsRef<OsStr>>(addr: A) -> io::Result<NamedPipe> {
        NamedPipe::_new(addr.as_ref())
    }

    fn _new(addr: &OsStr) -> io::Result<NamedPipe> {
        let pipe = pipe::NamedPipe::new(addr)?;
        unsafe { Ok(NamedPipe::from_raw_handle(pipe.into_raw_handle())) }
    }

    /// Attempts to call `ConnectNamedPipe`, if possible.
    ///
    /// This function will attempt to connect this pipe to a client in an
    /// asynchronous fashion. If the function immediately establishes a
    /// connection to a client then `Ok(())` is returned. Otherwise if a
    /// connection attempt was issued and is now in progress then a "would
    /// block" error is returned.
    ///
    /// When the connection is finished then this object will be flagged as
    /// being ready for a write, or otherwise in the writable state.
    ///
    /// # Errors
    ///
    /// This function will return a "would block" error if the pipe has not yet
    /// been registered with an event loop, if the connection operation has
    /// previously been issued but has not yet completed, or if the connect
    /// itself was issued and didn't finish immediately.
    ///
    /// Normal I/O errors from the call to `ConnectNamedPipe` are returned
    /// immediately.
    pub fn connect(&self) -> io::Result<()> {
        // Make sure we're associated with an IOCP object
        if !self.registered() {
            return Err(would_block());
        }

        // "Acquire the connecting lock" or otherwise just make sure we're the
        // only operation that's using the `connect` overlapped instance.
        if self.inner.connecting.swap(true, SeqCst) {
            return Err(would_block());
        }

        // Now that we've flagged ourselves in the connecting state, issue the
        // connection attempt. Afterwards interpret the return value and set
        // internal state accordingly.
        let res = unsafe {
            let overlapped = self.inner.connect.as_mut_ptr() as *mut _;
            self.inner.handle.connect_overlapped(overlapped)
        };

        match res {
            // The connection operation finished immediately, so let's schedule
            // reads/writes and such.
            Ok(true) => {
                trace!("connect done immediately");
                self.inner.connecting.store(false, SeqCst);
                Inner::post_register(&self.inner);
                Ok(())
            }

            // If the overlapped operation was successful and didn't finish
            // immediately then we forget a copy of the arc we hold
            // internally. This ensures that when the completion status comes
            // in for the I/O operation finishing it'll have a reference
            // associated with it and our data will still be valid. The
            // `connect_done` function will "reify" this forgotten pointer to
            // drop the refcount on the other side.
            Ok(false) => {
                trace!("connect in progress");
                mem::forget(self.inner.clone());
                Err(would_block())
            }

            // TODO: are we sure no IOCP notification comes in here?
            Err(e) => {
                trace!("connect error: {}", e);
                self.inner.connecting.store(false, SeqCst);
                Err(e)
            }
        }
    }

    /// Takes any internal error that has happened after the last I/O operation
    /// which hasn't been retrieved yet.
    ///
    /// This is particularly useful when detecting failed attempts to `connect`.
    /// After a completed `connect` flags this pipe as writable then callers
    /// must invoke this method to determine whether the connection actually
    /// succeeded. If this function returns `None` then a client is connected,
    /// otherwise it returns an error of what happened and a client shouldn't be
    /// connected.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        Ok(self.inner.io.lock().unwrap().connect_error.take())
    }

    /// Disconnects this named pipe from a connected client.
    ///
    /// This function will disconnect the pipe from a connected client, if any,
    /// transitively calling the `DisconnectNamedPipe` function. If the
    /// disconnection is successful then this object will no longer be readable
    /// or writable.
    ///
    /// After a `disconnect` is issued, then a `connect` may be called again to
    /// connect to another client.
    pub fn disconnect(&self) -> io::Result<()> {
        self.inner.handle.disconnect()?;
        self.inner
            .readiness
            .set_readiness(Ready::empty())
            .expect("event loop seems gone");
        Ok(())
    }

    fn registered(&self) -> bool {
        self.registered.load(SeqCst)
    }
}

impl Read for NamedPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        <&NamedPipe as Read>::read(&mut &*self, buf)
    }
}

impl Write for NamedPipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        <&NamedPipe as Write>::write(&mut &*self, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        <&NamedPipe as Write>::flush(&mut &*self)
    }
}

impl<'a> Read for &'a NamedPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Make sure we're registered
        if !self.registered() {
            return Err(would_block());
        }

        let mut state = self.inner.io.lock().unwrap();
        match mem::replace(&mut state.read, State::None) {
            // In theory not possible with `ready_registration` checked above,
            // but return would block for now.
            State::None => Err(would_block()),

            // A read is in flight, still waiting for it to finish
            State::Pending(buf, amt) => {
                state.read = State::Pending(buf, amt);
                Err(would_block())
            }

            // We previously read something into `data`, try to copy out some
            // data. If we copy out all the data schedule a new read and
            // otherwise store the buffer to get read later.
            State::Ok(data, cur) => {
                let n = {
                    let mut remaining = &data[cur..];
                    remaining.read(buf)?
                };
                let next = cur + n;
                if next != data.len() {
                    state.read = State::Ok(data, next);
                } else {
                    self.inner.put_buffer(data);
                    Inner::schedule_read(&self.inner, &mut state);
                }
                Ok(n)
            }

            // Looks like an in-flight read hit an error, return that here while
            // we schedule a new one.
            State::Err(e) => {
                Inner::schedule_read(&self.inner, &mut state);
                if e.raw_os_error() == Some(ERROR_BROKEN_PIPE as i32) {
                    Ok(0)
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl<'a> Write for &'a NamedPipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Make sure we're registered
        if !self.registered() {
            return Err(would_block());
        }

        // Make sure there's no writes pending
        let mut io = self.inner.io.lock().unwrap();
        match io.write {
            State::None => {}
            _ => return Err(would_block()),
        }

        // Move `buf` onto the heap and fire off the write
        let mut owned_buf = self.inner.get_buffer();
        owned_buf.extend(buf);
        Inner::schedule_write(&self.inner, owned_buf, 0, &mut io);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // TODO: `FlushFileBuffers` somehow?
        Ok(())
    }
}

impl Evented for NamedPipe {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        // First, register the handle with the event loop
        unsafe {
            self.poll_registration
                .register_handle(&self.inner.handle, token, poll)?;
        }
        poll.register(&self.ready_registration, token, interest, opts)?;
        self.registered.store(true, SeqCst);
        Inner::post_register(&self.inner);
        Ok(())
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        // Validate `Poll` and that we were previously registered
        unsafe {
            self.poll_registration
                .reregister_handle(&self.inner.handle, token, poll)?;
        }

        // At this point we should for sure have `ready_registration` unless
        // we're racing with `register` above, so just return a bland error if
        // the borrow fails.
        poll.reregister(&self.ready_registration, token, interest, opts)?;

        Inner::post_register(&self.inner);

        Ok(())
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        // Validate `Poll` and deregister ourselves
        unsafe {
            self.poll_registration
                .deregister_handle(&self.inner.handle, poll)?;
        }
        poll.deregister(&self.ready_registration)
    }
}

impl AsRawHandle for NamedPipe {
    fn as_raw_handle(&self) -> RawHandle {
        self.inner.handle.as_raw_handle()
    }
}

impl FromRawHandle for NamedPipe {
    unsafe fn from_raw_handle(handle: RawHandle) -> NamedPipe {
        let (r, s) = Registration::new2();
        NamedPipe {
            registered: AtomicBool::new(false),
            ready_registration: r,
            poll_registration: windows::Binding::new(),
            inner: FromRawArc::new(Inner {
                handle: pipe::NamedPipe::from_raw_handle(handle),
                readiness: s,
                connecting: AtomicBool::new(false),
                // transmutes to straddle winapi versions (mio 0.6 is on an
                // older winapi)
                connect: windows::Overlapped::new(mem::transmute(connect_done as fn(_))),
                read: windows::Overlapped::new(mem::transmute(read_done as fn(_))),
                write: windows::Overlapped::new(mem::transmute(write_done as fn(_))),
                io: Mutex::new(Io {
                    read: State::None,
                    write: State::None,
                    connect_error: None,
                }),
                pool: Mutex::new(BufferPool::with_capacity(2)),
            }),
        }
    }
}

impl fmt::Debug for NamedPipe {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.handle.fmt(f)
    }
}

impl Drop for NamedPipe {
    fn drop(&mut self) {
        // Cancel pending reads/connects, but don't cancel writes to ensure that
        // everything is flushed out.
        unsafe {
            if self.inner.connecting.load(SeqCst) {
                drop(cancel(&self.inner.handle, &self.inner.connect));
            }
            let io = self.inner.io.lock().unwrap();
            match io.read {
                State::Pending(..) => {
                    drop(cancel(&self.inner.handle, &self.inner.read));
                }
                _ => {}
            }
        }
    }
}

impl Inner {
    /// Schedules a read to happen in the background, executing an overlapped
    /// operation.
    ///
    /// This function returns `true` if a normal error happens or if the read
    /// is scheduled in the background. If the pipe is no longer connected
    /// (ERROR_PIPE_LISTENING) then `false` is returned and no read is
    /// scheduled.
    fn schedule_read(me: &FromRawArc<Inner>, io: &mut Io) -> bool {
        // Check to see if a read is already scheduled/completed
        match io.read {
            State::None => {}
            _ => return true,
        }

        // Turn off our read readiness
        let ready = me.readiness.readiness();
        me.readiness
            .set_readiness(ready & !Ready::readable())
            .expect("event loop seems gone");

        // Allocate a buffer and schedule the read.
        let mut buf = me.get_buffer();
        let e = unsafe {
            let overlapped = me.read.as_mut_ptr() as *mut _;
            let slice = slice::from_raw_parts_mut(buf.as_mut_ptr(), buf.capacity());
            me.handle.read_overlapped(slice, overlapped)
        };

        match e {
            // See `connect` above for the rationale behind `forget`
            Ok(e) => {
                trace!("schedule read success: {:?}", e);
                io.read = State::Pending(buf, 0); // 0 is ignored on read side
                mem::forget(me.clone());
                true
            }

            // If ERROR_PIPE_LISTENING happens then it's not a real read error,
            // we just need to wait for a connect.
            Err(ref e) if e.raw_os_error() == Some(ERROR_PIPE_LISTENING as i32) => false,

            // If some other error happened, though, we're now readable to give
            // out the error.
            Err(e) => {
                trace!("schedule read error: {}", e);
                io.read = State::Err(e);
                me.readiness
                    .set_readiness(ready | Ready::readable())
                    .expect("event loop still seems gone");
                true
            }
        }
    }

    fn schedule_write(me: &FromRawArc<Inner>, buf: Vec<u8>, pos: usize, io: &mut Io) {
        // Very similar to `schedule_read` above, just done for the write half.
        let ready = me.readiness.readiness();
        me.readiness
            .set_readiness(ready & !Ready::writable())
            .expect("event loop seems gone");

        let e = unsafe {
            let overlapped = me.write.as_mut_ptr() as *mut _;
            me.handle.write_overlapped(&buf[pos..], overlapped)
        };

        match e {
            // See `connect` above for the rationale behind `forget`
            Ok(e) => {
                trace!("schedule write success: {:?}", e);
                io.write = State::Pending(buf, pos);
                mem::forget(me.clone())
            }
            Err(e) => {
                trace!("schedule write error: {}", e);
                io.write = State::Err(e);
                me.add_readiness(Ready::writable());
            }
        }
    }

    fn add_readiness(&self, ready: Ready) {
        self.readiness
            .set_readiness(ready | self.readiness.readiness())
            .expect("event loop still seems gone");
    }

    fn post_register(me: &FromRawArc<Inner>) {
        let mut io = me.io.lock().unwrap();
        if Inner::schedule_read(&me, &mut io) {
            if let State::None = io.write {
                me.add_readiness(Ready::writable());
            }
        }
    }

    fn get_buffer(&self) -> Vec<u8> {
        self.pool.lock().unwrap().get(8 * 1024)
    }

    fn put_buffer(&self, buf: Vec<u8>) {
        self.pool.lock().unwrap().put(buf)
    }
}

unsafe fn cancel<T: AsRawHandle>(handle: &T, overlapped: &windows::Overlapped) -> io::Result<()> {
    let ret = CancelIoEx(handle.as_raw_handle(), overlapped.as_mut_ptr() as *mut _);
    if ret == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn connect_done(status: &OVERLAPPED_ENTRY) {
    let status = CompletionStatus::from_entry(status);
    trace!("connect done");

    // Acquire the `FromRawArc<Inner>`. Note that we should be guaranteed that
    // the refcount is available to us due to the `mem::forget` in
    // `connect` above.
    let me = unsafe { overlapped2arc!(status.overlapped(), Inner, connect) };

    // Flag ourselves as no longer using the `connect` overlapped instances.
    let prev = me.connecting.swap(false, SeqCst);
    assert!(prev, "wasn't previously connecting");

    // Stash away our connect error if one happened
    debug_assert_eq!(status.bytes_transferred(), 0);
    unsafe {
        match me.handle.result(status.overlapped()) {
            Ok(n) => debug_assert_eq!(n, 0),
            Err(e) => me.io.lock().unwrap().connect_error = Some(e),
        }
    }

    // We essentially just finished a registration, so kick off a
    // read and register write readiness.
    Inner::post_register(&me);
}

fn read_done(status: &OVERLAPPED_ENTRY) {
    let status = CompletionStatus::from_entry(status);
    trace!("read finished, bytes={}", status.bytes_transferred());

    // Acquire the `FromRawArc<Inner>`. Note that we should be guaranteed that
    // the refcount is available to us due to the `mem::forget` in
    // `schedule_read` above.
    let me = unsafe { overlapped2arc!(status.overlapped(), Inner, read) };

    // Move from the `Pending` to `Ok` state.
    let mut io = me.io.lock().unwrap();
    let mut buf = match mem::replace(&mut io.read, State::None) {
        State::Pending(buf, _) => buf,
        _ => unreachable!(),
    };
    unsafe {
        match me.handle.result(status.overlapped()) {
            Ok(n) => {
                debug_assert_eq!(status.bytes_transferred() as usize, n);
                buf.set_len(status.bytes_transferred() as usize);
                io.read = State::Ok(buf, 0);
            }
            Err(e) => {
                debug_assert_eq!(status.bytes_transferred(), 0);
                io.read = State::Err(e);
            }
        }
    }

    // Flag our readiness that we've got data.
    me.add_readiness(Ready::readable());
}

fn write_done(status: &OVERLAPPED_ENTRY) {
    let status = CompletionStatus::from_entry(status);
    trace!("write finished, bytes={}", status.bytes_transferred());
    // Acquire the `FromRawArc<Inner>`. Note that we should be guaranteed that
    // the refcount is available to us due to the `mem::forget` in
    // `schedule_write` above.
    let me = unsafe { overlapped2arc!(status.overlapped(), Inner, write) };

    // Make the state change out of `Pending`. If we wrote the entire buffer
    // then we're writable again and otherwise we schedule another write.
    let mut io = me.io.lock().unwrap();
    let (buf, pos) = match mem::replace(&mut io.write, State::None) {
        State::Pending(buf, pos) => (buf, pos),
        _ => unreachable!(),
    };

    unsafe {
        match me.handle.result(status.overlapped()) {
            Ok(n) => {
                debug_assert_eq!(status.bytes_transferred() as usize, n);
                let new_pos = pos + (status.bytes_transferred() as usize);
                if new_pos == buf.len() {
                    me.put_buffer(buf);
                    me.add_readiness(Ready::writable());
                } else {
                    Inner::schedule_write(&me, buf, new_pos, &mut io);
                }
            }
            Err(e) => {
                debug_assert_eq!(status.bytes_transferred(), 0);
                io.write = State::Err(e);
                me.add_readiness(Ready::writable());
            }
        }
    }
}

// Based on https://github.com/tokio-rs/mio/blob/13d5fc9/src/sys/windows/buffer_pool.rs
struct BufferPool {
    pool: Vec<Vec<u8>>,
}

impl BufferPool {
    fn with_capacity(cap: usize) -> BufferPool {
        BufferPool {
            pool: Vec::with_capacity(cap),
        }
    }

    fn get(&mut self, default_cap: usize) -> Vec<u8> {
        self.pool
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(default_cap))
    }

    fn put(&mut self, mut buf: Vec<u8>) {
        if self.pool.len() < self.pool.capacity() {
            unsafe {
                buf.set_len(0);
            }
            self.pool.push(buf);
        }
    }
}
