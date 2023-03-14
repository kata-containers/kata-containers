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

use std::io::Result;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use futures::{future::poll_fn, ready, stream::Stream};
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::unix::AsyncFd;

use crate::stream::VsockStream;
use crate::SockAddr;

/// An I/O object representing a Virtio socket listening for incoming connections.
#[derive(Debug)]
pub struct VsockListener {
    inner: AsyncFd<vsock::VsockListener>,
}

impl VsockListener {
    fn new(listener: vsock::VsockListener) -> Result<Self> {
        listener.set_nonblocking(true)?;
        Ok(Self {
            inner: AsyncFd::new(listener)?,
        })
    }

    /// Create a new Virtio socket listener associated with this event loop.
    pub fn bind(cid: u32, port: u32) -> Result<Self> {
        let l = vsock::VsockListener::bind_with_cid_port(cid, port)?;
        Self::new(l)
    }

    /// Accepts a new incoming connection to this listener.
    pub async fn accept(&mut self) -> Result<(VsockStream, SockAddr)> {
        poll_fn(|cx| self.poll_accept(cx)).await
    }

    /// Attempt to accept a connection and create a new connected socket if
    /// successful.
    pub fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<Result<(VsockStream, SockAddr)>> {
        let (inner, addr) = ready!(self.poll_accept_std(cx))?;
        let inner = VsockStream::new(inner)?;

        Ok((inner, addr)).into()
    }

    /// Attempt to accept a connection and create a new connected socket if
    /// successful.
    pub fn poll_accept_std(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(vsock::VsockStream, SockAddr)>> {
        loop {
            let mut guard = ready!(self.inner.poll_read_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().accept()) {
                Ok(Ok((inner, addr))) => return Ok((inner, addr)).into(),
                Ok(Err(e)) => return Err(e).into(),
                Err(_would_block) => continue,
            }
        }
    }

    /// The local address that this listener is bound to.
    pub fn local_addr(&self) -> Result<SockAddr> {
        self.inner.get_ref().local_addr()
    }

    /// Consumes this listener, returning a stream of the sockets this listener
    /// accepts.
    pub fn incoming(self) -> Incoming {
        Incoming::new(self)
    }
}

impl FromRawFd for VsockListener {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self::new(vsock::VsockListener::from_raw_fd(fd)).unwrap()
    }
}

impl AsRawFd for VsockListener {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.get_ref().as_raw_fd()
    }
}

impl IntoRawFd for VsockListener {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.inner.get_ref().as_raw_fd();
        mem::forget(self);
        fd
    }
}

/// Stream returned by the `VsockListener::incoming` representing sockets received from a listener.
#[derive(Debug)]
pub struct Incoming {
    inner: VsockListener,
}

impl Incoming {
    fn new(listener: VsockListener) -> Incoming {
        Incoming { inner: listener }
    }
}

impl Stream for Incoming {
    type Item = Result<VsockStream>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (socket, _) = ready!(self.inner.poll_accept(cx))?;
        Poll::Ready(Some(Ok(socket)))
    }
}

impl AsRawFd for Incoming {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}
