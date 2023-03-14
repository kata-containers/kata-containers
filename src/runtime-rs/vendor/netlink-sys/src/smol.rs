// SPDX-License-Identifier: MIT

use std::{
    io,
    os::unix::io::{AsRawFd, FromRawFd, RawFd},
    task::{Context, Poll},
};

use async_io::Async;

use futures::ready;

use log::trace;

use crate::{AsyncSocket, Socket, SocketAddr};

/// An I/O object representing a Netlink socket.
pub struct SmolSocket(Async<Socket>);

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

// async_io::Async<..>::{read,write}_with[_mut] functions try IO first,
// and only register context if it would block.
// replicate this in these poll functions:
impl SmolSocket {
    fn poll_write_with<F, R>(&mut self, cx: &mut Context<'_>, mut op: F) -> Poll<io::Result<R>>
    where
        F: FnMut(&mut Self) -> io::Result<R>,
    {
        loop {
            match op(self) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            // try again if writable now, otherwise come back later:
            ready!(self.0.poll_writable(cx))?;
        }
    }

    fn poll_read_with<F, R>(&mut self, cx: &mut Context<'_>, mut op: F) -> Poll<io::Result<R>>
    where
        F: FnMut(&mut Self) -> io::Result<R>,
    {
        loop {
            match op(self) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return Poll::Ready(res),
            }
            // try again if readable now, otherwise come back later:
            ready!(self.0.poll_readable(cx))?;
        }
    }
}

impl AsyncSocket for SmolSocket {
    fn socket_ref(&self) -> &Socket {
        self.0.get_ref()
    }

    /// Mutable access to underyling [`Socket`]
    fn socket_mut(&mut self) -> &mut Socket {
        self.0.get_mut()
    }

    fn new(protocol: isize) -> io::Result<Self> {
        let socket = Socket::new(protocol)?;
        Ok(Self(Async::new(socket)?))
    }

    fn poll_send(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        self.poll_write_with(cx, |this| this.0.get_mut().send(buf, 0))
    }

    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        addr: &SocketAddr,
    ) -> Poll<io::Result<usize>> {
        self.poll_write_with(cx, |this| this.0.get_mut().send_to(buf, addr, 0))
    }

    fn poll_recv<B>(&mut self, cx: &mut Context<'_>, buf: &mut B) -> Poll<io::Result<()>>
    where
        B: bytes::BufMut,
    {
        self.poll_read_with(cx, |this| this.0.get_mut().recv(buf, 0).map(|_len| ()))
    }

    fn poll_recv_from<B>(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<SocketAddr>>
    where
        B: bytes::BufMut,
    {
        self.poll_read_with(cx, |this| {
            let x = this.0.get_mut().recv_from(buf, 0);
            trace!("poll_recv_from: {:?}", x);
            x.map(|(_len, addr)| addr)
        })
    }

    fn poll_recv_from_full(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<(Vec<u8>, SocketAddr)>> {
        self.poll_read_with(cx, |this| this.0.get_mut().recv_from_full())
    }
}
