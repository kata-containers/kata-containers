use std::io;
use std::net::Shutdown;
use std::os::unix::net;
use std::os::unix::prelude::*;
use std::path::Path;

use libc;
use mio::event::Evented;
use mio::unix::EventedFd;
use mio::{Poll, Token, Ready, PollOpt};

use cvt;
use socket::{sockaddr_un, Socket};

/// A Unix datagram socket.
#[derive(Debug)]
pub struct UnixDatagram {
    inner: net::UnixDatagram,
}

impl UnixDatagram {
    /// Creates a Unix datagram socket bound to the given path.
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixDatagram> {
        UnixDatagram::_bind(path.as_ref())
    }

    fn _bind(path: &Path) -> io::Result<UnixDatagram> {
        unsafe {
            let (addr, len) = try!(sockaddr_un(path));
            let fd = try!(Socket::new(libc::SOCK_DGRAM));

            let addr = &addr as *const _ as *const _;
            try!(cvt(libc::bind(fd.fd(), addr, len)));

            Ok(UnixDatagram::from_raw_fd(fd.into_fd()))
        }
    }

    /// Consumes a standard library `UnixDatagram` and returns a wrapped
    /// `UnixDatagram` compatible with mio.
    ///
    /// The returned stream is moved into nonblocking mode and is otherwise
    /// ready to get associated with an event loop.
    pub fn from_datagram(stream: net::UnixDatagram) -> io::Result<UnixDatagram> {
        try!(stream.set_nonblocking(true));
        Ok(UnixDatagram { inner: stream })
    }

    /// Create an unnamed pair of connected sockets.
    ///
    /// Returns two `UnixDatagrams`s which are connected to each other.
    pub fn pair() -> io::Result<(UnixDatagram, UnixDatagram)> {
        unsafe {
            let (a, b) = try!(Socket::pair(libc::SOCK_DGRAM));
            Ok((UnixDatagram::from_raw_fd(a.into_fd()),
                UnixDatagram::from_raw_fd(b.into_fd())))
        }
    }

    /// Creates a Unix Datagram socket which is not bound to any address.
    pub fn unbound() -> io::Result<UnixDatagram> {
        let stream = try!(net::UnixDatagram::unbound());
        try!(stream.set_nonblocking(true));
        Ok(UnixDatagram { inner: stream })
    }

    /// Connects the socket to the specified address.
    ///
    /// The `send` method may be used to send data to the specified address.
    /// `recv` and `recv_from` will only receive data from that address.
    pub fn connect<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        self.inner.connect(path)
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UnixListener` is a reference to the same socket that this
    /// object references. Both handles can be used to accept incoming
    /// connections and options set on one listener will affect the other.
    pub fn try_clone(&self) -> io::Result<UnixDatagram> {
        self.inner.try_clone().map(|i| {
            UnixDatagram { inner: i }
        })
    }

    /// Returns the address of this socket.
    pub fn local_addr(&self) -> io::Result<net::SocketAddr> {
        self.inner.local_addr()
    }

    /// Returns the address of this socket's peer.
    ///
    /// The `connect` method will connect the socket to a peer.
    pub fn peer_addr(&self) -> io::Result<net::SocketAddr> {
        self.inner.peer_addr()
    }

    /// Receives data from the socket.
    ///
    /// On success, returns the number of bytes read and the address from
    /// whence the data came.
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, net::SocketAddr)> {
        self.inner.recv_from(buf)
    }

    /// Receives data from the socket.
    ///
    /// On success, returns the number of bytes read.
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.recv(buf)
    }

    /// Sends data on the socket to the specified address.
    ///
    /// On success, returns the number of bytes written.
    pub fn send_to<P: AsRef<Path>>(&self, buf: &[u8], path: P) -> io::Result<usize> {
        self.inner.send_to(buf, path)
    }

    /// Sends data on the socket to the socket's peer.
    ///
    /// The peer address may be set by the `connect` method, and this method
    /// will return an error if the socket has not already been connected.
    ///
    /// On success, returns the number of bytes written.
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.inner.send(buf)
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Shut down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of `Shutdown`).
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }
}

impl Evented for UnixDatagram {
    fn register(&self,
                poll: &Poll,
                token: Token,
                events: Ready,
                opts: PollOpt) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, events, opts)
    }

    fn reregister(&self,
                  poll: &Poll,
                  token: Token,
                  events: Ready,
                  opts: PollOpt) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, events, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}

impl AsRawFd for UnixDatagram {
    fn as_raw_fd(&self) -> i32 {
        self.inner.as_raw_fd()
    }
}

impl IntoRawFd for UnixDatagram {
    fn into_raw_fd(self) -> i32 {
        self.inner.into_raw_fd()
    }
}

impl FromRawFd for UnixDatagram {
    unsafe fn from_raw_fd(fd: i32) -> UnixDatagram {
        UnixDatagram { inner: net::UnixDatagram::from_raw_fd(fd) }
    }
}
