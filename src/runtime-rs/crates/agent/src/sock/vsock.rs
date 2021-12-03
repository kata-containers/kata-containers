// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use async_trait::async_trait;

use super::{ConnectConfig, Sock, Stream};

unsafe impl Send for Vsock {}
unsafe impl Sync for Vsock {}

#[derive(Debug, PartialEq)]
pub struct Vsock {
    cid: u32,
    port: u32,
}

impl Vsock {
    pub fn new(cid: u32, port: u32) -> Self {
        Self { cid, port }
    }
}

#[async_trait]
impl Sock for Vsock {
    async fn connect(&self, _config: &ConnectConfig) -> Result<Stream> {
        todo!()
    }
}
