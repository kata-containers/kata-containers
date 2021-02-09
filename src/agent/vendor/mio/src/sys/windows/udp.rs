use std::io;
use std::net::{self, SocketAddr};
use std::os::windows::io::FromRawSocket;
use std::os::windows::raw::SOCKET as StdSocket; // winapi uses usize, stdlib uses u32/u64.

use winapi::um::winsock2::{bind as win_bind, closesocket, SOCKET_ERROR, SOCK_DGRAM};

use crate::sys::windows::net::{init, new_ip_socket, socket_addr};

pub fn bind(addr: SocketAddr) -> io::Result<net::UdpSocket> {
    init();
    new_ip_socket(addr, SOCK_DGRAM).and_then(|socket| {
        let (raw_addr, raw_addr_length) = socket_addr(&addr);
        syscall!(
            win_bind(socket, raw_addr.as_ptr(), raw_addr_length,),
            PartialEq::eq,
            SOCKET_ERROR
        )
        .map_err(|err| {
            // Close the socket if we hit an error, ignoring the error
            // from closing since we can't pass back two errors.
            let _ = unsafe { closesocket(socket) };
            err
        })
        .map(|_| unsafe { net::UdpSocket::from_raw_socket(socket as StdSocket) })
    })
}
