// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::os::unix::prelude::{AsRawFd, FromRawFd};
use std::time::{Duration, Instant};

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
        let sock_addr = VsockAddr::new(self.vsock_cid, self.port);
        let deadline = Instant::now() + Duration::from_millis(config.reconnect_timeout_ms);

        let mut backoff = Duration::from_millis(config.dial_timeout_ms);

        let min_backoff = Duration::from_millis(10);
        let max_backoff = Duration::from_millis(500);
        if backoff < min_backoff {
            backoff = min_backoff;
        } else if backoff > max_backoff {
            backoff = max_backoff;
        }

        let mut last_err: Option<anyhow::Error> = None;
        let mut attempts: u64 = 0;

        while Instant::now() < deadline {
            attempts += 1;

            let sa = sock_addr;
            let res: Result<UnixStream> =
                tokio::task::spawn_blocking(move || -> Result<UnixStream> {
                    // Create socket fd
                    let fd = socket(
                        AddressFamily::Vsock,
                        SockType::Stream,
                        SockFlag::empty(),
                        None,
                    )
                    .context("failed to create vsock socket")?;

                    // Wrap fd so it closes on error
                    let socket = unsafe { std::os::unix::net::UnixStream::from_raw_fd(fd) };

                    // Blocking connect (usually returns quickly for vsock)
                    connect(socket.as_raw_fd(), &sa)
                        .with_context(|| format!("failed to connect to {sa}"))?;

                    // Tokio requires non-blocking std socket before from_std()
                    socket
                        .set_nonblocking(true)
                        .context("failed to set non-blocking")?;

                    UnixStream::from_std(socket).context("from_std")
                })
                .await
                .context("vsock: connect task join failed")?;

            match res {
                Ok(stream) => {
                    info!(
                        sl!(),
                        "vsock: connected to {:?} after {} attempts", self, attempts
                    );
                    return Ok(Stream::Vsock(stream));
                }
                Err(e) => {
                    last_err = Some(e);

                    let now = Instant::now();
                    if now >= deadline {
                        break;
                    }

                    let remaining = deadline.saturating_duration_since(now);
                    let sleep_dur = std::cmp::min(backoff, remaining);

                    trace!(
                        sl!(),
                        "vsock: failed to connect to {:?}, attempts {}, retry after {:?}, err {:?}",
                        self,
                        attempts,
                        sleep_dur,
                        last_err.as_ref().unwrap(),
                    );

                    tokio::time::sleep(sleep_dur).await;

                    backoff = std::cmp::min(backoff.saturating_mul(2), max_backoff);
                }
            }
        }

        Err(anyhow!(
            "vsock: failed to connect to {:?} within {:?} (attempts={}), last_err={:?}",
            self,
            Duration::from_millis(config.reconnect_timeout_ms),
            attempts,
            last_err
        ))
    }
}
