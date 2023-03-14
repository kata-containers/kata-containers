//! Support for Unix domain socket clients and servers.
#![warn(missing_docs)]
#![doc(html_root_url="https://doc.rust-lang.org/unix-socket/doc/v0.5.0")]

extern crate libc;

use std::ascii;
use std::cmp::{self, Ordering};
use std::convert::AsRef;
use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::iter::IntoIterator;
use std::mem;
use std::net::Shutdown;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{RawFd, AsRawFd, FromRawFd, IntoRawFd};
use std::path::Path;
use std::time::{Duration, Instant};

use libc::c_int;

fn sun_path_offset() -> usize {
    unsafe {
        // Work with an actual instance of the type since using a null pointer is UB
        let addr: libc::sockaddr_un = mem::uninitialized();
        let base = &addr as *const _ as usize;
        let path = &addr.sun_path as *const _ as usize;
        path - base
    }
}

fn cvt(v: libc::c_int) -> io::Result<libc::c_int> {
    if v < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(v)
    }
}

fn cvt_s(v: libc::ssize_t) -> io::Result<libc::ssize_t> {
    if v < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(v)
    }
}

struct Inner(RawFd);

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.0);
        }
    }
}

impl Inner {
    fn new(kind: libc::c_int) -> io::Result<Inner> {
        unsafe { cvt(libc::socket(libc::AF_UNIX, kind, 0)).map(Inner) }
    }

    fn new_pair(kind: libc::c_int) -> io::Result<(Inner, Inner)> {
        unsafe {
            let mut fds = [0, 0];
            try!(cvt(libc::socketpair(libc::AF_UNIX, kind, 0, fds.as_mut_ptr())));
            Ok((Inner(fds[0]), Inner(fds[1])))
        }
    }

    fn try_clone(&self) -> io::Result<Inner> {
        unsafe { cvt(libc::dup(self.0)).map(Inner) }
    }

    fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let how = match how {
            Shutdown::Read => libc::SHUT_RD,
            Shutdown::Write => libc::SHUT_WR,
            Shutdown::Both => libc::SHUT_RDWR,
        };

        unsafe { cvt(libc::shutdown(self.0, how)).map(|_| ()) }
    }

    fn timeout(&self, kind: libc::c_int) -> io::Result<Option<Duration>> {
        let timeout = unsafe {
            let mut timeout: libc::timeval = mem::zeroed();
            let mut size = mem::size_of::<libc::timeval>() as libc::socklen_t;
            try!(cvt(libc::getsockopt(self.0,
                                      libc::SOL_SOCKET,
                                      kind,
                                      &mut timeout as *mut _ as *mut _,
                                      &mut size as *mut _ as *mut _)));
            timeout
        };

        if timeout.tv_sec == 0 && timeout.tv_usec == 0 {
            Ok(None)
        } else {
            Ok(Some(Duration::new(timeout.tv_sec as u64, (timeout.tv_usec as u32) * 1000)))
        }
    }

    fn set_timeout(&self, dur: Option<Duration>, kind: libc::c_int) -> io::Result<()> {
        let timeout = match dur {
            Some(dur) => {
                if dur.as_secs() == 0 && dur.subsec_nanos() == 0 {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                              "cannot set a 0 duration timeout"));
                }

                let (secs, usecs) = if dur.as_secs() > libc::time_t::max_value() as u64 {
                    (libc::time_t::max_value(), 999_999)
                } else {
                    (dur.as_secs() as libc::time_t,
                     (dur.subsec_nanos() / 1000) as libc::suseconds_t)
                };
                let mut timeout = libc::timeval {
                    tv_sec: secs,
                    tv_usec: usecs,
                };
                if timeout.tv_sec == 0 && timeout.tv_usec == 0 {
                    timeout.tv_usec = 1;
                }
                timeout
            }
            None => {
                libc::timeval {
                    tv_sec: 0,
                    tv_usec: 0,
                }
            }
        };

        unsafe {
            cvt(libc::setsockopt(self.0,
                                 libc::SOL_SOCKET,
                                 kind,
                                 &timeout as *const _ as *const _,
                                 mem::size_of::<libc::timeval>() as libc::socklen_t))
                .map(|_| ())
        }
    }

    fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        let mut nonblocking = nonblocking as libc::c_ulong;
        unsafe { cvt(libc::ioctl(self.0, libc::FIONBIO, &mut nonblocking)).map(|_| ()) }
    }

    fn take_error(&self) -> io::Result<Option<io::Error>> {
        let mut errno: libc::c_int = 0;

        unsafe {
            try!(cvt(libc::getsockopt(self.0,
                                      libc::SOL_SOCKET,
                                      libc::SO_ERROR,
                                      &mut errno as *mut _ as *mut _,
                                      &mut mem::size_of_val(&errno) as *mut _ as *mut _)));
        }

        if errno == 0 {
            Ok(None)
        } else {
            Ok(Some(io::Error::from_raw_os_error(errno)))
        }
    }
}

unsafe fn sockaddr_un<P: AsRef<Path>>(path: P) -> io::Result<(libc::sockaddr_un, libc::socklen_t)> {
    let mut addr: libc::sockaddr_un = mem::zeroed();
    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;

    let bytes = path.as_ref().as_os_str().as_bytes();

    match (bytes.get(0), bytes.len().cmp(&addr.sun_path.len())) {
        // Abstract paths don't need a null terminator
        (Some(&0), Ordering::Greater) => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                      "path must be no longer than SUN_LEN"));
        }
        (Some(&0), _) => {},
        (_, Ordering::Greater) | (_, Ordering::Equal) => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                      "path must be shorter than SUN_LEN"));
        }
        _ => {}
    }
    for (dst, src) in addr.sun_path.iter_mut().zip(bytes.iter()) {
        *dst = *src as libc::c_char;
    }
    // null byte for pathname addresses is already there because we zeroed the
    // struct

    let mut len = sun_path_offset() + bytes.len();
    match bytes.get(0) {
        Some(&0) | None => {}
        Some(_) => len += 1,
    }
    Ok((addr, len as libc::socklen_t))
}

enum AddressKind<'a> {
    Unnamed,
    Pathname(&'a Path),
    Abstract(&'a [u8]),
}

/// An address associated with a Unix socket.
#[derive(Clone)]
pub struct SocketAddr {
    addr: libc::sockaddr_un,
    len: libc::socklen_t,
}

impl SocketAddr {
    fn new<F>(f: F) -> io::Result<SocketAddr>
        where F: FnOnce(*mut libc::sockaddr, *mut libc::socklen_t) -> libc::c_int
    {
        unsafe {
            let mut addr: libc::sockaddr_un = mem::zeroed();
            let mut len = mem::size_of::<libc::sockaddr_un>() as libc::socklen_t;
            try!(cvt(f(&mut addr as *mut _ as *mut _, &mut len)));

            if len == 0 {
                // When there is a datagram from unnamed unix socket
                // linux returns zero bytes of address
                len = sun_path_offset() as libc::socklen_t;  // i.e. zero-length address
            } else if addr.sun_family != libc::AF_UNIX as libc::sa_family_t {
                return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                          "file descriptor did not correspond to a Unix socket"));
            }

            Ok(SocketAddr {
                addr: addr,
                len: len,
            })
        }
    }

    /// Returns true iff the address is unnamed.
    pub fn is_unnamed(&self) -> bool {
        if let AddressKind::Unnamed = self.address() {
            true
        } else {
            false
        }
    }

    /// Returns the contents of this address if it is a `pathname` address.
    pub fn as_pathname(&self) -> Option<&Path> {
        if let AddressKind::Pathname(path) = self.address() {
            Some(path)
        } else {
            None
        }
    }

    fn address<'a>(&'a self) -> AddressKind<'a> {
        let len = self.len as usize - sun_path_offset();
        let path = unsafe { mem::transmute::<&[libc::c_char], &[u8]>(&self.addr.sun_path) };

        // OSX seems to return a len of 16 and a zeroed sun_path for unnamed addresses
        if len == 0 || (cfg!(not(target_os = "linux")) && self.addr.sun_path[0] == 0) {
            AddressKind::Unnamed
        } else if self.addr.sun_path[0] == 0 {
            AddressKind::Abstract(&path[1..len])
        } else {
            AddressKind::Pathname(OsStr::from_bytes(&path[..len - 1]).as_ref())
        }
    }
}

impl fmt::Debug for SocketAddr {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match self.address() {
            AddressKind::Unnamed => write!(fmt, "(unnamed)"),
            AddressKind::Abstract(name) => write!(fmt, "{} (abstract)", AsciiEscaped(name)),
            AddressKind::Pathname(path) => write!(fmt, "{:?} (pathname)", path),
        }
    }
}

struct AsciiEscaped<'a>(&'a [u8]);

impl<'a> fmt::Display for AsciiEscaped<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(fmt, "\""));
        for byte in self.0.iter().cloned().flat_map(ascii::escape_default) {
            try!(write!(fmt, "{}", byte as char));
        }
        write!(fmt, "\"")
    }
}

/// OS specific extension traits.
pub mod os {
    /// Linux specific extension traits.
    #[cfg(target_os = "linux")]
    pub mod linux {
        use {AddressKind, SocketAddr};

        /// Linux specific extensions for the `SocketAddr` type.
        pub trait SocketAddrExt {
            /// Returns the contents of this address (without the leading
            /// null byte) if it is an `abstract` address.
            fn as_abstract(&self) -> Option<&[u8]>;
        }

        impl SocketAddrExt for SocketAddr {
            fn as_abstract(&self) -> Option<&[u8]> {
                if let AddressKind::Abstract(path) = self.address() {
                    Some(path)
                } else {
                    None
                }
            }
        }
    }
}

/// A Unix stream socket.
///
/// # Examples
///
/// ```rust,no_run
/// use unix_socket::UnixStream;
/// use std::io::prelude::*;
///
/// let mut stream = UnixStream::connect("/path/to/my/socket").unwrap();
/// stream.write_all(b"hello world").unwrap();
/// let mut response = String::new();
/// stream.read_to_string(&mut response).unwrap();
/// println!("{}", response);
/// ```
pub struct UnixStream {
    inner: Inner,
}

impl fmt::Debug for UnixStream {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = fmt.debug_struct("UnixStream");
        builder.field("fd", &self.inner.0);
        if let Ok(addr) = self.local_addr() {
            builder.field("local", &addr);
        }
        if let Ok(addr) = self.peer_addr() {
            builder.field("peer", &addr);
        }
        builder.finish()
    }
}

impl UnixStream {
    /// Connects to the socket named by `path`.
    ///
    /// Linux provides, as a nonportable extension, a separate "abstract"
    /// address namespace as opposed to filesystem-based addressing. If `path`
    /// begins with a null byte, it will be interpreted as an "abstract"
    /// address. Otherwise, it will be interpreted as a "pathname" address,
    /// corresponding to a path on the filesystem.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// ```
    pub fn connect<P: AsRef<Path>>(path: P) -> io::Result<UnixStream> {
        unsafe {
            let inner = try!(Inner::new(libc::SOCK_STREAM));
            let (addr, len) = try!(sockaddr_un(path));

            let ret = libc::connect(inner.0, &addr as *const _ as *const _, len);
            if ret < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(UnixStream { inner: inner })
            }
        }
    }

    /// As `connect`, but time out after a specified duration.
    pub fn connect_timeout<P: AsRef<Path>>(path: P, timeout: Duration) -> io::Result<UnixStream> {
        let inner = try!(Inner::new(libc::SOCK_STREAM));

        inner.set_nonblocking(true)?;
        let r = unsafe {
            let (addr, len) = try!(sockaddr_un(path));
            let ret = libc::connect(inner.0, &addr as *const _ as *const _, len);
            if ret < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        };
        inner.set_nonblocking(false)?;

        match r {
            Ok(_) => return Ok(UnixStream { inner: inner }),
            // there's no ErrorKind for EINPROGRESS :(
            Err(ref e) if e.raw_os_error() == Some(libc::EINPROGRESS) => {}
            Err(e) => return Err(e),
        }

        let mut pollfd = libc::pollfd {
            fd: inner.0,
            events: libc::POLLOUT,
            revents: 0,
        };

        if timeout.as_secs() == 0 && timeout.subsec_nanos() == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                      "cannot set a 0 duration timeout"));
        }

        let start = Instant::now();

        loop {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "connection timed out"));
            }

            let timeout = timeout - elapsed;
            let mut timeout = timeout.as_secs()
                .saturating_mul(1_000)
                .saturating_add(timeout.subsec_nanos() as u64 / 1_000_000);
            if timeout == 0 {
                timeout = 1;
            }

            let timeout = cmp::min(timeout, c_int::max_value() as u64) as c_int;

            match unsafe { libc::poll(&mut pollfd, 1, timeout) } {
                -1 => {
                    let err = io::Error::last_os_error();
                    if err.kind() != io::ErrorKind::Interrupted {
                        return Err(err);
                    }
                }
                0 => {}
                _ => {
                    // linux returns POLLOUT|POLLERR|POLLHUP for refused connections (!), so look
                    // for POLLHUP rather than read readiness
                    if pollfd.revents & libc::POLLHUP != 0 {
                        let e = inner.take_error()?
                            .unwrap_or_else(|| {
                                io::Error::new(io::ErrorKind::Other, "no error set after POLLHUP")
                            });
                        return Err(e);
                    }

                    return Ok(UnixStream { inner: inner });
                }
            }
        }
    }


    /// Creates an unnamed pair of connected sockets.
    ///
    /// Returns two `UnixStream`s which are connected to each other.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    ///
    /// let (stream1, stream2) = UnixStream::pair().unwrap();
    /// ```
    pub fn pair() -> io::Result<(UnixStream, UnixStream)> {
        let (i1, i2) = try!(Inner::new_pair(libc::SOCK_STREAM));
        Ok((UnixStream { inner: i1 }, UnixStream { inner: i2 }))
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UnixStream` is a reference to the same stream that this
    /// object references. Both handles will read and write the same stream of
    /// data, and options set on one stream will be propogated to the other
    /// stream.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// let copy = stream.try_clone().unwrap();
    /// ```
    pub fn try_clone(&self) -> io::Result<UnixStream> {
        Ok(UnixStream { inner: try!(self.inner.try_clone()) })
    }

    /// Returns the socket address of the local half of this connection.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// println!("{}", match stream.local_addr() {
    ///     Ok(addr) => format!("local address: {:?}", addr),
    ///     Err(_) => "no local address".to_owned(),
    /// });
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { libc::getsockname(self.inner.0, addr, len) })
    }

    /// Returns the socket address of the remote half of this connection.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// println!("{}", match stream.peer_addr() {
    ///     Ok(addr) => format!("peer address: {:?}", addr),
    ///     Err(_) => "no peer address".to_owned(),
    /// });
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { libc::getpeername(self.inner.0, addr, len) })
    }

    /// Sets the read timeout for the socket.
    ///
    /// If the provided value is `None`, then `read` calls will block
    /// indefinitely. It is an error to pass the zero `Duration` to this
    /// method.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    /// use std::time::Duration;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// stream.set_read_timeout(Some(Duration::from_millis(1500))).unwrap();
    /// ```
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.inner.set_timeout(timeout, libc::SO_RCVTIMEO)
    }

    /// Sets the write timeout for the socket.
    ///
    /// If the provided value is `None`, then `write` calls will block
    /// indefinitely. It is an error to pass the zero `Duration` to this
    /// method.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    /// use std::time::Duration;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// stream.set_write_timeout(Some(Duration::from_millis(1500))).unwrap();
    /// ```
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.inner.set_timeout(timeout, libc::SO_SNDTIMEO)
    }

    /// Returns the read timeout of this socket.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// println!("{}", match stream.read_timeout() {
    ///     Ok(timeout) => format!("read timeout: {:?}", timeout),
    ///     Err(_) => "error".to_owned(),
    /// });
    /// ```
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.timeout(libc::SO_RCVTIMEO)
    }

    /// Returns the write timeout of this socket.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// println!("{}", match stream.write_timeout() {
    ///     Ok(timeout) => format!("write timeout: {:?}", timeout),
    ///     Err(_) => "error".to_owned(),
    /// });
    /// ```
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.timeout(libc::SO_SNDTIMEO)
    }

    /// Moves the socket into or out of nonblocking mode.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// stream.set_nonblocking(true).unwrap();
    /// ```
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }

    /// Returns the value of the `SO_ERROR` option.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixStream;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// println!("{}", match stream.take_error() {
    ///     Ok(ret) => format!("error: {:?}", ret),
    ///     Err(_) => "error".to_owned(),
    /// });
    /// ```
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Shuts down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of `Shutdown`).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::net::Shutdown;
    /// use unix_socket::UnixStream;
    ///
    /// let stream = UnixStream::connect("/path/to/my/socket").unwrap();
    /// stream.shutdown(Shutdown::Both).unwrap();
    /// ```
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }
}

impl io::Read for UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        io::Read::read(&mut &*self, buf)
    }
}

impl<'a> io::Read for &'a UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        unsafe {
            cvt_s(libc::recv(self.inner.0, buf.as_mut_ptr() as *mut _, buf.len(), 0))
                .map(|r| r as usize)
        }
    }
}

impl io::Write for UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        io::Write::write(&mut &*self, buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut &*self)
    }
}

impl<'a> io::Write for &'a UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            cvt_s(libc::send(self.inner.0, buf.as_ptr() as *const _, buf.len(), 0))
                .map(|r| r as usize)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsRawFd for UnixStream {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.0
    }
}

impl FromRawFd for UnixStream {
    unsafe fn from_raw_fd(fd: RawFd) -> UnixStream {
        UnixStream { inner: Inner(fd) }
    }
}

impl IntoRawFd for UnixStream {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.inner.0;
        mem::forget(self);
        fd
    }
}

/// A structure representing a Unix domain socket server.
///
/// # Examples
///
/// ```rust,no_run
/// use std::thread;
/// use unix_socket::{UnixStream, UnixListener};
///
/// fn handle_client(stream: UnixStream) {
///     // ...
/// }
///
/// let listener = UnixListener::bind("/path/to/the/socket").unwrap();
///
/// // accept connections and process them, spawning a new thread for each one
/// for stream in listener.incoming() {
///     match stream {
///         Ok(stream) => {
///             /* connection succeeded */
///             thread::spawn(|| handle_client(stream));
///         }
///         Err(err) => {
///             /* connection failed */
///             break;
///         }
///     }
/// }
///
/// // close the listener socket
/// drop(listener);
/// ```
pub struct UnixListener {
    inner: Inner,
}

impl fmt::Debug for UnixListener {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = fmt.debug_struct("UnixListener");
        builder.field("fd", &self.inner.0);
        if let Ok(addr) = self.local_addr() {
            builder.field("local", &addr);
        }
        builder.finish()
    }
}

impl UnixListener {
    /// Creates a new `UnixListener` bound to the specified socket.
    ///
    /// Linux provides, as a nonportable extension, a separate "abstract"
    /// address namespace as opposed to filesystem-based addressing. If `path`
    /// begins with a null byte, it will be interpreted as an "abstract"
    /// address. Otherwise, it will be interpreted as a "pathname" address,
    /// corresponding to a path on the filesystem.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixListener;
    ///
    /// let listener = UnixListener::bind("/path/to/the/socket").unwrap();
    /// ```
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixListener> {
        unsafe {
            let inner = try!(Inner::new(libc::SOCK_STREAM));
            let (addr, len) = try!(sockaddr_un(path));

            try!(cvt(libc::bind(inner.0, &addr as *const _ as *const _, len)));
            try!(cvt(libc::listen(inner.0, 128)));

            Ok(UnixListener { inner: inner })
        }
    }

    /// Accepts a new incoming connection to this listener.
    ///
    /// This function will block the calling thread until a new Unix connection
    /// is established. When established, the corersponding `UnixStream` and
    /// the remote peer's address will be returned.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixListener;
    ///
    /// let listener = UnixListener::bind("/path/to/the/socket").unwrap();
    /// let (client_stream, addr) = listener.accept().unwrap();
    /// ```
    pub fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        unsafe {
            let mut fd = 0;
            let addr = try!(SocketAddr::new(|addr, len| {
                fd = libc::accept(self.inner.0, addr, len);
                fd
            }));

            Ok((UnixStream { inner: Inner(fd) }, addr))
        }
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UnixListener` is a reference to the same socket that this
    /// object references. Both handles can be used to accept incoming
    /// connections and options set on one listener will affect the other.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixListener;
    ///
    /// let listener = UnixListener::bind("/path/to/the/socket").unwrap();
    /// let copy = listener.try_clone().unwrap();
    /// ```
    pub fn try_clone(&self) -> io::Result<UnixListener> {
        Ok(UnixListener { inner: try!(self.inner.try_clone()) })
    }

    /// Returns the local socket address of this listener.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixListener;
    ///
    /// let listener = UnixListener::bind("/path/to/the/socket").unwrap();
    /// println!("{}", match listener.local_addr() {
    ///     Ok(addr) => format!("local address: {:?}", addr),
    ///     Err(_) => "no local address".to_owned(),
    /// });
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { libc::getsockname(self.inner.0, addr, len) })
    }

    /// Moves the socket into or out of nonblocking mode.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixListener;
    ///
    /// let mut listener = UnixListener::bind("/path/to/the/socket").unwrap();
    /// listener.set_nonblocking(true).unwrap();
    /// ```
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }

    /// Returns the value of the `SO_ERROR` option.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixListener;
    ///
    /// let listener = UnixListener::bind("/path/to/the/socket").unwrap();
    /// println!("{}", match listener.take_error() {
    ///     Ok(ret) => format!("error: {:?}", ret),
    ///     Err(_) => "error".to_owned(),
    /// });
    /// ```
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Returns an iterator over incoming connections.
    ///
    /// The iterator will never return `None` and will also not yield the
    /// peer's `SocketAddr` structure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::thread;
    /// use unix_socket::{UnixStream, UnixListener};
    ///
    /// fn handle_client(stream: UnixStream) {
    ///     // ...
    /// }
    ///
    /// let listener = UnixListener::bind("/path/to/the/socket").unwrap();
    ///
    /// // accept connections and process them, spawning a new thread for each one
    /// for stream in listener.incoming() {
    ///     match stream {
    ///         Ok(stream) => {
    ///             /* connection succeeded */
    ///             println!("incoming connection succeeded!");
    ///             thread::spawn(|| handle_client(stream));
    ///         }
    ///         Err(err) => {
    ///             /* connection failed */
    ///             println!("incoming connection failed...");
    ///         }
    ///     }
    /// }
    /// ```
    pub fn incoming<'a>(&'a self) -> Incoming<'a> {
        Incoming { listener: self }
    }
}

impl AsRawFd for UnixListener {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.0
    }
}

impl FromRawFd for UnixListener {
    unsafe fn from_raw_fd(fd: RawFd) -> UnixListener {
        UnixListener { inner: Inner(fd) }
    }
}

impl IntoRawFd for UnixListener {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.inner.0;
        mem::forget(self);
        fd
    }
}

impl<'a> IntoIterator for &'a UnixListener {
    type Item = io::Result<UnixStream>;
    type IntoIter = Incoming<'a>;

    fn into_iter(self) -> Incoming<'a> {
        self.incoming()
    }
}

/// An iterator over incoming connections to a `UnixListener`.
///
/// It will never return `None`.
#[derive(Debug)]
pub struct Incoming<'a> {
    listener: &'a UnixListener,
}

impl<'a> Iterator for Incoming<'a> {
    type Item = io::Result<UnixStream>;

    fn next(&mut self) -> Option<io::Result<UnixStream>> {
        Some(self.listener.accept().map(|s| s.0))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::max_value(), None)
    }
}

/// A Unix datagram socket.
///
/// # Examples
///
/// ```rust,no_run
/// use unix_socket::UnixDatagram;
///
/// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
/// socket.send_to(b"hello world", "/path/to/other/socket").unwrap();
/// let mut buf = [0; 100];
/// let (count, address) = socket.recv_from(&mut buf).unwrap();
/// println!("socket {:?} sent us {:?}", address, &buf[..count]);
/// ```
pub struct UnixDatagram {
    inner: Inner,
}

impl fmt::Debug for UnixDatagram {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = fmt.debug_struct("UnixDatagram");
        builder.field("fd", &self.inner.0);
        if let Ok(addr) = self.local_addr() {
            builder.field("local", &addr);
        }
        if let Ok(addr) = self.peer_addr() {
            builder.field("peer", &addr);
        }
        builder.finish()
    }
}

impl UnixDatagram {
    /// Creates a Unix datagram socket bound to the given path.
    ///
    /// Linux provides, as a nonportable extension, a separate "abstract"
    /// address namespace as opposed to filesystem-based addressing. If `path`
    /// begins with a null byte, it will be interpreted as an "abstract"
    /// address. Otherwise, it will be interpreted as a "pathname" address,
    /// corresponding to a path on the filesystem.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// ```
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixDatagram> {
        unsafe {
            let inner = try!(Inner::new(libc::SOCK_DGRAM));
            let (addr, len) = try!(sockaddr_un(path));

            try!(cvt(libc::bind(inner.0, &addr as *const _ as *const _, len)));

            Ok(UnixDatagram { inner: inner })
        }
    }

    /// Creates a Unix Datagram socket which is not bound to any address.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::unbound().unwrap();
    /// ```
    pub fn unbound() -> io::Result<UnixDatagram> {
        let inner = try!(Inner::new(libc::SOCK_DGRAM));
        Ok(UnixDatagram { inner: inner })
    }

    /// Create an unnamed pair of connected sockets.
    ///
    /// Returns two `UnixDatagrams`s which are connected to each other.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let (socket1, socket2) = UnixDatagram::pair().unwrap();
    /// ```
    pub fn pair() -> io::Result<(UnixDatagram, UnixDatagram)> {
        let (i1, i2) = try!(Inner::new_pair(libc::SOCK_DGRAM));
        Ok((UnixDatagram { inner: i1 }, UnixDatagram { inner: i2 }))
    }

    /// Connects the socket to the specified address.
    ///
    /// The `send` method may be used to send data to the specified address.
    /// `recv` and `recv_from` will only receive data from that address.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// socket.connect("/path/to/other/socket").unwrap();
    /// ```
    pub fn connect<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        unsafe {
            let (addr, len) = try!(sockaddr_un(path));

            try!(cvt(libc::connect(self.inner.0, &addr as *const _ as *const _, len)));

            Ok(())
        }
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UnixListener` is a reference to the same socket that this
    /// object references. Both handles can be used to accept incoming
    /// connections and options set on one listener will affect the other.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::unbound().unwrap();
    /// let copy = socket.try_clone().unwrap();
    /// ```
    pub fn try_clone(&self) -> io::Result<UnixDatagram> {
        Ok(UnixDatagram { inner: try!(self.inner.try_clone()) })
    }

    /// Returns the address of this socket.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/the/socket").unwrap();
    /// println!("{}", match socket.local_addr() {
    ///     Ok(addr) => format!("local address: {:?}", addr),
    ///     Err(_) => "no local address".to_owned(),
    /// });
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { libc::getsockname(self.inner.0, addr, len) })
    }

    /// Returns the address of this socket's peer.
    ///
    /// The `connect` method will connect the socket to a peer.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/the/socket").unwrap();
    /// println!("{}", match socket.peer_addr() {
    ///     Ok(addr) => format!("peer address: {:?}", addr),
    ///     Err(_) => "no peer address".to_owned(),
    /// });
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { libc::getpeername(self.inner.0, addr, len) })
    }

    /// Receives data from the socket.
    ///
    /// On success, returns the number of bytes read and the address from
    /// whence the data came.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// let mut buf = [0; 100];
    /// let (count, address) = socket.recv_from(&mut buf).unwrap();
    /// println!("socket {:?} sent us {:?}", address, &buf[..count]);
    /// ```
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let mut count = 0;
        let addr = try!(SocketAddr::new(|addr, len| {
            unsafe {
                count = libc::recvfrom(self.inner.0,
                                       buf.as_mut_ptr() as *mut _,
                                       buf.len(),
                                       0,
                                       addr,
                                       len);
                if count > 0 {
                    1
                } else if count == 0 {
                    0
                } else {
                    -1
                }
            }
        }));

        Ok((count as usize, addr))
    }

    /// Receives data from the socket.
    ///
    /// On success, returns the number of bytes read.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// socket.connect("/path/to/other/socket").unwrap();
    /// let mut buf = [0; 100];
    /// let count = socket.recv(&mut buf).unwrap();
    /// println!("we received {:?}", &buf[..count]);
    /// ```
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        unsafe {
            let count = try!(cvt_s(libc::recv(self.inner.0,
                                              buf.as_mut_ptr() as *mut _,
                                              buf.len(),
                                              0)));
            Ok(count as usize)
        }
    }

    /// Sends data on the socket to the specified address.
    ///
    /// On success, returns the number of bytes written.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    ///
    /// let mut data = b"hello world";
    /// let mut sent = 0;
    /// while sent < data.len() {
    ///     match socket.send_to(&data[sent..(data.len() - sent)], "/path/to/other/socket") {
    ///         Ok(data_sent) => { sent += data_sent; }
    ///         Err(_) => {
    ///             println!("an error occured while sending data...");
    ///             break;
    ///         }
    ///     }
    /// }
    /// ```
    pub fn send_to<P: AsRef<Path>>(&self, buf: &[u8], path: P) -> io::Result<usize> {
        unsafe {
            let (addr, len) = try!(sockaddr_un(path));

            let count = try!(cvt_s(libc::sendto(self.inner.0,
                                                buf.as_ptr() as *const _,
                                                buf.len(),
                                                0,
                                                &addr as *const _ as *const _,
                                                len)));
            Ok(count as usize)
        }
    }

    /// Sends data on the socket to the socket's peer.
    ///
    /// The peer address may be set by the `connect` method, and this method
    /// will return an error if the socket has not already been connected.
    ///
    /// On success, returns the number of bytes written.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// socket.connect("/path/to/other/socket").unwrap();
    ///
    /// let mut data = b"hello world";
    /// let mut sent = 0;
    /// while sent < data.len() {
    ///     match socket.send(&data[sent..(data.len() - sent)]) {
    ///         Ok(data_sent) => { sent += data_sent; }
    ///         Err(_) => {
    ///             println!("an error occured while sending data...");
    ///             break;
    ///         }
    ///     }
    /// }
    /// ```
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            let count = try!(cvt_s(libc::send(self.inner.0,
                                              buf.as_ptr() as *const _,
                                              buf.len(),
                                              0)));
            Ok(count as usize)
        }
    }

    /// Sets the read timeout for the socket.
    ///
    /// If the provided value is `None`, then `recv` and `recv_from` calls will
    /// block indefinitely. It is an error to pass the zero `Duration` to this
    /// method.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// socket.set_read_timeout(Some(Duration::from_millis(1500))).unwrap();
    /// ```
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.inner.set_timeout(timeout, libc::SO_RCVTIMEO)
    }

    /// Sets the write timeout for the socket.
    ///
    /// If the provided value is `None`, then `send` and `send_to` calls will
    /// block indefinitely. It is an error to pass the zero `Duration` to this
    /// method.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// socket.set_write_timeout(Some(Duration::from_millis(1500))).unwrap();
    /// ```
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.inner.set_timeout(timeout, libc::SO_SNDTIMEO)
    }

    /// Returns the read timeout of this socket.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// println!("{}", match socket.read_timeout() {
    ///     Ok(timeout) => format!("read timeout: {:?}", timeout),
    ///     Err(_) => "error".to_owned(),
    /// });
    /// ```
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.timeout(libc::SO_RCVTIMEO)
    }

    /// Returns the write timeout of this socket.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// println!("{}", match socket.write_timeout() {
    ///     Ok(timeout) => format!("write timeout: {:?}", timeout),
    ///     Err(_) => "error".to_owned(),
    /// });
    /// ```
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.timeout(libc::SO_SNDTIMEO)
    }

    /// Moves the socket into or out of nonblocking mode.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// socket.set_nonblocking(true).unwrap();
    /// ```
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }

    /// Returns the value of the `SO_ERROR` option.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/the/socket").unwrap();
    /// println!("{}", match socket.take_error() {
    ///     Ok(ret) => format!("error: {:?}", ret),
    ///     Err(_) => "error".to_owned(),
    /// });
    /// ```
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    /// Shut down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of `Shutdown`).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use std::net::Shutdown;
    /// use unix_socket::UnixDatagram;
    ///
    /// let socket = UnixDatagram::bind("/path/to/my/socket").unwrap();
    /// socket.connect("/path/to/other/socket").unwrap();
    /// socket.shutdown(Shutdown::Both).unwrap();
    /// ```
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }
}

impl AsRawFd for UnixDatagram {
    fn as_raw_fd(&self) -> RawFd {
        self.inner.0
    }
}

impl FromRawFd for UnixDatagram {
    unsafe fn from_raw_fd(fd: RawFd) -> UnixDatagram {
        UnixDatagram { inner: Inner(fd) }
    }
}

impl IntoRawFd for UnixDatagram {
    fn into_raw_fd(self) -> RawFd {
        let fd = self.inner.0;
        mem::forget(self);
        fd
    }
}

#[cfg(test)]
mod test {
    extern crate tempdir;
    extern crate libc;

    use std::thread;
    use std::io;
    use std::io::prelude::*;
    use std::time::Duration;
    use self::tempdir::TempDir;
    use std::net::Shutdown;

    use super::*;

    macro_rules! or_panic {
        ($e:expr) => {
            match $e {
                Ok(e) => e,
                Err(e) => panic!("{}", e),
            }
        }
    }

    #[test]
    fn basic() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let socket_path = dir.path().join("sock");
        let msg1 = b"hello";
        let msg2 = b"world!";

        let listener = or_panic!(UnixListener::bind(&socket_path));
        let thread = thread::spawn(move || {
            let mut stream = or_panic!(listener.accept()).0;
            let mut buf = [0; 5];
            or_panic!(stream.read(&mut buf));
            assert_eq!(&msg1[..], &buf[..]);
            or_panic!(stream.write_all(msg2));
        });

        let mut stream = or_panic!(UnixStream::connect(&socket_path));
        assert_eq!(Some(&*socket_path),
                   stream.peer_addr().unwrap().as_pathname());
        or_panic!(stream.write_all(msg1));
        let mut buf = vec![];
        or_panic!(stream.read_to_end(&mut buf));
        assert_eq!(&msg2[..], &buf[..]);
        drop(stream);

        thread.join().unwrap();
    }

    #[test]
    fn pair() {
        let msg1 = b"hello";
        let msg2 = b"world!";

        let (mut s1, mut s2) = or_panic!(UnixStream::pair());
        let thread = thread::spawn(move || {
            // s1 must be moved in or the test will hang!
            let mut buf = [0; 5];
            or_panic!(s1.read(&mut buf));
            assert_eq!(&msg1[..], &buf[..]);
            or_panic!(s1.write_all(msg2));
        });

        or_panic!(s2.write_all(msg1));
        let mut buf = vec![];
        or_panic!(s2.read_to_end(&mut buf));
        assert_eq!(&msg2[..], &buf[..]);
        drop(s2);

        thread.join().unwrap();
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn abstract_address() {
        use os::linux::SocketAddrExt;

        let socket_path = "\0the path";
        let msg1 = b"hello";
        let msg2 = b"world!";

        let listener = or_panic!(UnixListener::bind(&socket_path));
        let thread = thread::spawn(move || {
            let mut stream = or_panic!(listener.accept()).0;
            let mut buf = [0; 5];
            or_panic!(stream.read(&mut buf));
            assert_eq!(&msg1[..], &buf[..]);
            or_panic!(stream.write_all(msg2));
        });

        let mut stream = or_panic!(UnixStream::connect(&socket_path));
        assert_eq!(Some(&b"the path"[..]),
                   stream.peer_addr().unwrap().as_abstract());
        or_panic!(stream.write_all(msg1));
        let mut buf = vec![];
        or_panic!(stream.read_to_end(&mut buf));
        assert_eq!(&msg2[..], &buf[..]);
        drop(stream);

        thread.join().unwrap();
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn abstract_address_max_len() {
        use os::linux::SocketAddrExt;
        use std::ffi::OsStr;
        use std::io::Write;
        use std::mem;
        use std::os::unix::ffi::OsStrExt;

        let len = unsafe {
            let addr: libc::sockaddr_un = mem::zeroed();
            addr.sun_path.len()
        };

        let mut socket_path = vec![0; len];
        (&mut socket_path[1..9]).write_all(b"the path").unwrap();
        let socket_path: &OsStr = OsStr::from_bytes(&socket_path).into();

        let msg1 = b"hello";
        let msg2 = b"world!";

        let listener = or_panic!(UnixListener::bind(&socket_path));
        let thread = thread::spawn(move || {
            let mut stream = or_panic!(listener.accept()).0;
            let mut buf = [0; 5];
            or_panic!(stream.read(&mut buf));
            assert_eq!(&msg1[..], &buf[..]);
            or_panic!(stream.write_all(msg2));
        });

        let mut stream = or_panic!(UnixStream::connect(&socket_path));
        assert_eq!(Some(&socket_path.as_bytes()[1..]),
                   stream.peer_addr().unwrap().as_abstract());
        or_panic!(stream.write_all(msg1));
        let mut buf = vec![];
        or_panic!(stream.read_to_end(&mut buf));
        assert_eq!(&msg2[..], &buf[..]);
        drop(stream);

        thread.join().unwrap();
    }

    #[test]
    fn try_clone() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let socket_path = dir.path().join("sock");
        let msg1 = b"hello";
        let msg2 = b"world";

        let listener = or_panic!(UnixListener::bind(&socket_path));
        let thread = thread::spawn(move || {
            let mut stream = or_panic!(listener.accept()).0;
            or_panic!(stream.write_all(msg1));
            or_panic!(stream.write_all(msg2));
        });

        let mut stream = or_panic!(UnixStream::connect(&socket_path));
        let mut stream2 = or_panic!(stream.try_clone());

        let mut buf = [0; 5];
        or_panic!(stream.read(&mut buf));
        assert_eq!(&msg1[..], &buf[..]);
        or_panic!(stream2.read(&mut buf));
        assert_eq!(&msg2[..], &buf[..]);

        thread.join().unwrap();
    }

    #[test]
    fn iter() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let socket_path = dir.path().join("sock");

        let listener = or_panic!(UnixListener::bind(&socket_path));
        let thread = thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut stream = or_panic!(stream);
                let mut buf = [0];
                or_panic!(stream.read(&mut buf));
            }
        });

        for _ in 0..2 {
            let mut stream = or_panic!(UnixStream::connect(&socket_path));
            or_panic!(stream.write_all(&[0]));
        }

        thread.join().unwrap();
    }

    #[test]
    fn long_path() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let socket_path = dir.path()
                             .join("asdfasdfasdfasdfasdfasdfasdfasdfasdfasdfasdfasdfasdfasdfasdfa\
                                    sasdfasdfasdasdfasdfasdfadfasdfasdfasdfasdfasdf");
        match UnixStream::connect(&socket_path) {
            Err(ref e) if e.kind() == io::ErrorKind::InvalidInput => {}
            Err(e) => panic!("unexpected error {}", e),
            Ok(_) => panic!("unexpected success"),
        }

        match UnixListener::bind(&socket_path) {
            Err(ref e) if e.kind() == io::ErrorKind::InvalidInput => {}
            Err(e) => panic!("unexpected error {}", e),
            Ok(_) => panic!("unexpected success"),
        }

        match UnixDatagram::bind(&socket_path) {
            Err(ref e) if e.kind() == io::ErrorKind::InvalidInput => {}
            Err(e) => panic!("unexpected error {}", e),
            Ok(_) => panic!("unexpected success"),
        }
    }

    #[test]
    fn timeouts() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let socket_path = dir.path().join("sock");

        let _listener = or_panic!(UnixListener::bind(&socket_path));

        let stream = or_panic!(UnixStream::connect(&socket_path));
        let dur = Duration::new(15410, 0);

        assert_eq!(None, or_panic!(stream.read_timeout()));

        or_panic!(stream.set_read_timeout(Some(dur)));
        assert_eq!(Some(dur), or_panic!(stream.read_timeout()));

        assert_eq!(None, or_panic!(stream.write_timeout()));

        or_panic!(stream.set_write_timeout(Some(dur)));
        assert_eq!(Some(dur), or_panic!(stream.write_timeout()));

        or_panic!(stream.set_read_timeout(None));
        assert_eq!(None, or_panic!(stream.read_timeout()));

        or_panic!(stream.set_write_timeout(None));
        assert_eq!(None, or_panic!(stream.write_timeout()));
    }

    #[test]
    fn test_read_timeout() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let socket_path = dir.path().join("sock");

        let _listener = or_panic!(UnixListener::bind(&socket_path));

        let mut stream = or_panic!(UnixStream::connect(&socket_path));
        or_panic!(stream.set_read_timeout(Some(Duration::from_millis(1000))));

        let mut buf = [0; 10];
        let kind = stream.read(&mut buf).err().expect("expected error").kind();
        assert!(kind == io::ErrorKind::WouldBlock || kind == io::ErrorKind::TimedOut);
    }

    #[test]
    fn test_read_with_timeout() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let socket_path = dir.path().join("sock");

        let listener = or_panic!(UnixListener::bind(&socket_path));

        let mut stream = or_panic!(UnixStream::connect(&socket_path));
        or_panic!(stream.set_read_timeout(Some(Duration::from_millis(1000))));

        let mut other_end = or_panic!(listener.accept()).0;
        or_panic!(other_end.write_all(b"hello world"));

        let mut buf = [0; 11];
        or_panic!(stream.read(&mut buf));
        assert_eq!(b"hello world", &buf[..]);

        let kind = stream.read(&mut buf).err().expect("expected error").kind();
        assert!(kind == io::ErrorKind::WouldBlock || kind == io::ErrorKind::TimedOut);
    }

    #[test]
    fn test_unix_datagram() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let path1 = dir.path().join("sock1");
        let path2 = dir.path().join("sock2");

        let sock1 = or_panic!(UnixDatagram::bind(&path1));
        let sock2 = or_panic!(UnixDatagram::bind(&path2));

        let msg = b"hello world";
        or_panic!(sock1.send_to(msg, &path2));
        let mut buf = [0; 11];
        or_panic!(sock2.recv_from(&mut buf));
        assert_eq!(msg, &buf[..]);
    }

    #[test]
    fn test_unnamed_unix_datagram() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let path1 = dir.path().join("sock1");

        let sock1 = or_panic!(UnixDatagram::bind(&path1));
        let sock2 = or_panic!(UnixDatagram::unbound());

        let msg = b"hello world";
        or_panic!(sock2.send_to(msg, &path1));
        let mut buf = [0; 11];
        let (usize, addr) = or_panic!(sock1.recv_from(&mut buf));
        assert_eq!(usize, 11);
        assert!(addr.is_unnamed());
        assert_eq!(msg, &buf[..]);
    }

    #[test]
    fn test_connect_unix_datagram() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let path1 = dir.path().join("sock1");
        let path2 = dir.path().join("sock2");

        let bsock1 = or_panic!(UnixDatagram::bind(&path1));
        let bsock2 = or_panic!(UnixDatagram::bind(&path2));
        let sock = or_panic!(UnixDatagram::unbound());
        or_panic!(sock.connect(&path1));

        // Check send()
        let msg = b"hello there";
        or_panic!(sock.send(msg));
        let mut buf = [0; 11];
        let (usize, addr) = or_panic!(bsock1.recv_from(&mut buf));
        assert_eq!(usize, 11);
        assert!(addr.is_unnamed());
        assert_eq!(msg, &buf[..]);

        // Changing default socket works too
        or_panic!(sock.connect(&path2));
        or_panic!(sock.send(msg));
        or_panic!(bsock2.recv_from(&mut buf));
    }

    #[test]
    fn test_unix_datagram_recv() {
        let dir = or_panic!(TempDir::new("unix_socket"));
        let path1 = dir.path().join("sock1");

        let sock1 = or_panic!(UnixDatagram::bind(&path1));
        let sock2 = or_panic!(UnixDatagram::unbound());
        or_panic!(sock2.connect(&path1));

        let msg = b"hello world";
        or_panic!(sock2.send(msg));
        let mut buf = [0; 11];
        let size = or_panic!(sock1.recv(&mut buf));
        assert_eq!(size, 11);
        assert_eq!(msg, &buf[..]);
    }

    #[test]
    fn datagram_pair() {
        let msg1 = b"hello";
        let msg2 = b"world!";

        let (s1, s2) = or_panic!(UnixDatagram::pair());
        let thread = thread::spawn(move || {
            // s1 must be moved in or the test will hang!
            let mut buf = [0; 5];
            or_panic!(s1.recv(&mut buf));
            assert_eq!(&msg1[..], &buf[..]);
            or_panic!(s1.send(msg2));
        });

        or_panic!(s2.send(msg1));
        let mut buf = [0; 6];
        or_panic!(s2.recv(&mut buf));
        assert_eq!(&msg2[..], &buf[..]);
        drop(s2);

        thread.join().unwrap();
    }

    #[test]
    fn datagram_shutdown() {
        let s1 = UnixDatagram::unbound().unwrap();
        let s2 = s1.try_clone().unwrap();

        let thread = thread::spawn(move || {
            let mut buf = [0; 1];
            assert_eq!(0, s1.recv_from(&mut buf).unwrap().0);
        });

        thread::sleep(Duration::from_millis(100));
        s2.shutdown(Shutdown::Read).unwrap();;

        thread.join().unwrap();
    }
}
