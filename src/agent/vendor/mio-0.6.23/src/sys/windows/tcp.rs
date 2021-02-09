use std::fmt;
use std::io::{self, Read, ErrorKind};
use std::mem;
use std::net::{self, SocketAddr, Shutdown};
use std::os::windows::prelude::*;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use miow::iocp::CompletionStatus;
use miow::net::*;
use net2::{TcpBuilder, TcpStreamExt as Net2TcpExt};
use winapi::*;
use iovec::IoVec;

use {poll, Ready, Poll, PollOpt, Token};
use event::Evented;
use sys::windows::from_raw_arc::FromRawArc;
use sys::windows::selector::{Overlapped, ReadyBinding};
use sys::windows::Family;

pub struct TcpStream {
    /// Separately stored implementation to ensure that the `Drop`
    /// implementation on this type is only executed when it's actually dropped
    /// (many clones of this `imp` are made).
    imp: StreamImp,
    registration: Mutex<Option<poll::Registration>>,
}

pub struct TcpListener {
    imp: ListenerImp,
    registration: Mutex<Option<poll::Registration>>,
}

#[derive(Clone)]
struct StreamImp {
    /// A stable address and synchronized access for all internals. This serves
    /// to ensure that all `Overlapped` pointers are valid for a long period of
    /// time as well as allowing completion callbacks to have access to the
    /// internals without having ownership.
    ///
    /// Note that the reference count also allows us "loan out" copies to
    /// completion ports while I/O is running to guarantee that this stays alive
    /// until the I/O completes. You'll notice a number of calls to
    /// `mem::forget` below, and these only happen on successful scheduling of
    /// I/O and are paired with `overlapped2arc!` macro invocations in the
    /// completion callbacks (to have a decrement match the increment).
    inner: FromRawArc<StreamIo>,
}

#[derive(Clone)]
struct ListenerImp {
    inner: FromRawArc<ListenerIo>,
}

struct StreamIo {
    inner: Mutex<StreamInner>,
    read: Overlapped, // also used for connect
    write: Overlapped,
    socket: net::TcpStream,
}

struct ListenerIo {
    inner: Mutex<ListenerInner>,
    accept: Overlapped,
    family: Family,
    socket: net::TcpListener,
}

struct StreamInner {
    iocp: ReadyBinding,
    deferred_connect: Option<SocketAddr>,
    read: State<(), ()>,
    write: State<(Vec<u8>, usize), (Vec<u8>, usize)>,
    /// whether we are instantly notified of success
    /// (FILE_SKIP_COMPLETION_PORT_ON_SUCCESS,
    ///  without a roundtrip through the event loop)
    instant_notify: bool,
}

struct ListenerInner {
    iocp: ReadyBinding,
    accept: State<net::TcpStream, (net::TcpStream, SocketAddr)>,
    accept_buf: AcceptAddrsBuf,
    instant_notify: bool,
}

enum State<T, U> {
    Empty,              // no I/O operation in progress
    Pending(T),         // an I/O operation is in progress
    Ready(U),           // I/O has finished with this value
    Error(io::Error),   // there was an I/O error
}

impl TcpStream {
    fn new(socket: net::TcpStream,
           deferred_connect: Option<SocketAddr>) -> TcpStream {
        TcpStream {
            registration: Mutex::new(None),
            imp: StreamImp {
                inner: FromRawArc::new(StreamIo {
                    read: Overlapped::new(read_done),
                    write: Overlapped::new(write_done),
                    socket: socket,
                    inner: Mutex::new(StreamInner {
                        iocp: ReadyBinding::new(),
                        deferred_connect: deferred_connect,
                        read: State::Empty,
                        write: State::Empty,
                        instant_notify: false,
                    }),
                }),
            },
        }
    }

    pub fn connect(socket: net::TcpStream, addr: &SocketAddr)
                   -> io::Result<TcpStream> {
        socket.set_nonblocking(true)?;
        Ok(TcpStream::new(socket, Some(*addr)))
    }

    pub fn from_stream(stream: net::TcpStream) -> TcpStream {
        TcpStream::new(stream, None)
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.imp.inner.socket.peer_addr()
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.imp.inner.socket.local_addr()
    }

    pub fn try_clone(&self) -> io::Result<TcpStream> {
        self.imp.inner.socket.try_clone().map(|s| TcpStream::new(s, None))
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.imp.inner.socket.shutdown(how)
    }

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.imp.inner.socket.set_nodelay(nodelay)
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        self.imp.inner.socket.nodelay()
    }

    pub fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        self.imp.inner.socket.set_recv_buffer_size(size)
    }

    pub fn recv_buffer_size(&self) -> io::Result<usize> {
        self.imp.inner.socket.recv_buffer_size()
    }

    pub fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        self.imp.inner.socket.set_send_buffer_size(size)
    }

    pub fn send_buffer_size(&self) -> io::Result<usize> {
        self.imp.inner.socket.send_buffer_size()
    }

    pub fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()> {
        self.imp.inner.socket.set_keepalive(keepalive)
    }

    pub fn keepalive(&self) -> io::Result<Option<Duration>> {
        self.imp.inner.socket.keepalive()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.imp.inner.socket.set_ttl(ttl)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.imp.inner.socket.ttl()
    }

    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.imp.inner.socket.set_only_v6(only_v6)
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        self.imp.inner.socket.only_v6()
    }

    pub fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        self.imp.inner.socket.set_linger(dur)
    }

    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.imp.inner.socket.linger()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        if let Some(e) = self.imp.inner.socket.take_error()? {
            return Ok(Some(e))
        }

        // If the syscall didn't return anything then also check to see if we've
        // squirreled away an error elsewhere for example as part of a connect
        // operation.
        //
        // Typically this is used like so:
        //
        // 1. A `connect` is issued
        // 2. Wait for the socket to be writable
        // 3. Call `take_error` to see if the connect succeeded.
        //
        // Right now the `connect` operation finishes in `read_done` below and
        // fill will in `State::Error` in the `read` slot if it fails, so we
        // extract that here.
        let mut me = self.inner();
        match mem::replace(&mut me.read, State::Empty) {
            State::Error(e) => {
                self.imp.schedule_read(&mut me);
                Ok(Some(e))
            }
            other => {
                me.read = other;
                Ok(None)
            }
        }
    }

    fn inner(&self) -> MutexGuard<StreamInner> {
        self.imp.inner()
    }

    fn before_read(&self) -> io::Result<MutexGuard<StreamInner>> {
        let mut me = self.inner();

        match me.read {
            // Empty == we're not associated yet, and if we're pending then
            // these are both cases where we return "would block"
            State::Empty |
            State::Pending(()) => return Err(io::ErrorKind::WouldBlock.into()),

            // If we got a delayed error as part of a `read_overlapped` below,
            // return that here. Also schedule another read in case it was
            // transient.
            State::Error(_) => {
                let e = match mem::replace(&mut me.read, State::Empty) {
                    State::Error(e) => e,
                    _ => panic!(),
                };
                self.imp.schedule_read(&mut me);
                return Err(e)
            }

            // If we're ready for a read then some previous 0-byte read has
            // completed. In that case the OS's socket buffer has something for
            // us, so we just keep pulling out bytes while we can in the loop
            // below.
            State::Ready(()) => {}
        }

        Ok(me)
    }

    fn post_register(&self, interest: Ready, me: &mut StreamInner) {
        if interest.is_readable() {
            self.imp.schedule_read(me);
        }

        // At least with epoll, if a socket is registered with an interest in
        // writing and it's immediately writable then a writable event is
        // generated immediately, so do so here.
        if interest.is_writable() {
            if let State::Empty = me.write {
                self.imp.add_readiness(me, Ready::writable());
            }
        }
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        match IoVec::from_bytes_mut(buf) {
            Some(vec) => self.readv(&mut [vec]),
            None => Ok(0),
        }
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut me = self.before_read()?;

        match (&self.imp.inner.socket).peek(buf) {
            Ok(n) => Ok(n),
            Err(e) => {
                me.read = State::Empty;
                self.imp.schedule_read(&mut me);
                Err(e)
            }
        }
    }

    pub fn readv(&self, bufs: &mut [&mut IoVec]) -> io::Result<usize> {
        let mut me = self.before_read()?;

        // TODO: Does WSARecv work on a nonblocking sockets? We ideally want to
        //       call that instead of looping over all the buffers and calling
        //       `recv` on each buffer. I'm not sure though if an overlapped
        //       socket in nonblocking mode would work with that use case,
        //       however, so for now we just call `recv`.

        let mut amt = 0;
        for buf in bufs {
            match (&self.imp.inner.socket).read(buf) {
                // If we did a partial read, then return what we've read so far
                Ok(n) if n < buf.len() => return Ok(amt + n),

                // Otherwise filled this buffer entirely, so try to fill the
                // next one as well.
                Ok(n) => amt += n,

                // If we hit an error then things get tricky if we've already
                // read some data. If the error is "would block" then we just
                // return the data we've read so far while scheduling another
                // 0-byte read.
                //
                // If we've read data and the error kind is not "would block",
                // then we stash away the error to get returned later and return
                // the data that we've read.
                //
                // Finally if we haven't actually read any data we just
                // reschedule a 0-byte read to happen again and then return the
                // error upwards.
                Err(e) => {
                    if amt > 0 && e.kind() == io::ErrorKind::WouldBlock {
                        me.read = State::Empty;
                        self.imp.schedule_read(&mut me);
                        return Ok(amt)
                    } else if amt > 0 {
                        me.read = State::Error(e);
                        return Ok(amt)
                    } else {
                        me.read = State::Empty;
                        self.imp.schedule_read(&mut me);
                        return Err(e)
                    }
                }
            }
        }

        Ok(amt)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        match IoVec::from_bytes(buf) {
            Some(vec) => self.writev(&[vec]),
            None => Ok(0),
        }
    }

    pub fn writev(&self, bufs: &[&IoVec]) -> io::Result<usize> {
        let mut me = self.inner();
        let me = &mut *me;

        match mem::replace(&mut me.write, State::Empty) {
            State::Empty => {}
            State::Error(e) => return Err(e),
            other => {
                me.write = other;
                return Err(io::ErrorKind::WouldBlock.into())
            }
        }

        if !me.iocp.registered() {
            return Err(io::ErrorKind::WouldBlock.into())
        }

        if bufs.is_empty() {
            return Ok(0)
        }

        let len = bufs.iter().map(|b| b.len()).fold(0, |a, b| a + b);
        let mut intermediate = me.iocp.get_buffer(len);
        for buf in bufs {
            intermediate.extend_from_slice(buf);
        }
        self.imp.schedule_write(intermediate, 0, me);
        Ok(len)
    }

    pub fn flush(&self) -> io::Result<()> {
        Ok(())
    }
}

impl StreamImp {
    fn inner(&self) -> MutexGuard<StreamInner> {
        self.inner.inner.lock().unwrap()
    }

    fn schedule_connect(&self, addr: &SocketAddr) -> io::Result<()> {
        unsafe {
            trace!("scheduling a connect");
            self.inner.socket.connect_overlapped(addr, &[], self.inner.read.as_mut_ptr())?;
        }
        // see docs above on StreamImp.inner for rationale on forget
        mem::forget(self.clone());
        Ok(())
    }

    /// Schedule a read to happen on this socket, enqueuing us to receive a
    /// notification when a read is ready.
    ///
    /// Note that this does *not* work with a buffer. When reading a TCP stream
    /// we actually read into a 0-byte buffer so Windows will send us a
    /// notification when the socket is otherwise ready for reading. This allows
    /// us to avoid buffer allocations for in-flight reads.
    fn schedule_read(&self, me: &mut StreamInner) {
        match me.read {
            State::Empty => {}
            State::Ready(_) | State::Error(_) => {
                self.add_readiness(me, Ready::readable());
                return;
            }
            _ => return,
        }

        me.iocp.set_readiness(me.iocp.readiness() - Ready::readable());

        trace!("scheduling a read");
        let res = unsafe {
            self.inner.socket.read_overlapped(&mut [], self.inner.read.as_mut_ptr())
        };
        match res {
            // Note that `Ok(true)` means that this completed immediately and
            // our socket is readable. This typically means that the caller of
            // this function (likely `read` above) can try again as an
            // optimization and return bytes quickly.
            //
            // Normally, though, although the read completed immediately
            // there's still an IOCP completion packet enqueued that we're going
            // to receive.
            //
            // You can configure this behavior (miow) with
            // SetFileCompletionNotificationModes to indicate that `Ok(true)`
            // does **not** enqueue a completion packet. (This is the case
            // for me.instant_notify)
            //
            // Note that apparently libuv has scary code to work around bugs in
            // `WSARecv` for UDP sockets apparently for handles which have had
            // the `SetFileCompletionNotificationModes` function called on them,
            // worth looking into!
            Ok(Some(_)) if me.instant_notify => {
                me.read = State::Ready(());
                self.add_readiness(me, Ready::readable());
            }
            Ok(_) => {
                // see docs above on StreamImp.inner for rationale on forget
                me.read = State::Pending(());
                mem::forget(self.clone());
            }
            Err(e) => {
                me.read = State::Error(e);
                self.add_readiness(me, Ready::readable());
            }
        }
    }

    /// Similar to `schedule_read`, except that this issues, well, writes.
    ///
    /// This function will continually attempt to write the entire contents of
    /// the buffer `buf` until they have all been written. The `pos` argument is
    /// the current offset within the buffer up to which the contents have
    /// already been written.
    ///
    /// A new writable event (e.g. allowing another write) will only happen once
    /// the buffer has been written completely (or hit an error).
    fn schedule_write(&self,
                      buf: Vec<u8>,
                      mut pos: usize,
                      me: &mut StreamInner) {

        // About to write, clear any pending level triggered events
        me.iocp.set_readiness(me.iocp.readiness() - Ready::writable());

        loop {
            trace!("scheduling a write of {} bytes", buf[pos..].len());
            let ret = unsafe {
                self.inner.socket.write_overlapped(&buf[pos..], self.inner.write.as_mut_ptr())
            };
            match ret {
                Ok(Some(transferred_bytes)) if me.instant_notify => {
                    trace!("done immediately with {} bytes", transferred_bytes);
                    if transferred_bytes == buf.len() - pos {
                        self.add_readiness(me, Ready::writable());
                        me.write = State::Empty;
                        break;
                    }
                    pos += transferred_bytes;
                }
                Ok(_) => {
                    trace!("scheduled for later");
                    // see docs above on StreamImp.inner for rationale on forget
                    me.write = State::Pending((buf, pos));
                    mem::forget(self.clone());
                    break;
                }
                Err(e) => {
                    trace!("write error: {}", e);
                    me.write = State::Error(e);
                    self.add_readiness(me, Ready::writable());
                    me.iocp.put_buffer(buf);
                    break;
                }
            }
        }
    }

    /// Pushes an event for this socket onto the selector its registered for.
    ///
    /// When an event is generated on this socket, if it happened after the
    /// socket was closed then we don't want to actually push the event onto our
    /// selector as otherwise it's just a spurious notification.
    fn add_readiness(&self, me: &mut StreamInner, set: Ready) {
        me.iocp.set_readiness(set | me.iocp.readiness());
    }
}

fn read_done(status: &OVERLAPPED_ENTRY) {
    let status = CompletionStatus::from_entry(status);
    let me2 = StreamImp {
        inner: unsafe { overlapped2arc!(status.overlapped(), StreamIo, read) },
    };

    let mut me = me2.inner();
    match mem::replace(&mut me.read, State::Empty) {
        State::Pending(()) => {
            trace!("finished a read: {}", status.bytes_transferred());
            assert_eq!(status.bytes_transferred(), 0);
            me.read = State::Ready(());
            return me2.add_readiness(&mut me, Ready::readable())
        }
        s => me.read = s,
    }

    // If a read didn't complete, then the connect must have just finished.
    trace!("finished a connect");

    // By guarding with socket.result(), we ensure that a connection
    // was successfully made before performing operations requiring a
    // connected socket.
    match unsafe { me2.inner.socket.result(status.overlapped()) }
        .and_then(|_| me2.inner.socket.connect_complete())
    {
        Ok(()) => {
            me2.add_readiness(&mut me, Ready::writable());
            me2.schedule_read(&mut me);
        }
        Err(e) => {
            me2.add_readiness(&mut me, Ready::readable() | Ready::writable());
            me.read = State::Error(e);
        }
    }
}

fn write_done(status: &OVERLAPPED_ENTRY) {
    let status = CompletionStatus::from_entry(status);
    trace!("finished a write {}", status.bytes_transferred());
    let me2 = StreamImp {
        inner: unsafe { overlapped2arc!(status.overlapped(), StreamIo, write) },
    };
    let mut me = me2.inner();
    let (buf, pos) = match mem::replace(&mut me.write, State::Empty) {
        State::Pending(pair) => pair,
        _ => unreachable!(),
    };
    let new_pos = pos + (status.bytes_transferred() as usize);
    if new_pos == buf.len() {
        me2.add_readiness(&mut me, Ready::writable());
    } else {
        me2.schedule_write(buf, new_pos, &mut me);
    }
}

impl Evented for TcpStream {
    fn register(&self, poll: &Poll, token: Token,
                interest: Ready, opts: PollOpt) -> io::Result<()> {
        let mut me = self.inner();
        me.iocp.register_socket(&self.imp.inner.socket, poll, token,
                                     interest, opts, &self.registration)?;

        unsafe {
            super::no_notify_on_instant_completion(self.imp.inner.socket.as_raw_socket() as HANDLE)?;
            me.instant_notify = true;
        }

        // If we were connected before being registered process that request
        // here and go along our merry ways. Note that the callback for a
        // successful connect will worry about generating writable/readable
        // events and scheduling a new read.
        if let Some(addr) = me.deferred_connect.take() {
            return self.imp.schedule_connect(&addr).map(|_| ())
        }
        self.post_register(interest, &mut me);
        Ok(())
    }

    fn reregister(&self, poll: &Poll, token: Token,
                  interest: Ready, opts: PollOpt) -> io::Result<()> {
        let mut me = self.inner();
        me.iocp.reregister_socket(&self.imp.inner.socket, poll, token,
                                       interest, opts, &self.registration)?;
        self.post_register(interest, &mut me);
        Ok(())
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.inner().iocp.deregister(&self.imp.inner.socket,
                                     poll, &self.registration)
    }
}

impl fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TcpStream")
            .finish()
    }
}

impl Drop for TcpStream {
    fn drop(&mut self) {
        // If we're still internally reading, we're no longer interested. Note
        // though that we don't cancel any writes which may have been issued to
        // preserve the same semantics as Unix.
        //
        // Note that "Empty" here may mean that a connect is pending, so we
        // cancel even if that happens as well.
        unsafe {
            match self.inner().read {
                State::Pending(_) | State::Empty => {
                    trace!("cancelling active TCP read");
                    drop(super::cancel(&self.imp.inner.socket,
                                       &self.imp.inner.read));
                }
                State::Ready(_) | State::Error(_) => {}
            }
        }
    }
}

impl TcpListener {
    pub fn new(socket: net::TcpListener)
               -> io::Result<TcpListener> {
        let addr = socket.local_addr()?;
        Ok(TcpListener::new_family(socket, match addr {
            SocketAddr::V4(..) => Family::V4,
            SocketAddr::V6(..) => Family::V6,
        }))
    }

    fn new_family(socket: net::TcpListener, family: Family) -> TcpListener {
        TcpListener {
            registration: Mutex::new(None),
            imp: ListenerImp {
                inner: FromRawArc::new(ListenerIo {
                    accept: Overlapped::new(accept_done),
                    family: family,
                    socket: socket,
                    inner: Mutex::new(ListenerInner {
                        iocp: ReadyBinding::new(),
                        accept: State::Empty,
                        accept_buf: AcceptAddrsBuf::new(),
                        instant_notify: false,
                    }),
                }),
            },
        }
    }

    pub fn accept(&self) -> io::Result<(net::TcpStream, SocketAddr)> {
        let mut me = self.inner();

        let ret = match mem::replace(&mut me.accept, State::Empty) {
            State::Empty => return Err(io::ErrorKind::WouldBlock.into()),
            State::Pending(t) => {
                me.accept = State::Pending(t);
                return Err(io::ErrorKind::WouldBlock.into());
            }
            State::Ready((s, a)) => Ok((s, a)),
            State::Error(e) => Err(e),
        };

        self.imp.schedule_accept(&mut me);

        return ret
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.imp.inner.socket.local_addr()
    }

    pub fn try_clone(&self) -> io::Result<TcpListener> {
        self.imp.inner.socket.try_clone().map(|s| {
            TcpListener::new_family(s, self.imp.inner.family)
        })
    }

    #[allow(deprecated)]
    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.imp.inner.socket.set_only_v6(only_v6)
    }

    #[allow(deprecated)]
    pub fn only_v6(&self) -> io::Result<bool> {
        self.imp.inner.socket.only_v6()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.imp.inner.socket.set_ttl(ttl)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.imp.inner.socket.ttl()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.imp.inner.socket.take_error()
    }

    fn inner(&self) -> MutexGuard<ListenerInner> {
        self.imp.inner()
    }
}

impl ListenerImp {
    fn inner(&self) -> MutexGuard<ListenerInner> {
        self.inner.inner.lock().unwrap()
    }

    fn schedule_accept(&self, me: &mut ListenerInner) {
        match me.accept {
            State::Empty => {}
            _ => return
        }

        me.iocp.set_readiness(me.iocp.readiness() - Ready::readable());

        let res = match self.inner.family {
            Family::V4 => TcpBuilder::new_v4(),
            Family::V6 => TcpBuilder::new_v6(),
        }.and_then(|builder| unsafe {
            trace!("scheduling an accept");
            self.inner.socket.accept_overlapped(&builder, &mut me.accept_buf,
                                                self.inner.accept.as_mut_ptr())
        });
        match res {
            Ok((socket, _)) => {
                // see docs above on StreamImp.inner for rationale on forget
                me.accept = State::Pending(socket);
                mem::forget(self.clone());
            }
            Err(e) => {
                me.accept = State::Error(e);
                self.add_readiness(me, Ready::readable());
            }
        }
    }

    // See comments in StreamImp::push
    fn add_readiness(&self, me: &mut ListenerInner, set: Ready) {
        me.iocp.set_readiness(set | me.iocp.readiness());
    }
}

fn accept_done(status: &OVERLAPPED_ENTRY) {
    let status = CompletionStatus::from_entry(status);
    let me2 = ListenerImp {
        inner: unsafe { overlapped2arc!(status.overlapped(), ListenerIo, accept) },
    };

    let mut me = me2.inner();
    let socket = match mem::replace(&mut me.accept, State::Empty) {
        State::Pending(s) => s,
        _ => unreachable!(),
    };
    trace!("finished an accept");
    let result = me2.inner.socket.accept_complete(&socket).and_then(|()| {
        me.accept_buf.parse(&me2.inner.socket)
    }).and_then(|buf| {
        buf.remote().ok_or_else(|| {
            io::Error::new(ErrorKind::Other, "could not obtain remote address")
        })
    });
    me.accept = match result {
        Ok(remote_addr) => State::Ready((socket, remote_addr)),
        Err(e) => State::Error(e),
    };
    me2.add_readiness(&mut me, Ready::readable());
}

impl Evented for TcpListener {
    fn register(&self, poll: &Poll, token: Token,
                interest: Ready, opts: PollOpt) -> io::Result<()> {
        let mut me = self.inner();
        me.iocp.register_socket(&self.imp.inner.socket, poll, token,
                                     interest, opts, &self.registration)?;

        unsafe {
            super::no_notify_on_instant_completion(self.imp.inner.socket.as_raw_socket() as HANDLE)?;
            me.instant_notify = true;
        }

        self.imp.schedule_accept(&mut me);
        Ok(())
    }

    fn reregister(&self, poll: &Poll, token: Token,
                  interest: Ready, opts: PollOpt) -> io::Result<()> {
        let mut me = self.inner();
        me.iocp.reregister_socket(&self.imp.inner.socket, poll, token,
                                       interest, opts, &self.registration)?;
        self.imp.schedule_accept(&mut me);
        Ok(())
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.inner().iocp.deregister(&self.imp.inner.socket,
                                     poll, &self.registration)
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TcpListener")
            .finish()
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        // If we're still internally reading, we're no longer interested.
        unsafe {
            match self.inner().accept {
                State::Pending(_) => {
                    trace!("cancelling active TCP accept");
                    drop(super::cancel(&self.imp.inner.socket,
                                       &self.imp.inner.accept));
                }
                State::Empty |
                State::Ready(_) |
                State::Error(_) => {}
            }
        }
    }
}
