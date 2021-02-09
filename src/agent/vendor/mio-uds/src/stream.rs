use std::cmp;
use std::io::prelude::*;
use std::io;
use std::os::unix::net;
use std::os::unix::prelude::*;
use std::path::Path;
use std::net::Shutdown;

use iovec::IoVec;
use iovec::unix as iovec;
use libc;
use mio::event::Evented;
use mio::unix::EventedFd;
use mio::{Poll, Token, Ready, PollOpt};

use cvt;
use socket::{sockaddr_un, Socket};

/// A Unix stream socket.
///
/// This type represents a `SOCK_STREAM` connection of the `AF_UNIX` family,
/// otherwise known as Unix domain sockets or Unix sockets. This stream is
/// readable/writable and acts similarly to a TCP stream where reads/writes are
/// all in order with respect to the other connected end.
///
/// Streams can either be connected to paths locally or another ephemeral socket
/// created by the `pair` function.
///
/// A `UnixStream` implements the `Read`, `Write`, `Evented`, `AsRawFd`,
/// `IntoRawFd`, and `FromRawFd` traits for interoperating with other I/O code.
///
/// Note that all values of this type are typically in nonblocking mode, so the
/// `read` and `write` methods may return an error with the kind of
/// `WouldBlock`, indicating that it's not ready to read/write just yet.
#[derive(Debug)]
pub struct UnixStream {
    inner: net::UnixStream,
}

impl UnixStream {
    /// Connects to the socket named by `path`.
    ///
    /// The socket returned may not be readable and/or writable yet, as the
    /// connection may be in progress. The socket should be registered with an
    /// event loop to wait on both of these properties being available.
    pub fn connect<P: AsRef<Path>>(p: P) -> io::Result<UnixStream> {
        UnixStream::_connect(p.as_ref())
    }

    fn _connect(path: &Path) -> io::Result<UnixStream> {
        unsafe {
            let (addr, len) = try!(sockaddr_un(path));
            let socket = try!(Socket::new(libc::SOCK_STREAM));
            let addr = &addr as *const _ as *const _;
            match cvt(libc::connect(socket.fd(), addr, len)) {
                Ok(_) => {}
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }

            Ok(UnixStream::from_raw_fd(socket.into_fd()))
        }
    }

    /// Consumes a standard library `UnixStream` and returns a wrapped
    /// `UnixStream` compatible with mio.
    ///
    /// The returned stream is moved into nonblocking mode and is otherwise
    /// ready to get associated with an event loop.
    pub fn from_stream(stream: net::UnixStream) -> io::Result<UnixStream> {
        try!(stream.set_nonblocking(true));
        Ok(UnixStream { inner: stream })
    }

    /// Creates an unnamed pair of connected sockets.
    ///
    /// Returns two `UnixStream`s which are connected to each other.
    pub fn pair() -> io::Result<(UnixStream, UnixStream)> {
        Socket::pair(libc::SOCK_STREAM).map(|(a, b)| unsafe {
            (UnixStream::from_raw_fd(a.into_fd()),
             UnixStream::from_raw_fd(b.into_fd()))
        })
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UnixStream` is a reference to the same stream that this
    /// object references. Both handles will read and write the same stream of
    /// data, and options set on one stream will be propogated to the other
    /// stream.
    pub fn try_clone(&self) -> io::Result<UnixStream> {
        self.inner.try_clone().map(|s| {
            UnixStream { inner: s }
        })
    }

    /// Returns the socket address of the local half of this connection.
    pub fn local_addr(&self) -> io::Result<net::SocketAddr> {
        self.inner.local_addr()
    }

    /// Returns the socket address of the remote half of this connection.
    pub fn peer_addr(&self) -> io::Result<net::SocketAddr> {
        self.inner.peer_addr()
    }

    /// Returns the value of the `SO_ERROR` option.
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of `Shutdown`).
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    /// Read in a list of buffers all at once.
    ///
    /// This operation will attempt to read bytes from this socket and place
    /// them into the list of buffers provided. Note that each buffer is an
    /// `IoVec` which can be created from a byte slice.
    ///
    /// The buffers provided will be filled in sequentially. A buffer will be
    /// entirely filled up before the next is written to.
    ///
    /// The number of bytes read is returned, if successful, or an error is
    /// returned otherwise. If no bytes are available to be read yet then
    /// a "would block" error is returned. This operation does not block.
    pub fn read_bufs(&self, bufs: &mut [&mut IoVec]) -> io::Result<usize> {
        unsafe {
            let slice = iovec::as_os_slice_mut(bufs);
            let len = cmp::min(<libc::c_int>::max_value() as usize, slice.len());
            let rc = libc::readv(self.inner.as_raw_fd(),
                                slice.as_ptr(),
                                len as libc::c_int);
            if rc < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(rc as usize)
            }
        }
    }

    /// Write a list of buffers all at once.
    ///
    /// This operation will attempt to write a list of byte buffers to this
    /// socket. Note that each buffer is an `IoVec` which can be created from a
    /// byte slice.
    ///
    /// The buffers provided will be written sequentially. A buffer will be
    /// entirely written before the next is written.
    ///
    /// The number of bytes written is returned, if successful, or an error is
    /// returned otherwise. If the socket is not currently writable then a
    /// "would block" error is returned. This operation does not block.
    pub fn write_bufs(&self, bufs: &[&IoVec]) -> io::Result<usize> {
        unsafe {
            let slice = iovec::as_os_slice(bufs);
            let len = cmp::min(<libc::c_int>::max_value() as usize, slice.len());
            let rc = libc::writev(self.inner.as_raw_fd(),
                                 slice.as_ptr(),
                                 len as libc::c_int);
            if rc < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(rc as usize)
            }
        }
    }
}

impl Evented for UnixStream {
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

impl Read for UnixStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.inner.read(bytes)
    }
}

impl<'a> Read for &'a UnixStream {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        (&self.inner).read(bytes)
    }
}

impl Write for UnixStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a> Write for &'a UnixStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        (&self.inner).write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.inner).flush()
    }
}

impl AsRawFd for UnixStream {
    fn as_raw_fd(&self) -> i32 {
        self.inner.as_raw_fd()
    }
}

impl IntoRawFd for UnixStream {
    fn into_raw_fd(self) -> i32 {
        self.inner.into_raw_fd()
    }
}

impl FromRawFd for UnixStream {
    unsafe fn from_raw_fd(fd: i32) -> UnixStream {
        UnixStream { inner: net::UnixStream::from_raw_fd(fd) }
    }
}
