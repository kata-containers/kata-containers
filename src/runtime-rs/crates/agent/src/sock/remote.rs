// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

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
        let mut last_err = None;
        let retry_times = config.reconnect_timeout_ms / config.dial_timeout_ms;

        for i in 0..retry_times {
            match connect_helper(&self.path).await {
                Ok(stream) => {
                    info!(sl!(), "remote sock: connected to {:?}", self);
                    return Ok(Stream::Unix(stream));
                }
                Err(err) => {
                    trace!(
                        sl!(),
                        "remote sock: failed to connect to {:?}, err {:?}, attempts {}, will retry after {} ms",
                        self,
                        err,
                        i,
                        config.dial_timeout_ms
                    );
                    last_err = Some(err);
                    tokio::time::sleep(std::time::Duration::from_millis(config.dial_timeout_ms))
                        .await;
                    continue;
                }
            }
        }

        // Safe to unwrap the last_err, as this line will be unreachable if
        // no errors occurred.
        Err(anyhow!(
            "remote sock: failed to connect to {:?}, err {:?}",
            self,
            last_err.unwrap()
        ))
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
