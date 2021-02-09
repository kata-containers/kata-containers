// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::os::windows::io::{FromRawSocket, RawSocket, AsRawSocket};
use winapi::um::winsock2::SOCKET;

use {TcpBuilder, UdpBuilder, FromInner, AsInner};
use socket::Socket;
use sys;

impl FromRawSocket for TcpBuilder {
    unsafe fn from_raw_socket(fd: RawSocket) -> TcpBuilder {
        let sock = sys::Socket::from_inner(fd as SOCKET);
        TcpBuilder::from_inner(Socket::from_inner(sock))
    }
}

impl AsRawSocket for TcpBuilder {
    fn as_raw_socket(&self) -> RawSocket {
        // TODO: this unwrap() is very bad
        self.as_inner().borrow().as_ref().unwrap().as_inner().raw() as RawSocket
    }
}

impl FromRawSocket for UdpBuilder {
    unsafe fn from_raw_socket(fd: RawSocket) -> UdpBuilder {
        let sock = sys::Socket::from_inner(fd as SOCKET);
        UdpBuilder::from_inner(Socket::from_inner(sock))
    }
}

impl AsRawSocket for UdpBuilder {
    fn as_raw_socket(&self) -> RawSocket {
        // TODO: this unwrap() is very bad
        self.as_inner().borrow().as_ref().unwrap().as_inner().raw() as RawSocket
    }
}
