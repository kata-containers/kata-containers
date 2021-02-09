use {io, Ready, PollOpt};
use libc;
use zircon;
use std::mem;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::{Deref, DerefMut};
use std::os::unix::io::RawFd;

mod awakener;
mod handles;
mod eventedfd;
mod net;
mod ready;
mod selector;

use self::eventedfd::{EventedFd, EventedFdInner};
use self::ready::assert_fuchsia_ready_repr;

pub use self::awakener::Awakener;
pub use self::handles::EventedHandle;
pub use self::net::{TcpListener, TcpStream, UdpSocket};
pub use self::selector::{Events, Selector};
pub use self::ready::{FuchsiaReady, zx_signals_t};

// Set non-blocking (workaround since the std version doesn't work in fuchsia)
// TODO: fix the std version and replace this
pub fn set_nonblock(fd: RawFd) -> io::Result<()> {
    cvt(unsafe { libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) }).map(|_| ())
}

/// Workaround until fuchsia's recv_from is fixed
unsafe fn recv_from(fd: RawFd, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
    let flags = 0;

    let n = cvt(
        libc::recv(fd,
                   buf.as_mut_ptr() as *mut libc::c_void,
                   buf.len(),
                   flags)
    )?;

    // random address-- we don't use it
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    Ok((n as usize, addr))
}

mod sys {
    #![allow(non_camel_case_types)]
    use std::os::unix::io::RawFd;
    pub use zircon_sys::{zx_handle_t, zx_signals_t};

    // 17 fn pointers we don't need for mio :)
    pub type fdio_ops_t = [usize; 17];

    pub type atomic_int_fast32_t = usize; // TODO: https://github.com/rust-lang/libc/issues/631

    #[repr(C)]
    pub struct fdio_t {
        pub ops: *const fdio_ops_t,
        pub magic: u32,
        pub refcount: atomic_int_fast32_t,
        pub dupcount: u32,
        pub flags: u32,
    }

    #[link(name="fdio")]
    extern {
        pub fn __fdio_fd_to_io(fd: RawFd) -> *const fdio_t;
        pub fn __fdio_release(io: *const fdio_t);

        pub fn __fdio_wait_begin(
            io: *const fdio_t,
            events: u32,
            handle_out: &mut zx_handle_t,
            signals_out: &mut zx_signals_t,
        );
        pub fn __fdio_wait_end(
            io: *const fdio_t,
            signals: zx_signals_t,
            events_out: &mut u32,
        );
    }
}

fn epoll_event_to_ready(epoll: u32) -> Ready {
    let epoll = epoll as i32; // casts the bits directly
    let mut kind = Ready::empty();

    if (epoll & libc::EPOLLIN) != 0 || (epoll & libc::EPOLLPRI) != 0 {
        kind = kind | Ready::readable();
    }

    if (epoll & libc::EPOLLOUT) != 0 {
        kind = kind | Ready::writable();
    }

    kind

    /* TODO: support?
    // EPOLLHUP - Usually means a socket error happened
    if (epoll & libc::EPOLLERR) != 0 {
        kind = kind | UnixReady::error();
    }

    if (epoll & libc::EPOLLRDHUP) != 0 || (epoll & libc::EPOLLHUP) != 0 {
        kind = kind | UnixReady::hup();
    }
    */
}

fn poll_opts_to_wait_async(poll_opts: PollOpt) -> zircon::WaitAsyncOpts {
    if poll_opts.is_oneshot() {
        zircon::WaitAsyncOpts::Once
    } else {
        zircon::WaitAsyncOpts::Repeating
    }
}

trait IsMinusOne {
    fn is_minus_one(&self) -> bool;
}

impl IsMinusOne for i32 {
    fn is_minus_one(&self) -> bool { *self == -1 }
}

impl IsMinusOne for isize {
    fn is_minus_one(&self) -> bool { *self == -1 }
}

fn cvt<T: IsMinusOne>(t: T) -> ::io::Result<T> {
    use std::io;

    if t.is_minus_one() {
        Err(io::Error::last_os_error())
    } else {
        Ok(t)
    }
}

/// Utility type to prevent the type inside of it from being dropped.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct DontDrop<T>(Option<T>);

impl<T> DontDrop<T> {
    fn new(t: T) -> DontDrop<T> {
        DontDrop(Some(t))
    }

    fn inner_ref(&self) -> &T {
        self.0.as_ref().unwrap()
    }

    fn inner_mut(&mut self) -> &mut T {
        self.0.as_mut().unwrap()
    }
}

impl<T> Deref for DontDrop<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.inner_ref()
    }
}

impl<T> DerefMut for DontDrop<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner_mut()
    }
}

impl<T> Drop for DontDrop<T> {
    fn drop(&mut self) {
        let inner = self.0.take();
        mem::forget(inner);
    }
}
