use crate::sys::unix::net::{new_ip_socket, socket_addr};

use std::io;
use std::net::{self, SocketAddr};
use std::os::unix::io::FromRawFd;

pub fn bind(addr: SocketAddr) -> io::Result<net::UdpSocket> {
    // Gives a warning for non Apple platforms.
    #[allow(clippy::let_and_return)]
    let socket = new_ip_socket(addr, libc::SOCK_DGRAM);

    socket.and_then(|socket| {
        let (raw_addr, raw_addr_length) = socket_addr(&addr);
        syscall!(bind(socket, raw_addr.as_ptr(), raw_addr_length))
            .map_err(|err| {
                // Close the socket if we hit an error, ignoring the error
                // from closing since we can't pass back two errors.
                let _ = unsafe { libc::close(socket) };
                err
            })
            .map(|_| unsafe { net::UdpSocket::from_raw_fd(socket) })
    })
}
