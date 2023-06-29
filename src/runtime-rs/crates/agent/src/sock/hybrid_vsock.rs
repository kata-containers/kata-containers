// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::os::unix::prelude::AsRawFd;

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
        let retry_times = config.reconnect_timeout_ms / config.dial_timeout_ms;
        for i in 0..retry_times {
            match connect_helper(&self.uds, self.port).await {
                Ok(stream) => {
                    info!(
                        sl!(),
                        "connect success on {} current client fd {}",
                        i,
                        stream.as_raw_fd()
                    );
                    return Ok(Stream::Unix(stream));
                }
                Err(err) => {
                    debug!(sl!(), "connect on {} err : {:?}", i, err);
                    tokio::time::sleep(std::time::Duration::from_millis(config.dial_timeout_ms))
                        .await;
                    continue;
                }
            }
        }
        Err(anyhow!("cannot connect to agent ttrpc server {:?}", config))
    }
}

async fn connect_helper(uds: &str, port: u32) -> Result<UnixStream> {
    info!(sl!(), "connect uds {:?} port {}", &uds, port);
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
