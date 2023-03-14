use crate::{
    buf::{IoBuf, IoBufMut},
    driver::Socket,
};
use socket2::SockAddr;
use std::{io, net::SocketAddr};

/// A UDP socket.
///
/// UDP is "connectionless", unlike TCP. Meaning, regardless of what address you've bound to, a `UdpSocket`
/// is free to communicate with many different remotes. In tokio there are basically two main ways to use `UdpSocket`:
///
/// * one to many: [`bind`](`UdpSocket::bind`) and use [`send_to`](`UdpSocket::send_to`)
///   and [`recv_from`](`UdpSocket::recv_from`) to communicate with many different addresses
/// * one to one: [`connect`](`UdpSocket::connect`) and associate with a single address, using [`write`](`UdpSocket::write`)
///   and [`read`](`UdpSocket::read`) to communicate only with that remote address
///
/// # Examples
/// Bind and connect a pair of sockets and send a packet:
///
/// ```
/// use tokio_uring::net::UdpSocket;
/// use std::net::SocketAddr;
/// fn main() -> std::io::Result<()> {
///     tokio_uring::start(async {
///         let first_addr: SocketAddr = "127.0.0.1:2401".parse().unwrap();
///         let second_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
///
///         // bind sockets
///         let socket = UdpSocket::bind(first_addr.clone()).await?;
///         let other_socket = UdpSocket::bind(second_addr.clone()).await?;
///
///         // connect sockets
///         socket.connect(second_addr).await.unwrap();
///         other_socket.connect(first_addr).await.unwrap();
///
///         let buf = vec![0; 32];
///
///         // write data
///         let (result, _) = socket.write(b"hello world".as_slice()).await;
///         result.unwrap();
///
///         // read data
///         let (result, buf) = other_socket.read(buf).await;
///         let n_bytes = result.unwrap();
///
///         assert_eq!(b"hello world", &buf[..n_bytes]);
///
///         Ok(())
///     })
/// }
/// ```
/// Send and receive packets without connecting:
///
/// ```
/// use tokio_uring::net::UdpSocket;
/// use std::net::SocketAddr;
/// fn main() -> std::io::Result<()> {
///     tokio_uring::start(async {
///         let first_addr: SocketAddr = "127.0.0.1:2401".parse().unwrap();
///         let second_addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
///
///         // bind sockets
///         let socket = UdpSocket::bind(first_addr.clone()).await?;
///         let other_socket = UdpSocket::bind(second_addr.clone()).await?;
///
///         let buf = vec![0; 32];
///
///         // write data
///         let (result, _) = socket.send_to(b"hello world".as_slice(), second_addr).await;
///         result.unwrap();
///
///         // read data
///         let (result, buf) = other_socket.recv_from(buf).await;
///         let (n_bytes, addr) = result.unwrap();
///
///         assert_eq!(addr, first_addr);
///         assert_eq!(b"hello world", &buf[..n_bytes]);
///
///         Ok(())
///     })
/// }
/// ```
pub struct UdpSocket {
    pub(super) inner: Socket,
}

impl UdpSocket {
    /// Creates a new UDP socket and attempt to bind it to the addr provided.
    pub async fn bind(socket_addr: SocketAddr) -> io::Result<UdpSocket> {
        let socket = Socket::bind(socket_addr, libc::SOCK_DGRAM)?;
        Ok(UdpSocket { inner: socket })
    }

    /// Connects this UDP socket to a remote address, allowing the `write` and
    /// `read` syscalls to be used to send data and also applies filters to only
    /// receive data from the specified address.
    ///
    /// Note that usually, a successful `connect` call does not specify
    /// that there is a remote server listening on the port, rather, such an
    /// error would only be detected after the first send.
    pub async fn connect(&self, socket_addr: SocketAddr) -> io::Result<()> {
        self.inner.connect(SockAddr::from(socket_addr)).await
    }

    /// Sends data on the socket to the given address. On success, returns the
    /// number of bytes written.
    pub async fn send_to<T: IoBuf>(
        &self,
        buf: T,
        socket_addr: SocketAddr,
    ) -> crate::BufResult<usize, T> {
        self.inner.send_to(buf, socket_addr).await
    }

    /// Receives a single datagram message on the socket. On success, returns
    /// the number of bytes read and the origin.
    pub async fn recv_from<T: IoBufMut>(&self, buf: T) -> crate::BufResult<(usize, SocketAddr), T> {
        self.inner.recv_from(buf).await
    }

    /// Read a packet of data from the socket into the buffer, returning the original buffer and
    /// quantity of data read.
    pub async fn read<T: IoBufMut>(&self, buf: T) -> crate::BufResult<usize, T> {
        self.inner.read(buf).await
    }

    /// Write some data to the socket from the buffer, returning the original buffer and
    /// quantity of data written.
    pub async fn write<T: IoBuf>(&self, buf: T) -> crate::BufResult<usize, T> {
        self.inner.write(buf).await
    }
}
