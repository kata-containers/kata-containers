// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyperlocal::{UnixConnector, Uri};

pub struct ShimClient {
    socket_path: PathBuf,
    timeout: Duration,
}

impl ShimClient {
    pub fn new(socket_path: PathBuf, timeout: Duration) -> Self {
        Self {
            socket_path,
            timeout,
        }
    }

    pub async fn get(&self, path: &str) -> Result<Vec<u8>> {
        let client: Client<UnixConnector, Full<Bytes>> =
            Client::builder(TokioExecutor::new()).build(UnixConnector);
        let uri: hyper::Uri = Uri::new(&self.socket_path, path).into();

        let resp = tokio::time::timeout(self.timeout, client.get(uri))
            .await
            .map_err(|_| anyhow!("timeout after {:?} for GET {}", self.timeout, path))?
            .map_err(|e| anyhow!("request failed: {}", e))?;

        let body = resp.into_body().collect().await?.to_bytes();
        Ok(body.to_vec())
    }
}
