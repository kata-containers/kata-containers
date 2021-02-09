// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(non_camel_case_types)]
use libc::{self, c_int, __wasi_fd_t};
use std::io;
use std::mem;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::os::wasi::io::FromRawFd;

mod impls;

pub mod c {
    pub use libc::*;

    pub type sa_family_t = u16;
    pub type socklen_t = u32;
    pub type in_port_t = u16;

    pub const SOCK_DGRAM: c_int = 0x00;
    pub const SOL_SOCKET: c_int = 0x00;
    pub const SO_RCVBUF: c_int = 0x00;
    pub const SO_SNDBUF: c_int = 0x00;
    pub const TCP_NODELAY: c_int = 0x00;
    pub const IPPROTO_TCP: c_int = 0x00;
    pub const SO_RCVTIMEO: c_int = 0x00;
    pub const SO_SNDTIMEO: c_int = 0x00;
    pub const IPPROTO_IP: c_int = 0x00;
    pub const IP_TTL: c_int = 0x00;
    pub const IPPROTO_IPV6: c_int = 0x00;
    pub const IPV6_V6ONLY: c_int = 0x00;
    pub const SO_ERROR: c_int = 0x00;
    pub const SO_LINGER: c_int = 0x00;
    pub const SO_BROADCAST: c_int = 0x00;
    pub const IP_MULTICAST_LOOP: c_int = 0x00;
    pub const IP_MULTICAST_TTL: c_int = 0x00;
    pub const IPV6_MULTICAST_HOPS: c_int = 0x00;
    pub const IPV6_MULTICAST_LOOP: c_int = 0x00;
    pub const IP_MULTICAST_IF: c_int = 0x00;
    pub const IPV6_MULTICAST_IF: c_int = 0x00;
    pub const IPV6_UNICAST_HOPS: c_int = 0x00;
    pub const IP_ADD_MEMBERSHIP: c_int = 0x00;
    pub const IPV6_ADD_MEMBERSHIP: c_int = 0x00;
    pub const IP_DROP_MEMBERSHIP: c_int = 0x00;
    pub const IPV6_DROP_MEMBERSHIP: c_int = 0x00;
    pub const SO_REUSEADDR: c_int = 0x00;
    pub const SOCK_STREAM: c_int = 0x00;
    pub const AF_INET: c_int = 0x00;
    pub const AF_INET6: c_int = 0x01;

    #[repr(C)]
    pub struct sockaddr_storage {
        pub ss_family: sa_family_t,
    }
    #[repr(C)]
    pub struct sockaddr {
        pub sa_family: sa_family_t,
        pub sa_data: [c_char; 14],
    }

    #[repr(C)]
    pub struct sockaddr_in6 {
        pub sin6_family: sa_family_t,
        pub sin6_port: in_port_t,
        pub sin6_flowinfo: u32,
        pub sin6_addr: in6_addr,
        pub sin6_scope_id: u32,
    }

    #[repr(C)]
    pub struct sockaddr_in {
        pub sin_family: sa_family_t,
        pub sin_port: in_port_t,
        pub sin_addr: in_addr,
        pub sin_zero: [u8; 8],
    }

    #[repr(align(4))]
    #[derive(Copy, Clone)]
    pub struct in6_addr {
        pub s6_addr: [u8; 16],
    }

    #[derive(Copy, Clone)]
    pub struct ipv6_mreq {
        pub ipv6mr_multiaddr: in6_addr,
        pub ipv6mr_interface: c_uint,
    }

    pub type in_addr_t = u32;
    #[derive(Copy, Clone)]
    pub struct in_addr {
        pub s_addr: in_addr_t,
    }

    #[derive(Copy, Clone)]
    pub struct linger {
        pub l_onoff: c_int,
        pub l_linger: c_int,
    }

    #[derive(Copy, Clone)]
    pub struct ip_mreq {
        pub imr_multiaddr: in_addr,
        pub imr_interface: in_addr,
    }

    pub unsafe fn getsockname(_socket: __wasi_fd_t, _address: *mut sockaddr,
                    _address_len: *mut socklen_t) -> c_int {
        unimplemented!()
    }
    pub unsafe fn connect(_socket: __wasi_fd_t, _address: *const sockaddr,
                _len: socklen_t) -> c_int {
        unimplemented!()
    }
    pub unsafe fn listen(_socket: __wasi_fd_t, _backlog: c_int) -> c_int {
        unimplemented!()
    }
    pub unsafe fn bind(_socket: __wasi_fd_t, _address: *const sockaddr,
            _address_len: socklen_t) -> c_int {
        unimplemented!()
    }

    pub fn sockaddr_in_u32(sa: &sockaddr_in) -> u32 {
        ::ntoh((*sa).sin_addr.s_addr)
    }

    pub fn in_addr_to_u32(addr: &in_addr) -> u32 {
        ::ntoh(addr.s_addr)
    }
}

pub struct Socket {
    fd: __wasi_fd_t,
}

impl Socket {
    pub fn new(_family: c_int, _ty: c_int) -> io::Result<Socket> {
        unimplemented!()
    }

    pub fn raw(&self) -> libc::__wasi_fd_t {
        self.fd
    }

    fn into_fd(self) -> libc::__wasi_fd_t {
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
    type Inner = libc::__wasi_fd_t;
    fn from_inner(fd: libc::__wasi_fd_t) -> Socket {
        Socket { fd: fd }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        // unsafe {
        //     let _ = libc::close(self.fd);
        // }
    }
}