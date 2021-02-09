use super::socket_addr;
use crate::net::{SocketAddr, UnixStream};
use crate::sys::unix::net::new_socket;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::os::unix::net;
use std::path::Path;
use std::{io, mem};

pub(crate) fn bind(path: &Path) -> io::Result<net::UnixListener> {
    let socket = new_socket(libc::AF_UNIX, libc::SOCK_STREAM)?;
    let (sockaddr, socklen) = socket_addr(path)?;
    let sockaddr = &sockaddr as *const libc::sockaddr_un as *const libc::sockaddr;

    syscall!(bind(socket, sockaddr, socklen))
        .and_then(|_| syscall!(listen(socket, 1024)))
        .map_err(|err| {
            // Close the socket if we hit an error, ignoring the error from
            // closing since we can't pass back two errors.
            let _ = unsafe { libc::close(socket) };
            err
        })
        .map(|_| unsafe { net::UnixListener::from_raw_fd(socket) })
}

pub(crate) fn accept(listener: &net::UnixListener) -> io::Result<(UnixStream, SocketAddr)> {
    let sockaddr = mem::MaybeUninit::<libc::sockaddr_un>::zeroed();

    // This is safe to assume because a `libc::sockaddr_un` filled with `0`
    // bytes is properly initialized.
    //
    // `0` is a valid value for `sockaddr_un::sun_family`; it is
    // `libc::AF_UNSPEC`.
    //
    // `[0; 108]` is a valid value for `sockaddr_un::sun_path`; it begins an
    // abstract path.
    let mut sockaddr = unsafe { sockaddr.assume_init() };

    sockaddr.sun_family = libc::AF_UNIX as libc::sa_family_t;
    let mut socklen = mem::size_of_val(&sockaddr) as libc::socklen_t;

    #[cfg(not(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "solaris"
    )))]
    let socket = {
        let flags = libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;
        syscall!(accept4(
            listener.as_raw_fd(),
            &mut sockaddr as *mut libc::sockaddr_un as *mut libc::sockaddr,
            &mut socklen,
            flags
        ))
        .map(|socket| unsafe { net::UnixStream::from_raw_fd(socket) })
    };

    #[cfg(any(
        target_os = "ios",
        target_os = "macos",
        target_os = "netbsd",
        target_os = "solaris"
    ))]
    let socket = syscall!(accept(
        listener.as_raw_fd(),
        &mut sockaddr as *mut libc::sockaddr_un as *mut libc::sockaddr,
        &mut socklen,
    ))
    .and_then(|socket| {
        // Ensure the socket is closed if either of the `fcntl` calls
        // error below.
        let s = unsafe { net::UnixStream::from_raw_fd(socket) };
        syscall!(fcntl(socket, libc::F_SETFD, libc::FD_CLOEXEC)).map(|_| s)
    });

    socket
        .map(UnixStream::from_std)
        .map(|stream| (stream, SocketAddr::from_parts(sockaddr, socklen)))
}

pub(crate) fn local_addr(listener: &net::UnixListener) -> io::Result<SocketAddr> {
    super::local_addr(listener.as_raw_fd())
}
