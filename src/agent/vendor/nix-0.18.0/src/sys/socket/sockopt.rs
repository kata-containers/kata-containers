use cfg_if::cfg_if;
use super::{GetSockOpt, SetSockOpt};
use crate::Result;
use crate::errno::Errno;
use crate::sys::time::TimeVal;
use libc::{self, c_int, c_void, socklen_t};
use std::mem::{
    self,
    MaybeUninit
};
use std::os::unix::io::RawFd;
use std::ffi::{OsStr, OsString};
#[cfg(target_family = "unix")]
use std::os::unix::ffi::OsStrExt;

// Constants
// TCP_CA_NAME_MAX isn't defined in user space include files
#[cfg(any(target_os = "freebsd", target_os = "linux"))] 
const TCP_CA_NAME_MAX: usize = 16;

/// Helper for implementing `SetSockOpt` for a given socket option. See
/// [`::sys::socket::SetSockOpt`](sys/socket/trait.SetSockOpt.html).
///
/// This macro aims to help implementing `SetSockOpt` for different socket options that accept
/// different kinds of data to be used with `setsockopt`.
///
/// Instead of using this macro directly consider using `sockopt_impl!`, especially if the option
/// you are implementing represents a simple type.
///
/// # Arguments
///
/// * `$name:ident`: name of the type you want to implement `SetSockOpt` for.
/// * `$level:path` : socket layer, or a `protocol level`: could be *raw sockets*
///    (`libc::SOL_SOCKET`), *ip protocol* (libc::IPPROTO_IP), *tcp protocol* (`libc::IPPROTO_TCP`),
///    and more. Please refer to your system manual for more options. Will be passed as the second
///    argument (`level`) to the `setsockopt` call.
/// * `$flag:path`: a flag name to set. Some examples: `libc::SO_REUSEADDR`, `libc::TCP_NODELAY`,
///    `libc::IP_ADD_MEMBERSHIP` and others. Will be passed as the third argument (`option_name`)
///    to the `setsockopt` call.
/// * Type of the value that you are going to set.
/// * Type that implements the `Set` trait for the type from the previous item (like `SetBool` for
///    `bool`, `SetUsize` for `usize`, etc.).
macro_rules! setsockopt_impl {
    ($name:ident, $level:path, $flag:path, $ty:ty, $setter:ty) => {
        impl SetSockOpt for $name {
            type Val = $ty;

            fn set(&self, fd: RawFd, val: &$ty) -> Result<()> {
                unsafe {
                    let setter: $setter = Set::new(val);

                    let res = libc::setsockopt(fd, $level, $flag,
                                               setter.ffi_ptr(),
                                               setter.ffi_len());
                    Errno::result(res).map(drop)
                }
            }
        }
    }
}

/// Helper for implementing `GetSockOpt` for a given socket option. See
/// [`::sys::socket::GetSockOpt`](sys/socket/trait.GetSockOpt.html).
///
/// This macro aims to help implementing `GetSockOpt` for different socket options that accept
/// different kinds of data to be use with `getsockopt`.
///
/// Instead of using this macro directly consider using `sockopt_impl!`, especially if the option
/// you are implementing represents a simple type.
///
/// # Arguments
///
/// * Name of the type you want to implement `GetSockOpt` for.
/// * Socket layer, or a `protocol level`: could be *raw sockets* (`lic::SOL_SOCKET`),  *ip
///    protocol* (libc::IPPROTO_IP), *tcp protocol* (`libc::IPPROTO_TCP`),  and more. Please refer
///    to your system manual for more options. Will be passed as the second argument (`level`) to
///    the `getsockopt` call.
/// * A flag to set. Some examples: `libc::SO_REUSEADDR`, `libc::TCP_NODELAY`,
///    `libc::SO_ORIGINAL_DST` and others. Will be passed as the third argument (`option_name`) to
///    the `getsockopt` call.
/// * Type of the value that you are going to get.
/// * Type that implements the `Get` trait for the type from the previous item (`GetBool` for
///    `bool`, `GetUsize` for `usize`, etc.).
macro_rules! getsockopt_impl {
    ($name:ident, $level:path, $flag:path, $ty:ty, $getter:ty) => {
        impl GetSockOpt for $name {
            type Val = $ty;

            fn get(&self, fd: RawFd) -> Result<$ty> {
                unsafe {
                    let mut getter: $getter = Get::uninit();

                    let res = libc::getsockopt(fd, $level, $flag,
                                               getter.ffi_ptr(),
                                               getter.ffi_len());
                    Errno::result(res)?;

                    Ok(getter.assume_init())
                }
            }
        }
    }
}

/// Helper to generate the sockopt accessors. See
/// [`::sys::socket::GetSockOpt`](sys/socket/trait.GetSockOpt.html) and
/// [`::sys::socket::SetSockOpt`](sys/socket/trait.SetSockOpt.html).
///
/// This macro aims to help implementing `GetSockOpt` and `SetSockOpt` for different socket options
/// that accept different kinds of data to be use with `getsockopt` and `setsockopt` respectively.
///
/// Basically this macro wraps up the [`getsockopt_impl!`](macro.getsockopt_impl.html) and
/// [`setsockopt_impl!`](macro.setsockopt_impl.html) macros.
///
/// # Arguments
///
/// * `GetOnly`, `SetOnly` or `Both`: whether you want to implement only getter, only setter or
///    both of them.
/// * `$name:ident`: name of type `GetSockOpt`/`SetSockOpt` will be implemented for.
/// * `$level:path` : socket layer, or a `protocol level`: could be *raw sockets*
///    (`lic::SOL_SOCKET`), *ip protocol* (libc::IPPROTO_IP), *tcp protocol* (`libc::IPPROTO_TCP`),
///    and more. Please refer to your system manual for more options. Will be passed as the second
///    argument (`level`) to the `getsockopt`/`setsockopt` call.
/// * `$flag:path`: a flag name to set. Some examples: `libc::SO_REUSEADDR`, `libc::TCP_NODELAY`,
///    `libc::IP_ADD_MEMBERSHIP` and others. Will be passed as the third argument (`option_name`)
///    to the `setsockopt`/`getsockopt` call.
/// * `$ty:ty`: type of the value that will be get/set.
/// * `$getter:ty`: `Get` implementation; optional; only for `GetOnly` and `Both`.
/// * `$setter:ty`: `Set` implementation; optional; only for `SetOnly` and `Both`.
macro_rules! sockopt_impl {
    (GetOnly, $name:ident, $level:path, $flag:path, bool) => {
        sockopt_impl!(GetOnly, $name, $level, $flag, bool, GetBool);
    };

    (GetOnly, $name:ident, $level:path, $flag:path, u8) => {
        sockopt_impl!(GetOnly, $name, $level, $flag, u8, GetU8);
    };

    (GetOnly, $name:ident, $level:path, $flag:path, usize) => {
        sockopt_impl!(GetOnly, $name, $level, $flag, usize, GetUsize);
    };

    (SetOnly, $name:ident, $level:path, $flag:path, bool) => {
        sockopt_impl!(SetOnly, $name, $level, $flag, bool, SetBool);
    };

    (SetOnly, $name:ident, $level:path, $flag:path, u8) => {
        sockopt_impl!(SetOnly, $name, $level, $flag, u8, SetU8);
    };

    (SetOnly, $name:ident, $level:path, $flag:path, usize) => {
        sockopt_impl!(SetOnly, $name, $level, $flag, usize, SetUsize);
    };

    (Both, $name:ident, $level:path, $flag:path, bool) => {
        sockopt_impl!(Both, $name, $level, $flag, bool, GetBool, SetBool);
    };

    (Both, $name:ident, $level:path, $flag:path, u8) => {
        sockopt_impl!(Both, $name, $level, $flag, u8, GetU8, SetU8);
    };

    (Both, $name:ident, $level:path, $flag:path, usize) => {
        sockopt_impl!(Both, $name, $level, $flag, usize, GetUsize, SetUsize);
    };

    (Both, $name:ident, $level:path, $flag:path, OsString<$array:ty>) => {
        sockopt_impl!(Both, $name, $level, $flag, OsString, GetOsString<$array>, SetOsString);
    };

    /*
     * Matchers with generic getter types must be placed at the end, so
     * they'll only match _after_ specialized matchers fail
     */
    (GetOnly, $name:ident, $level:path, $flag:path, $ty:ty) => {
        sockopt_impl!(GetOnly, $name, $level, $flag, $ty, GetStruct<$ty>);
    };

    (GetOnly, $name:ident, $level:path, $flag:path, $ty:ty, $getter:ty) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name;

        getsockopt_impl!($name, $level, $flag, $ty, $getter);
    };

    (SetOnly, $name:ident, $level:path, $flag:path, $ty:ty) => {
        sockopt_impl!(SetOnly, $name, $level, $flag, $ty, SetStruct<$ty>);
    };

    (SetOnly, $name:ident, $level:path, $flag:path, $ty:ty, $setter:ty) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name;

        setsockopt_impl!($name, $level, $flag, $ty, $setter);
    };

    (Both, $name:ident, $level:path, $flag:path, $ty:ty, $getter:ty, $setter:ty) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub struct $name;

        setsockopt_impl!($name, $level, $flag, $ty, $setter);
        getsockopt_impl!($name, $level, $flag, $ty, $getter);
    };

    (Both, $name:ident, $level:path, $flag:path, $ty:ty) => {
        sockopt_impl!(Both, $name, $level, $flag, $ty, GetStruct<$ty>, SetStruct<$ty>);
    };
}

/*
 *
 * ===== Define sockopts =====
 *
 */

sockopt_impl!(Both, ReuseAddr, libc::SOL_SOCKET, libc::SO_REUSEADDR, bool);
sockopt_impl!(Both, ReusePort, libc::SOL_SOCKET, libc::SO_REUSEPORT, bool);
sockopt_impl!(Both, TcpNoDelay, libc::IPPROTO_TCP, libc::TCP_NODELAY, bool);
sockopt_impl!(Both, Linger, libc::SOL_SOCKET, libc::SO_LINGER, libc::linger);
sockopt_impl!(SetOnly, IpAddMembership, libc::IPPROTO_IP, libc::IP_ADD_MEMBERSHIP, super::IpMembershipRequest);
sockopt_impl!(SetOnly, IpDropMembership, libc::IPPROTO_IP, libc::IP_DROP_MEMBERSHIP, super::IpMembershipRequest);
cfg_if! {
    if #[cfg(any(target_os = "android", target_os = "linux"))] {
        sockopt_impl!(SetOnly, Ipv6AddMembership, libc::IPPROTO_IPV6, libc::IPV6_ADD_MEMBERSHIP, super::Ipv6MembershipRequest);
        sockopt_impl!(SetOnly, Ipv6DropMembership, libc::IPPROTO_IPV6, libc::IPV6_DROP_MEMBERSHIP, super::Ipv6MembershipRequest);
    } else if #[cfg(any(target_os = "dragonfly",
                        target_os = "freebsd",
                        target_os = "ios",
                        target_os = "macos",
                        target_os = "netbsd",
                        target_os = "openbsd"))] {
        sockopt_impl!(SetOnly, Ipv6AddMembership, libc::IPPROTO_IPV6, libc::IPV6_JOIN_GROUP, super::Ipv6MembershipRequest);
        sockopt_impl!(SetOnly, Ipv6DropMembership, libc::IPPROTO_IPV6, libc::IPV6_LEAVE_GROUP, super::Ipv6MembershipRequest);
    }
}
sockopt_impl!(Both, IpMulticastTtl, libc::IPPROTO_IP, libc::IP_MULTICAST_TTL, u8);
sockopt_impl!(Both, IpMulticastLoop, libc::IPPROTO_IP, libc::IP_MULTICAST_LOOP, bool);
sockopt_impl!(Both, ReceiveTimeout, libc::SOL_SOCKET, libc::SO_RCVTIMEO, TimeVal);
sockopt_impl!(Both, SendTimeout, libc::SOL_SOCKET, libc::SO_SNDTIMEO, TimeVal);
sockopt_impl!(Both, Broadcast, libc::SOL_SOCKET, libc::SO_BROADCAST, bool);
sockopt_impl!(Both, OobInline, libc::SOL_SOCKET, libc::SO_OOBINLINE, bool);
sockopt_impl!(GetOnly, SocketError, libc::SOL_SOCKET, libc::SO_ERROR, i32);
sockopt_impl!(Both, KeepAlive, libc::SOL_SOCKET, libc::SO_KEEPALIVE, bool);
#[cfg(any(target_os = "android", target_os = "linux"))]
sockopt_impl!(GetOnly, PeerCredentials, libc::SOL_SOCKET, libc::SO_PEERCRED, super::UnixCredentials);
#[cfg(any(target_os = "ios",
          target_os = "macos"))]
sockopt_impl!(Both, TcpKeepAlive, libc::IPPROTO_TCP, libc::TCP_KEEPALIVE, u32);
#[cfg(any(target_os = "android",
          target_os = "dragonfly",
          target_os = "freebsd",
          target_os = "linux",
          target_os = "nacl"))]
sockopt_impl!(Both, TcpKeepIdle, libc::IPPROTO_TCP, libc::TCP_KEEPIDLE, u32);
sockopt_impl!(Both, RcvBuf, libc::SOL_SOCKET, libc::SO_RCVBUF, usize);
sockopt_impl!(Both, SndBuf, libc::SOL_SOCKET, libc::SO_SNDBUF, usize);
#[cfg(any(target_os = "android", target_os = "linux"))]
sockopt_impl!(SetOnly, RcvBufForce, libc::SOL_SOCKET, libc::SO_RCVBUFFORCE, usize);
#[cfg(any(target_os = "android", target_os = "linux"))]
sockopt_impl!(SetOnly, SndBufForce, libc::SOL_SOCKET, libc::SO_SNDBUFFORCE, usize);
sockopt_impl!(GetOnly, SockType, libc::SOL_SOCKET, libc::SO_TYPE, super::SockType);
sockopt_impl!(GetOnly, AcceptConn, libc::SOL_SOCKET, libc::SO_ACCEPTCONN, bool);
#[cfg(any(target_os = "android", target_os = "linux"))]
sockopt_impl!(Both, BindToDevice, libc::SOL_SOCKET, libc::SO_BINDTODEVICE, OsString<[u8; libc::IFNAMSIZ]>);
#[cfg(any(target_os = "android", target_os = "linux"))]
sockopt_impl!(GetOnly, OriginalDst, libc::SOL_IP, libc::SO_ORIGINAL_DST, libc::sockaddr_in);
sockopt_impl!(Both, ReceiveTimestamp, libc::SOL_SOCKET, libc::SO_TIMESTAMP, bool);
#[cfg(any(target_os = "android", target_os = "linux"))]
sockopt_impl!(Both, IpTransparent, libc::SOL_IP, libc::IP_TRANSPARENT, bool);
#[cfg(target_os = "openbsd")]
sockopt_impl!(Both, BindAny, libc::SOL_SOCKET, libc::SO_BINDANY, bool);
#[cfg(target_os = "freebsd")]
sockopt_impl!(Both, BindAny, libc::IPPROTO_IP, libc::IP_BINDANY, bool);
#[cfg(target_os = "linux")]
sockopt_impl!(Both, Mark, libc::SOL_SOCKET, libc::SO_MARK, u32);
#[cfg(any(target_os = "android", target_os = "linux"))]
sockopt_impl!(Both, PassCred, libc::SOL_SOCKET, libc::SO_PASSCRED, bool);
#[cfg(any(target_os = "freebsd", target_os = "linux"))] 
sockopt_impl!(Both, TcpCongestion, libc::IPPROTO_TCP, libc::TCP_CONGESTION, OsString<[u8; TCP_CA_NAME_MAX]>);
#[cfg(any(
    target_os = "android",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
))]
sockopt_impl!(Both, Ipv4PacketInfo, libc::IPPROTO_IP, libc::IP_PKTINFO, bool);
#[cfg(any(
    target_os = "android",
    target_os = "freebsd",
    target_os = "ios",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
))]
sockopt_impl!(Both, Ipv6RecvPacketInfo, libc::IPPROTO_IPV6, libc::IPV6_RECVPKTINFO, bool);
#[cfg(any(
    target_os = "freebsd",
    target_os = "ios",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
))]
sockopt_impl!(Both, Ipv4RecvIf, libc::IPPROTO_IP, libc::IP_RECVIF, bool);
#[cfg(any(
    target_os = "freebsd",
    target_os = "ios",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "openbsd",
))]
sockopt_impl!(Both, Ipv4RecvDstAddr, libc::IPPROTO_IP, libc::IP_RECVDSTADDR, bool);
#[cfg(target_os = "linux")]
sockopt_impl!(Both, UdpGsoSegment, libc::SOL_UDP, libc::UDP_SEGMENT, libc::c_int);
#[cfg(target_os = "linux")]
sockopt_impl!(Both, UdpGroSegment, libc::IPPROTO_UDP, libc::UDP_GRO, bool);

#[cfg(any(target_os = "android", target_os = "linux"))]
#[derive(Copy, Clone, Debug)]
pub struct AlgSetAeadAuthSize;

// ALG_SET_AEAD_AUTH_SIZE read the length from passed `option_len`
// See https://elixir.bootlin.com/linux/v4.4/source/crypto/af_alg.c#L222
#[cfg(any(target_os = "android", target_os = "linux"))]
impl SetSockOpt for AlgSetAeadAuthSize {
    type Val = usize;

    fn set(&self, fd: RawFd, val: &usize) -> Result<()> {
        unsafe {
            let res = libc::setsockopt(fd,
                                       libc::SOL_ALG,
                                       libc::ALG_SET_AEAD_AUTHSIZE,
                                       ::std::ptr::null(),
                                       *val as libc::socklen_t);
            Errno::result(res).map(drop)
        }
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
#[derive(Clone, Debug)]
pub struct AlgSetKey<T>(::std::marker::PhantomData<T>);

#[cfg(any(target_os = "android", target_os = "linux"))]
impl<T> Default for AlgSetKey<T> {
    fn default() -> Self {
        AlgSetKey(Default::default())
    }
}

#[cfg(any(target_os = "android", target_os = "linux"))]
impl<T> SetSockOpt for AlgSetKey<T> where T: AsRef<[u8]> + Clone {
    type Val = T;

    fn set(&self, fd: RawFd, val: &T) -> Result<()> {
        unsafe {
            let res = libc::setsockopt(fd,
                                       libc::SOL_ALG,
                                       libc::ALG_SET_KEY,
                                       val.as_ref().as_ptr() as *const _,
                                       val.as_ref().len() as libc::socklen_t);
            Errno::result(res).map(drop)
        }
    }
}

/*
 *
 * ===== Accessor helpers =====
 *
 */

/// Helper trait that describes what is expected from a `GetSockOpt` getter.
unsafe trait Get<T> {
    /// Returns an uninitialized value.
    unsafe fn uninit() -> Self;
    /// Returns a pointer to the stored value. This pointer will be passed to the system's
    /// `getsockopt` call (`man 3p getsockopt`, argument `option_value`).
    fn ffi_ptr(&mut self) -> *mut c_void;
    /// Returns length of the stored value. This pointer will be passed to the system's
    /// `getsockopt` call (`man 3p getsockopt`, argument `option_len`).
    fn ffi_len(&mut self) -> *mut socklen_t;
    /// Returns the hopefully initialized inner value.
    unsafe fn assume_init(self) -> T;
}

/// Helper trait that describes what is expected from a `SetSockOpt` setter.
unsafe trait Set<'a, T> {
    /// Initialize the setter with a given value.
    fn new(val: &'a T) -> Self;
    /// Returns a pointer to the stored value. This pointer will be passed to the system's
    /// `setsockopt` call (`man 3p setsockopt`, argument `option_value`).
    fn ffi_ptr(&self) -> *const c_void;
    /// Returns length of the stored value. This pointer will be passed to the system's
    /// `setsockopt` call (`man 3p setsockopt`, argument `option_len`).
    fn ffi_len(&self) -> socklen_t;
}

/// Getter for an arbitrary `struct`.
struct GetStruct<T> {
    len: socklen_t,
    val: MaybeUninit<T>,
}

unsafe impl<T> Get<T> for GetStruct<T> {
    unsafe fn uninit() -> Self {
        GetStruct {
            len: mem::size_of::<T>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr() as *mut c_void
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> T {
        assert_eq!(self.len as usize, mem::size_of::<T>(), "invalid getsockopt implementation");
        self.val.assume_init()
    }
}

/// Setter for an arbitrary `struct`.
struct SetStruct<'a, T: 'static> {
    ptr: &'a T,
}

unsafe impl<'a, T> Set<'a, T> for SetStruct<'a, T> {
    fn new(ptr: &'a T) -> SetStruct<'a, T> {
        SetStruct { ptr }
    }

    fn ffi_ptr(&self) -> *const c_void {
        self.ptr as *const T as *const c_void
    }

    fn ffi_len(&self) -> socklen_t {
        mem::size_of::<T>() as socklen_t
    }
}

/// Getter for a boolean value.
struct GetBool {
    len: socklen_t,
    val: MaybeUninit<c_int>,
}

unsafe impl Get<bool> for GetBool {
    unsafe fn uninit() -> Self {
        GetBool {
            len: mem::size_of::<c_int>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr() as *mut c_void
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> bool {
        assert_eq!(self.len as usize, mem::size_of::<c_int>(), "invalid getsockopt implementation");
        self.val.assume_init() != 0
    }
}

/// Setter for a boolean value.
struct SetBool {
    val: c_int,
}

unsafe impl<'a> Set<'a, bool> for SetBool {
    fn new(val: &'a bool) -> SetBool {
        SetBool { val: if *val { 1 } else { 0 } }
    }

    fn ffi_ptr(&self) -> *const c_void {
        &self.val as *const c_int as *const c_void
    }

    fn ffi_len(&self) -> socklen_t {
        mem::size_of::<c_int>() as socklen_t
    }
}

/// Getter for an `u8` value.
struct GetU8 {
    len: socklen_t,
    val: MaybeUninit<u8>,
}

unsafe impl Get<u8> for GetU8 {
    unsafe fn uninit() -> Self {
        GetU8 {
            len: mem::size_of::<u8>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr() as *mut c_void
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> u8 {
        assert_eq!(self.len as usize, mem::size_of::<u8>(), "invalid getsockopt implementation");
        self.val.assume_init()
    }
}

/// Setter for an `u8` value.
struct SetU8 {
    val: u8,
}

unsafe impl<'a> Set<'a, u8> for SetU8 {
    fn new(val: &'a u8) -> SetU8 {
        SetU8 { val: *val as u8 }
    }

    fn ffi_ptr(&self) -> *const c_void {
        &self.val as *const u8 as *const c_void
    }

    fn ffi_len(&self) -> socklen_t {
        mem::size_of::<c_int>() as socklen_t
    }
}

/// Getter for an `usize` value.
struct GetUsize {
    len: socklen_t,
    val: MaybeUninit<c_int>,
}

unsafe impl Get<usize> for GetUsize {
    unsafe fn uninit() -> Self {
        GetUsize {
            len: mem::size_of::<c_int>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr() as *mut c_void
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> usize {
        assert_eq!(self.len as usize, mem::size_of::<c_int>(), "invalid getsockopt implementation");
        self.val.assume_init() as usize
    }
}

/// Setter for an `usize` value.
struct SetUsize {
    val: c_int,
}

unsafe impl<'a> Set<'a, usize> for SetUsize {
    fn new(val: &'a usize) -> SetUsize {
        SetUsize { val: *val as c_int }
    }

    fn ffi_ptr(&self) -> *const c_void {
        &self.val as *const c_int as *const c_void
    }

    fn ffi_len(&self) -> socklen_t {
        mem::size_of::<c_int>() as socklen_t
    }
}

/// Getter for a `OsString` value.
struct GetOsString<T: AsMut<[u8]>> {
    len: socklen_t,
    val: MaybeUninit<T>,
}

unsafe impl<T: AsMut<[u8]>> Get<OsString> for GetOsString<T> {
    unsafe fn uninit() -> Self {
        GetOsString {
            len: mem::size_of::<T>() as socklen_t,
            val: MaybeUninit::uninit(),
        }
    }

    fn ffi_ptr(&mut self) -> *mut c_void {
        self.val.as_mut_ptr() as *mut c_void
    }

    fn ffi_len(&mut self) -> *mut socklen_t {
        &mut self.len
    }

    unsafe fn assume_init(self) -> OsString {
        let len = self.len as usize;
        let mut v = self.val.assume_init();
        OsStr::from_bytes(&v.as_mut()[0..len]).to_owned()
    }
}

/// Setter for a `OsString` value.
struct SetOsString<'a> {
    val: &'a OsStr,
}

unsafe impl<'a> Set<'a, OsString> for SetOsString<'a> {
    fn new(val: &'a OsString) -> SetOsString {
        SetOsString { val: val.as_os_str() }
    }

    fn ffi_ptr(&self) -> *const c_void {
        self.val.as_bytes().as_ptr() as *const c_void
    }

    fn ffi_len(&self) -> socklen_t {
        self.val.len() as socklen_t
    }
}


#[cfg(test)]
mod test {
    #[cfg(any(target_os = "android", target_os = "linux"))]
    #[test]
    fn can_get_peercred_on_unix_socket() {
        use super::super::*;

        let (a, b) = socketpair(AddressFamily::Unix, SockType::Stream, None, SockFlag::empty()).unwrap();
        let a_cred = getsockopt(a, super::PeerCredentials).unwrap();
        let b_cred = getsockopt(b, super::PeerCredentials).unwrap();
        assert_eq!(a_cred, b_cred);
        assert!(a_cred.pid() != 0);
    }

    #[test]
    fn is_socket_type_unix() {
        use super::super::*;
        use crate::unistd::close;

        let (a, b) = socketpair(AddressFamily::Unix, SockType::Stream, None, SockFlag::empty()).unwrap();
        let a_type = getsockopt(a, super::SockType).unwrap();
        assert_eq!(a_type, SockType::Stream);
        close(a).unwrap();
        close(b).unwrap();
    }

    #[test]
    fn is_socket_type_dgram() {
        use super::super::*;
        use crate::unistd::close;

        let s = socket(AddressFamily::Inet, SockType::Datagram, SockFlag::empty(), None).unwrap();
        let s_type = getsockopt(s, super::SockType).unwrap();
        assert_eq!(s_type, SockType::Datagram);
        close(s).unwrap();
    }

    #[cfg(any(target_os = "freebsd",
              target_os = "linux",
              target_os = "nacl"))]
    #[test]
    fn can_get_listen_on_tcp_socket() {
        use super::super::*;
        use crate::unistd::close;

        let s = socket(AddressFamily::Inet, SockType::Stream, SockFlag::empty(), None).unwrap();
        let s_listening = getsockopt(s, super::AcceptConn).unwrap();
        assert!(!s_listening);
        listen(s, 10).unwrap();
        let s_listening2 = getsockopt(s, super::AcceptConn).unwrap();
        assert!(s_listening2);
        close(s).unwrap();
    }

}
