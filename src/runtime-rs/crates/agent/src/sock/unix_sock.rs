// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use slog::{info, trace};
use tokio::{io::Interest, net::UnixStream};

use super::{ConnectConfig, Sock, Stream};

/// Unix socket connection logic for both remote and unix schemes
#[derive(Debug, PartialEq)]
pub struct UnixSock {
    path: String,
    scheme: &'static str,
}

impl UnixSock {
    pub fn new(path: String, scheme: &'static str) -> Self {
        Self { path, scheme }
    }
}

#[async_trait]
impl Sock for UnixSock {
    async fn connect(&self, config: &ConnectConfig) -> Result<Stream> {
        let mut last_err = None;
        let retry_times = config.reconnect_timeout_ms / config.dial_timeout_ms;

        for i in 0..retry_times {
            match connect_helper(&self.path).await {
                Ok(stream) => {
                    info!(sl!(), "{}: connected to {:?}", self.scheme, self);
                    return Ok(Stream::Unix(stream));
                }
                Err(err) => {
                    trace!(
                        sl!(),
                        "{}: failed to connect to {:?}, err {:?}, attempts {}, will retry after {} ms",
                        self.scheme,
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
            "{}: failed to connect to {:?}, err {:?}",
            self.scheme,
            self,
            last_err.unwrap()
        ))
    }
}

async fn connect_helper(path: &str) -> Result<UnixStream> {
    let stream = UnixStream::connect(Path::new(path))
        .await
        .context("failed to connect to Unix domain socket")?;
    stream
        .ready(Interest::READABLE | Interest::WRITABLE)
        .await?;
    Ok(stream)
}
