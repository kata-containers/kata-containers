use super::sa_family_t;
use {Error, Result, NixPath};
use errno::Errno;
use libc;
use std::{fmt, mem, net, ptr, slice};
use std::ffi::OsStr;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::os::unix::ffi::OsStrExt;
#[cfg(any(target_os = "android", target_os = "linux"))]
use ::sys::socket::addr::netlink::NetlinkAddr;
#[cfg(any(target_os = "android", target_os = "linux"))]
use ::sys::socket::addr::alg::AlgAddr;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use std::os::unix::io::RawFd;
#[cfg(any(target_os = "ios", target_os = "macos"))]
use ::sys::socket::addr::sys_control::SysControlAddr;
#[cfg(any(target_os = "android",
          target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "ios",
          target_os = "linux",
          target_os = "macos",
          target_os = "netbsd",
          target_os = "openbsd"))]
pub use self::datalink::LinkAddr;
#[cfg(target_os = "linux")]
pub use self::vsock::VsockAddr;

/// These constants specify the protocol family to be used
/// in [`socket`](fn.socket.html) and [`socketpair`](fn.socketpair.html)
#[repr(i32)]
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub enum AddressFamily {
    /// Local communication (see [`unix(7)`](http://man7.org/linux/man-pages/man7/unix.7.html))
    Unix = libc::AF_UNIX,
    /// IPv4 Internet protocols (see [`ip(7)`](http://man7.org/linux/man-pages/man7/ip.7.html))
    Inet = libc::AF_INET,
    /// IPv6 Internet protocols (see [`ipv6(7)`](http://man7.org/linux/man-pages/man7/ipv6.7.html))
    Inet6 = libc::AF_INET6,
    /// Kernel user interface device (see [`netlink(7)`](http://man7.org/linux/man-pages/man7/netlink.7.html))
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Netlink = libc::AF_NETLINK,
    /// Low level packet interface (see [`packet(7)`](http://man7.org/linux/man-pages/man7/packet.7.html))
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Packet = libc::AF_PACKET,
    /// KEXT Controls and Notifications
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    System = libc::AF_SYSTEM,
    /// Amateur radio AX.25 protocol
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Ax25 = libc::AF_AX25,
    /// IPX - Novell protocols
    Ipx = libc::AF_IPX,
    /// AppleTalk
    AppleTalk = libc::AF_APPLETALK,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    NetRom = libc::AF_NETROM,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Bridge = libc::AF_BRIDGE,
    /// Access to raw ATM PVCs
    #[cfg(any(target_os = "android", target_os = "linux"))]
    AtmPvc = libc::AF_ATMPVC,
    /// ITU-T X.25 / ISO-8208 protocol (see [`x25(7)`](http://man7.org/linux/man-pages/man7/x25.7.html))
    #[cfg(any(target_os = "android", target_os = "linux"))]
    X25 = libc::AF_X25,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Rose = libc::AF_ROSE,
    Decnet = libc::AF_DECnet,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    NetBeui = libc::AF_NETBEUI,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Security = libc::AF_SECURITY,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Key = libc::AF_KEY,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Ash = libc::AF_ASH,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Econet = libc::AF_ECONET,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    AtmSvc = libc::AF_ATMSVC,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Rds = libc::AF_RDS,
    Sna = libc::AF_SNA,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Irda = libc::AF_IRDA,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Pppox = libc::AF_PPPOX,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Wanpipe = libc::AF_WANPIPE,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Llc = libc::AF_LLC,
    #[cfg(target_os = "linux")]
    Ib = libc::AF_IB,
    #[cfg(target_os = "linux")]
    Mpls = libc::AF_MPLS,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Can = libc::AF_CAN,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Tipc = libc::AF_TIPC,
    #[cfg(not(any(target_os = "ios", target_os = "macos")))]
    Bluetooth = libc::AF_BLUETOOTH,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Iucv = libc::AF_IUCV,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    RxRpc = libc::AF_RXRPC,
    Isdn = libc::AF_ISDN,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Phonet = libc::AF_PHONET,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Ieee802154 = libc::AF_IEEE802154,
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Caif = libc::AF_CAIF,
    /// Interface to kernel crypto API
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Alg = libc::AF_ALG,
    #[cfg(target_os = "linux")]
    Nfc = libc::AF_NFC,
    #[cfg(target_os = "linux")]
    Vsock = libc::AF_VSOCK,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    ImpLink = libc::AF_IMPLINK,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Pup = libc::AF_PUP,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Chaos = libc::AF_CHAOS,
    #[cfg(any(target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Ns = libc::AF_NS,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Iso = libc::AF_ISO,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Datakit = libc::AF_DATAKIT,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Ccitt = libc::AF_CCITT,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Dli = libc::AF_DLI,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Lat = libc::AF_LAT,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Hylink = libc::AF_HYLINK,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Link = libc::AF_LINK,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Coip = libc::AF_COIP,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Cnt = libc::AF_CNT,
    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Natm = libc::AF_NATM,
    /// Unspecified address family, (see [`getaddrinfo(3)`](http://man7.org/linux/man-pages/man3/getaddrinfo.3.html))
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Unspec = libc::AF_UNSPEC,
}

impl AddressFamily {
    /// Create a new `AddressFamily` from an integer value retrieved from `libc`, usually from
    /// the `sa_family` field of a `sockaddr`.
    ///
    /// Currently only supports these address families: Unix, Inet (v4 & v6), Netlink, Link/Packet
    /// and System. Returns None for unsupported or unknown address families.
    pub fn from_i32(family: i32) -> Option<AddressFamily> {
        match family {
            libc::AF_UNIX => Some(AddressFamily::Unix),
            libc::AF_INET => Some(AddressFamily::Inet),
            libc::AF_INET6 => Some(AddressFamily::Inet6),
            #[cfg(any(target_os = "android", target_os = "linux"))]
            libc::AF_NETLINK => Some(AddressFamily::Netlink),
            #[cfg(any(target_os = "macos", target_os = "macos"))]
            libc::AF_SYSTEM => Some(AddressFamily::System),
            #[cfg(any(target_os = "android", target_os = "linux"))]
            libc::AF_PACKET => Some(AddressFamily::Packet),
            #[cfg(any(target_os = "dragonfly",
                      target_os = "freebsd",
                      target_os = "ios",
                      target_os = "macos",
                      target_os = "netbsd",
                      target_os = "openbsd"))]
            libc::AF_LINK => Some(AddressFamily::Link),
            #[cfg(target_os = "linux")]
            libc::AF_VSOCK => Some(AddressFamily::Vsock),
            _ => None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InetAddr {
    V4(libc::sockaddr_in),
    V6(libc::sockaddr_in6),
}

impl InetAddr {
    pub fn from_std(std: &net::SocketAddr) -> InetAddr {
        match *std {
            net::SocketAddr::V4(ref addr) => {
                InetAddr::V4(libc::sockaddr_in {
                    sin_family: AddressFamily::Inet as sa_family_t,
                    sin_port: addr.port().to_be(),  // network byte order
                    sin_addr: Ipv4Addr::from_std(addr.ip()).0,
                    .. unsafe { mem::zeroed() }
                })
            }
            net::SocketAddr::V6(ref addr) => {
                InetAddr::V6(libc::sockaddr_in6 {
                    sin6_family: AddressFamily::Inet6 as sa_family_t,
                    sin6_port: addr.port().to_be(),  // network byte order
                    sin6_addr: Ipv6Addr::from_std(addr.ip()).0,
                    sin6_flowinfo: addr.flowinfo(),  // host byte order
                    sin6_scope_id: addr.scope_id(),  // host byte order
                    .. unsafe { mem::zeroed() }
                })
            }
        }
    }

    pub fn new(ip: IpAddr, port: u16) -> InetAddr {
        match ip {
            IpAddr::V4(ref ip) => {
                InetAddr::V4(libc::sockaddr_in {
                    sin_family: AddressFamily::Inet as sa_family_t,
                    sin_port: port.to_be(),
                    sin_addr: ip.0,
                    .. unsafe { mem::zeroed() }
                })
            }
            IpAddr::V6(ref ip) => {
                InetAddr::V6(libc::sockaddr_in6 {
                    sin6_family: AddressFamily::Inet6 as sa_family_t,
                    sin6_port: port.to_be(),
                    sin6_addr: ip.0,
                    .. unsafe { mem::zeroed() }
                })
            }
        }
    }
    /// Gets the IP address associated with this socket address.
    pub fn ip(&self) -> IpAddr {
        match *self {
            InetAddr::V4(ref sa) => IpAddr::V4(Ipv4Addr(sa.sin_addr)),
            InetAddr::V6(ref sa) => IpAddr::V6(Ipv6Addr(sa.sin6_addr)),
        }
    }

    /// Gets the port number associated with this socket address
    pub fn port(&self) -> u16 {
        match *self {
            InetAddr::V6(ref sa) => u16::from_be(sa.sin6_port),
            InetAddr::V4(ref sa) => u16::from_be(sa.sin_port),
        }
    }

    pub fn to_std(&self) -> net::SocketAddr {
        match *self {
            InetAddr::V4(ref sa) => net::SocketAddr::V4(
                net::SocketAddrV4::new(
                    Ipv4Addr(sa.sin_addr).to_std(),
                    self.port())),
            InetAddr::V6(ref sa) => net::SocketAddr::V6(
                net::SocketAddrV6::new(
                    Ipv6Addr(sa.sin6_addr).to_std(),
                    self.port(),
                    sa.sin6_flowinfo,
                    sa.sin6_scope_id)),
        }
    }

    pub fn to_str(&self) -> String {
        format!("{}", self)
    }
}

impl fmt::Display for InetAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            InetAddr::V4(_) => write!(f, "{}:{}", self.ip(), self.port()),
            InetAddr::V6(_) => write!(f, "[{}]:{}", self.ip(), self.port()),
        }
    }
}

/*
 *
 * ===== IpAddr =====
 *
 */
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum IpAddr {
    V4(Ipv4Addr),
    V6(Ipv6Addr),
}

impl IpAddr {
    /// Create a new IpAddr that contains an IPv4 address.
    ///
    /// The result will represent the IP address a.b.c.d
    pub fn new_v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    /// Create a new IpAddr that contains an IPv6 address.
    ///
    /// The result will represent the IP address a:b:c:d:e:f
    #[allow(clippy::many_single_char_names)]
    #[allow(clippy::too_many_arguments)]
    pub fn new_v6(a: u16, b: u16, c: u16, d: u16, e: u16, f: u16, g: u16, h: u16) -> IpAddr {
        IpAddr::V6(Ipv6Addr::new(a, b, c, d, e, f, g, h))
    }

    pub fn from_std(std: &net::IpAddr) -> IpAddr {
        match *std {
            net::IpAddr::V4(ref std) => IpAddr::V4(Ipv4Addr::from_std(std)),
            net::IpAddr::V6(ref std) => IpAddr::V6(Ipv6Addr::from_std(std)),
        }
    }

    pub fn to_std(&self) -> net::IpAddr {
        match *self {
            IpAddr::V4(ref ip) => net::IpAddr::V4(ip.to_std()),
            IpAddr::V6(ref ip) => net::IpAddr::V6(ip.to_std()),
        }
    }
}

impl fmt::Display for IpAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IpAddr::V4(ref v4) => v4.fmt(f),
            IpAddr::V6(ref v6) => v6.fmt(f)
        }
    }
}

/*
 *
 * ===== Ipv4Addr =====
 *
 */

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Ipv4Addr(pub libc::in_addr);

impl Ipv4Addr {
    #[allow(clippy::identity_op)]   // More readable this way
    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
        let ip = ((u32::from(a) << 24) |
                  (u32::from(b) << 16) |
                  (u32::from(c) <<  8) |
                  (u32::from(d) <<  0)).to_be();

        Ipv4Addr(libc::in_addr { s_addr: ip })
    }

    // Use pass by reference for symmetry with Ipv6Addr::from_std
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn from_std(std: &net::Ipv4Addr) -> Ipv4Addr {
        let bits = std.octets();
        Ipv4Addr::new(bits[0], bits[1], bits[2], bits[3])
    }

    pub fn any() -> Ipv4Addr {
        Ipv4Addr(libc::in_addr { s_addr: libc::INADDR_ANY })
    }

    pub fn octets(self) -> [u8; 4] {
        let bits = u32::from_be(self.0.s_addr);
        [(bits >> 24) as u8, (bits >> 16) as u8, (bits >> 8) as u8, bits as u8]
    }

    pub fn to_std(self) -> net::Ipv4Addr {
        let bits = self.octets();
        net::Ipv4Addr::new(bits[0], bits[1], bits[2], bits[3])
    }
}

impl fmt::Display for Ipv4Addr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let octets = self.octets();
        write!(fmt, "{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
    }
}

/*
 *
 * ===== Ipv6Addr =====
 *
 */

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Ipv6Addr(pub libc::in6_addr);

// Note that IPv6 addresses are stored in big endian order on all architectures.
// See https://tools.ietf.org/html/rfc1700 or consult your favorite search
// engine.

macro_rules! to_u8_array {
    ($($num:ident),*) => {
        [ $(($num>>8) as u8, ($num&0xff) as u8,)* ]
    }
}

macro_rules! to_u16_array {
    ($slf:ident, $($first:expr, $second:expr),*) => {
        [$( (($slf.0.s6_addr[$first] as u16) << 8) + $slf.0.s6_addr[$second] as u16,)*]
    }
}

impl Ipv6Addr {
    #[allow(clippy::many_single_char_names)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(a: u16, b: u16, c: u16, d: u16, e: u16, f: u16, g: u16, h: u16) -> Ipv6Addr {
        Ipv6Addr(libc::in6_addr{s6_addr: to_u8_array!(a,b,c,d,e,f,g,h)})
    }

    pub fn from_std(std: &net::Ipv6Addr) -> Ipv6Addr {
        let s = std.segments();
        Ipv6Addr::new(s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7])
    }

    /// Return the eight 16-bit segments that make up this address
    pub fn segments(&self) -> [u16; 8] {
        to_u16_array!(self, 0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15)
    }

    pub fn to_std(&self) -> net::Ipv6Addr {
        let s = self.segments();
        net::Ipv6Addr::new(s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7])
    }
}

impl fmt::Display for Ipv6Addr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        self.to_std().fmt(fmt)
    }
}

/// A wrapper around `sockaddr_un`.
///
/// This also tracks the length of `sun_path` address (excluding
/// a terminating null), because it may not be null-terminated.  For example,
/// unconnected and Linux abstract sockets are never null-terminated, and POSIX
/// does not require that `sun_len` include the terminating null even for normal
/// sockets.  Note that the actual sockaddr length is greater by
/// `offset_of!(libc::sockaddr_un, sun_path)`
#[derive(Clone, Copy, Debug)]
pub struct UnixAddr(pub libc::sockaddr_un, pub usize);

impl UnixAddr {
    /// Create a new sockaddr_un representing a filesystem path.
    pub fn new<P: ?Sized + NixPath>(path: &P) -> Result<UnixAddr> {
        path.with_nix_path(|cstr| {
            unsafe {
                let mut ret = libc::sockaddr_un {
                    sun_family: AddressFamily::Unix as sa_family_t,
                    .. mem::zeroed()
                };

                let bytes = cstr.to_bytes();

                if bytes.len() > ret.sun_path.len() {
                    return Err(Error::Sys(Errno::ENAMETOOLONG));
                }

                ptr::copy_nonoverlapping(bytes.as_ptr(),
                                         ret.sun_path.as_mut_ptr() as *mut u8,
                                         bytes.len());

                Ok(UnixAddr(ret, bytes.len()))
            }
        })?
    }

    /// Create a new `sockaddr_un` representing an address in the "abstract namespace".
    ///
    /// The leading null byte for the abstract namespace is automatically added;
    /// thus the input `path` is expected to be the bare name, not null-prefixed.
    /// This is a Linux-specific extension, primarily used to allow chrooted
    /// processes to communicate with processes having a different filesystem view.
    #[cfg(any(target_os = "android", target_os = "linux"))]
    pub fn new_abstract(path: &[u8]) -> Result<UnixAddr> {
        unsafe {
            let mut ret = libc::sockaddr_un {
                sun_family: AddressFamily::Unix as sa_family_t,
                .. mem::zeroed()
            };

            if path.len() + 1 > ret.sun_path.len() {
                return Err(Error::Sys(Errno::ENAMETOOLONG));
            }

            // Abstract addresses are represented by sun_path[0] ==
            // b'\0', so copy starting one byte in.
            ptr::copy_nonoverlapping(path.as_ptr(),
                                     ret.sun_path.as_mut_ptr().offset(1) as *mut u8,
                                     path.len());

            Ok(UnixAddr(ret, path.len() + 1))
        }
    }

    fn sun_path(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.0.sun_path.as_ptr() as *const u8, self.1) }
    }

    /// If this address represents a filesystem path, return that path.
    pub fn path(&self) -> Option<&Path> {
        if self.1 == 0 || self.0.sun_path[0] == 0 {
            // unnamed or abstract
            None
        } else {
            let p = self.sun_path();
            // POSIX only requires that `sun_len` be at least long enough to
            // contain the pathname, and it need not be null-terminated.  So we
            // need to create a string that is the shorter of the
            // null-terminated length or the full length.
            let ptr = &self.0.sun_path as *const libc::c_char;
            let reallen = unsafe { libc::strnlen(ptr, p.len()) };
            Some(Path::new(<OsStr as OsStrExt>::from_bytes(&p[..reallen])))
        }
    }

    /// If this address represents an abstract socket, return its name.
    ///
    /// For abstract sockets only the bare name is returned, without the
    /// leading null byte. `None` is returned for unnamed or path-backed sockets.
    #[cfg(any(target_os = "android", target_os = "linux"))]
    pub fn as_abstract(&self) -> Option<&[u8]> {
        if self.1 >= 1 && self.0.sun_path[0] == 0 {
            Some(&self.sun_path()[1..])
        } else {
            // unnamed or filesystem path
            None
        }
    }
}

impl fmt::Display for UnixAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.1 == 0 {
            f.write_str("<unbound UNIX socket>")
        } else if let Some(path) = self.path() {
            path.display().fmt(f)
        } else {
            let display = String::from_utf8_lossy(&self.sun_path()[1..]);
            write!(f, "@{}", display)
        }
    }
}

impl PartialEq for UnixAddr {
    fn eq(&self, other: &UnixAddr) -> bool {
        self.sun_path() == other.sun_path()
    }
}

impl Eq for UnixAddr {}

impl Hash for UnixAddr {
    fn hash<H: Hasher>(&self, s: &mut H) {
        ( self.0.sun_family, self.sun_path() ).hash(s)
    }
}

/// Represents a socket address
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SockAddr {
    Inet(InetAddr),
    Unix(UnixAddr),
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Netlink(NetlinkAddr),
    #[cfg(any(target_os = "android", target_os = "linux"))]
    Alg(AlgAddr),
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    SysControl(SysControlAddr),
    /// Datalink address (MAC)
    #[cfg(any(target_os = "android",
              target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "linux",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    Link(LinkAddr),
    #[cfg(target_os = "linux")]
    Vsock(VsockAddr),
}

impl SockAddr {
    pub fn new_inet(addr: InetAddr) -> SockAddr {
        SockAddr::Inet(addr)
    }

    pub fn new_unix<P: ?Sized + NixPath>(path: &P) -> Result<SockAddr> {
        Ok(SockAddr::Unix(UnixAddr::new(path)?))
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    pub fn new_netlink(pid: u32, groups: u32) -> SockAddr {
        SockAddr::Netlink(NetlinkAddr::new(pid, groups))
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    pub fn new_alg(alg_type: &str, alg_name: &str) -> SockAddr {
        SockAddr::Alg(AlgAddr::new(alg_type, alg_name))
    }

    #[cfg(any(target_os = "ios", target_os = "macos"))]
    pub fn new_sys_control(sockfd: RawFd, name: &str, unit: u32) -> Result<SockAddr> {
        SysControlAddr::from_name(sockfd, name, unit).map(|a| SockAddr::SysControl(a))
    }

    #[cfg(target_os = "linux")]
    pub fn new_vsock(cid: u32, port: u32) -> SockAddr {
        SockAddr::Vsock(VsockAddr::new(cid, port))
    }

    pub fn family(&self) -> AddressFamily {
        match *self {
            SockAddr::Inet(InetAddr::V4(..)) => AddressFamily::Inet,
            SockAddr::Inet(InetAddr::V6(..)) => AddressFamily::Inet6,
            SockAddr::Unix(..) => AddressFamily::Unix,
            #[cfg(any(target_os = "android", target_os = "linux"))]
            SockAddr::Netlink(..) => AddressFamily::Netlink,
            #[cfg(any(target_os = "android", target_os = "linux"))]
            SockAddr::Alg(..) => AddressFamily::Alg,
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            SockAddr::SysControl(..) => AddressFamily::System,
            #[cfg(any(target_os = "android", target_os = "linux"))]
            SockAddr::Link(..) => AddressFamily::Packet,
            #[cfg(any(target_os = "dragonfly",
                      target_os = "freebsd",
                      target_os = "ios",
                      target_os = "macos",
                      target_os = "netbsd",
                      target_os = "openbsd"))]
            SockAddr::Link(..) => AddressFamily::Link,
            #[cfg(target_os = "linux")]
            SockAddr::Vsock(..) => AddressFamily::Vsock,
        }
    }

    pub fn to_str(&self) -> String {
        format!("{}", self)
    }

    /// Creates a `SockAddr` struct from libc's sockaddr.
    ///
    /// Supports only the following address families: Unix, Inet (v4 & v6), Netlink and System.
    /// Returns None for unsupported families.
    pub unsafe fn from_libc_sockaddr(addr: *const libc::sockaddr) -> Option<SockAddr> {
        if addr.is_null() {
            None
        } else {
            match AddressFamily::from_i32(i32::from((*addr).sa_family)) {
                Some(AddressFamily::Unix) => None,
                Some(AddressFamily::Inet) => Some(SockAddr::Inet(
                    InetAddr::V4(*(addr as *const libc::sockaddr_in)))),
                Some(AddressFamily::Inet6) => Some(SockAddr::Inet(
                    InetAddr::V6(*(addr as *const libc::sockaddr_in6)))),
                #[cfg(any(target_os = "android", target_os = "linux"))]
                Some(AddressFamily::Netlink) => Some(SockAddr::Netlink(
                    NetlinkAddr(*(addr as *const libc::sockaddr_nl)))),
                #[cfg(any(target_os = "ios", target_os = "macos"))]
                Some(AddressFamily::System) => Some(SockAddr::SysControl(
                    SysControlAddr(*(addr as *const libc::sockaddr_ctl)))),
                #[cfg(any(target_os = "android", target_os = "linux"))]
                Some(AddressFamily::Packet) => Some(SockAddr::Link(
                    LinkAddr(*(addr as *const libc::sockaddr_ll)))),
                #[cfg(any(target_os = "dragonfly",
                          target_os = "freebsd",
                          target_os = "ios",
                          target_os = "macos",
                          target_os = "netbsd",
                          target_os = "openbsd"))]
                Some(AddressFamily::Link) => {
                    let ether_addr = LinkAddr(*(addr as *const libc::sockaddr_dl));
                    if ether_addr.is_empty() {
                        None
                    } else {
                        Some(SockAddr::Link(ether_addr))
                    }
                },
                #[cfg(target_os = "linux")]
                Some(AddressFamily::Vsock) => Some(SockAddr::Vsock(
                    VsockAddr(*(addr as *const libc::sockaddr_vm)))),
                // Other address families are currently not supported and simply yield a None
                // entry instead of a proper conversion to a `SockAddr`.
                Some(_) | None => None,
            }
        }
    }

    /// Conversion from nix's SockAddr type to the underlying libc sockaddr type.
    ///
    /// This is useful for interfacing with other libc functions that don't yet have nix wrappers.
    /// Returns a reference to the underlying data type (as a sockaddr reference) along
    /// with the size of the actual data type. sockaddr is commonly used as a proxy for
    /// a superclass as C doesn't support inheritance, so many functions that take
    /// a sockaddr * need to take the size of the underlying type as well and then internally cast it back.
    pub unsafe fn as_ffi_pair(&self) -> (&libc::sockaddr, libc::socklen_t) {
        match *self {
            SockAddr::Inet(InetAddr::V4(ref addr)) => (
                &*(addr as *const libc::sockaddr_in as *const libc::sockaddr),
                mem::size_of_val(addr) as libc::socklen_t
            ),
            SockAddr::Inet(InetAddr::V6(ref addr)) => (
                &*(addr as *const libc::sockaddr_in6 as *const libc::sockaddr),
                mem::size_of_val(addr) as libc::socklen_t
            ),
            SockAddr::Unix(UnixAddr(ref addr, len)) => (
                &*(addr as *const libc::sockaddr_un as *const libc::sockaddr),
                (len + offset_of!(libc::sockaddr_un, sun_path)) as libc::socklen_t
            ),
            #[cfg(any(target_os = "android", target_os = "linux"))]
            SockAddr::Netlink(NetlinkAddr(ref sa)) => (
                &*(sa as *const libc::sockaddr_nl as *const libc::sockaddr),
                mem::size_of_val(sa) as libc::socklen_t
            ),
            #[cfg(any(target_os = "android", target_os = "linux"))]
            SockAddr::Alg(AlgAddr(ref sa)) => (
                &*(sa as *const libc::sockaddr_alg as *const libc::sockaddr),
                mem::size_of_val(sa) as libc::socklen_t
            ),
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            SockAddr::SysControl(SysControlAddr(ref sa)) => (
                &*(sa as *const libc::sockaddr_ctl as *const libc::sockaddr),
                mem::size_of_val(sa) as libc::socklen_t

            ),
            #[cfg(any(target_os = "android", target_os = "linux"))]
            SockAddr::Link(LinkAddr(ref addr)) => (
                &*(addr as *const libc::sockaddr_ll as *const libc::sockaddr),
                mem::size_of_val(addr) as libc::socklen_t
            ),
            #[cfg(any(target_os = "dragonfly",
                      target_os = "freebsd",
                      target_os = "ios",
                      target_os = "macos",
                      target_os = "netbsd",
                      target_os = "openbsd"))]
            SockAddr::Link(LinkAddr(ref addr)) => (
                &*(addr as *const libc::sockaddr_dl as *const libc::sockaddr),
                mem::size_of_val(addr) as libc::socklen_t
            ),
            #[cfg(target_os = "linux")]
            SockAddr::Vsock(VsockAddr(ref sa)) => (
                &*(sa as *const libc::sockaddr_vm as *const libc::sockaddr),
                mem::size_of_val(sa) as libc::socklen_t
            ),
        }
    }
}

impl fmt::Display for SockAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SockAddr::Inet(ref inet) => inet.fmt(f),
            SockAddr::Unix(ref unix) => unix.fmt(f),
            #[cfg(any(target_os = "android", target_os = "linux"))]
            SockAddr::Netlink(ref nl) => nl.fmt(f),
            #[cfg(any(target_os = "android", target_os = "linux"))]
            SockAddr::Alg(ref nl) => nl.fmt(f),
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            SockAddr::SysControl(ref sc) => sc.fmt(f),
            #[cfg(any(target_os = "android",
                      target_os = "dragonfly",
                      target_os = "freebsd",
                      target_os = "ios",
                      target_os = "linux",
                      target_os = "macos",
                      target_os = "netbsd",
                      target_os = "openbsd"))]
            SockAddr::Link(ref ether_addr) => ether_addr.fmt(f),
            #[cfg(target_os = "linux")]
            SockAddr::Vsock(ref svm) => svm.fmt(f),
        }
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub mod netlink {
    use ::sys::socket::addr::AddressFamily;
    use libc::{sa_family_t, sockaddr_nl};
    use std::{fmt, mem};

    #[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
    pub struct NetlinkAddr(pub sockaddr_nl);

    impl NetlinkAddr {
        pub fn new(pid: u32, groups: u32) -> NetlinkAddr {
            let mut addr: sockaddr_nl = unsafe { mem::zeroed() };
            addr.nl_family = AddressFamily::Netlink as sa_family_t;
            addr.nl_pid = pid;
            addr.nl_groups = groups;

            NetlinkAddr(addr)
        }

        pub fn pid(&self) -> u32 {
            self.0.nl_pid
        }

        pub fn groups(&self) -> u32 {
            self.0.nl_groups
        }
    }

    impl fmt::Display for NetlinkAddr {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "pid: {} groups: {}", self.pid(), self.groups())
        }
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
pub mod alg {
    use libc::{AF_ALG, sockaddr_alg, c_char};
    use std::{fmt, mem, str};
    use std::hash::{Hash, Hasher};
    use std::ffi::CStr;

    #[derive(Copy, Clone)]
    pub struct AlgAddr(pub sockaddr_alg);

    // , PartialEq, Eq, Debug, Hash
    impl PartialEq for AlgAddr {
        fn eq(&self, other: &Self) -> bool {
            let (inner, other) = (self.0, other.0);
            (inner.salg_family, &inner.salg_type[..], inner.salg_feat, inner.salg_mask, &inner.salg_name[..]) ==
            (other.salg_family, &other.salg_type[..], other.salg_feat, other.salg_mask, &other.salg_name[..])
        }
    }

    impl Eq for AlgAddr {}

    impl Hash for AlgAddr {
        fn hash<H: Hasher>(&self, s: &mut H) {
            let inner = self.0;
            (inner.salg_family, &inner.salg_type[..], inner.salg_feat, inner.salg_mask, &inner.salg_name[..]).hash(s);
        }
    }

    impl AlgAddr {
        pub fn new(alg_type: &str, alg_name: &str) -> AlgAddr {
            let mut addr: sockaddr_alg = unsafe { mem::zeroed() };
            addr.salg_family = AF_ALG as u16;
            addr.salg_type[..alg_type.len()].copy_from_slice(alg_type.to_string().as_bytes());
            addr.salg_name[..alg_name.len()].copy_from_slice(alg_name.to_string().as_bytes());

            AlgAddr(addr)
        }


        pub fn alg_type(&self) -> &CStr {
            unsafe { CStr::from_ptr(self.0.salg_type.as_ptr() as *const c_char) }
        }

        pub fn alg_name(&self) -> &CStr {
            unsafe { CStr::from_ptr(self.0.salg_name.as_ptr() as *const c_char) }
        }
    }

    impl fmt::Display for AlgAddr {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "type: {} alg: {}",
                   self.alg_name().to_string_lossy(),
                   self.alg_type().to_string_lossy())
        }
    }

    impl fmt::Debug for AlgAddr {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            fmt::Display::fmt(self, f)
        }
    }
}

#[cfg(any(target_os = "ios", target_os = "macos"))]
pub mod sys_control {
    use ::sys::socket::addr::AddressFamily;
    use libc::{self, c_uchar};
    use std::{fmt, mem};
    use std::os::unix::io::RawFd;
    use {Errno, Error, Result};

    // FIXME: Move type into `libc`
    #[repr(C)]
    #[derive(Clone, Copy)]
    #[allow(missing_debug_implementations)]
    pub struct ctl_ioc_info {
        pub ctl_id: u32,
        pub ctl_name: [c_uchar; MAX_KCTL_NAME],
    }

    const CTL_IOC_MAGIC: u8 = 'N' as u8;
    const CTL_IOC_INFO: u8 = 3;
    const MAX_KCTL_NAME: usize = 96;

    ioctl_readwrite!(ctl_info, CTL_IOC_MAGIC, CTL_IOC_INFO, ctl_ioc_info);

    #[repr(C)]
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub struct SysControlAddr(pub libc::sockaddr_ctl);

    impl SysControlAddr {
        pub fn new(id: u32, unit: u32) -> SysControlAddr {
            let addr = libc::sockaddr_ctl {
                sc_len: mem::size_of::<libc::sockaddr_ctl>() as c_uchar,
                sc_family: AddressFamily::System as c_uchar,
                ss_sysaddr: libc::AF_SYS_CONTROL as u16,
                sc_id: id,
                sc_unit: unit,
                sc_reserved: [0; 5]
            };

            SysControlAddr(addr)
        }

        pub fn from_name(sockfd: RawFd, name: &str, unit: u32) -> Result<SysControlAddr> {
            if name.len() > MAX_KCTL_NAME {
                return Err(Error::Sys(Errno::ENAMETOOLONG));
            }

            let mut ctl_name = [0; MAX_KCTL_NAME];
            ctl_name[..name.len()].clone_from_slice(name.as_bytes());
            let mut info = ctl_ioc_info { ctl_id: 0, ctl_name };

            unsafe { ctl_info(sockfd, &mut info)?; }

            Ok(SysControlAddr::new(info.ctl_id, unit))
        }

        pub fn id(&self) -> u32 {
            self.0.sc_id
        }

        pub fn unit(&self) -> u32 {
            self.0.sc_unit
        }
    }

    impl fmt::Display for SysControlAddr {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            fmt::Debug::fmt(self, f)
        }
    }
}


#[cfg(any(target_os = "android", target_os = "linux"))]
mod datalink {
    use super::{libc, fmt, AddressFamily};

    /// Hardware Address
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub struct LinkAddr(pub libc::sockaddr_ll);

    impl LinkAddr {
        /// Always AF_PACKET
        pub fn family(&self) -> AddressFamily {
            assert_eq!(self.0.sll_family as i32, libc::AF_PACKET);
            AddressFamily::Packet
        }

        /// Physical-layer protocol
        pub fn protocol(&self) -> u16 {
            self.0.sll_protocol
        }

        /// Interface number
        pub fn ifindex(&self) -> usize {
            self.0.sll_ifindex as usize
        }

        /// ARP hardware type
        pub fn hatype(&self) -> u16 {
            self.0.sll_hatype
        }

        /// Packet type
        pub fn pkttype(&self) -> u8 {
            self.0.sll_pkttype
        }

        /// Length of MAC address
        pub fn halen(&self) -> usize {
            self.0.sll_halen as usize
        }

        /// Physical-layer address (MAC)
        pub fn addr(&self) -> [u8; 6] {
            [
                self.0.sll_addr[0] as u8,
                self.0.sll_addr[1] as u8,
                self.0.sll_addr[2] as u8,
                self.0.sll_addr[3] as u8,
                self.0.sll_addr[4] as u8,
                self.0.sll_addr[5] as u8,
            ]
        }
    }

    impl fmt::Display for LinkAddr {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let addr = self.addr();
            write!(f, "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                addr[0],
                addr[1],
                addr[2],
                addr[3],
                addr[4],
                addr[5])
        }
    }
}

#[cfg(any(target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "ios",
          target_os = "macos",
          target_os = "netbsd",
          target_os = "openbsd"))]
mod datalink {
    use super::{libc, fmt, AddressFamily};

    /// Hardware Address
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    pub struct LinkAddr(pub libc::sockaddr_dl);

    impl LinkAddr {
        /// Total length of sockaddr
        pub fn len(&self) -> usize {
            self.0.sdl_len as usize
        }

        /// always == AF_LINK
        pub fn family(&self) -> AddressFamily {
            assert_eq!(i32::from(self.0.sdl_family), libc::AF_LINK);
            AddressFamily::Link
        }

        /// interface index, if != 0, system given index for interface
        pub fn ifindex(&self) -> usize {
            self.0.sdl_index as usize
        }

        /// Datalink type
        pub fn datalink_type(&self) -> u8 {
            self.0.sdl_type
        }

        // MAC address start position
        pub fn nlen(&self) -> usize {
            self.0.sdl_nlen as usize
        }

        /// link level address length
        pub fn alen(&self) -> usize {
            self.0.sdl_alen as usize
        }

        /// link layer selector length
        pub fn slen(&self) -> usize {
            self.0.sdl_slen as usize
        }

        /// if link level address length == 0,
        /// or `sdl_data` not be larger.
        pub fn is_empty(&self) -> bool {
            let nlen = self.nlen();
            let alen = self.alen();
            let data_len = self.0.sdl_data.len();

            alen == 0 || nlen + alen >= data_len
        }

        /// Physical-layer address (MAC)
        pub fn addr(&self) -> [u8; 6] {
            let nlen = self.nlen();
            let data = self.0.sdl_data;

            assert!(!self.is_empty());

            [
                data[nlen] as u8,
                data[nlen + 1] as u8,
                data[nlen + 2] as u8,
                data[nlen + 3] as u8,
                data[nlen + 4] as u8,
                data[nlen + 5] as u8,
            ]
        }
    }

    impl fmt::Display for LinkAddr {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let addr = self.addr();
            write!(f, "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                addr[0],
                addr[1],
                addr[2],
                addr[3],
                addr[4],
                addr[5])
        }
    }
}

#[cfg(target_os = "linux")]
pub mod vsock {
    use ::sys::socket::addr::AddressFamily;
    use libc::{sa_family_t, sockaddr_vm};
    use std::{fmt, mem};
    use std::hash::{Hash, Hasher};

    #[derive(Copy, Clone)]
    pub struct VsockAddr(pub sockaddr_vm);

    impl PartialEq for VsockAddr {
        fn eq(&self, other: &Self) -> bool {
            let (inner, other) = (self.0, other.0);
            (inner.svm_family, inner.svm_cid, inner.svm_port) ==
            (other.svm_family, other.svm_cid, other.svm_port)
        }
    }

    impl Eq for VsockAddr {}

    impl Hash for VsockAddr {
        fn hash<H: Hasher>(&self, s: &mut H) {
            let inner = self.0;
            (inner.svm_family, inner.svm_cid, inner.svm_port).hash(s);
        }
    }

    /// VSOCK Address
    ///
    /// The address for AF_VSOCK socket is defined as a combination of a
    /// 32-bit Context Identifier (CID) and a 32-bit port number.
    impl VsockAddr {
        pub fn new(cid: u32, port: u32) -> VsockAddr {
            let mut addr: sockaddr_vm = unsafe { mem::zeroed() };
            addr.svm_family = AddressFamily::Vsock as sa_family_t;
            addr.svm_cid = cid;
            addr.svm_port = port;

            VsockAddr(addr)
        }

        /// Context Identifier (CID)
        pub fn cid(&self) -> u32 {
            self.0.svm_cid
        }

        /// Port number
        pub fn port(&self) -> u32 {
            self.0.svm_port
        }
    }

    impl fmt::Display for VsockAddr {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "cid: {} port: {}", self.cid(), self.port())
        }
    }

    impl fmt::Debug for VsockAddr {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            fmt::Display::fmt(self, f)
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(any(target_os = "android",
              target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "linux",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    use super::*;

    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    #[test]
    fn test_macos_loopback_datalink_addr() {
        let bytes = [20i8, 18, 1, 0, 24, 3, 0, 0, 108, 111, 48, 0, 0, 0, 0, 0];
        let sa = bytes.as_ptr() as *const libc::sockaddr;
        let _sock_addr = unsafe { SockAddr::from_libc_sockaddr(sa) };
        assert!(_sock_addr.is_none());
    }

    #[cfg(any(target_os = "dragonfly",
              target_os = "freebsd",
              target_os = "ios",
              target_os = "macos",
              target_os = "netbsd",
              target_os = "openbsd"))]
    #[test]
    fn test_macos_tap_datalink_addr() {
        let bytes = [20i8, 18, 7, 0, 6, 3, 6, 0, 101, 110, 48, 24, 101, -112, -35, 76, -80];
        let ptr = bytes.as_ptr();
        let sa = ptr as *const libc::sockaddr;
        let _sock_addr = unsafe { SockAddr::from_libc_sockaddr(sa) };

        assert!(_sock_addr.is_some());

        let sock_addr = _sock_addr.unwrap();

        assert_eq!(sock_addr.family(), AddressFamily::Link);

        match sock_addr {
            SockAddr::Link(ether_addr) => {
                assert_eq!(ether_addr.addr(), [24u8, 101, 144, 221, 76, 176]);
            },
            _ => { unreachable!() }
        };
    }

    #[cfg(any(target_os = "android", target_os = "linux"))]
    #[test]
    fn test_abstract_sun_path() {
        let name = String::from("nix\0abstract\0test");
        let addr = UnixAddr::new_abstract(name.as_bytes()).unwrap();

        let sun_path1 = addr.sun_path();
        let sun_path2 = [0u8, 110, 105, 120, 0, 97, 98, 115, 116, 114, 97, 99, 116, 0, 116, 101, 115, 116];
        assert_eq!(sun_path1.len(), sun_path2.len());
        for i in 0..sun_path1.len() {
            assert_eq!(sun_path1[i], sun_path2[i]);
        }
    }
}
