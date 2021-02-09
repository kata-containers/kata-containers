// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

//! Common functions and macros.

#![allow(unused_macros)]

use crate::error::{Error, Result};
use nix::fcntl::{fcntl, FcntlArg, FdFlag, OFlag};
use nix::sys::socket::*;
use std::os::unix::io::RawFd;
use std::str::FromStr;

#[derive(Debug)]
pub enum Domain {
    Unix,
    Vsock,
}

/// Message header of ttrpc.
#[derive(Default, Debug)]
pub struct MessageHeader {
    pub length: u32,
    pub stream_id: u32,
    pub type_: u8,
    pub flags: u8,
}

pub fn do_listen(listener: RawFd) -> Result<()> {
    if let Err(e) = fcntl(listener, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)) {
        return Err(Error::Others(format!(
            "failed to set listener fd: {} as non block: {}",
            listener, e
        )));
    }

    listen(listener, 10).map_err(|e| Error::Socket(e.to_string()))
}

pub fn parse_host(host: &str) -> Result<(Domain, Vec<&str>)> {
    let hostv: Vec<&str> = host.trim().split("://").collect();
    if hostv.len() != 2 {
        return Err(Error::Others(format!("Host {} is not right", host)));
    }

    let domain = match &hostv[0].to_lowercase()[..] {
        "unix" => Domain::Unix,
        "vsock" => Domain::Vsock,
        x => return Err(Error::Others(format!("Scheme {:?} is not supported", x))),
    };

    Ok((domain, hostv))
}

pub fn set_fd_close_exec(fd: RawFd) -> Result<RawFd> {
    if let Err(e) = fcntl(fd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC)) {
        return Err(Error::Others(format!(
            "failed to set fd: {} as close-on-exec: {}",
            fd, e
        )));
    }
    Ok(fd)
}

pub fn do_bind(host: &str) -> Result<(RawFd, Domain)> {
    let (domain, hostv) = parse_host(host)?;

    let sockaddr: SockAddr;
    let fd: RawFd;

    match domain {
        Domain::Unix => {
            fd = socket(
                AddressFamily::Unix,
                SockType::Stream,
                SockFlag::SOCK_CLOEXEC,
                None,
            )
            .map_err(|e| Error::Socket(e.to_string()))?;
            let sockaddr_h = hostv[1].to_owned() + &"\x00".to_string();
            let sockaddr_u =
                UnixAddr::new_abstract(sockaddr_h.as_bytes()).map_err(err_to_others_err!(e, ""))?;
            sockaddr = SockAddr::Unix(sockaddr_u);
        }
        Domain::Vsock => {
            let host_port_v: Vec<&str> = hostv[1].split(':').collect();
            if host_port_v.len() != 2 {
                return Err(Error::Others(format!(
                    "Host {} is not right for vsock",
                    host
                )));
            }
            let cid = libc::VMADDR_CID_ANY;
            let port: u32 =
                FromStr::from_str(host_port_v[1]).expect("the vsock port is not an number");
            fd = socket(
                AddressFamily::Vsock,
                SockType::Stream,
                SockFlag::SOCK_CLOEXEC,
                None,
            )
            .map_err(|e| Error::Socket(e.to_string()))?;
            sockaddr = SockAddr::new_vsock(cid, port);
        }
    };

    setsockopt(fd, sockopt::ReusePort, &true).ok();
    bind(fd, &sockaddr).map_err(err_to_others_err!(e, ""))?;

    Ok((fd, domain))
}

macro_rules! cfg_sync {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "sync")]
            #[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
            #[doc(inline)]
            $item
        )*
    }
}

macro_rules! cfg_async {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "async")]
            #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
            #[doc(inline)]
            $item
        )*
    }
}

pub const MESSAGE_HEADER_LENGTH: usize = 10;
pub const MESSAGE_LENGTH_MAX: usize = 4 << 20;

pub const MESSAGE_TYPE_REQUEST: u8 = 0x1;
pub const MESSAGE_TYPE_RESPONSE: u8 = 0x2;
