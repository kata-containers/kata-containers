// SPDX-License-Identifier: MIT

use std::{
    io,
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    task::{Context, Poll},
};

use futures::ready;
use log::trace;
use tokio::io::unix::AsyncFd;

use crate::{AsyncSocket, Socket, SocketAddr};

/// An I/O object representing a Netlink socket.
pub struct TokioSocket(AsyncFd<Socket>);

impl FromRawFd for TokioSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        let socket = Socket::from_raw_fd(fd);
        socket.set_non_blocking(true).unwrap();
        TokioSocket(AsyncFd::new(socket).unwrap())
    }
}

impl AsRawFd for TokioSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.0.get_ref().as_raw_fd()
    }
}

impl AsyncSocket for TokioSocket {
    fn socket_ref(&self) -> &Socket {
        self.0.get_ref()
    }

    /// Mutable access to underyling [`Socket`]
    fn socket_mut(&mut self) -> &mut Socket {
        self.0.get_mut()
    }

    fn new(protocol: isize) -> io::Result<Self> {
        let socket = Socket::new(protocol)?;
        socket.set_non_blocking(true)?;
        Ok(Self(AsyncFd::new(socket)?))
    }

    fn poll_send(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        loop {
            // Check if the socket it writable. If
            // AsyncFd::poll_write_ready returns NotReady, it will
            // already have arranged for the current task to be
            // notified when the socket becomes writable, so we can
            // just return Pending
            let mut guard = ready!(self.0.poll_write_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().send(buf, 0)) {
                Ok(x) => return Poll::Ready(x),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        addr: &SocketAddr,
    ) -> Poll<io::Result<usize>> {
        loop {
            let mut guard = ready!(self.0.poll_write_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().send_to(buf, addr, 0)) {
                Ok(x) => return Poll::Ready(x),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_recv<B>(&mut self, cx: &mut Context<'_>, buf: &mut B) -> Poll<io::Result<()>>
    where
        B: bytes::BufMut,
    {
        loop {
            // Check if the socket is readable. If not,
            // AsyncFd::poll_read_ready would have arranged for the
            // current task to be polled again when the socket becomes
            // readable, so we can just return Pending
            let mut guard = ready!(self.0.poll_read_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().recv(buf, 0)) {
                Ok(x) => return Poll::Ready(x.map(|_len| ())),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_recv_from<B>(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<SocketAddr>>
    where
        B: bytes::BufMut,
    {
        loop {
            trace!("poll_recv_from called");
            let mut guard = ready!(self.0.poll_read_ready(cx))?;
            trace!("poll_recv_from socket is ready for reading");

            match guard.try_io(|inner| inner.get_ref().recv_from(buf, 0)) {
                Ok(x) => {
                    trace!("poll_recv_from {:?} bytes read", x);
                    return Poll::Ready(x.map(|(_len, addr)| addr));
                }
                Err(_would_block) => {
                    trace!("poll_recv_from socket would block");
                    continue;
                }
            }
        }
    }

    fn poll_recv_from_full(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<(Vec<u8>, SocketAddr)>> {
        loop {
            trace!("poll_recv_from_full called");
            let mut guard = ready!(self.0.poll_read_ready(cx))?;
            trace!("poll_recv_from_full socket is ready for reading");

            match guard.try_io(|inner| inner.get_ref().recv_from_full()) {
                Ok(x) => {
                    trace!("poll_recv_from_full {:?} bytes read", x);
                    return Poll::Ready(x);
                }
                Err(_would_block) => {
                    trace!("poll_recv_from_full socket would block");
                    continue;
                }
            }
        }
    }
}
