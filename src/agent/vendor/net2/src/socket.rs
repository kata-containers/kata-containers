// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;
use std::io;
use std::mem;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
#[cfg(any(unix, target_os = "redox", target_os = "wasi"))]
use libc::c_int;
#[cfg(windows)]
use winapi::ctypes::c_int;

use sys;
use sys::c;

pub struct Socket {
    inner: sys::Socket,
}

impl Socket {
    pub fn new(family: c_int, ty: c_int) -> io::Result<Socket> {
        Ok(Socket { inner: try!(sys::Socket::new(family, ty)) })
    }

    pub fn bind(&self, addr: &SocketAddr) -> io::Result<()> {
        let (addr, len) = addr2raw(addr);
        unsafe {
            ::cvt(c::bind(self.inner.raw(), addr.as_ptr(), len as c::socklen_t)).map(|_| ())
        }
    }

    pub fn listen(&self, backlog: i32) -> io::Result<()> {
        unsafe {
            ::cvt(c::listen(self.inner.raw(), backlog)).map(|_| ())
        }
    }

    pub fn connect(&self, addr: &SocketAddr) -> io::Result<()> {
        let (addr, len) = addr2raw(addr);
        unsafe {
            ::cvt(c::connect(self.inner.raw(), addr.as_ptr(), len)).map(|_| ())
        }
    }

    pub fn getsockname(&self) -> io::Result<SocketAddr> {
        unsafe {
            let mut storage: c::sockaddr_storage = mem::zeroed();
            let mut len = mem::size_of_val(&storage) as c::socklen_t;
            try!(::cvt(c::getsockname(self.inner.raw(),
                                      &mut storage as *mut _ as *mut _,
                                      &mut len)));
            raw2addr(&storage, len)
        }
    }
}

impl fmt::Debug for Socket {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.raw().fmt(f)
    }
}

impl ::AsInner for Socket {
    type Inner = sys::Socket;
    fn as_inner(&self) -> &sys::Socket { &self.inner }
}

impl ::FromInner for Socket {
    type Inner = sys::Socket;
    fn from_inner(sock: sys::Socket) -> Socket {
        Socket { inner: sock }
    }
}

impl ::IntoInner for Socket {
    type Inner = sys::Socket;
    fn into_inner(self) -> sys::Socket { self.inner }
}

/// A type with the same memory layout as `c::sockaddr`. Used in converting Rust level
/// SocketAddr* types into their system representation. The benefit of this specific
/// type over using `c::sockaddr_storage` is that this type is exactly as large as it
/// needs to be and not a lot larger.
#[repr(C)]
pub(crate) union SocketAddrCRepr {
    v4: c::sockaddr_in,
    v6: c::sockaddr_in6,
}

impl SocketAddrCRepr {
    pub(crate) fn as_ptr(&self) -> *const c::sockaddr {
        self as *const _ as *const c::sockaddr
    }
}

fn addr2raw(addr: &SocketAddr) -> (SocketAddrCRepr, c::socklen_t) {
    match addr {
        &SocketAddr::V4(ref v4) => addr2raw_v4(v4),
        &SocketAddr::V6(ref v6) => addr2raw_v6(v6),
    }
}

#[cfg(unix)]
fn addr2raw_v4(addr: &SocketAddrV4) -> (SocketAddrCRepr, c::socklen_t) {
    let sin_addr = c::in_addr {
        s_addr: u32::from(*addr.ip()).to_be(),
    };

    let sockaddr = SocketAddrCRepr {
        v4: c::sockaddr_in {
            sin_family: c::AF_INET as c::sa_family_t,
            sin_port: addr.port().to_be(),
            sin_addr,
            #[cfg(not(target_os = "haiku"))]
            sin_zero: [0; 8],
            #[cfg(target_os = "haiku")]
            sin_zero: [0; 24],
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "haiku",
            ))]
            sin_len: 0,
        },
    };
    (sockaddr, mem::size_of::<c::sockaddr_in>() as c::socklen_t)
}

#[cfg(windows)]
fn addr2raw_v4(addr: &SocketAddrV4) -> (SocketAddrCRepr, c::socklen_t) {
    let sin_addr = unsafe {
        let mut s_un = mem::zeroed::<c::in_addr_S_un>();
        *s_un.S_addr_mut() = u32::from(*addr.ip()).to_be();
        c::IN_ADDR { S_un: s_un }
    };

    let sockaddr = SocketAddrCRepr {
        v4: c::sockaddr_in {
            sin_family: c::AF_INET as c::sa_family_t,
            sin_port: addr.port().to_be(),
            sin_addr,
            sin_zero: [0; 8],
        },
    };
    (sockaddr, mem::size_of::<c::sockaddr_in>() as c::socklen_t)
}

#[cfg(unix)]
fn addr2raw_v6(addr: &SocketAddrV6) -> (SocketAddrCRepr, c::socklen_t) {
    let sin6_addr = {
        let mut sin6_addr = unsafe { mem::zeroed::<c::in6_addr>() };
        sin6_addr.s6_addr = addr.ip().octets();
        sin6_addr
    };

    let sockaddr = SocketAddrCRepr {
        v6: c::sockaddr_in6 {
            sin6_family: c::AF_INET6 as c::sa_family_t,
            sin6_port: addr.port().to_be(),
            sin6_addr,
            sin6_flowinfo: addr.flowinfo(),
            sin6_scope_id: addr.scope_id(),
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "ios",
                target_os = "macos",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "haiku",
            ))]
            sin6_len: 0,
            #[cfg(any(target_os = "solaris", target_os = "illumos"))]
            __sin6_src_id: 0,
        },
    };
    (sockaddr, mem::size_of::<c::sockaddr_in6>() as c::socklen_t)
}

#[cfg(windows)]
fn addr2raw_v6(addr: &SocketAddrV6) -> (SocketAddrCRepr, c::socklen_t) {
    let sin6_addr = unsafe {
        let mut u = mem::zeroed::<c::in6_addr_u>();
        *u.Byte_mut() = addr.ip().octets();
        c::IN6_ADDR { u }
    };
    let scope_id = unsafe {
        let mut u = mem::zeroed::<c::SOCKADDR_IN6_LH_u>();
        *u.sin6_scope_id_mut() = addr.scope_id();
        u
    };

    let sockaddr = SocketAddrCRepr {
        v6: c::sockaddr_in6 {
            sin6_family: c::AF_INET6 as c::sa_family_t,
            sin6_port: addr.port().to_be(),
            sin6_addr,
            sin6_flowinfo: addr.flowinfo(),
            u: scope_id,
        },
    };
    (sockaddr, mem::size_of::<c::sockaddr_in6>() as c::socklen_t)
}

fn raw2addr(storage: &c::sockaddr_storage, len: c::socklen_t) -> io::Result<SocketAddr> {
    match storage.ss_family as c_int {
        c::AF_INET => {
            unsafe {
                assert!(len as usize >= mem::size_of::<c::sockaddr_in>());
                let sa = storage as *const _ as *const c::sockaddr_in;
                let bits = c::sockaddr_in_u32(&(*sa));
                let ip = Ipv4Addr::new((bits >> 24) as u8,
                                       (bits >> 16) as u8,
                                       (bits >> 8) as u8,
                                       bits as u8);
                Ok(SocketAddr::V4(SocketAddrV4::new(ip, ::ntoh((*sa).sin_port))))
            }
        }
        c::AF_INET6 => {
            unsafe {
                assert!(len as usize >= mem::size_of::<c::sockaddr_in6>());

                let sa = storage as *const _ as *const c::sockaddr_in6;
                #[cfg(windows)]      let arr = (*sa).sin6_addr.u.Byte();
                #[cfg(not(windows))] let arr = (*sa).sin6_addr.s6_addr;

                let ip = Ipv6Addr::new(
                    (arr[0] as u16) << 8 | (arr[1] as u16),
                    (arr[2] as u16) << 8 | (arr[3] as u16),
                    (arr[4] as u16) << 8 | (arr[5] as u16),
                    (arr[6] as u16) << 8 | (arr[7] as u16),
                    (arr[8] as u16) << 8 | (arr[9] as u16),
                    (arr[10] as u16) << 8 | (arr[11] as u16),
                    (arr[12] as u16) << 8 | (arr[13] as u16),
                    (arr[14] as u16) << 8 | (arr[15] as u16),
                );

                #[cfg(windows)]      let sin6_scope_id = *(*sa).u.sin6_scope_id();
                #[cfg(not(windows))] let sin6_scope_id = (*sa).sin6_scope_id;

                Ok(SocketAddr::V6(SocketAddrV6::new(ip,
                                                    ::ntoh((*sa).sin6_port),
                                                    (*sa).sin6_flowinfo,
                                                    sin6_scope_id)))
            }
        }
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid argument")),
    }
}
