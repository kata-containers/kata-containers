// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//! Async support for pipe or something has file descriptor

use nix::unistd;
use std::{
    fmt, io,
    io::{Read, Result, Write},
    mem,
    os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
    pin::Pin,
    task::{Context, Poll},
};

use futures::ready;
use tokio::io::{unix::AsyncFd, AsyncRead, AsyncWrite, ReadBuf};

fn set_nonblocking(fd: RawFd) {
    unsafe {
        libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK);
    }
}

struct StreamFd(RawFd);

impl io::Read for &StreamFd {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match unistd::read(self.0, buf) {
            Ok(l) => Ok(l),
            Err(e) => Err(e.as_errno().unwrap().into()),
        }
    }
}

impl io::Write for &StreamFd {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match unistd::write(self.0, buf) {
            Ok(l) => Ok(l),
            Err(e) => Err(e.as_errno().unwrap().into()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl StreamFd {
    fn close(&mut self) -> io::Result<()> {
        match unistd::close(self.0) {
            Ok(()) => Ok(()),
            Err(e) => Err(e.as_errno().unwrap().into()),
        }
    }
}

impl Drop for StreamFd {
    fn drop(&mut self) {
        self.close().ok();
    }
}

impl AsRawFd for StreamFd {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

pub struct PipeStream(AsyncFd<StreamFd>);

impl PipeStream {
    pub fn new(fd: RawFd) -> Result<Self> {
        set_nonblocking(fd);
        Ok(Self(AsyncFd::new(StreamFd(fd))?))
    }

    pub fn from_fd(fd: RawFd) -> Self {
        unsafe { Self::from_raw_fd(fd) }
    }
}

impl AsRawFd for PipeStream {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl IntoRawFd for PipeStream {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.as_raw_fd();
        mem::forget(self);
        fd
    }
}

impl FromRawFd for PipeStream {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self::new(fd).unwrap()
    }
}

impl fmt::Debug for PipeStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PipeStream({})", self.as_raw_fd())
    }
}

impl AsyncRead for PipeStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        let b;
        unsafe {
            b = &mut *(buf.unfilled_mut() as *mut [mem::MaybeUninit<u8>] as *mut [u8]);
        };

        loop {
            let mut guard = ready!(self.0.poll_read_ready(cx))?;

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

impl AsyncWrite for PipeStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        loop {
            let mut guard = ready!(self.0.poll_write_ready(cx))?;

            match guard.try_io(|inner| inner.get_ref().write(buf)) {
                Ok(result) => return Poll::Ready(result),
                Err(_would_block) => continue,
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Do nothing in shutdown is very important
        // The only right way to shutdown pipe is drop it
        // Otherwise PipeStream will conflict with its twins
        // Because they both have same fd, and both registered.
        Poll::Ready(Ok(()))
    }
}
