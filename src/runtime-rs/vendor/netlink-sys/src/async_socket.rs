// SPDX-License-Identifier: MIT

use std::{
    io,
    task::{Context, Poll},
};

use crate::{Socket, SocketAddr};

/// Trait to support different async backends
pub trait AsyncSocket: Sized + Unpin {
    /// Access underyling [`Socket`]
    fn socket_ref(&self) -> &Socket;

    /// Mutable access to underyling [`Socket`]
    fn socket_mut(&mut self) -> &mut Socket;

    /// Wrapper for [`Socket::new`]
    fn new(protocol: isize) -> io::Result<Self>;

    /// Polling wrapper for [`Socket::send`]
    fn poll_send(&mut self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>>;

    /// Polling wrapper for [`Socket::send_to`]
    fn poll_send_to(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        addr: &SocketAddr,
    ) -> Poll<io::Result<usize>>;

    /// Polling wrapper for [`Socket::recv`]
    ///
    /// Passes 0 for flags, and ignores the returned length (the buffer will have advanced by the amount read).
    fn poll_recv<B>(&mut self, cx: &mut Context<'_>, buf: &mut B) -> Poll<io::Result<()>>
    where
        B: bytes::BufMut;

    /// Polling wrapper for [`Socket::recv_from`]
    ///
    /// Passes 0 for flags, and ignores the returned length - just returns the address (the buffer will have advanced by the amount read).
    fn poll_recv_from<B>(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut B,
    ) -> Poll<io::Result<SocketAddr>>
    where
        B: bytes::BufMut;

    /// Polling wrapper for [`Socket::recv_from_full`]
    ///
    /// Passes 0 for flags, and ignores the returned length - just returns the address (the buffer will have advanced by the amount read).
    fn poll_recv_from_full(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<(Vec<u8>, SocketAddr)>>;
}
