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

use std::io::{Error, Read, Result, Write};
use std::net::Shutdown;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use crate::{SockAddr, VsockAddr};
use futures::ready;
use libc::*;
use std::mem::{self, size_of};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// An I/O object representing a Virtio socket connected to a remote endpoint.
#[derive(Debug)]
pub struct VsockStream {
    inner: AsyncFd<vsock::VsockStream>,
}

impl VsockStream {
    pub(crate) fn new(connected: vsock::VsockStream) -> Result<Self> {
        connected.set_nonblocking(true)?;
        Ok(Self {
            inner: AsyncFd::new(connected)?,
        })
    }

    /// Open a connection to a remote host.
    pub async fn connect(cid: u32, port: u32) -> Result<Self> {
        let vsock_addr = VsockAddr::new(cid, port);

        let socket = unsafe { socket(AF_VSOCK, SOCK_STREAM | SOCK_CLOEXEC, 0) };
        if socket < 0 {
            return Err(Error::last_os_error());
        }

        if unsafe { fcntl(socket, F_SETFL, O_NONBLOCK) } < 0 {
            let _ = unsafe { close(socket) };
            return Err(Error::last_os_error());
        }

        if unsafe {
            connect(
                socket,
                &vsock_addr as *const _ as *const sockaddr,
                size_of::<sockaddr_vm>() as u32,
            )
        } < 0
        {
            let err = Error::last_os_error();
            if let Some(os_err) = err.raw_os_error() {
                // Connect hasn't finished, that's fine.
                if os_err != EINPROGRESS {
                    // Close the socket if we hit an error, ignoring the error
                    // from closing since we can't pass back two errors.
                    let _ = unsafe { close(socket) };
                    return Err(err);
                }
            }
        }

        loop {
            let stream = unsafe { vsock::VsockStream::from_raw_fd(socket) };
            let stream = Self::new(stream)?;
            let mut guard = stream.inner.writable().await?;

            match guard.try_io(|_| Ok(())) {
                Ok(_) => return Ok(stream),
                Err(_would_block) => continue,
            }
        }
    }

    /// The local address that this socket is bound to.
    pub fn local_addr(&self) -> Result<SockAddr> {
        self.inner.get_ref().local_addr()
    }

    /// The remote address that this socket is connected to.
    pub fn peer_addr(&self) -> Result<SockAddr> {
        self.inner.get_ref().peer_addr()
    }

    /// Shuts down the read, write, or both halves of this connection.
    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        self.inner.get_ref().shutdown(how)
    }

    pub(crate) fn poll_write_priv(&self, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        loop {
            let mut guard = ready!(self.inner.poll_write_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().write(buf)) {
                Ok(Ok(n)) => return Ok(n).into(),
                Ok(Err(e)) => return Err(e).into(),
                Err(_would_block) => continue,
            }
        }
    }

    pub(crate) fn poll_read_priv(
        &self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let b;
        unsafe {
            b = &mut *(buf.unfilled_mut() as *mut [mem::MaybeUninit<u8>] as *mut [u8]);
        };

        loop {
            let mut guard = ready!(self.inner.poll_read_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().read(b)) {
                Ok(Ok(n)) => {
                    unsafe {
                        buf.assume_init(n);
                    }
                    buf.advance(n);
                    return Ok(()).into();
                }
                Ok(Err(e)) => return Err(e).into(),
                Err(_would_block) => {
                    continue;
                }
            }
        }
    }
}

impl AsRawFd for VsockStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.get_ref().as_raw_fd()
    }
}

impl IntoRawFd for VsockStream {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.inner.get_ref().as_raw_fd();
        mem::forget(self);
        fd
    }
}

impl Write for VsockStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.inner.get_ref().write(buf)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Read for VsockStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.get_ref().read(buf)
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
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        self.poll_read_priv(cx, buf)
    }
}
