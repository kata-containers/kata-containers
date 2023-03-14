// Copyright (c) 2020 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

//! Common functions and macros.

use crate::error::{Error, Result};
use byteorder::{BigEndian, ByteOrder};
#[cfg(any(feature = "async", not(target_os = "linux")))]
use nix::fcntl::FdFlag;
use nix::fcntl::{fcntl, FcntlArg, OFlag};
use nix::sys::socket::*;
use std::os::unix::io::RawFd;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Domain {
    Unix,
    #[cfg(target_os = "linux")]
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

impl<T> From<T> for MessageHeader
where
    T: AsRef<[u8]>,
{
    fn from(buf: T) -> Self {
        let buf = buf.as_ref();
        debug_assert!(buf.len() >= MESSAGE_HEADER_LENGTH);
        Self {
            length: BigEndian::read_u32(&buf[..4]),
            stream_id: BigEndian::read_u32(&buf[4..8]),
            type_: buf[8],
            flags: buf[9],
        }
    }
}

impl From<MessageHeader> for Vec<u8> {
    fn from(mh: MessageHeader) -> Self {
        let mut buf = vec![0u8; MESSAGE_HEADER_LENGTH];
        mh.into_buf(&mut buf);
        buf
    }
}

impl MessageHeader {
    pub(crate) fn into_buf(self, mut buf: impl AsMut<[u8]>) {
        let buf = buf.as_mut();
        debug_assert!(buf.len() >= MESSAGE_HEADER_LENGTH);

        let covbuf: &mut [u8] = &mut buf[..4];
        BigEndian::write_u32(covbuf, self.length);
        let covbuf: &mut [u8] = &mut buf[4..8];
        BigEndian::write_u32(covbuf, self.stream_id);
        buf[8] = self.type_;
        buf[9] = self.flags;
    }
}

pub(crate) fn do_listen(listener: RawFd) -> Result<()> {
    if let Err(e) = fcntl(listener, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)) {
        return Err(Error::Others(format!(
            "failed to set listener fd: {} as non block: {}",
            listener, e
        )));
    }

    listen(listener, 10).map_err(|e| Error::Socket(e.to_string()))
}

#[cfg(target_os = "linux")]
fn parse_sockaddr(addr: &str) -> Result<(Domain, &str)> {
    if let Some(addr) = addr.strip_prefix("unix://") {
        return Ok((Domain::Unix, addr));
    }

    if let Some(addr) = addr.strip_prefix("vsock://") {
        return Ok((Domain::Vsock, addr));
    }

    Err(Error::Others(format!("Scheme {:?} is not supported", addr)))
}

#[cfg(not(target_os = "linux"))]
fn parse_sockaddr(addr: &str) -> Result<(Domain, &str)> {
    if let Some(addr) = addr.strip_prefix("unix://") {
        if addr.starts_with('@') {
            return Err(Error::Others(
                "Abstract unix domain socket is not support on this platform".to_string(),
            ));
        }
        return Ok((Domain::Unix, addr));
    }

    Err(Error::Others(format!("Scheme {:?} is not supported", addr)))
}

#[cfg(any(feature = "async", not(target_os = "linux")))]
pub(crate) fn set_fd_close_exec(fd: RawFd) -> Result<RawFd> {
    if let Err(e) = fcntl(fd, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC)) {
        return Err(Error::Others(format!(
            "failed to set fd: {} as close-on-exec: {}",
            fd, e
        )));
    }
    Ok(fd)
}

// SOCK_CLOEXEC flag is Linux specific
#[cfg(target_os = "linux")]
pub(crate) const SOCK_CLOEXEC: SockFlag = SockFlag::SOCK_CLOEXEC;
#[cfg(not(target_os = "linux"))]
pub(crate) const SOCK_CLOEXEC: SockFlag = SockFlag::empty();

#[cfg(target_os = "linux")]
fn make_addr(domain: Domain, sockaddr: &str) -> Result<UnixAddr> {
    match domain {
        Domain::Unix => {
            if let Some(sockaddr) = sockaddr.strip_prefix('@') {
                UnixAddr::new_abstract(sockaddr.as_bytes()).map_err(err_to_others_err!(e, ""))
            } else {
                UnixAddr::new(sockaddr).map_err(err_to_others_err!(e, ""))
            }
        }
        Domain::Vsock => Err(Error::Others(
            "function make_addr does not support create vsock socket".to_string(),
        )),
    }
}

#[cfg(not(target_os = "linux"))]
fn make_addr(_domain: Domain, sockaddr: &str) -> Result<UnixAddr> {
    UnixAddr::new(sockaddr).map_err(err_to_others_err!(e, ""))
}

fn make_socket(addr: (&str, u32)) -> Result<(RawFd, Domain, SockAddr)> {
    let (sockaddr, _) = addr;
    let (domain, sockaddrv) = parse_sockaddr(sockaddr)?;

    let get_sock_addr = |domain, sockaddr| -> Result<(RawFd, SockAddr)> {
        let fd = socket(AddressFamily::Unix, SockType::Stream, SOCK_CLOEXEC, None)
            .map_err(|e| Error::Socket(e.to_string()))?;

        // MacOS doesn't support atomic creation of a socket descriptor with SOCK_CLOEXEC flag,
        // so there is a chance of leak if fork + exec happens in between of these calls.
        #[cfg(target_os = "macos")]
        set_fd_close_exec(fd)?;

        let sockaddr = SockAddr::Unix(make_addr(domain, sockaddr)?);
        Ok((fd, sockaddr))
    };

    let (fd, sockaddr) = match domain {
        Domain::Unix => get_sock_addr(domain, sockaddrv)?,
        #[cfg(target_os = "linux")]
        Domain::Vsock => {
            let sockaddr_port_v: Vec<&str> = sockaddrv.split(':').collect();
            if sockaddr_port_v.len() != 2 {
                return Err(Error::Others(format!(
                    "sockaddr {} is not right for vsock",
                    sockaddr
                )));
            }
            let port: u32 = sockaddr_port_v[1]
                .parse()
                .expect("the vsock port is not an number");
            let fd = socket(
                AddressFamily::Vsock,
                SockType::Stream,
                SockFlag::SOCK_CLOEXEC,
                None,
            )
            .map_err(|e| Error::Socket(e.to_string()))?;
            let cid = addr.1;
            let sockaddr = SockAddr::new_vsock(cid, port);
            (fd, sockaddr)
        }
    };

    Ok((fd, domain, sockaddr))
}

// Vsock is not supported on non Linux.
#[cfg(target_os = "linux")]
use libc::VMADDR_CID_ANY;
#[cfg(not(target_os = "linux"))]
const VMADDR_CID_ANY: u32 = 0;
#[cfg(target_os = "linux")]
use libc::VMADDR_CID_HOST;
#[cfg(not(target_os = "linux"))]
const VMADDR_CID_HOST: u32 = 0;

pub(crate) fn do_bind(sockaddr: &str) -> Result<(RawFd, Domain)> {
    let (fd, domain, sockaddr) = make_socket((sockaddr, VMADDR_CID_ANY))?;

    setsockopt(fd, sockopt::ReusePort, &true)?;
    bind(fd, &sockaddr).map_err(err_to_others_err!(e, ""))?;

    Ok((fd, domain))
}

/// Creates a unix socket for client.
pub(crate) unsafe fn client_connect(sockaddr: &str) -> Result<RawFd> {
    let (fd, _, sockaddr) = make_socket((sockaddr, VMADDR_CID_HOST))?;

    connect(fd, &sockaddr)?;

    Ok(fd)
}

macro_rules! cfg_sync {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "sync")]
            #[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
            $item
        )*
    }
}

macro_rules! cfg_async {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "async")]
            #[cfg_attr(docsrs, doc(cfg(feature = "async")))]
            $item
        )*
    }
}

pub const MESSAGE_HEADER_LENGTH: usize = 10;
pub const MESSAGE_LENGTH_MAX: usize = 4 << 20;

pub const MESSAGE_TYPE_REQUEST: u8 = 0x1;
pub const MESSAGE_TYPE_RESPONSE: u8 = 0x2;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn test_parse_sockaddr() {
        for i in &[
            (
                "unix:///run/a.sock",
                Some(Domain::Unix),
                "/run/a.sock",
                true,
            ),
            ("vsock://8:1024", Some(Domain::Vsock), "8:1024", true),
            ("Vsock://8:1025", Some(Domain::Vsock), "8:1025", false),
            (
                "unix://@/run/b.sock",
                Some(Domain::Unix),
                "@/run/b.sock",
                true,
            ),
            ("abc:///run/c.sock", None, "", false),
        ] {
            let (input, domain, addr, success) = (i.0, i.1, i.2, i.3);
            let r = parse_sockaddr(input);
            if success {
                let (rd, ra) = r.unwrap();
                assert_eq!(rd, domain.unwrap());
                assert_eq!(ra, addr);
            } else {
                assert!(r.is_err());
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_parse_sockaddr() {
        for i in &[
            (
                "unix:///run/a.sock",
                Some(Domain::Unix),
                "/run/a.sock",
                true,
            ),
            ("vsock:///run/c.sock", None, "", false),
            ("Vsock:///run/c.sock", None, "", false),
            ("unix://@/run/b.sock", None, "", false),
            ("abc:///run/c.sock", None, "", false),
        ] {
            let (input, domain, addr, success) = (i.0, i.1, i.2, i.3);
            let r = parse_sockaddr(input);
            if success {
                let (rd, ra) = r.unwrap();
                assert_eq!(rd, domain.unwrap());
                assert_eq!(ra, addr);
            } else {
                assert!(r.is_err());
            }
        }
    }

    #[test]
    fn message_header() {
        let buf = vec![
            0x10, 0x0, 0x0, 0x0, // length
            0x0, 0x0, 0x0, 0x03, // stream_id
            0x2,  // type_
            0xef, // flags
        ];
        let mh = MessageHeader::from(&buf);
        assert_eq!(mh.length, 0x1000_0000);
        assert_eq!(mh.stream_id, 0x3);
        assert_eq!(mh.type_, MESSAGE_TYPE_RESPONSE);
        assert_eq!(mh.flags, 0xef);

        let mut buf2 = vec![0; MESSAGE_HEADER_LENGTH];
        mh.into_buf(&mut buf2);
        assert_eq!(&buf, &buf2);
    }
}
