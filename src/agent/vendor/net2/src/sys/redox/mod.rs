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
    pub fn new(family: c_int, ty: c_int) -> io::Result<Socket> {
        unsafe {
            let fd = ::cvt(libc::socket(family, ty, 0))?;
            let mut flags = ::cvt(libc::fcntl(fd, libc::F_GETFD))?;
            flags |= libc::O_CLOEXEC;
            ::cvt(libc::fcntl(fd, libc::F_SETFD, flags))?;
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
        unsafe { TcpListener::from_raw_fd(self.into_fd() as usize) }
    }

    pub fn into_tcp_stream(self) -> TcpStream {
        unsafe { TcpStream::from_raw_fd(self.into_fd() as usize) }
    }

    pub fn into_udp_socket(self) -> UdpSocket {
        unsafe { UdpSocket::from_raw_fd(self.into_fd() as usize) }
    }
}

impl ::FromInner for Socket {
    type Inner = usize;
    fn from_inner(fd: usize) -> Socket {
        Socket { fd: fd as c_int }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::close(self.fd);
        }
    }
}
