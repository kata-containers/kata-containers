// SPDX-License-Identifier: MIT

use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use crate::{AsyncSocket, SocketAddr};

/// Support trait for [`AsyncSocket`]
///
/// Provides awaitable variants of the poll functions from [`AsyncSocket`].
pub trait AsyncSocketExt: AsyncSocket {
    /// `async fn send(&mut self, buf: &[u8]) -> io::Result<usize>`
    fn send<'a, 'b>(&'a mut self, buf: &'b [u8]) -> PollSend<'a, 'b, Self> {
        PollSend { socket: self, buf }
    }

    /// `async fn send(&mut self, buf: &[u8]) -> io::Result<usize>`
    fn send_to<'a, 'b>(
        &'a mut self,
        buf: &'b [u8],
        addr: &'b SocketAddr,
    ) -> PollSendTo<'a, 'b, Self> {
        PollSendTo {
            socket: self,
            buf,
            addr,
        }
    }

    /// `async fn recv<B>(&mut self, buf: &mut [u8]) -> io::Result<()>`
    fn recv<'a, 'b, B>(&'a mut self, buf: &'b mut B) -> PollRecv<'a, 'b, Self, B>
    where
        B: bytes::BufMut,
    {
        PollRecv { socket: self, buf }
    }

    /// `async fn recv<B>(&mut self, buf: &mut [u8]) -> io::Result<SocketAddr>`
    fn recv_from<'a, 'b, B>(&'a mut self, buf: &'b mut B) -> PollRecvFrom<'a, 'b, Self, B>
    where
        B: bytes::BufMut,
    {
        PollRecvFrom { socket: self, buf }
    }

    /// `async fn recrecv_from_full(&mut self) -> io::Result<(Vec<u8>, SocketAddr)>`
    fn recv_from_full(&mut self) -> PollRecvFromFull<'_, Self> {
        PollRecvFromFull { socket: self }
    }
}

impl<S: AsyncSocket> AsyncSocketExt for S {}

pub struct PollSend<'a, 'b, S> {
    socket: &'a mut S,
    buf: &'b [u8],
}

impl<S> Future for PollSend<'_, '_, S>
where
    S: AsyncSocket,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this: &mut Self = Pin::into_inner(self);
        this.socket.poll_send(cx, this.buf)
    }
}

pub struct PollSendTo<'a, 'b, S> {
    socket: &'a mut S,
    buf: &'b [u8],
    addr: &'b SocketAddr,
}

impl<S> Future for PollSendTo<'_, '_, S>
where
    S: AsyncSocket,
{
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this: &mut Self = Pin::into_inner(self);
        this.socket.poll_send_to(cx, this.buf, this.addr)
    }
}

pub struct PollRecv<'a, 'b, S, B> {
    socket: &'a mut S,
    buf: &'b mut B,
}

impl<S, B> Future for PollRecv<'_, '_, S, B>
where
    S: AsyncSocket,
    B: bytes::BufMut,
{
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this: &mut Self = Pin::into_inner(self);
        this.socket.poll_recv(cx, this.buf)
    }
}

pub struct PollRecvFrom<'a, 'b, S, B> {
    socket: &'a mut S,
    buf: &'b mut B,
}

impl<S, B> Future for PollRecvFrom<'_, '_, S, B>
where
    S: AsyncSocket,
    B: bytes::BufMut,
{
    type Output = io::Result<SocketAddr>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this: &mut Self = Pin::into_inner(self);
        this.socket.poll_recv_from(cx, this.buf)
    }
}

pub struct PollRecvFromFull<'a, S> {
    socket: &'a mut S,
}

impl<S> Future for PollRecvFromFull<'_, S>
where
    S: AsyncSocket,
{
    type Output = io::Result<(Vec<u8>, SocketAddr)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this: &mut Self = Pin::into_inner(self);
        this.socket.poll_recv_from_full(cx)
    }
}
