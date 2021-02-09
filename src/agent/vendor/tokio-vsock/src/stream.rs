/*
 * Tokio Reference TCP Implementation
 * Copyright (c) 2019 Tokio Contributors
 *
 * Permission is hereby granted, free of charge, to any
 * person obtaining a copy of this software and associated
 * documentation files (the "Software"), to deal in the
 * Software without restriction, including without
 * limitation the rights to use, copy, modify, merge,
 * publish, distribute, sublicense, and/or sell copies of
 * the Software, and to permit persons to whom the Software
 * is furnished to do so, subject to the following
 * conditions:
 *
 * The above copyright notice and this permission notice
 * shall be included in all copies or substantial portions
 * of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
 * ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
 * TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
 * PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
 * SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
 * CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
 * OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
 * IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

/*
 * Copyright 2019 fsyncd, Berlin, Germany.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::io::{ErrorKind, Read, Result, Write};
use std::net::Shutdown;
use std::os::unix::io::{AsRawFd, IntoRawFd, RawFd};

use futures::{future::poll_fn, ready};
use mio::Ready;
use nix::sys::socket::SockAddr;
use std::mem::{self, MaybeUninit};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::PollEvented;
use tokio::io::{AsyncRead, AsyncWrite};

/// An I/O object representing a Virtio socket connected to a remote endpoint.
#[derive(Debug)]
pub struct VsockStream {
    io: PollEvented<super::mio::VsockStream>,
}

impl VsockStream {
    pub(crate) fn new(connected: super::mio::VsockStream) -> Result<Self> {
        let io = PollEvented::new(connected)?;
        Ok(Self { io })
    }

    pub async fn connect(addr: &SockAddr) -> Result<Self> {
        let stream = super::mio::VsockStream::connect(addr)?;
        let stream = Self::new(stream)?;

        poll_fn(|cx| stream.io.poll_write_ready(cx)).await?;
        Ok(stream)
    }

    /// Create a new socket from an existing blocking socket.
    pub fn from_std(stream: vsock::VsockStream) -> Result<Self> {
        let io = super::mio::VsockStream::from_std(stream)?;
        let io = PollEvented::new(io)?;
        Ok(VsockStream { io })
    }

    /// Check the socket's read readiness state.
    pub fn poll_read_ready(&self, cx: &mut Context, mask: Ready) -> Poll<Result<Ready>> {
        self.io.poll_read_ready(cx, mask)
    }

    /// Check the socket's write readiness state.
    pub fn poll_write_ready(&self, cx: &mut Context) -> Poll<Result<Ready>> {
        self.io.poll_write_ready(cx)
    }

    /// The local address that this socket is bound to.
    pub fn local_addr(&self) -> Result<SockAddr> {
        self.io.get_ref().local_addr()
    }

    /// The remote address that this socket is connected to.
    pub fn peer_addr(&self) -> Result<SockAddr> {
        self.io.get_ref().peer_addr()
    }

    /// Shuts down the read, write, or both halves of this connection.
    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        self.io.get_ref().shutdown(how)
    }

    pub(crate) fn poll_write_priv(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        ready!(self.io.poll_write_ready(cx))?;

        match self.io.get_ref().write(buf) {
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                self.io.clear_write_ready(cx)?;
                Poll::Pending
            }
            x => Poll::Ready(x),
        }
    }

    pub(crate) fn poll_read_priv(
        &self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        ready!(self.io.poll_read_ready(cx, Ready::readable()))?;

        match self.io.get_ref().read(buf) {
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                self.io.clear_read_ready(cx, Ready::readable())?;
                Poll::Pending
            }
            x => Poll::Ready(x),
        }
    }
}

impl AsRawFd for VsockStream {
    fn as_raw_fd(&self) -> RawFd {
        self.io.get_ref().as_raw_fd()
    }
}

impl IntoRawFd for VsockStream {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.io.get_ref().as_raw_fd();
        mem::forget(self);
        fd
    }
}

impl Write for VsockStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.io.get_ref().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Read for VsockStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.io.get_ref().read(buf)
    }
}

impl AsyncWrite for VsockStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        self.poll_write_priv(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        self.shutdown(std::net::Shutdown::Write)?;
        Poll::Ready(Ok(()))
    }
}

impl AsyncRead for VsockStream {
    unsafe fn prepare_uninitialized_buffer(&self, _: &mut [MaybeUninit<u8>]) -> bool {
        false
    }

    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        self.poll_read_priv(cx, buf)
    }
}
