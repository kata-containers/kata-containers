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

use std::cmp;
use std::io::{self, Error, ErrorKind, IoSlice, IoSliceMut, Read, Result, Write};
use std::mem::size_of;
use std::net::Shutdown;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use libc::*;
use mio::unix::EventedFd;
use mio::{Evented, Poll, PollOpt, Ready, Token};
use nix::sys::socket::SockAddr;

use super::iovec::unix as iovec;
use super::iovec::IoVec;

#[derive(Debug)]
pub struct VsockStream {
    inner: vsock::VsockStream,
}

impl VsockStream {
    pub fn connect(addr: &SockAddr) -> Result<VsockStream> {
        let vsock_addr = if let SockAddr::Vsock(addr) = addr {
            addr.0
        } else {
            return Err(Error::new(
                ErrorKind::Other,
                "requires a virtio socket address",
            ));
        };

        let socket = unsafe { socket(AF_VSOCK, SOCK_STREAM, 0) };
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

        Ok(Self {
            inner: unsafe { vsock::VsockStream::from_raw_fd(socket) },
        })
    }

    pub fn from_std(inner: vsock::VsockStream) -> Result<VsockStream> {
        inner.set_nonblocking(true)?;
        Ok(VsockStream { inner })
    }

    pub fn peer_addr(&self) -> Result<SockAddr> {
        self.inner.peer_addr()
    }

    pub fn local_addr(&self) -> Result<SockAddr> {
        self.inner.local_addr()
    }

    pub fn try_clone(&self) -> Result<VsockStream> {
        self.inner.try_clone().map(|s| VsockStream { inner: s })
    }

    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        self.inner.shutdown(how)
    }

    pub fn take_error(&self) -> Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Read in a list of buffers all at once.
    ///
    /// This operation will attempt to read bytes from this socket and place
    /// them into the list of buffers provided. Note that each buffer is an
    /// `IoVec` which can be created from a byte slice.
    ///
    /// The buffers provided will be filled in sequentially. A buffer will be
    /// entirely filled up before the next is written to.
    ///
    /// The number of bytes read is returned, if successful, or an error is
    /// returned otherwise. If no bytes are available to be read yet then
    /// a "would block" error is returned. This operation does not block.
    ///
    /// On Unix this corresponds to the `readv` syscall.
    pub fn read_bufs(&self, bufs: &mut [&mut IoVec]) -> Result<usize> {
        unsafe {
            let slice = iovec::as_os_slice_mut(bufs);
            let len = cmp::min(<libc::c_int>::max_value() as usize, slice.len());
            let rc = readv(self.as_raw_fd(), slice.as_ptr(), len as libc::c_int);
            if rc < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(rc as usize)
            }
        }
    }

    /// Write a list of buffers all at once.
    ///
    /// This operation will attempt to write a list of byte buffers to this
    /// socket. Note that each buffer is an `IoVec` which can be created from a
    /// byte slice.
    ///
    /// The buffers provided will be written sequentially. A buffer will be
    /// entirely written before the next is written.
    ///
    /// The number of bytes written is returned, if successful, or an error is
    /// returned otherwise. If the socket is not currently writable then a
    /// "would block" error is returned. This operation does not block.
    ///
    /// On Unix this corresponds to the `writev` syscall.
    pub fn write_bufs(&self, bufs: &[&IoVec]) -> Result<usize> {
        unsafe {
            let slice = iovec::as_os_slice(bufs);
            let len = cmp::min(<libc::c_int>::max_value() as usize, slice.len());
            let rc = writev(self.as_raw_fd(), slice.as_ptr(), len as libc::c_int);
            if rc < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(rc as usize)
            }
        }
    }
}

impl<'a> Read for &'a VsockStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        (&self.inner).read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        (&self.inner).read_vectored(bufs)
    }
}

impl<'a> Write for &'a VsockStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        (&self.inner).write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        (&self.inner).write_vectored(bufs)
    }

    fn flush(&mut self) -> Result<()> {
        (&self.inner).flush()
    }
}

impl Read for VsockStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        <&Self>::read(&mut &*self, buf)
    }
}

impl Write for VsockStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        <&Self>::write(&mut &*self, buf)
    }

    fn flush(&mut self) -> Result<()> {
        <&Self>::flush(&mut &*self)
    }
}

impl FromRawFd for VsockStream {
    unsafe fn from_raw_fd(fd: RawFd) -> VsockStream {
        VsockStream {
            inner: vsock::VsockStream::from_raw_fd(fd),
        }
    }
}

impl IntoRawFd for VsockStream {
    fn into_raw_fd(self) -> RawFd {
        self.inner.into_raw_fd()
    }
}

impl AsRawFd for VsockStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl Evented for VsockStream {
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
