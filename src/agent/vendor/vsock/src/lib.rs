/*
 * Copyright 2019 fsyncd, Berlin, Germany.
 * Additional material Copyright the Rust project and it's contributors.
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

//! Virtio socket support for Rust.

use std::io::{Error, ErrorKind, Read, Result, Write};
use std::mem::size_of;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, RawFd};

use libc::*;
use nix::sys::socket::{SockAddr, VsockAddr};
use std::ffi::c_void;
use std::net::Shutdown;
use std::time::Duration;

fn new_socket() -> libc::c_int {
    unsafe { socket(AF_VSOCK, SOCK_STREAM | SOCK_CLOEXEC, 0) }
}

/// An iterator that infinitely accepts connections on a VsockListener.
#[derive(Debug)]
pub struct Incoming<'a> {
    listener: &'a VsockListener,
}

impl<'a> Iterator for Incoming<'a> {
    type Item = Result<VsockStream>;

    fn next(&mut self) -> Option<Result<VsockStream>> {
        Some(self.listener.accept().map(|p| p.0))
    }
}

/// A virtio socket server, listening for connections.
#[derive(Debug, Clone)]
pub struct VsockListener {
    socket: RawFd,
}

impl VsockListener {
    /// Create a new VsockListener which is bound and listening on the socket address.
    pub fn bind(addr: &SockAddr) -> Result<VsockListener> {
        let mut vsock_addr = if let SockAddr::Vsock(addr) = addr {
            addr.0
        } else {
            return Err(Error::new(
                ErrorKind::Other,
                "requires a virtio socket address",
            ));
        };

        let socket = new_socket();
        if socket < 0 {
            return Err(Error::last_os_error());
        }

        let res = unsafe {
            bind(
                socket,
                &mut vsock_addr as *mut _ as *mut sockaddr,
                size_of::<sockaddr_vm>() as u32,
            )
        };
        if res < 0 {
            return Err(Error::last_os_error());
        }

        // rust stdlib uses a 128 connection backlog
        let res = unsafe { listen(socket, 128) };
        if res < 0 {
            return Err(Error::last_os_error());
        }

        Ok(Self { socket })
    }

    /// The local socket address of the listener.
    pub fn local_addr(&self) -> Result<SockAddr> {
        let mut vsock_addr = sockaddr_vm {
            svm_family: AF_VSOCK as sa_family_t,
            svm_reserved1: 0,
            svm_port: 0,
            svm_cid: 0,
            svm_zero: [0u8; 4],
        };
        let mut vsock_addr_len = size_of::<sockaddr_vm>() as socklen_t;
        if unsafe {
            getsockname(
                self.socket,
                &mut vsock_addr as *mut _ as *mut sockaddr,
                &mut vsock_addr_len,
            )
        } < 0
        {
            Err(Error::last_os_error())
        } else {
            Ok(SockAddr::Vsock(VsockAddr(vsock_addr)))
        }
    }

    /// Create a new independently owned handle to the underlying socket.
    pub fn try_clone(&self) -> Result<Self> {
        Ok(self.clone())
    }

    /// Accept a new incoming connection from this listener.
    pub fn accept(&self) -> Result<(VsockStream, SockAddr)> {
        let mut vsock_addr = sockaddr_vm {
            svm_family: AF_VSOCK as sa_family_t,
            svm_reserved1: 0,
            svm_port: 0,
            svm_cid: 0,
            svm_zero: [0u8; 4],
        };
        let mut vsock_addr_len = size_of::<sockaddr_vm>() as socklen_t;
        let socket = unsafe {
            accept(
                self.socket,
                &mut vsock_addr as *mut _ as *mut sockaddr,
                &mut vsock_addr_len,
            )
        };
        if socket < 0 {
            Err(Error::last_os_error())
        } else {
            Ok((
                unsafe { VsockStream::from_raw_fd(socket as RawFd) },
                SockAddr::Vsock(VsockAddr::new(vsock_addr.svm_cid, vsock_addr.svm_port)),
            ))
        }
    }

    /// An iterator over the connections being received on this listener.
    pub fn incoming(&self) -> Incoming {
        Incoming { listener: self }
    }

    /// Retrieve the latest error associated with the underlying socket.
    pub fn take_error(&self) -> Result<Option<Error>> {
        let mut error: i32 = 0;
        let mut error_len: socklen_t = 0;
        if unsafe {
            getsockopt(
                self.socket,
                SOL_SOCKET,
                SO_ERROR,
                &mut error as *mut _ as *mut c_void,
                &mut error_len,
            )
        } < 0
        {
            Err(Error::last_os_error())
        } else {
            Ok(if error == 0 {
                None
            } else {
                Some(Error::from_raw_os_error(error))
            })
        }
    }

    /// Move this stream in and out of nonblocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<()> {
        let mut nonblocking: i32 = if nonblocking { 1 } else { 0 };
        if unsafe { ioctl(self.socket, FIONBIO, &mut nonblocking) } < 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

impl AsRawFd for VsockListener {
    fn as_raw_fd(&self) -> RawFd {
        self.socket
    }
}

impl FromRawFd for VsockListener {
    unsafe fn from_raw_fd(socket: RawFd) -> Self {
        Self { socket }
    }
}

impl IntoRawFd for VsockListener {
    fn into_raw_fd(self) -> RawFd {
        self.socket
    }
}

impl Drop for VsockListener {
    fn drop(&mut self) {
        unsafe { close(self.socket) };
    }
}

/// A virtio stream between a local and a remote socket.
#[derive(Debug, Clone)]
pub struct VsockStream {
    socket: RawFd,
}

impl VsockStream {
    /// Open a connection to a remote host.
    pub fn connect(addr: &SockAddr) -> Result<Self> {
        let vsock_addr = if let SockAddr::Vsock(addr) = addr {
            addr.0
        } else {
            return Err(Error::new(
                ErrorKind::Other,
                "requires a virtio socket address",
            ));
        };

        let sock = new_socket();
        if sock < 0 {
            return Err(Error::last_os_error());
        }
        if unsafe {
            connect(
                sock,
                &vsock_addr as *const _ as *const sockaddr,
                size_of::<sockaddr_vm>() as u32,
            )
        } < 0
        {
            Err(Error::last_os_error())
        } else {
            Ok(unsafe { VsockStream::from_raw_fd(sock) })
        }
    }

    /// Virtio socket address of the remote peer associated with this connection.
    pub fn peer_addr(&self) -> Result<SockAddr> {
        let mut vsock_addr = sockaddr_vm {
            svm_family: AF_VSOCK as sa_family_t,
            svm_reserved1: 0,
            svm_port: 0,
            svm_cid: 0,
            svm_zero: [0u8; 4],
        };
        let mut vsock_addr_len = size_of::<sockaddr_vm>() as socklen_t;
        if unsafe {
            getpeername(
                self.socket,
                &mut vsock_addr as *mut _ as *mut sockaddr,
                &mut vsock_addr_len,
            )
        } < 0
        {
            Err(Error::last_os_error())
        } else {
            Ok(SockAddr::Vsock(VsockAddr(vsock_addr)))
        }
    }

    /// Virtio socket address of the local address associated with this connection.
    pub fn local_addr(&self) -> Result<SockAddr> {
        let mut vsock_addr = sockaddr_vm {
            svm_family: AF_VSOCK as sa_family_t,
            svm_reserved1: 0,
            svm_port: 0,
            svm_cid: 0,
            svm_zero: [0u8; 4],
        };
        let mut vsock_addr_len = size_of::<sockaddr_vm>() as socklen_t;
        if unsafe {
            getsockname(
                self.socket,
                &mut vsock_addr as *mut _ as *mut sockaddr,
                &mut vsock_addr_len,
            )
        } < 0
        {
            Err(Error::last_os_error())
        } else {
            Ok(SockAddr::Vsock(VsockAddr(vsock_addr)))
        }
    }

    /// Shutdown the read, write, or both halves of this connection.
    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        let how = match how {
            Shutdown::Write => SHUT_WR,
            Shutdown::Read => SHUT_RD,
            Shutdown::Both => SHUT_RDWR,
        };
        if unsafe { shutdown(self.socket, how) } < 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Create a new independently owned handle to the underlying socket.
    pub fn try_clone(&self) -> Result<Self> {
        Ok(self.clone())
    }

    /// Set the timeout on read operations.
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> Result<()> {
        let timeout = Self::timeval_from_duration(dur)?;
        if unsafe {
            setsockopt(
                self.socket,
                SOL_SOCKET,
                SO_SNDTIMEO,
                &timeout as *const _ as *const c_void,
                size_of::<timeval>() as u32,
            )
        } < 0
        {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Set the timeout on write operations.
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> Result<()> {
        let timeout = Self::timeval_from_duration(dur)?;
        if unsafe {
            setsockopt(
                self.socket,
                SOL_SOCKET,
                SO_RCVTIMEO,
                &timeout as *const _ as *const c_void,
                size_of::<timeval>() as u32,
            )
        } < 0
        {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Retrieve the latest error associated with the underlying socket.
    pub fn take_error(&self) -> Result<Option<Error>> {
        let mut error: i32 = 0;
        let mut error_len: socklen_t = 0;
        if unsafe {
            getsockopt(
                self.socket,
                SOL_SOCKET,
                SO_ERROR,
                &mut error as *mut _ as *mut c_void,
                &mut error_len,
            )
        } < 0
        {
            Err(Error::last_os_error())
        } else {
            Ok(if error == 0 {
                None
            } else {
                Some(Error::from_raw_os_error(error))
            })
        }
    }

    /// Move this stream in and out of nonblocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<()> {
        let mut nonblocking: i32 = if nonblocking { 1 } else { 0 };
        if unsafe { ioctl(self.socket, FIONBIO, &mut nonblocking) } < 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn timeval_from_duration(dur: Option<Duration>) -> Result<timeval> {
        match dur {
            Some(dur) => {
                if dur.as_secs() == 0 && dur.subsec_nanos() == 0 {
                    return Err(Error::new(
                        ErrorKind::InvalidInput,
                        "cannot set a zero duration timeout",
                    ));
                }

                let secs = if dur.as_secs() > time_t::max_value() as u64 {
                    time_t::max_value()
                } else {
                    dur.as_secs() as time_t
                };
                let mut timeout = timeval {
                    tv_sec: secs,
                    tv_usec: i64::from(dur.subsec_micros()) as suseconds_t,
                };
                if timeout.tv_sec == 0 && timeout.tv_usec == 0 {
                    timeout.tv_usec = 1;
                }
                Ok(timeout)
            }
            None => Ok(timeval {
                tv_sec: 0,
                tv_usec: 0,
            }),
        }
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
        Ok(())
    }
}

impl Read for &VsockStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let ret = unsafe { recv(self.socket, buf.as_mut_ptr() as *mut c_void, buf.len(), 0) };
        if ret < 0 {
            Err(Error::last_os_error())
        } else {
            Ok(ret as usize)
        }
    }
}

impl Write for &VsockStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let ret = unsafe {
            send(
                self.socket,
                buf.as_ptr() as *const c_void,
                buf.len(),
                MSG_NOSIGNAL,
            )
        };
        if ret < 0 {
            Err(Error::last_os_error())
        } else {
            Ok(ret as usize)
        }
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

impl AsRawFd for VsockStream {
    fn as_raw_fd(&self) -> RawFd {
        self.socket
    }
}

impl FromRawFd for VsockStream {
    unsafe fn from_raw_fd(socket: RawFd) -> Self {
        Self { socket }
    }
}

impl IntoRawFd for VsockStream {
    fn into_raw_fd(self) -> RawFd {
        self.socket
    }
}

impl Drop for VsockStream {
    fn drop(&mut self) {
        unsafe { close(self.socket) };
    }
}
