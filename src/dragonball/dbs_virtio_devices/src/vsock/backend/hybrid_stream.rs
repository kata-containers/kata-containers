// Copyright 2023 Ant Group. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::Any;
use std::io::{Error, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

use log::error;
use nix::errno::Errno;

use super::{VsockBackendType, VsockStream};

pub struct HybridStream {
    pub hybrid_stream: std::fs::File,
    pub slave_stream: Option<Box<dyn VsockStream>>,
}

impl AsRawFd for HybridStream {
    fn as_raw_fd(&self) -> RawFd {
        self.hybrid_stream.as_raw_fd()
    }
}

impl Read for HybridStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.hybrid_stream.read(buf)
    }
}

impl Write for HybridStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // The slave stream was only used to reply the connect result "ok <port>",
        // thus it was only used once here, and the data would be replied by the
        // main stream.
        if let Some(mut stream) = self.slave_stream.take() {
            stream.write(buf)
        } else {
            self.hybrid_stream.write(buf)
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.hybrid_stream.flush()
    }
}

impl VsockStream for HybridStream {
    fn backend_type(&self) -> VsockBackendType {
        VsockBackendType::HybridStream
    }

    fn set_nonblocking(&mut self, nonblocking: bool) -> std::io::Result<()> {
        let fd = self.hybrid_stream.as_raw_fd();
        let mut flag = unsafe { libc::fcntl(fd, libc::F_GETFL) };

        if nonblocking {
            flag |= libc::O_NONBLOCK;
        } else {
            flag |= !libc::O_NONBLOCK;
        }

        let ret = unsafe { libc::fcntl(fd, libc::F_SETFL, flag) };

        if ret < 0 {
            error!("failed to set fcntl for fd {} with ret {}", fd, ret);
            return Err(Error::last_os_error());
        }

        Ok(())
    }

    fn set_read_timeout(&mut self, _dur: Option<Duration>) -> std::io::Result<()> {
        error!("unsupported!");
        Err(Errno::ENOPROTOOPT.into())
    }

    fn set_write_timeout(&mut self, _dur: Option<Duration>) -> std::io::Result<()> {
        error!("unsupported!");
        Err(Errno::ENOPROTOOPT.into())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn recv_data_fd(
        &self,
        _bytes: &mut [u8],
        _fds: &mut [RawFd],
    ) -> std::io::Result<(usize, usize)> {
        Err(Errno::ENOPROTOOPT.into())
    }
}
