// Copyright 2022 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::Any;
use std::net::{TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

use log::info;

use super::super::{Result, VsockError};
use super::{VsockBackend, VsockBackendType, VsockStream};

impl VsockStream for TcpStream {
    fn backend_type(&self) -> VsockBackendType {
        VsockBackendType::Tcp
    }

    fn set_nonblocking(&mut self, nonblocking: bool) -> std::io::Result<()> {
        TcpStream::set_nonblocking(self, nonblocking)
    }

    fn set_read_timeout(&mut self, dur: Option<Duration>) -> std::io::Result<()> {
        TcpStream::set_read_timeout(self, dur)
    }

    fn set_write_timeout(&mut self, dur: Option<Duration>) -> std::io::Result<()> {
        TcpStream::set_write_timeout(self, dur)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The backend implementation that using TCP Socket.
#[allow(dead_code)]
pub struct VsockTcpBackend {
    /// The TCP socket, through which host-initiated connections are accepted.
    tcp_sock: TcpListener,
    /// The address of TCP socket.
    tcp_sock_addr: String,
}

impl VsockTcpBackend {
    pub fn new(tcp_sock_addr: String) -> Result<Self> {
        info!("open vsock tcp: {}", tcp_sock_addr);
        // Open/bind/listen on the host Unix socket, so we can accept
        // host-initiated connections.
        let tcp_sock = TcpListener::bind(&tcp_sock_addr)
            .and_then(|sock| sock.set_nonblocking(true).map(|_| sock))
            .map_err(VsockError::Backend)?;
        info!("vsock tcp opened");

        Ok(VsockTcpBackend {
            tcp_sock,
            tcp_sock_addr,
        })
    }
}

impl AsRawFd for VsockTcpBackend {
    fn as_raw_fd(&self) -> RawFd {
        self.tcp_sock.as_raw_fd()
    }
}

impl VsockBackend for VsockTcpBackend {
    fn accept(&mut self) -> std::io::Result<Box<dyn VsockStream>> {
        let (stream, _) = self.tcp_sock.accept()?;
        stream.set_nonblocking(true)?;

        Ok(Box::new(stream))
    }

    // Peer connection doesn't supported by tcp backend yet.
    fn connect(&self, _dst_port: u32) -> std::io::Result<Box<dyn VsockStream>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "vsock net backend doesn't support incoming connection request",
        ))
    }

    fn r#type(&self) -> VsockBackendType {
        VsockBackendType::Tcp
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    use super::*;

    #[test]
    fn test_tcp_backend_bind() {
        let tcp_sock_addr = String::from("127.0.0.2:9000");
        assert!(VsockTcpBackend::new(tcp_sock_addr).is_ok());
    }

    #[test]
    fn test_tcp_backend_accept() {
        let tcp_sock_addr = String::from("127.0.0.2:9001");

        let mut vsock_backend = VsockTcpBackend::new(tcp_sock_addr.clone()).unwrap();
        let _stream = TcpStream::connect(&tcp_sock_addr).unwrap();

        assert!(vsock_backend.accept().is_ok());
    }

    #[test]
    fn test_tcp_backend_communication() {
        let tcp_sock_addr = String::from("127.0.0.2:9002");
        let test_string = String::from("TEST");
        let mut buffer = [0; 10];

        let mut vsock_backend = VsockTcpBackend::new(tcp_sock_addr.clone()).unwrap();
        let mut stream_connect = TcpStream::connect(&tcp_sock_addr).unwrap();
        stream_connect.set_nonblocking(true).unwrap();
        let mut stream_backend = vsock_backend.accept().unwrap();

        assert!(stream_connect
            .write(&test_string.clone().into_bytes())
            .is_ok());
        assert!(stream_backend.read(&mut buffer).is_ok());
        assert_eq!(&buffer[0..test_string.len()], test_string.as_bytes());

        assert!(stream_backend
            .write(&test_string.clone().into_bytes())
            .is_ok());
        assert!(stream_connect.read(&mut buffer).is_ok());
        assert_eq!(&buffer[0..test_string.len()], test_string.as_bytes());
    }

    #[test]
    fn test_tcp_backend_connect() {
        let tcp_sock_addr = String::from("127.0.0.2:9003");
        let vsock_backend = VsockTcpBackend::new(tcp_sock_addr).unwrap();
        // tcp backend don't support peer connection
        assert!(vsock_backend.connect(0).is_err());
    }

    #[test]
    fn test_tcp_backend_type() {
        let tcp_sock_addr = String::from("127.0.0.2:9004");
        let vsock_backend = VsockTcpBackend::new(tcp_sock_addr).unwrap();
        assert_eq!(vsock_backend.r#type(), VsockBackendType::Tcp);
    }

    #[test]
    fn test_tcp_backend_vsock_stream() {
        let tcp_sock_addr = String::from("127.0.0.2:9005");
        let _vsock_backend = VsockTcpBackend::new(tcp_sock_addr.clone()).unwrap();
        let vsock_stream = TcpStream::connect(&tcp_sock_addr).unwrap();

        assert!(vsock_stream.set_nonblocking(true).is_ok());
        assert!(vsock_stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .is_ok());
        assert!(vsock_stream.set_read_timeout(None).is_ok());
        assert!(vsock_stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .is_ok());
    }
}
