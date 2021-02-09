use std::io;
use std::net::{self, SocketAddr};

pub fn bind(_: SocketAddr) -> io::Result<net::UdpSocket> {
    os_required!()
}
