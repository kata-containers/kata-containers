use std::cmp::Ordering;
use std::io;
use std::mem;
use std::os::unix::prelude::*;
use std::path::Path;

use libc::{self, c_int, c_ulong};

use cvt;

// See below for the usage of SOCK_CLOEXEC, but this constant is only defined on
// Linux currently (e.g. support doesn't exist on other platforms). In order to
// get name resolution to work and things to compile we just define a dummy
// SOCK_CLOEXEC here for other platforms. Note that the dummy constant isn't
// actually ever used (the blocks below are wrapped in `if cfg!` as well.
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "illumos",
    target_os = "solaris"
))]
use libc::{SOCK_CLOEXEC, SOCK_NONBLOCK};
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "illumos",
    target_os = "solaris"
)))]
const SOCK_CLOEXEC: c_int = 0;
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "illumos",
    target_os = "solaris"
)))]
const SOCK_NONBLOCK: c_int = 0;

pub struct Socket {
    fd: c_int,
}

impl Socket {
    pub fn new(ty: c_int) -> io::Result<Socket> {
        unsafe {
            // On linux we first attempt to pass the SOCK_CLOEXEC flag to
            // atomically create the socket and set it as CLOEXEC. Support for
            // this option, however, was added in 2.6.27, and we still support
            // 2.6.18 as a kernel, so if the returned error is EINVAL we
            // fallthrough to the fallback.
            if cfg!(any(
                target_os = "linux",
                target_os = "android",
                target_os = "illumos",
                target_os = "solaris"
            )) {
                let flags = ty | SOCK_CLOEXEC | SOCK_NONBLOCK;
                match cvt(libc::socket(libc::AF_UNIX, flags, 0)) {
                    Ok(fd) => return Ok(Socket { fd: fd }),
                    Err(ref e) if e.raw_os_error() == Some(libc::EINVAL) => {}
                    Err(e) => return Err(e),
                }
            }

            let fd = Socket { fd: try!(cvt(libc::socket(libc::AF_UNIX, ty, 0))) };
            try!(cvt(libc::ioctl(fd.fd, libc::FIOCLEX)));
            let mut nonblocking = 1 as c_ulong;
            try!(cvt(libc::ioctl(fd.fd, libc::FIONBIO, &mut nonblocking)));
            Ok(fd)
        }
    }

    pub fn pair(ty: c_int) -> io::Result<(Socket, Socket)> {
        unsafe {
            let mut fds = [0, 0];

            // Like above, see if we can set cloexec atomically
            if cfg!(any(
                target_os = "linux",
                target_os = "android",
                target_os = "illumos",
                target_os = "solaris"
            )) {
                let flags = ty | SOCK_CLOEXEC | SOCK_NONBLOCK;
                match cvt(libc::socketpair(libc::AF_UNIX, flags, 0, fds.as_mut_ptr())) {
                    Ok(_) => {
                        return Ok((Socket { fd: fds[0] }, Socket { fd: fds[1] }))
                    }
                    Err(ref e) if e.raw_os_error() == Some(libc::EINVAL) => {},
                    Err(e) => return Err(e),
                }
            }

            try!(cvt(libc::socketpair(libc::AF_UNIX, ty, 0, fds.as_mut_ptr())));
            let a = Socket { fd: fds[0] };
            let b = Socket { fd: fds[1] };
            try!(cvt(libc::ioctl(a.fd, libc::FIOCLEX)));
            try!(cvt(libc::ioctl(b.fd, libc::FIOCLEX)));
            let mut nonblocking = 1 as c_ulong;
            try!(cvt(libc::ioctl(a.fd, libc::FIONBIO, &mut nonblocking)));
            try!(cvt(libc::ioctl(b.fd, libc::FIONBIO, &mut nonblocking)));
            Ok((a, b))
        }
    }

    pub fn fd(&self) -> c_int {
        self.fd
    }

    pub fn into_fd(self) -> c_int {
        let ret = self.fd;
        mem::forget(self);
        ret
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::close(self.fd);
        }
    }
}

pub unsafe fn sockaddr_un(path: &Path)
                          -> io::Result<(libc::sockaddr_un, libc::socklen_t)> {
    let mut addr: libc::sockaddr_un = mem::zeroed();
    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;

    let bytes = path.as_os_str().as_bytes();

    match (bytes.get(0), bytes.len().cmp(&addr.sun_path.len())) {
        // Abstract paths don't need a null terminator
        (Some(&0), Ordering::Greater) => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                      "path must be no longer than SUN_LEN"));
        }
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

fn sun_path_offset() -> usize {
    unsafe {
        // Work with an actual instance of the type since using a null pointer is UB
        let addr: libc::sockaddr_un = mem::uninitialized();
        let base = &addr as *const _ as usize;
        let path = &addr.sun_path as *const _ as usize;
        path - base
    }
}

