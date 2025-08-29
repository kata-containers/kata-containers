// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod hybrid_vsock;
pub use hybrid_vsock::HybridVsock;
mod vsock;
pub use vsock::Vsock;
mod unix_sock;
pub use unix_sock::UnixSock;

use std::{
    pin::Pin,
    task::{Context as TaskContext, Poll},
    {
        os::unix::{io::IntoRawFd, prelude::RawFd},
        sync::Arc,
    },
};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use slog::{error, info};
use tokio::{
    io::{AsyncRead, ReadBuf},
    net::UnixStream,
};
use url::Url;

const VSOCK_SCHEME: &str = "vsock";
const HYBRID_VSOCK_SCHEME: &str = "hvsock";
const REMOTE_SCHEME: &str = "remote";
const UNIX_SCHEME: &str = "unix";

/// Socket stream
pub enum Stream {
    // hvsock://<path>:<port>. Firecracker/Dragonball implements the virtio-vsock device
    // model, and mediates communication between AF_UNIX sockets (on the host end)
    // and AF_VSOCK sockets (on the guest end).
    Unix(UnixStream),
    // vsock://<cid>:<port>
    Vsock(UnixStream),
}

impl Stream {
    fn poll_read_priv(
        &mut self,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // Safety: `UnixStream::read` correctly handles reads into uninitialized memory
        match self {
            Stream::Unix(stream) | Stream::Vsock(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl IntoRawFd for Stream {
    fn into_raw_fd(self) -> RawFd {
        match self {
            Stream::Unix(stream) | Stream::Vsock(stream) => match stream.into_std() {
                Ok(stream) => stream.into_raw_fd(),
                Err(err) => {
                    error!(sl!(), "failed to into std unix stream {:?}", err);
                    -1
                }
            },
        }
    }
}

impl AsyncRead for Stream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut TaskContext<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // we know this is safe because doesn't moved
        let me = unsafe { self.get_unchecked_mut() };
        me.poll_read_priv(cx, buf)
    }
}

/// Connect config
#[derive(Debug)]
pub struct ConnectConfig {
    dial_timeout_ms: u64,
    reconnect_timeout_ms: u64,
}

impl ConnectConfig {
    pub fn new(dial_timeout_ms: u64, reconnect_timeout_ms: u64) -> Self {
        Self {
            dial_timeout_ms,
            reconnect_timeout_ms,
        }
    }
}

#[derive(Debug, PartialEq)]
enum SockType {
    Vsock(Vsock),
    HybridVsock(HybridVsock),
    Unix(UnixSock),
}

#[async_trait]
pub trait Sock: Send + Sync + std::fmt::Debug {
    async fn connect(&self, config: &ConnectConfig) -> Result<Stream>;
}

// Supported sock address formats are:
//   - vsock://<cid>:<port> (port is required)
//   - hvsock://<path>:<port> (port is required). Firecracker implements the virtio-vsock device
//     model, and mediates communication between AF_UNIX sockets (on the host end)
//     and AF_VSOCK sockets (on the guest end).
//   - unix://<path> (port is not needed). Direct Unix domain socket connection. Supports both absolute
//     paths (e.g., unix:///tmp/console.sock) and relative paths (e.g., unix://console.sock).
//     Relative paths are resolved relative to the current working directory.
//   - remote://<path> (port is not needed). Remote Unix domain socket connection for network-based protocols.
pub fn new(address: &str, port: Option<u32>) -> Result<Arc<dyn Sock>> {
    match parse(address, port).context("parse url")? {
        SockType::Vsock(sock) => Ok(Arc::new(sock)),
        SockType::HybridVsock(sock) => Ok(Arc::new(sock)),
        SockType::Unix(sock) => Ok(Arc::new(sock)),
    }
}

fn parse(address: &str, port: Option<u32>) -> Result<SockType> {
    let url = Url::parse(address).context("parse url")?;
    match url.scheme() {
        VSOCK_SCHEME => {
            let port = port.ok_or_else(|| anyhow!("port is required for vsock"))?;
            let vsock_cid = url
                .host_str()
                .unwrap_or_default()
                .parse::<u32>()
                .context("parse vsock cid")?;
            Ok(SockType::Vsock(Vsock::new(vsock_cid, port)))
        }
        HYBRID_VSOCK_SCHEME => {
            let port = port.ok_or_else(|| anyhow!("port is required for hvsock"))?;
            let path: Vec<&str> = url.path().split(':').collect();
            if path.len() != 1 {
                return Err(anyhow!("invalid path {:?}", path));
            }
            let uds = path[0];
            Ok(SockType::HybridVsock(HybridVsock::new(uds, port)))
        }
        REMOTE_SCHEME | UNIX_SCHEME => {
            // For both remote and unix URLs, port is not needed
            let socket_path = if let Some(host) = url.host_str() {
                host.to_string()
            } else {
                url.path().to_string()
            };

            info!(
                sl!(),
                "{} URL parsing: host={:?}, path={:?}, socket_path={:?}",
                url.scheme(),
                url.host_str(),
                url.path(),
                socket_path
            );

            let scheme = if url.scheme() == REMOTE_SCHEME {
                "remote"
            } else {
                "unix"
            };
            Ok(SockType::Unix(UnixSock::new(socket_path, scheme)))
        }
        _ => Err(anyhow!("Unsupported scheme")),
    }
}

#[cfg(test)]
mod test {
    use super::{
        hybrid_vsock::HybridVsock, new, parse, unix_sock::UnixSock, vsock::Vsock, SockType,
    };

    #[test]
    fn test_parse_url() {
        // check vsock
        let vsock = parse("vsock://123", Some(456)).unwrap();
        assert_eq!(vsock, SockType::Vsock(Vsock::new(123, 456)));

        // check hybrid vsock
        let hvsock = parse("hvsock:///tmp/test.hvsock", Some(456)).unwrap();
        assert_eq!(
            hvsock,
            SockType::HybridVsock(HybridVsock::new("/tmp/test.hvsock", 456))
        );
        // check remote scheme
        let remote = parse("remote://123", None).unwrap();
        assert_eq!(
            remote,
            SockType::Unix(UnixSock::new("123".to_string(), "remote"))
        );
        // check unix scheme with absolute path
        let unix_absolute = parse("unix:///tmp/console.sock", None).unwrap();
        assert_eq!(
            unix_absolute,
            SockType::Unix(UnixSock::new("/tmp/console.sock".to_string(), "unix"))
        );
        // check unix scheme with relative path
        let unix_relative = parse("unix://console.sock", None).unwrap();
        assert_eq!(
            unix_relative,
            SockType::Unix(UnixSock::new("console.sock".to_string(), "unix"))
        );
    }

    #[test]
    fn test_new_functions() {
        let _vsock_sock = new("vsock://123", Some(456)).unwrap();
        let _hybrid_vsock_sock = new("hvsock://123", Some(456)).unwrap();
        let _remote_sock = new("remote://123", None).unwrap();
        let _unix_sock = new("unix:///tmp/test.sock", None).unwrap();
    }
}
