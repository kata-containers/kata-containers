// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


use std::io;
use std::mem;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::os::unix::io::FromRawFd;
use libc::{self, c_int};
#[cfg(not(any(target_os = "emscripten", target_os = "haiku", target_os = "illumos", target_os = "solaris")))]
use libc::{ioctl, FIOCLEX};

mod impls;

pub mod c {
    pub use libc::*;

    pub fn sockaddr_in_u32(sa: &sockaddr_in) -> u32 {
        ::ntoh((*sa).sin_addr.s_addr)
    }

    pub fn in_addr_to_u32(addr: &in_addr) -> u32 {
        ::ntoh(addr.s_addr)
    }
}

pub struct Socket {
    fd: c_int,
}

impl Socket {
    #[cfg(not(any(target_os = "emscripten", target_os = "haiku", target_os = "illumos", target_os = "solaris")))]
    pub fn new(family: c_int, ty: c_int) -> io::Result<Socket> {
        unsafe {
            // Linux >2.6.26 overloads the type argument to accept SOCK_CLOEXEC,
            // avoiding a race with another thread running fork/exec between
            // socket() and ioctl()
            #[cfg(any(target_os = "linux", target_os = "android"))]
            match ::cvt(libc::socket(family, ty | libc::SOCK_CLOEXEC, 0)) {
                Ok(fd) => return Ok(Socket { fd: fd }),
                // Older versions of Linux return EINVAL; fall back to ioctl
                Err(ref e) if e.raw_os_error() == Some(libc::EINVAL) => {}
                Err(e) => return Err(e),
            }

            let fd = try!(::cvt(libc::socket(family, ty, 0)));
            ioctl(fd, FIOCLEX);
            Ok(Socket { fd: fd })
        }
    }

    // ioctl(FIOCLEX) is not supported by Solaris/illumos or emscripten,
    // use fcntl(FD_CLOEXEC) instead
    #[cfg(any(target_os = "emscripten", target_os = "haiku", target_os = "illumos", target_os = "solaris"))]
    pub fn new(family: c_int, ty: c_int) -> io::Result<Socket> {
        unsafe {
            let fd = try!(::cvt(libc::socket(family, ty, 0)));
            libc::fcntl(fd, libc::FD_CLOEXEC);
            Ok(Socket { fd: fd })
        }
    }

    pub fn raw(&self) -> c_int { self.fd }

    fn into_fd(self) -> c_int {
        let fd = self.fd;
        mem::forget(self);
        fd
    }

    pub fn into_tcp_listener(self) -> TcpListener {
        unsafe { TcpListener::from_raw_fd(self.into_fd()) }
    }

    pub fn into_tcp_stream(self) -> TcpStream {
        unsafe { TcpStream::from_raw_fd(self.into_fd()) }
    }

    pub fn into_udp_socket(self) -> UdpSocket {
        unsafe { UdpSocket::from_raw_fd(self.into_fd()) }
    }
}

impl ::FromInner for Socket {
    type Inner = c_int;
    fn from_inner(fd: c_int) -> Socket {
        Socket { fd: fd }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::close(self.fd);
        }
    }
}
