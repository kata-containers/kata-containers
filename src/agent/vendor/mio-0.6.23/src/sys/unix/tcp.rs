use std::fmt;
use std::io::{Read, Write};
use std::net::{self, SocketAddr};
use std::os::unix::io::{RawFd, FromRawFd, IntoRawFd, AsRawFd};
use std::time::Duration;

use libc;
use net2::TcpStreamExt;
use iovec::IoVec;

use {io, Ready, Poll, PollOpt, Token};
use event::Evented;

use sys::unix::eventedfd::EventedFd;
use sys::unix::io::set_nonblock;
use sys::unix::uio::VecIo;

pub struct TcpStream {
    inner: net::TcpStream,
}

pub struct TcpListener {
    inner: net::TcpListener,
}

impl TcpStream {
    pub fn connect(stream: net::TcpStream, addr: &SocketAddr) -> io::Result<TcpStream> {
        set_nonblock(stream.as_raw_fd())?;

        match stream.connect(addr) {
            Ok(..) => {}
            Err(ref e) if e.raw_os_error() == Some(libc::EINPROGRESS) => {}
            Err(e) => return Err(e),
        }

        Ok(TcpStream {
            inner: stream,
        })
    }

    pub fn from_stream(stream: net::TcpStream) -> TcpStream {
        TcpStream {
            inner: stream,
        }
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    pub fn try_clone(&self) -> io::Result<TcpStream> {
        self.inner.try_clone().map(|s| {
            TcpStream {
                inner: s,
            }
        })
    }

    pub fn shutdown(&self, how: net::Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.set_nodelay(nodelay)
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.nodelay()
    }

    pub fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        self.inner.set_recv_buffer_size(size)
    }

    pub fn recv_buffer_size(&self) -> io::Result<usize> {
        self.inner.recv_buffer_size()
    }

    pub fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        self.inner.set_send_buffer_size(size)
    }

    pub fn send_buffer_size(&self) -> io::Result<usize> {
        self.inner.send_buffer_size()
    }

    pub fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()> {
        self.inner.set_keepalive(keepalive)
    }

    pub fn keepalive(&self) -> io::Result<Option<Duration>> {
        self.inner.keepalive()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.ttl()
    }

    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.inner.set_only_v6(only_v6)
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        self.inner.only_v6()
    }

    pub fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_linger(dur)
    }

    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.inner.linger()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.peek(buf)
    }

    pub fn readv(&self, bufs: &mut [&mut IoVec]) -> io::Result<usize> {
        self.inner.readv(bufs)
    }

    pub fn writev(&self, bufs: &[&IoVec]) -> io::Result<usize> {
        self.inner.writev(bufs)
    }
}

impl<'a> Read for &'a TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&self.inner).read(buf)
    }
}

impl<'a> Write for &'a TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&self.inner).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&self.inner).flush()
    }
}

impl Evented for TcpStream {
    fn register(&self, poll: &Poll, token: Token,
                interest: Ready, opts: PollOpt) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token,
                  interest: Ready, opts: PollOpt) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}

impl fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl FromRawFd for TcpStream {
    unsafe fn from_raw_fd(fd: RawFd) -> TcpStream {
        TcpStream {
            inner: net::TcpStream::from_raw_fd(fd),
        }
    }
}

impl IntoRawFd for TcpStream {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_raw_fd()
    }
}

impl AsRawFd for TcpStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl TcpListener {
    pub fn new(inner: net::TcpListener) -> io::Result<TcpListener> {
        set_nonblock(inner.as_raw_fd())?;
        Ok(TcpListener {
            inner,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    pub fn try_clone(&self) -> io::Result<TcpListener> {
        self.inner.try_clone().map(|s| {
            TcpListener {
                inner: s,
            }
        })
    }

    pub fn accept(&self) -> io::Result<(net::TcpStream, SocketAddr)> {
        self.inner.accept()
    }

    #[allow(deprecated)]
    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.inner.set_only_v6(only_v6)
    }

    #[allow(deprecated)]
    pub fn only_v6(&self) -> io::Result<bool> {
        self.inner.only_v6()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.ttl()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }
}

impl Evented for TcpListener {
    fn register(&self, poll: &Poll, token: Token,
                interest: Ready, opts: PollOpt) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token,
                  interest: Ready, opts: PollOpt) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl FromRawFd for TcpListener {
    unsafe fn from_raw_fd(fd: RawFd) -> TcpListener {
        TcpListener {
            inner: net::TcpListener::from_raw_fd(fd),
        }
    }
}

impl IntoRawFd for TcpListener {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_raw_fd()
    }
}

impl AsRawFd for TcpListener {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

