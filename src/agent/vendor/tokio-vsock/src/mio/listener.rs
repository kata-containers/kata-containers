/*
 * MIO Reference TCP Implementation
 * Copyright (c) 2014 Carl Lerche and other MIO contributors
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
 * THE SOFTWARE.
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

use mio::unix::EventedFd;
use mio::{Evented, Poll, PollOpt, Ready, Token};
use nix::sys::socket::SockAddr;

#[derive(Debug)]
pub struct VsockListener {
    inner: vsock::VsockListener,
}

impl VsockListener {
    pub fn bind(addr: &SockAddr) -> Result<Self> {
        let vsock_listener = vsock::VsockListener::bind(addr)?;
        Ok(Self {
            inner: vsock_listener,
        })
    }

    pub fn from_std(inner: vsock::VsockListener) -> Result<Self> {
        inner.set_nonblocking(true)?;
        Ok(Self { inner })
    }

    pub fn local_addr(&self) -> Result<SockAddr> {
        self.inner.local_addr()
    }

    /// This method is the same as `accept`, except that it returns a Virtio socket
    /// *in blocking mode* which isn't bound to `mio`. This can be later then
    /// converted to a `mio` type, if necessary.
    pub fn accept_std(&self) -> Result<(vsock::VsockStream, SockAddr)> {
        self.inner.accept()
    }
}

impl FromRawFd for VsockListener {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self {
            inner: vsock::VsockListener::from_raw_fd(fd),
        }
    }
}

impl IntoRawFd for VsockListener {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_raw_fd()
    }
}

impl AsRawFd for VsockListener {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl Evented for VsockListener {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> Result<()> {
        EventedFd(&self.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> Result<()> {
        EventedFd(&self.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> Result<()> {
        EventedFd(&self.as_raw_fd()).deregister(poll)
    }
}
