use std::io;
use std::os::unix::net;
use std::os::unix::prelude::*;
use std::path::Path;

use libc;
use mio::event::Evented;
use mio::unix::EventedFd;
use mio::{Poll, PollOpt, Ready, Token};

use UnixStream;
use cvt;
use socket::{sockaddr_un, Socket};

/// A structure representing a Unix domain socket server.
///
/// This listener can be used to accept new streams connected to a remote
/// endpoint, through which the `read` and `write` methods can be used to
/// communicate.
#[derive(Debug)]
pub struct UnixListener {
    inner: net::UnixListener,
}

impl UnixListener {
    /// Creates a new `UnixListener` bound to the specified socket.
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixListener> {
        UnixListener::_bind(path.as_ref())
    }

    fn _bind(path: &Path) -> io::Result<UnixListener> {
        unsafe {
            let (addr, len) = try!(sockaddr_un(path));
            let fd = try!(Socket::new(libc::SOCK_STREAM));

            let addr = &addr as *const _ as *const _;
            try!(cvt(libc::bind(fd.fd(), addr, len)));
            try!(cvt(libc::listen(fd.fd(), 128)));

            Ok(UnixListener::from_raw_fd(fd.into_fd()))
        }
    }

    /// Consumes a standard library `UnixListener` and returns a wrapped
    /// `UnixListener` compatible with mio.
    ///
    /// The returned stream is moved into nonblocking mode and is otherwise
    /// ready to get associated with an event loop.
    pub fn from_listener(stream: net::UnixListener) -> io::Result<UnixListener> {
        try!(stream.set_nonblocking(true));
        Ok(UnixListener { inner: stream })
    }

    /// Accepts a new incoming connection to this listener.
    ///
    /// When established, the corresponding `UnixStream` and the remote peer's
    /// address will be returned as `Ok(Some(...))`. If there is no connection
    /// waiting to be accepted, then `Ok(None)` is returned.
    ///
    /// If an error happens while accepting, `Err` is returned.
    pub fn accept(&self) -> io::Result<Option<(UnixStream, net::SocketAddr)>> {
        match try!(self.accept_std()) {
            Some((stream, addr)) => Ok(Some((UnixStream::from_stream(stream)?, addr))),
            None => Ok(None),
        }
    }

    /// Accepts a new incoming connection to this listener.
    ///
    /// This method is the same as `accept`, except that it returns a `net::UnixStream` *in blocking mode*
    /// which isn't bound to a `mio` type. This can later be converted to a `mio` type, if
    /// necessary.
    ///
    /// If an error happens while accepting, `Err` is returned.
    pub fn accept_std(&self) -> io::Result<Option<(net::UnixStream, net::SocketAddr)>> {
        match self.inner.accept() {
            Ok((socket, addr)) => Ok(Some(unsafe {
                (net::UnixStream::from_raw_fd(socket.into_raw_fd()), addr)
            })),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UnixListener` is a reference to the same socket that this
    /// object references. Both handles can be used to accept incoming
    /// connections and options set on one listener will affect the other.
    pub fn try_clone(&self) -> io::Result<UnixListener> {
        self.inner.try_clone().map(|l| UnixListener { inner: l })
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<net::SocketAddr> {
        self.inner.local_addr()
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }
}

impl Evented for UnixListener {
    fn register(&self, poll: &Poll, token: Token, events: Ready, opts: PollOpt) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, events, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        events: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, events, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}

impl AsRawFd for UnixListener {
    fn as_raw_fd(&self) -> i32 {
        self.inner.as_raw_fd()
    }
}

impl IntoRawFd for UnixListener {
    fn into_raw_fd(self) -> i32 {
        self.inner.into_raw_fd()
    }
}

impl FromRawFd for UnixListener {
    unsafe fn from_raw_fd(fd: i32) -> UnixListener {
        UnixListener {
            inner: net::UnixListener::from_raw_fd(fd),
        }
    }
}
