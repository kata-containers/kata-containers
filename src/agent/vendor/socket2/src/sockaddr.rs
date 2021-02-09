use std::fmt;
use std::mem::{self, MaybeUninit};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::ptr;

#[cfg(any(unix, target_os = "redox"))]
use libc::{
    in6_addr, in_addr, sa_family_t, sockaddr, sockaddr_in, sockaddr_in6, sockaddr_storage,
    socklen_t, AF_INET, AF_INET6,
};
#[cfg(windows)]
use winapi::shared::in6addr::{in6_addr_u, IN6_ADDR as in6_addr};
#[cfg(windows)]
use winapi::shared::inaddr::{in_addr_S_un, IN_ADDR as in_addr};
#[cfg(windows)]
use winapi::shared::ws2def::{
    ADDRESS_FAMILY as sa_family_t, AF_INET, AF_INET6, SOCKADDR as sockaddr,
    SOCKADDR_IN as sockaddr_in, SOCKADDR_STORAGE as sockaddr_storage,
};
#[cfg(windows)]
use winapi::shared::ws2ipdef::{SOCKADDR_IN6_LH_u, SOCKADDR_IN6_LH as sockaddr_in6};
#[cfg(windows)]
use winapi::um::ws2tcpip::socklen_t;

/// The address of a socket.
///
/// `SockAddr`s may be constructed directly to and from the standard library
/// `SocketAddr`, `SocketAddrV4`, and `SocketAddrV6` types.
pub struct SockAddr {
    storage: sockaddr_storage,
    len: socklen_t,
}

impl fmt::Debug for SockAddr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = fmt.debug_struct("SockAddr");
        builder.field("family", &self.family());
        if let Some(addr) = self.as_inet() {
            builder.field("inet", &addr);
        } else if let Some(addr) = self.as_inet6() {
            builder.field("inet6", &addr);
        }
        builder.finish()
    }
}

impl SockAddr {
    /// Constructs a `SockAddr` from its raw components.
    pub unsafe fn from_raw_parts(addr: *const sockaddr, len: socklen_t) -> SockAddr {
        let mut storage = MaybeUninit::<sockaddr_storage>::zeroed();
        ptr::copy_nonoverlapping(
            addr as *const _ as *const u8,
            storage.as_mut_ptr() as *mut u8,
            len as usize,
        );

        SockAddr {
            // This is safe as we written the address to `storage` above.
            storage: storage.assume_init(),
            len,
        }
    }

    /// Constructs a `SockAddr` with the family `AF_UNIX` and the provided path.
    ///
    /// This function is only available on Unix when the `unix` feature is
    /// enabled.
    ///
    /// # Failure
    ///
    /// Returns an error if the path is longer than `SUN_LEN`.
    #[cfg(all(unix, feature = "unix"))]
    pub fn unix<P>(path: P) -> ::std::io::Result<SockAddr>
    where
        P: AsRef<::std::path::Path>,
    {
        use libc::{c_char, sockaddr_un, AF_UNIX};
        use std::cmp::Ordering;
        use std::io;
        use std::os::unix::ffi::OsStrExt;

        unsafe {
            let mut addr = mem::zeroed::<sockaddr_un>();
            addr.sun_family = AF_UNIX as sa_family_t;

            let bytes = path.as_ref().as_os_str().as_bytes();

            match (bytes.get(0), bytes.len().cmp(&addr.sun_path.len())) {
                // Abstract paths don't need a null terminator
                (Some(&0), Ordering::Greater) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "path must be no longer than SUN_LEN",
                    ));
                }
                (Some(&0), _) => {}
                (_, Ordering::Greater) | (_, Ordering::Equal) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "path must be shorter than SUN_LEN",
                    ));
                }
                _ => {}
            }

            for (dst, src) in addr.sun_path.iter_mut().zip(bytes) {
                *dst = *src as c_char;
            }
            // null byte for pathname is already there since we zeroed up front

            let base = &addr as *const _ as usize;
            let path = &addr.sun_path as *const _ as usize;
            let sun_path_offset = path - base;

            let mut len = sun_path_offset + bytes.len();
            match bytes.get(0) {
                Some(&0) | None => {}
                Some(_) => len += 1,
            }
            Ok(SockAddr::from_raw_parts(
                &addr as *const _ as *const _,
                len as socklen_t,
            ))
        }
    }

    /// Returns this address as a `SocketAddrV4` if it is in the `AF_INET`
    /// family.
    pub fn as_inet(&self) -> Option<SocketAddrV4> {
        match self.as_std() {
            Some(SocketAddr::V4(addr)) => Some(addr),
            _ => None,
        }
    }

    /// Returns this address as a `SocketAddrV6` if it is in the `AF_INET6`
    /// family.
    pub fn as_inet6(&self) -> Option<SocketAddrV6> {
        match self.as_std() {
            Some(SocketAddr::V6(addr)) => Some(addr),
            _ => None,
        }
    }

    /// Returns this address as a `SocketAddr` if it is in the `AF_INET`
    /// or `AF_INET6` family, otherwise returns `None`.
    pub fn as_std(&self) -> Option<SocketAddr> {
        if self.storage.ss_family == AF_INET as sa_family_t {
            // Safety: if the ss_family field is AF_INET then storage must be a sockaddr_in.
            let addr = unsafe { &*(&self.storage as *const _ as *const sockaddr_in) };

            #[cfg(unix)]
            let ip = Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes());
            #[cfg(windows)]
            let ip = {
                let ip_bytes = unsafe { addr.sin_addr.S_un.S_un_b() };
                Ipv4Addr::from([ip_bytes.s_b1, ip_bytes.s_b2, ip_bytes.s_b3, ip_bytes.s_b4])
            };
            let port = u16::from_be(addr.sin_port);
            Some(SocketAddr::V4(SocketAddrV4::new(ip, port)))
        } else if self.storage.ss_family == AF_INET6 as sa_family_t {
            // Safety: if the ss_family field is AF_INET6 then storage must be a sockaddr_in6.
            let addr = unsafe { &*(&self.storage as *const _ as *const sockaddr_in6) };

            #[cfg(unix)]
            let ip = Ipv6Addr::from(addr.sin6_addr.s6_addr);
            #[cfg(windows)]
            let ip = Ipv6Addr::from(*unsafe { addr.sin6_addr.u.Byte() });
            let port = u16::from_be(addr.sin6_port);
            Some(SocketAddr::V6(SocketAddrV6::new(
                ip,
                port,
                addr.sin6_flowinfo,
                #[cfg(unix)]
                addr.sin6_scope_id,
                #[cfg(windows)]
                unsafe {
                    *addr.u.sin6_scope_id()
                },
            )))
        } else {
            None
        }
    }

    /// Returns this address's family.
    pub fn family(&self) -> sa_family_t {
        self.storage.ss_family
    }

    /// Returns the size of this address in bytes.
    pub fn len(&self) -> socklen_t {
        self.len
    }

    /// Returns a raw pointer to the address.
    pub fn as_ptr(&self) -> *const sockaddr {
        &self.storage as *const _ as *const _
    }
}

impl From<SocketAddrV4> for SockAddr {
    fn from(addr: SocketAddrV4) -> SockAddr {
        #[cfg(unix)]
        let sin_addr = in_addr {
            s_addr: u32::from_ne_bytes(addr.ip().octets()),
        };
        #[cfg(windows)]
        let sin_addr = unsafe {
            let mut s_un = mem::zeroed::<in_addr_S_un>();
            *s_un.S_addr_mut() = u32::from_ne_bytes(addr.ip().octets());
            in_addr { S_un: s_un }
        };

        let sockaddr_in = sockaddr_in {
            sin_family: AF_INET as sa_family_t,
            sin_port: addr.port().to_be(),
            sin_addr,
            sin_zero: Default::default(),
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "haiku",
                target_os = "ios",
                target_os = "macos",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            sin_len: 0,
        };
        let mut storage = MaybeUninit::<sockaddr_storage>::zeroed();
        // Safety: A `sockaddr_in` is memory compatible with a `sockaddr_storage`
        unsafe { (storage.as_mut_ptr() as *mut sockaddr_in).write(sockaddr_in) };
        SockAddr {
            storage: unsafe { storage.assume_init() },
            len: mem::size_of::<sockaddr_in>() as socklen_t,
        }
    }
}

impl From<SocketAddrV6> for SockAddr {
    fn from(addr: SocketAddrV6) -> SockAddr {
        #[cfg(unix)]
        let sin6_addr = in6_addr {
            s6_addr: addr.ip().octets(),
        };
        #[cfg(windows)]
        let sin6_addr = unsafe {
            let mut u = mem::zeroed::<in6_addr_u>();
            *u.Byte_mut() = addr.ip().octets();
            in6_addr { u }
        };
        #[cfg(windows)]
        let u = unsafe {
            let mut u = mem::zeroed::<SOCKADDR_IN6_LH_u>();
            *u.sin6_scope_id_mut() = addr.scope_id();
            u
        };

        let sockaddr_in6 = sockaddr_in6 {
            sin6_family: AF_INET6 as sa_family_t,
            sin6_port: addr.port().to_be(),
            sin6_addr,
            sin6_flowinfo: addr.flowinfo(),
            #[cfg(unix)]
            sin6_scope_id: addr.scope_id(),
            #[cfg(windows)]
            u,
            #[cfg(any(
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "haiku",
                target_os = "ios",
                target_os = "macos",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            sin6_len: 0,
            #[cfg(any(target_os = "solaris", target_os = "illumos"))]
            __sin6_src_id: 0,
        };
        let mut storage = MaybeUninit::<sockaddr_storage>::zeroed();
        // Safety: A `sockaddr_in6` is memory compatible with a `sockaddr_storage`
        unsafe { (storage.as_mut_ptr() as *mut sockaddr_in6).write(sockaddr_in6) };
        SockAddr {
            storage: unsafe { storage.assume_init() },
            len: mem::size_of::<sockaddr_in6>() as socklen_t,
        }
    }
}

impl From<SocketAddr> for SockAddr {
    fn from(addr: SocketAddr) -> SockAddr {
        match addr {
            SocketAddr::V4(addr) => addr.into(),
            SocketAddr::V6(addr) => addr.into(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn inet() {
        let raw = "127.0.0.1:80".parse::<SocketAddrV4>().unwrap();
        let addr = SockAddr::from(raw);
        assert!(addr.as_inet6().is_none());
        let addr = addr.as_inet().unwrap();
        assert_eq!(raw, addr);
    }

    #[test]
    fn inet6() {
        let raw = "[2001:db8::ff00:42:8329]:80"
            .parse::<SocketAddrV6>()
            .unwrap();
        let addr = SockAddr::from(raw);
        assert!(addr.as_inet().is_none());
        let addr = addr.as_inet6().unwrap();
        assert_eq!(raw, addr);
    }
}
