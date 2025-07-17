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
        let mut last_err = None;
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

            // Started from tokio v1.44.0+, it would panic when giving
            // `from_std()` a blocking socket. A workaround is to set the
            // socket to non-blocking, see [1].
            //
            // https://github.com/tokio-rs/tokio/issues/7172
            socket
                .set_nonblocking(true)
                .context("failed to set non-blocking")?;

            // Finally, convert the std UnixSocket to tokio's UnixSocket.
            UnixStream::from_std(socket).context("from_std")
        };

        for i in 0..retry_times {
            match connect_once() {
                Ok(stream) => {
                    info!(sl!(), "vsock: connected to {:?}", self);
                    return Ok(Stream::Vsock(stream));
                }
                Err(e) => {
                    trace!(
                        sl!(),
                        "vsock: failed to connect to {:?}, err {:?}, attempts {}, will retry after {} ms",
                        self,
                        e,
                        i,
                        config.dial_timeout_ms,
                    );
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(config.dial_timeout_ms)).await;
                }
            }
        }

        // Safe to unwrap the last_err, as this line will be unreachable if
        // no errors occurred.
        Err(anyhow!(
            "vsock: failed to connect to {:?}, err {:?}",
            self,
            last_err.unwrap()
        ))
    }
}
