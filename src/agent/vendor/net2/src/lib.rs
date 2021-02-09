// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Extensions to `std::net` networking types.
//!
//! This crate implements a number of extensions to the standard `std::net`
//! networking types, hopefully being slated for inclusion into the standard
//! library in the future. The goal of this crate is to expose all sorts of
//! cross-platform and platform-specific configuration options of UDP/TCP
//! sockets. System APIs are wrapped with as thin a layer as possible instead of
//! bundling multiple actions into one API call.
//!
//! More information about the design of this crate can be found in the
//! [associated rfc][rfc]
//!
//! [rfc]: https://github.com/rust-lang/rfcs/pull/1158
//!
//! # Examples
//!
//! ```no_run
//! use net2::TcpBuilder;
//!
//! let tcp = TcpBuilder::new_v4().unwrap();
//! tcp.reuse_address(true).unwrap()
//!    .only_v6(false).unwrap();
//!
//! let mut stream = tcp.connect("127.0.0.1:80").unwrap();
//!
//! // use `stream` as a TcpStream
//! ```

#![doc(html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
       html_favicon_url = "https://doc.rust-lang.org/favicon.ico",
       html_root_url = "https://doc.rust-lang.org/net2-rs")]
#![deny(missing_docs, warnings)]

// Silence warnings about deprecated try!() usage
#![allow(deprecated)]

#![cfg_attr(target_os = "wasi", feature(wasi_ext))]

#[cfg(any(target_os = "redox", target_os = "wasi", unix))] extern crate libc;

#[cfg(windows)] extern crate winapi;

#[macro_use] extern crate cfg_if;

use std::io;
use std::ops::Neg;
use std::net::{ToSocketAddrs, SocketAddr};

use utils::{One, NetInt};

mod tcp;
mod udp;
mod socket;
mod ext;
mod utils;

#[cfg(target_os="redox")] #[path = "sys/redox/mod.rs"] mod sys;
#[cfg(unix)] #[path = "sys/unix/mod.rs"] mod sys;
#[cfg(windows)] #[path = "sys/windows/mod.rs"] mod sys;
#[cfg(target_os = "wasi")] #[path = "sys/wasi/mod.rs"] mod sys;
#[cfg(all(unix, not(any(target_os = "solaris", target_os = "illumos"))))] pub mod unix;

pub use tcp::TcpBuilder;
pub use udp::UdpBuilder;
pub use ext::{TcpStreamExt, TcpListenerExt, UdpSocketExt};

fn one_addr<T: ToSocketAddrs>(tsa: T) -> io::Result<SocketAddr> {
    let mut addrs = try!(tsa.to_socket_addrs());
    let addr = match addrs.next() {
        Some(addr) => addr,
        None => return Err(io::Error::new(io::ErrorKind::Other,
                                          "no socket addresses could be resolved"))
    };
    if addrs.next().is_none() {
        Ok(addr)
    } else {
        Err(io::Error::new(io::ErrorKind::Other,
                           "more than one address resolved"))
    }
}

fn cvt<T: One + PartialEq + Neg<Output=T>>(t: T) -> io::Result<T> {
    let one: T = T::one();
    if t == -one {
        Err(io::Error::last_os_error())
    } else {
        Ok(t)
    }
}

#[cfg(windows)]
fn cvt_win<T: PartialEq + utils::Zero>(t: T) -> io::Result<T> {
    if t == T::zero() {
        Err(io::Error::last_os_error())
    } else {
        Ok(t)
    }
}

fn hton<I: NetInt>(i: I) -> I { i.to_be() }

fn ntoh<I: NetInt>(i: I) -> I { I::from_be(i) }

trait AsInner {
    type Inner;
    fn as_inner(&self) -> &Self::Inner;
}

trait FromInner {
    type Inner;
    fn from_inner(inner: Self::Inner) -> Self;
}

trait IntoInner {
    type Inner;
    fn into_inner(self) -> Self::Inner;
}
