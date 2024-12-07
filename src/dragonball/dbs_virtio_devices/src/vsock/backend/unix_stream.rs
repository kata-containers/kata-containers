// Copyright 2022 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::any::Any;
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

use log::info;
use sendfd::RecvWithFd;

use super::super::{Result, VsockError};
use super::{VsockBackend, VsockBackendType, VsockStream};

impl VsockStream for UnixStream {
    fn backend_type(&self) -> VsockBackendType {
        VsockBackendType::UnixStream
    }

    fn set_nonblocking(&mut self, nonblocking: bool) -> std::io::Result<()> {
        UnixStream::set_nonblocking(self, nonblocking)
    }

    fn set_read_timeout(&mut self, dur: Option<Duration>) -> std::io::Result<()> {
        UnixStream::set_read_timeout(self, dur)
    }

    fn set_write_timeout(&mut self, dur: Option<Duration>) -> std::io::Result<()> {
        UnixStream::set_write_timeout(self, dur)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn recv_data_fd(&self, bytes: &mut [u8], fds: &mut [RawFd]) -> std::io::Result<(usize, usize)> {
        self.recv_with_fd(bytes, fds)
    }
}

/// The backend implementation that using Unix Stream.
pub struct VsockUnixStreamBackend {
    /// The Unix socket, through which host-initiated connections are accepted.
    pub(crate) host_sock: UnixListener,
    /// The file system path of the host-side Unix socket.
    pub(crate) host_sock_path: String,
}

impl VsockUnixStreamBackend {
    pub fn new(host_sock_path: String) -> Result<Self> {
        info!("Open vsock uds: {}", host_sock_path);
        // Open/bind/listen on the host Unix socket, so we can accept
        // host-initiated connections.
        let host_sock = UnixListener::bind(&host_sock_path)
            .and_then(|sock| sock.set_nonblocking(true).map(|_| sock))
            .map_err(VsockError::Backend)?;
        info!("vsock uds opened");

        Ok(VsockUnixStreamBackend {
            host_sock,
            host_sock_path,
        })
    }
}

impl AsRawFd for VsockUnixStreamBackend {
    fn as_raw_fd(&self) -> RawFd {
        self.host_sock.as_raw_fd()
    }
}

impl VsockBackend for VsockUnixStreamBackend {
    fn accept(&mut self) -> std::io::Result<Box<dyn VsockStream>> {
        let (stream, _) = self.host_sock.accept()?;
        stream.set_nonblocking(true)?;

        Ok(Box::new(stream))
    }

    fn connect(&self, dst_port: u32) -> std::io::Result<Box<dyn VsockStream>> {
        // We can figure out the path to Unix sockets listening on specific
        // ports using `host_sock_path` field. I.e. "<this path>_<port number>".
        let port_path = format!("{}_{}", self.host_sock_path, dst_port);
        let stream = UnixStream::connect(port_path)?;
        stream.set_nonblocking(true)?;

        Ok(Box::new(stream))
    }

    fn r#type(&self) -> VsockBackendType {
        VsockBackendType::UnixStream
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Drop for VsockUnixStreamBackend {
    fn drop(&mut self) {
        std::fs::remove_file(&self.host_sock_path).ok();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::path::Path;

    use super::*;

    #[test]
    fn test_unix_backend_bind() {
        let host_sock_path = String::from("/tmp/host_sock_path_1");
        fs::remove_file(Path::new(&host_sock_path)).unwrap_or_default();

        assert!(VsockUnixStreamBackend::new(host_sock_path.clone()).is_ok());

        fs::remove_file(Path::new(&host_sock_path)).unwrap_or_default();
    }

    #[test]
    fn test_unix_backend_accept() {
        let host_sock_path = String::from("/tmp/host_sock_path_2");
        fs::remove_file(Path::new(&host_sock_path)).unwrap_or_default();

        let mut vsock_backend = VsockUnixStreamBackend::new(host_sock_path.clone()).unwrap();
        let _stream = UnixStream::connect(&host_sock_path).unwrap();

        assert!(vsock_backend.accept().is_ok());

        fs::remove_file(Path::new(&host_sock_path)).unwrap_or_default();
    }

    #[test]
    fn test_unix_backend_communication() {
        let host_sock_path = String::from("/tmp/host_sock_path_3");
        fs::remove_file(Path::new(&host_sock_path)).unwrap_or_default();
        let test_string = String::from("TEST");
        let mut buffer = [0; 10];

        let mut vsock_backend = VsockUnixStreamBackend::new(host_sock_path.clone()).unwrap();
        let mut stream_connect = UnixStream::connect(&host_sock_path).unwrap();
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

        fs::remove_file(Path::new(&host_sock_path)).unwrap_or_default();
    }

    #[test]
    fn test_unix_backend_connect() {
        let host_sock_path = String::from("/tmp/host_sock_path_4");
        let local_server_port = 1;
        let local_server_path = format!("{host_sock_path}_{local_server_port}");
        fs::remove_file(Path::new(&host_sock_path)).unwrap_or_default();
        fs::remove_file(Path::new(&local_server_path)).unwrap_or_default();

        let _local_listener = UnixListener::bind(&local_server_path).unwrap();
        let vsock_backend = VsockUnixStreamBackend::new(host_sock_path.clone()).unwrap();

        assert!(vsock_backend.connect(local_server_port).is_ok());

        fs::remove_file(Path::new(&host_sock_path)).unwrap_or_default();
        fs::remove_file(Path::new(&local_server_path)).unwrap_or_default();
    }

    #[test]
    fn test_unix_backend_type() {
        let host_sock_path = String::from("/tmp/host_sock_path_5");
        fs::remove_file(Path::new(&host_sock_path)).unwrap_or_default();

        let vsock_backend = VsockUnixStreamBackend::new(host_sock_path.clone()).unwrap();
        assert_eq!(vsock_backend.r#type(), VsockBackendType::UnixStream);

        fs::remove_file(Path::new(&host_sock_path)).unwrap_or_default();
    }

    #[test]
    fn test_unix_backend_vsock_stream() {
        let (sock1, _sock2) = UnixStream::pair().unwrap();
        let mut vsock_stream: Box<dyn VsockStream> = Box::new(sock1);

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
