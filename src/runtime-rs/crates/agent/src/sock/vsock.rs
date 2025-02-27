// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    os::unix::prelude::{AsRawFd, FromRawFd},
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use nix::sys::socket::{connect, socket, AddressFamily, SockFlag, SockType, VsockAddr};
use tokio::net::UnixStream;

use super::{ConnectConfig, Sock, Stream};

#[derive(Debug, PartialEq)]
pub struct Vsock {
    vsock_cid: u32,
    port: u32,
}

impl Vsock {
    pub fn new(vsock_cid: u32, port: u32) -> Self {
        Self { vsock_cid, port }
    }
}

#[async_trait]
impl Sock for Vsock {
    async fn connect(&self, config: &ConnectConfig) -> Result<Stream> {
        let retry_times = config.reconnect_timeout_ms / config.dial_timeout_ms;
        let sock_addr = VsockAddr::new(self.vsock_cid, self.port);
        let connect_once = || {
            // Create socket fd
            let socket = socket(
                AddressFamily::Vsock,
                SockType::Stream,
                SockFlag::empty(),
                None,
            )
            .context("failed to create vsock socket")?;

            // Wrap the socket fd in a UnixStream, so that it is closed when
            // anything fails.
            // We MUST NOT reuse a vsock socket which has failed a connection
            // attempt before, since a ECONNRESET error marks the whole socket as
            // broken and non-reusable.
            let socket = unsafe { std::os::unix::net::UnixStream::from_raw_fd(socket) };

            // Connect the socket to vsock server.
            connect(socket.as_raw_fd(), &sock_addr)
                .with_context(|| format!("failed to connect to {}", sock_addr))?;

            // Finally, convert the std UnixSocket to tokio's UnixSocket.
            UnixStream::from_std(socket).context("from_std")
        };

        for i in 0..retry_times {
            match connect_once() {
                Ok(stream) => {
                    info!(
                        sl!(),
                        "connect vsock success on {} current client fd {}",
                        i,
                        stream.as_raw_fd()
                    );
                    return Ok(Stream::Vsock(stream));
                }
                Err(e) => {
                    debug!(sl!(), "retry after {} ms: failed to connect to agent via vsock at {} attempts: {:?}", config.dial_timeout_ms, i, e);
                    tokio::time::sleep(Duration::from_millis(config.dial_timeout_ms)).await;
                }
            }
        }
        Err(anyhow!(
            "cannot connect vsock to agent ttrpc server {:?}",
            config
        ))
    }
}
