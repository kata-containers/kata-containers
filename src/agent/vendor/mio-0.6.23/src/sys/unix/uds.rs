use std::io::{Read, Write};
use std::mem;
use std::net::Shutdown;
use std::os::unix::prelude::*;
use std::path::Path;
use std::ptr;

use libc;

use {io, Ready, Poll, PollOpt, Token};
use event::Evented;
use sys::unix::{cvt, Io};
use sys::unix::io::{set_nonblock, set_cloexec};

trait MyInto<T> {
    fn my_into(self) -> T;
}

impl MyInto<u32> for usize {
    fn my_into(self) -> u32 { self as u32 }
}

impl MyInto<usize> for usize {
    fn my_into(self) -> usize { self }
}

unsafe fn sockaddr_un(path: &Path)
                      -> io::Result<(libc::sockaddr_un, libc::socklen_t)> {
    let mut addr: libc::sockaddr_un = mem::zeroed();
    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;

    let bytes = path.as_os_str().as_bytes();

    if bytes.len() >= addr.sun_path.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
                                  "path must be shorter than SUN_LEN"))
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

#[derive(Debug)]
pub struct UnixSocket {
    io: Io,
}

impl UnixSocket {
    /// Returns a new, unbound, non-blocking Unix domain socket
    pub fn stream() -> io::Result<UnixSocket> {
        #[cfg(target_os = "linux")]
        use libc::{SOCK_CLOEXEC, SOCK_NONBLOCK};
        #[cfg(not(target_os = "linux"))]
        const SOCK_CLOEXEC: libc::c_int = 0;
        #[cfg(not(target_os = "linux"))]
        const SOCK_NONBLOCK: libc::c_int = 0;

        unsafe {
            if cfg!(target_os = "linux") {
                let flags = libc::SOCK_STREAM | SOCK_CLOEXEC | SOCK_NONBLOCK;
                match cvt(libc::socket(libc::AF_UNIX, flags, 0)) {
                    Ok(fd) => return Ok(UnixSocket::from_raw_fd(fd)),
                    Err(ref e) if e.raw_os_error() == Some(libc::EINVAL) => {}
                    Err(e) => return Err(e),
                }
            }

            let fd = cvt(libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0))?;
            let fd = UnixSocket::from_raw_fd(fd);
            set_cloexec(fd.as_raw_fd())?;
            set_nonblock(fd.as_raw_fd())?;
            Ok(fd)
        }
    }

    /// Connect the socket to the specified address
    pub fn connect<P: AsRef<Path> + ?Sized>(&self, addr: &P) -> io::Result<()> {
        unsafe {
            let (addr, len) = sockaddr_un(addr.as_ref())?;
            cvt(libc::connect(self.as_raw_fd(),
                                   &addr as *const _ as *const _,
                                   len))?;
            Ok(())
        }
    }

    /// Listen for incoming requests
    pub fn listen(&self, backlog: usize) -> io::Result<()> {
        unsafe {
            cvt(libc::listen(self.as_raw_fd(), backlog as i32))?;
            Ok(())
        }
    }

    pub fn accept(&self) -> io::Result<UnixSocket> {
        unsafe {
            let fd = cvt(libc::accept(self.as_raw_fd(),
                                           ptr::null_mut(),
                                           ptr::null_mut()))?;
            let fd = Io::from_raw_fd(fd);
            set_cloexec(fd.as_raw_fd())?;
            set_nonblock(fd.as_raw_fd())?;
            Ok(UnixSocket { io: fd })
        }
    }

    /// Bind the socket to the specified address
    pub fn bind<P: AsRef<Path> + ?Sized>(&self, addr: &P) -> io::Result<()> {
        unsafe {
            let (addr, len) = sockaddr_un(addr.as_ref())?;
            cvt(libc::bind(self.as_raw_fd(),
                                &addr as *const _ as *const _,
                                len))?;
            Ok(())
        }
    }

    pub fn try_clone(&self) -> io::Result<UnixSocket> {
        Ok(UnixSocket { io: self.io.try_clone()? })
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let how = match how {
            Shutdown::Read => libc::SHUT_RD,
            Shutdown::Write => libc::SHUT_WR,
            Shutdown::Both => libc::SHUT_RDWR,
        };
        unsafe {
            cvt(libc::shutdown(self.as_raw_fd(), how))?;
            Ok(())
        }
    }

    pub fn read_recv_fd(&mut self, buf: &mut [u8]) -> io::Result<(usize, Option<RawFd>)> {
        unsafe {
            let mut iov = libc::iovec {
                iov_base: buf.as_mut_ptr() as *mut _,
                iov_len: buf.len(),
            };
            struct Cmsg {
                hdr: libc::cmsghdr,
                data: [libc::c_int; 1],
            }
            let mut cmsg: Cmsg = mem::zeroed();
            let mut msg: libc::msghdr = mem::zeroed();
            msg.msg_iov = &mut iov;
            msg.msg_iovlen = 1;
            msg.msg_control = &mut cmsg as *mut _ as *mut _;
            msg.msg_controllen = mem::size_of_val(&cmsg).my_into();
            let bytes = cvt(libc::recvmsg(self.as_raw_fd(), &mut msg, 0))?;

            const SCM_RIGHTS: libc::c_int = 1;

            let fd = if cmsg.hdr.cmsg_level == libc::SOL_SOCKET &&
                        cmsg.hdr.cmsg_type == SCM_RIGHTS {
                Some(cmsg.data[0])
            } else {
                None
            };
            Ok((bytes as usize, fd))
        }
    }

    pub fn write_send_fd(&mut self, buf: &[u8], fd: RawFd) -> io::Result<usize> {
        unsafe {
            let mut iov = libc::iovec {
                iov_base: buf.as_ptr() as *mut _,
                iov_len: buf.len(),
            };
            struct Cmsg {
                hdr: libc::cmsghdr,
                data: [libc::c_int; 1],
            }
            let mut cmsg: Cmsg = mem::zeroed();
            cmsg.hdr.cmsg_len = mem::size_of_val(&cmsg).my_into();
            cmsg.hdr.cmsg_level = libc::SOL_SOCKET;
            cmsg.hdr.cmsg_type = 1; // SCM_RIGHTS
            cmsg.data[0] = fd;
            let mut msg: libc::msghdr = mem::zeroed();
            msg.msg_iov = &mut iov;
            msg.msg_iovlen = 1;
            msg.msg_control = &mut cmsg as *mut _ as *mut _;
            msg.msg_controllen = mem::size_of_val(&cmsg).my_into();
            let bytes = cvt(libc::sendmsg(self.as_raw_fd(), &msg, 0))?;
            Ok(bytes as usize)
        }
    }
}

impl Read for UnixSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.read(buf)
    }
}

impl Write for UnixSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.io.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.io.flush()
    }
}

impl Evented for UnixSocket {
    fn register(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.io.register(poll, token, interest, opts)
    }

    fn reregister(&self, poll: &Poll, token: Token, interest: Ready, opts: PollOpt) -> io::Result<()> {
        self.io.reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        self.io.deregister(poll)
    }
}


impl From<Io> for UnixSocket {
    fn from(io: Io) -> UnixSocket {
        UnixSocket { io }
    }
}

impl FromRawFd for UnixSocket {
    unsafe fn from_raw_fd(fd: RawFd) -> UnixSocket {
        UnixSocket { io: Io::from_raw_fd(fd) }
    }
}

impl IntoRawFd for UnixSocket {
    fn into_raw_fd(self) -> RawFd {
        self.io.into_raw_fd()
    }
}

impl AsRawFd for UnixSocket {
    fn as_raw_fd(&self) -> RawFd {
        self.io.as_raw_fd()
    }
}
