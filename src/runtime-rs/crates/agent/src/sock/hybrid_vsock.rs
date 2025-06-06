// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

use super::{ConnectConfig, Sock, Stream};

#[derive(Debug, PartialEq)]
pub struct HybridVsock {
    uds: String,
    port: u32,
}

impl HybridVsock {
    pub fn new(uds: &str, port: u32) -> Self {
        Self {
            uds: uds.to_string(),
            port,
        }
    }
}

#[async_trait]
impl Sock for HybridVsock {
    async fn connect(&self, config: &ConnectConfig) -> Result<Stream> {
        let mut last_err = None;
        let retry_times = config.reconnect_timeout_ms / config.dial_timeout_ms;

        for i in 0..retry_times {
            match connect_helper(&self.uds, self.port).await {
                Ok(stream) => {
                    info!(sl!(), "hybrid vsock: connected to {:?}", self);
                    return Ok(Stream::Unix(stream));
                }
                Err(err) => {
                    trace!(
                        sl!(),
                        "hybrid vsock: failed to connect to {:?}, err {:?}, attempts {}, will retry after {} ms",
                        self,
                        err,
                        i,
                        config.dial_timeout_ms,
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
            "hybrid vsock: failed to connect to {:?}, err {:?}",
            self,
            last_err.unwrap()
        ))
    }
}

async fn connect_helper(uds: &str, port: u32) -> Result<UnixStream> {
    let mut stream = UnixStream::connect(&uds).await.context("connect")?;
    stream
        .write_all(format!("connect {}\n", port).as_bytes())
        .await
        .context("write all")?;
    let mut reads = BufReader::new(&mut stream);
    let mut response = String::new();
    reads.read_line(&mut response).await.context("read line")?;
    //info!(sl!(), "get socket resp: {}", response);
    if !response.contains("OK") {
        return Err(anyhow!(
            "handshake error: malformed response code: {:?}",
            response
        ));
    }
    Ok(stream)
}
