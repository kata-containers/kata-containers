use std::{
    io,
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    task::{Context, Poll},
};

use futures::{future::poll_fn, ready};
use log::trace;
use mio::{event::Evented, unix::EventedFd};
use tokio::io::PollEvented;

use crate::{Socket, SocketAddr};

impl Evented for Socket {
    fn register(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &mio::Poll,
        token: mio::Token,
        interest: mio::Ready,
        opts: mio::PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}

/// An I/O object representing a Netlink socket.
pub struct TokioSocket(PollEvented<Socket>);

impl TokioSocket {
    /// This function will create a new Netlink socket and attempt to bind it to
    /// the `addr` provided.
    pub fn bind(&mut self, addr: &SocketAddr) -> io::Result<()> {
        self.0.get_mut().bind(addr)
    }

    pub fn bind_auto(&mut self) -> io::Result<SocketAddr> {
        self.0.get_mut().bind_auto()
    }

    pub fn new(protocol: isize) -> io::Result<Self> {
        let socket = Socket::new(protocol)?;
        socket.set_non_blocking(true)?;
        Ok(TokioSocket(PollEvented::new(socket)?))
    }

    pub fn connect(&self, addr: &SocketAddr) -> io::Result<()> {
        self.0.get_ref().connect(addr)
    }

    pub async fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        poll_fn(|cx| {
            // Check if the socket it writable. If
            // PollEvented::poll_write_ready returns NotReady, it will
            // already have arranged for the current task to be
            // notified when the socket becomes writable, so we can
            // just return Pending
            ready!(self.0.poll_write_ready(cx))?;

            match self.0.get_ref().send(buf, 0) {
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    self.0.clear_write_ready(cx)?;
                    Poll::Pending
                }
                x => Poll::Ready(x),
            }
        })
        .await
    }

    pub async fn send_to(&mut self, buf: &[u8], addr: &SocketAddr) -> io::Result<usize> {
        poll_fn(|cx| self.poll_send_to(cx, buf, addr)).await
    }

    pub fn poll_send_to(
        &mut self,
        cx: &mut Context,
        buf: &[u8],
        addr: &SocketAddr,
    ) -> Poll<io::Result<usize>> {
        ready!(self.0.poll_write_ready(cx))?;
        match self.0.get_ref().send_to(buf, addr, 0) {
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.0.clear_write_ready(cx)?;
                Poll::Pending
            }
            x => Poll::Ready(x),
        }
    }

    pub async fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        poll_fn(|cx| {
            // Check if the socket is readable. If not,
            // PollEvented::poll_read_ready would have arranged for the
            // current task to be polled again when the socket becomes
            // readable, so we can just return Pending
            ready!(self.0.poll_read_ready(cx, mio::Ready::readable()))?;

            match self.0.get_ref().recv(buf, 0) {
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // If the socket is not readable, make sure the
                    // current task get notified when the socket becomes
                    // readable again.
                    self.0.clear_read_ready(cx, mio::Ready::readable())?;
                    Poll::Pending
                }
                x => Poll::Ready(x),
            }
        })
        .await
    }

    pub async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        poll_fn(|cx| self.poll_recv_from(cx, buf)).await
    }

    pub async fn recv_from_full(&mut self) -> io::Result<(Vec<u8>, SocketAddr)> {
        poll_fn(|cx| self.poll_recv_from_full(cx)).await
    }

    pub fn poll_recv_from(
        &mut self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        trace!("poll_recv_from called");
        ready!(self.0.poll_read_ready(cx, mio::Ready::readable()))?;

        trace!("poll_recv_from socket is ready for reading");
        match self.0.get_ref().recv_from(buf, 0) {
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                trace!("poll_recv_from socket would block");
                self.0.clear_read_ready(cx, mio::Ready::readable())?;
                Poll::Pending
            }
            x => {
                trace!("poll_recv_from {:?} bytes read", x);
                Poll::Ready(x)
            }
        }
    }

    pub fn poll_recv_from_full(
        &mut self,
        cx: &mut Context,
    ) -> Poll<io::Result<(Vec<u8>, SocketAddr)>> {
        trace!("poll_recv_from_full called");
        ready!(self.0.poll_read_ready(cx, mio::Ready::readable()))?;

        trace!("poll_recv_from_full socket is ready for reading");
        match self.0.get_ref().recv_from_full() {
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                trace!("poll_recv_from_full socket would block");
                self.0.clear_read_ready(cx, mio::Ready::readable())?;
                Poll::Pending
            }
            x => {
                trace!("poll_recv_from_full {:?} bytes read", x);
                Poll::Ready(x)
            }
        }
    }

    pub fn set_pktinfo(&mut self, value: bool) -> io::Result<()> {
        self.0.get_mut().set_pktinfo(value)
    }

    pub fn get_pktinfo(&self) -> io::Result<bool> {
        self.0.get_ref().get_pktinfo()
    }

    pub fn add_membership(&mut self, group: u32) -> io::Result<()> {
        self.0.get_mut().add_membership(group)
    }

    pub fn drop_membership(&mut self, group: u32) -> io::Result<()> {
        self.0.get_mut().drop_membership(group)
    }

    // pub fn list_membership(&self) -> Vec<u32> {
    //     self.0.get_ref().list_membership()
    // }

    /// `NETLINK_BROADCAST_ERROR` (since Linux 2.6.30). When not set, `netlink_broadcast()` only
    /// reports `ESRCH` errors and silently ignore `NOBUFS` errors.
    pub fn set_broadcast_error(&mut self, value: bool) -> io::Result<()> {
        self.0.get_mut().set_broadcast_error(value)
    }

    pub fn get_broadcast_error(&self) -> io::Result<bool> {
        self.0.get_ref().get_broadcast_error()
    }

    /// `NETLINK_NO_ENOBUFS` (since Linux 2.6.30). This flag can be used by unicast and broadcast
    /// listeners to avoid receiving `ENOBUFS` errors.
    pub fn set_no_enobufs(&mut self, value: bool) -> io::Result<()> {
        self.0.get_mut().set_no_enobufs(value)
    }

    pub fn get_no_enobufs(&self) -> io::Result<bool> {
        self.0.get_ref().get_no_enobufs()
    }

    /// `NETLINK_LISTEN_ALL_NSID` (since Linux 4.2). When set, this socket will receive netlink
    /// notifications from  all  network  namespaces that have an nsid assigned into the network
    /// namespace where the socket has been opened. The nsid is sent to user space via an ancillary
    /// data.
    pub fn set_listen_all_namespaces(&mut self, value: bool) -> io::Result<()> {
        self.0.get_mut().set_listen_all_namespaces(value)
    }

    pub fn get_listen_all_namespaces(&self) -> io::Result<bool> {
        self.0.get_ref().get_listen_all_namespaces()
    }

    /// `NETLINK_CAP_ACK` (since Linux 4.2). The kernel may fail to allocate the necessary room
    /// for the acknowledgment message back to user space.  This option trims off the payload of
    /// the original netlink message. The netlink message header is still included, so the user can
    /// guess from the sequence  number which message triggered the acknowledgment.
    pub fn set_cap_ack(&mut self, value: bool) -> io::Result<()> {
        self.0.get_mut().set_cap_ack(value)
    }

    pub fn get_cap_ack(&self) -> io::Result<bool> {
        self.0.get_ref().get_cap_ack()
    }
}

impl FromRawFd for TokioSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        let socket = Socket::from_raw_fd(fd);
        socket.set_non_blocking(true).unwrap();
        TokioSocket(PollEvented::new(socket).unwrap())
    }
}

impl AsRawFd for TokioSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.0.get_ref().as_raw_fd()
    }
}
