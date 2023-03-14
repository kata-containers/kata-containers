use std::{
    io,
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    task::{Context, Poll},
};

use async_io::Async;

use futures::ready;

use log::trace;

use crate::{Socket, SocketAddr};

/// An I/O object representing a Netlink socket.
pub struct SmolSocket(Async<Socket>);

impl SmolSocket {
    pub fn new(protocol: isize) -> io::Result<Self> {
        let socket = Socket::new(protocol)?;
        Ok(SmolSocket(Async::new(socket)?))
    }

    pub fn bind(&mut self, addr: &SocketAddr) -> io::Result<()> {
        self.0.get_mut().bind(addr)
    }

    pub fn bind_auto(&mut self) -> io::Result<SocketAddr> {
        self.0.get_mut().bind_auto()
    }

    pub fn connect(&self, addr: &SocketAddr) -> io::Result<()> {
        self.0.get_ref().connect(addr)
    }

    pub async fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write_with_mut(|sock| sock.send(buf, 0)).await
    }

    pub async fn send_to(&mut self, buf: &[u8], addr: &SocketAddr) -> io::Result<usize> {
        self.0
            .write_with_mut(|sock| sock.send_to(buf, addr, 0))
            .await
    }

    pub async fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read_with_mut(|sock| sock.recv(buf, 0)).await
    }

    pub async fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.0.read_with_mut(|sock| sock.recv_from(buf, 0)).await
    }

    pub async fn recv_from_full(&mut self) -> io::Result<(Vec<u8>, SocketAddr)> {
        self.0.read_with_mut(|sock| sock.recv_from_full()).await
    }

    pub fn poll_recv_from(
        &mut self,
        cx: &mut Context,
        buf: &mut [u8],
    ) -> Poll<io::Result<(usize, SocketAddr)>> {
        loop {
            trace!("poll_recv_from called");
            let _guard = ready!(self.0.poll_readable(cx))?;
            trace!("poll_recv_from socket is ready for reading");

            match self.0.get_ref().recv_from(buf, 0) {
                Ok(x) => {
                    trace!("poll_recv_from {:?} bytes read", x);
                    return Poll::Ready(Ok(x));
                }
                Err(_would_block) => {
                    trace!("poll_recv_from socket would block");
                    continue;
                }
            }
        }
    }

    pub fn poll_send_to(
        &mut self,
        cx: &mut Context,
        buf: &[u8],
        addr: &SocketAddr,
    ) -> Poll<io::Result<usize>> {
        loop {
            let _guard = ready!(self.0.poll_writable(cx))?;

            match self.0.get_ref().send_to(buf, addr, 0) {
                Ok(x) => return Poll::Ready(Ok(x)),
                Err(_would_block) => continue,
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

impl FromRawFd for SmolSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        let socket = Socket::from_raw_fd(fd);
        socket.set_non_blocking(true).unwrap();
        SmolSocket(Async::new(socket).unwrap())
    }
}

impl AsRawFd for SmolSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.0.get_ref().as_raw_fd()
    }
}
