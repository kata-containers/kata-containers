// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::os::unix::io::{FromRawFd, AsRawFd};

use {TcpBuilder, UdpBuilder, FromInner, AsInner};
use socket::Socket;
use sys;

impl FromRawFd for TcpBuilder {
    unsafe fn from_raw_fd(fd: usize) -> TcpBuilder {
        let sock = sys::Socket::from_inner(fd);
        TcpBuilder::from_inner(Socket::from_inner(sock))
    }
}

impl AsRawFd for TcpBuilder {
    fn as_raw_fd(&self) -> usize {
        // TODO: this unwrap() is very bad
        self.as_inner().borrow().as_ref().unwrap().as_inner().raw() as usize
    }
}

impl FromRawFd for UdpBuilder {
    unsafe fn from_raw_fd(fd: usize) -> UdpBuilder {
        let sock = sys::Socket::from_inner(fd);
        UdpBuilder::from_inner(Socket::from_inner(sock))
    }
}

impl AsRawFd for UdpBuilder {
    fn as_raw_fd(&self) -> usize {
        // TODO: this unwrap() is very bad
        self.as_inner().borrow().as_ref().unwrap().as_inner().raw() as usize
    }
}
