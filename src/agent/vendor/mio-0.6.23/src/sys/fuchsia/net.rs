use {io, Evented, Ready, Poll, PollOpt, Token};
use iovec::IoVec;
use iovec::unix as iovec;
use libc;
use net2::TcpStreamExt;
#[allow(unused_imports)] // only here for Rust 1.8
use net2::UdpSocketExt;
use sys::fuchsia::{recv_from, set_nonblock, EventedFd, DontDrop};
use std::cmp;
use std::io::{Read, Write};
use std::net::{self, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::os::unix::io::AsRawFd;
use std::time::Duration;

#[derive(Debug)]
pub struct TcpStream {
    io: DontDrop<net::TcpStream>,
    evented_fd: EventedFd,
}

impl TcpStream {
    pub fn connect(stream: net::TcpStream, addr: &SocketAddr) -> io::Result<TcpStream> {
        try!(set_nonblock(stream.as_raw_fd()));

        let connected = stream.connect(addr);
        match connected {
            Ok(..) => {}
            Err(ref e) if e.raw_os_error() == Some(libc::EINPROGRESS) => {}
            Err(e) => return Err(e),
        }

        let evented_fd = unsafe { EventedFd::new(stream.as_raw_fd()) };

        return Ok(TcpStream {
            io: DontDrop::new(stream),
            evented_fd: evented_fd,
        })
    }

    pub fn from_stream(stream: net::TcpStream) -> TcpStream {
        let evented_fd = unsafe { EventedFd::new(stream.as_raw_fd()) };

        TcpStream {
            io: DontDrop::new(stream),
            evented_fd: evented_fd,
        }
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.io.peer_addr()
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.local_addr()
    }

    pub fn try_clone(&self) -> io::Result<TcpStream> {
        self.io.try_clone().map(|s| {
            let evented_fd = unsafe { EventedFd::new(s.as_raw_fd()) };
            TcpStream {
                io: DontDrop::new(s),
                evented_fd: evented_fd,
            }
        })
    }

    pub fn shutdown(&self, how: net::Shutdown) -> io::Result<()> {
        self.io.shutdown(how)
    }

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.io.set_nodelay(nodelay)
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        self.io.nodelay()
    }

    pub fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        self.io.set_recv_buffer_size(size)
    }

    pub fn recv_buffer_size(&self) -> io::Result<usize> {
        self.io.recv_buffer_size()
    }

    pub fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        self.io.set_send_buffer_size(size)
    }

    pub fn send_buffer_size(&self) -> io::Result<usize> {
        self.io.send_buffer_size()
    }

    pub fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()> {
        self.io.set_keepalive(keepalive)
    }

    pub fn keepalive(&self) -> io::Result<Option<Duration>> {
        self.io.keepalive()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.io.set_ttl(ttl)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.io.ttl()
    }

    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.io.set_only_v6(only_v6)
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        self.io.only_v6()
    }

    pub fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        self.io.set_linger(dur)
    }

    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.io.linger()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.io.take_error()
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.peek(buf)
    }

    pub fn readv(&self, bufs: &mut [&mut IoVec]) -> io::Result<usize> {
        unsafe {
            let slice = iovec::as_os_slice_mut(bufs);
            let len = cmp::min(<libc::c_int>::max_value() as usize, slice.len());
            let rc = libc::readv(self.io.as_raw_fd(),
                                 slice.as_ptr(),
                                 len as libc::c_int);
            if rc < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(rc as usize)
            }
        }
    }

    pub fn writev(&self, bufs: &[&IoVec]) -> io::Result<usize> {
        unsafe {
            let slice = iovec::as_os_slice(bufs);
            let len = cmp::min(<libc::c_int>::max_value() as usize, slice.len());
            let rc = libc::writev(self.io.as_raw_fd(),
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

impl<'a> Read for &'a TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.inner_ref().read(buf)
    }
}

impl<'a> Write for &'a TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.inner_ref().write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.io.inner_ref().flush()
    }
}

impl Evented for TcpStream {
    fn register(&self,
                poll: &Poll,
                token: Token,
                interest: Ready,
                opts: PollOpt) -> io::Result<()>
    {
        self.evented_fd.register(poll, token, interest, opts)
    }

    fn reregister(&self,
                  poll: &Poll,
                  token: Token,
                  interest: Ready,
                  opts: PollOpt) -> io::Result<()>
    {
        self.evented_fd.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.evented_fd.deregister(poll)
    }
}

#[derive(Debug)]
pub struct TcpListener {
    io: DontDrop<net::TcpListener>,
    evented_fd: EventedFd,
}

impl TcpListener {
    pub fn new(inner: net::TcpListener) -> io::Result<TcpListener> {
        set_nonblock(inner.as_raw_fd())?;

        let evented_fd = unsafe { EventedFd::new(inner.as_raw_fd()) };

        Ok(TcpListener {
            io: DontDrop::new(inner),
            evented_fd: evented_fd,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.local_addr()
    }

    pub fn try_clone(&self) -> io::Result<TcpListener> {
        self.io.try_clone().map(|io| {
            let evented_fd = unsafe { EventedFd::new(io.as_raw_fd()) };
            TcpListener {
                io: DontDrop::new(io),
                evented_fd: evented_fd,
            }
        })
    }

    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        self.io.accept().and_then(|(s, a)| {
            set_nonblock(s.as_raw_fd())?;
            let evented_fd = unsafe { EventedFd::new(s.as_raw_fd()) };
            return Ok((TcpStream {
                io: DontDrop::new(s),
                evented_fd: evented_fd,
            }, a))
        })
    }

    #[allow(deprecated)]
    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.io.set_only_v6(only_v6)
    }

    #[allow(deprecated)]
    pub fn only_v6(&self) -> io::Result<bool> {
        self.io.only_v6()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.io.set_ttl(ttl)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.io.ttl()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.io.take_error()
    }
}

impl Evented for TcpListener {
    fn register(&self,
                poll: &Poll,
                token: Token,
                interest: Ready,
                opts: PollOpt) -> io::Result<()>
    {
        self.evented_fd.register(poll, token, interest, opts)
    }

    fn reregister(&self,
                  poll: &Poll,
                  token: Token,
                  interest: Ready,
                  opts: PollOpt) -> io::Result<()>
    {
        self.evented_fd.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.evented_fd.deregister(poll)
    }
}

#[derive(Debug)]
pub struct UdpSocket {
    io: DontDrop<net::UdpSocket>,
    evented_fd: EventedFd,
}

impl UdpSocket {
    pub fn new(socket: net::UdpSocket) -> io::Result<UdpSocket> {
        set_nonblock(socket.as_raw_fd())?;

        let evented_fd = unsafe { EventedFd::new(socket.as_raw_fd()) };

        Ok(UdpSocket {
            io: DontDrop::new(socket),
            evented_fd: evented_fd,
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.io.local_addr()
    }

    pub fn try_clone(&self) -> io::Result<UdpSocket> {
        self.io.try_clone().and_then(|io| {
            UdpSocket::new(io)
        })
    }

    pub fn send_to(&self, buf: &[u8], target: &SocketAddr) -> io::Result<usize> {
        self.io.send_to(buf, target)
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        unsafe { recv_from(self.io.as_raw_fd(), buf) }
    }

    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.io.send(buf)
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.recv(buf)
    }

    pub fn connect(&self, addr: SocketAddr)
                   -> io::Result<()> {
        self.io.connect(addr)
    }

    pub fn broadcast(&self) -> io::Result<bool> {
        self.io.broadcast()
    }

    pub fn set_broadcast(&self, on: bool) -> io::Result<()> {
        self.io.set_broadcast(on)
    }

    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.io.multicast_loop_v4()
    }

    pub fn set_multicast_loop_v4(&self, on: bool) -> io::Result<()> {
        self.io.set_multicast_loop_v4(on)
    }

    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.io.multicast_ttl_v4()
    }

    pub fn set_multicast_ttl_v4(&self, ttl: u32) -> io::Result<()> {
        self.io.set_multicast_ttl_v4(ttl)
    }

    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.io.multicast_loop_v6()
    }

    pub fn set_multicast_loop_v6(&self, on: bool) -> io::Result<()> {
        self.io.set_multicast_loop_v6(on)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        self.io.ttl()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.io.set_ttl(ttl)
    }

    pub fn join_multicast_v4(&self,
                             multiaddr: &Ipv4Addr,
                             interface: &Ipv4Addr) -> io::Result<()> {
        self.io.join_multicast_v4(multiaddr, interface)
    }

    pub fn join_multicast_v6(&self,
                             multiaddr: &Ipv6Addr,
                             interface: u32) -> io::Result<()> {
        self.io.join_multicast_v6(multiaddr, interface)
    }

    pub fn leave_multicast_v4(&self,
                              multiaddr: &Ipv4Addr,
                              interface: &Ipv4Addr) -> io::Result<()> {
        self.io.leave_multicast_v4(multiaddr, interface)
    }

    pub fn leave_multicast_v6(&self,
                              multiaddr: &Ipv6Addr,
                              interface: u32) -> io::Result<()> {
        self.io.leave_multicast_v6(multiaddr, interface)
    }

    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.io.set_only_v6(only_v6)
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        self.io.only_v6()
    }


    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.io.take_error()
    }
}

impl Evented for UdpSocket {
    fn register(&self,
                poll: &Poll,
                token: Token,
                interest: Ready,
                opts: PollOpt) -> io::Result<()>
    {
        self.evented_fd.register(poll, token, interest, opts)
    }

    fn reregister(&self,
                  poll: &Poll,
                  token: Token,
                  interest: Ready,
                  opts: PollOpt) -> io::Result<()>
    {
        self.evented_fd.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.evented_fd.deregister(poll)
    }
}
