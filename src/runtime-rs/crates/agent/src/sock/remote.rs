// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{os::unix::prelude::AsRawFd, path::Path};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use tokio::{io::Interest, net::UnixStream};

use super::{ConnectConfig, Sock, Stream};

#[derive(Debug, PartialEq)]
pub struct Remote {
    path: String,
}

impl Remote {
    pub fn new(path: String) -> Self {
        Self { path }
    }
}

#[async_trait]
impl Sock for Remote {
    async fn connect(&self, config: &ConnectConfig) -> Result<Stream> {
        let retry_times = config.reconnect_timeout_ms / config.dial_timeout_ms;
        for i in 0..retry_times {
            match connect_helper(&self.path).await {
                Ok(stream) => {
                    info!(
                        sl!(),
                        "remote connect success on {} current client fd {}",
                        i,
                        stream.as_raw_fd()
                    );
                    return Ok(Stream::Unix(stream));
                }
                Err(err) => {
                    debug!(sl!(), "remote connect on {} err : {:?}", i, err);
                    tokio::time::sleep(std::time::Duration::from_millis(config.dial_timeout_ms))
                        .await;
                    continue;
                }
            }
        }
        Err(anyhow!("cannot connect to agent ttrpc server {:?}", config))
    }
}

async fn connect_helper(address: &str) -> Result<UnixStream> {
    let stream = UnixStream::connect(Path::new(&address))
        .await
        .context("failed to create UnixAddr")?;
    stream
        .ready(Interest::READABLE | Interest::WRITABLE)
        .await?;
    Ok(stream)
}
