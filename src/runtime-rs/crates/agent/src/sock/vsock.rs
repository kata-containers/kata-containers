// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    os::unix::prelude::AsRawFd,
    time::Duration,
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio_vsock::VsockAddr;

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

        for i in 0..retry_times {
            match tokio_vsock::VsockStream::connect(sock_addr).await {
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
