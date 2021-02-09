// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Utilities for handling sockets
//!
//! This crate is sort of an evolution of the `net2` crate after seeing the
//! issues on it over time. The intention of this crate is to provide as direct
//! as possible access to the system's functionality for sockets as possible. No
//! extra fluff (e.g. multiple syscalls or builders) provided in this crate. As
//! a result using this crate can be a little wordy, but it should give you
//! maximal flexibility over configuration of sockets.
//!
//! # Examples
//!
//! ```no_run
//! use std::net::SocketAddr;
//! use socket2::{Socket, Domain, Type};
//!
//! // create a TCP listener bound to two addresses
//! let socket = Socket::new(Domain::ipv6(), Type::stream(), None).unwrap();
//!
//! socket.bind(&"[::1]:12345".parse::<SocketAddr>().unwrap().into()).unwrap();
//! socket.set_only_v6(false);
//! socket.listen(128).unwrap();
//!
//! let listener = socket.into_tcp_listener();
//! // ...
//! ```

#![doc(html_root_url = "https://docs.rs/socket2/0.3")]
#![deny(missing_docs)]

use crate::utils::NetInt;

/// Macro to implement `fmt::Debug` for a type, printing the constant names
/// rather than a number.
///
/// Note this is used in the `sys` module and thus must be defined before
/// defining the modules.
macro_rules! impl_debug {
    (
        // Type name for which to implement `fmt::Debug`.
        $type: path,
        $(
            $(#[$target: meta])*
            // The flag(s) to check.
            // Need to specific the libc crate because Windows doesn't use
            // `libc` but `winapi`.
            $libc: ident :: $flag: ident
        ),+ $(,)*
    ) => {
        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let string = match self.0 {
                    $(
                        $(#[$target])*
                        $libc :: $flag => stringify!($flag),
                    )+
                    n => return write!(f, "{}", n),
                };
                f.write_str(string)
            }
        }
    };
}

mod sockaddr;
mod socket;
mod utils;

#[cfg(test)]
mod tests;

#[cfg(unix)]
#[path = "sys/unix.rs"]
mod sys;
#[cfg(windows)]
#[path = "sys/windows.rs"]
mod sys;

use sys::c_int;

pub use sockaddr::SockAddr;
pub use socket::Socket;

/// Specification of the communication domain for a socket.
///
/// This is a newtype wrapper around an integer which provides a nicer API in
/// addition to an injection point for documentation. Convenience constructors
/// such as `Domain::ipv4`, `Domain::ipv6`, etc, are provided to avoid reaching
/// into libc for various constants.
///
/// This type is freely interconvertible with C's `int` type, however, if a raw
/// value needs to be provided.
#[derive(Copy, Clone)]
pub struct Domain(c_int);

impl Domain {
    /// Domain for IPv4 communication, corresponding to `AF_INET`.
    pub fn ipv4() -> Domain {
        Domain(sys::AF_INET)
    }

    /// Domain for IPv6 communication, corresponding to `AF_INET6`.
    pub fn ipv6() -> Domain {
        Domain(sys::AF_INET6)
    }
}

impl From<c_int> for Domain {
    fn from(d: c_int) -> Domain {
        Domain(d)
    }
}

impl From<Domain> for c_int {
    fn from(d: Domain) -> c_int {
        d.0
    }
}

/// Specification of communication semantics on a socket.
///
/// This is a newtype wrapper around an integer which provides a nicer API in
/// addition to an injection point for documentation. Convenience constructors
/// such as `Type::stream`, `Type::dgram`, etc, are provided to avoid reaching
/// into libc for various constants.
///
/// This type is freely interconvertible with C's `int` type, however, if a raw
/// value needs to be provided.
#[derive(Copy, Clone)]
pub struct Type(c_int);

impl Type {
    /// Type corresponding to `SOCK_STREAM`.
    ///
    /// Used for protocols such as TCP.
    pub fn stream() -> Type {
        Type(sys::SOCK_STREAM)
    }

    /// Type corresponding to `SOCK_DGRAM`.
    ///
    /// Used for protocols such as UDP.
    pub fn dgram() -> Type {
        Type(sys::SOCK_DGRAM)
    }

    /// Type corresponding to `SOCK_SEQPACKET`.
    pub fn seqpacket() -> Type {
        Type(sys::SOCK_SEQPACKET)
    }

    /// Type corresponding to `SOCK_RAW`.
    #[cfg(not(target_os = "redox"))]
    pub fn raw() -> Type {
        Type(sys::SOCK_RAW)
    }
}

impl From<c_int> for Type {
    fn from(t: c_int) -> Type {
        Type(t)
    }
}

impl From<Type> for c_int {
    fn from(t: Type) -> c_int {
        t.0
    }
}

/// Protocol specification used for creating sockets via `Socket::new`.
///
/// This is a newtype wrapper around an integer which provides a nicer API in
/// addition to an injection point for documentation.
///
/// This type is freely interconvertible with C's `int` type, however, if a raw
/// value needs to be provided.
#[derive(Copy, Clone)]
pub struct Protocol(c_int);

impl Protocol {
    /// Protocol corresponding to `ICMPv4`.
    pub fn icmpv4() -> Self {
        Protocol(sys::IPPROTO_ICMP)
    }

    /// Protocol corresponding to `ICMPv6`.
    pub fn icmpv6() -> Self {
        Protocol(sys::IPPROTO_ICMPV6)
    }

    /// Protocol corresponding to `TCP`.
    pub fn tcp() -> Self {
        Protocol(sys::IPPROTO_TCP)
    }

    /// Protocol corresponding to `UDP`.
    pub fn udp() -> Self {
        Protocol(sys::IPPROTO_UDP)
    }
}

impl From<c_int> for Protocol {
    fn from(p: c_int) -> Protocol {
        Protocol(p)
    }
}

impl From<Protocol> for c_int {
    fn from(p: Protocol) -> c_int {
        p.0
    }
}

fn hton<I: NetInt>(i: I) -> I {
    i.to_be()
}

fn ntoh<I: NetInt>(i: I) -> I {
    I::from_be(i)
}
