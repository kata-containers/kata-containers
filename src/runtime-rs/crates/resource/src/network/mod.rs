// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod endpoint;
pub use endpoint::Endpoint;
mod network_entity;
mod network_info;
pub use network_info::NetworkInfo;
mod network_model;
pub use network_model::NetworkModel;
mod network_with_netns;
pub use network_with_netns::NetworkWithNetNsConfig;
use network_with_netns::NetworkWithNetns;
mod network_pair;
use network_pair::NetworkPair;
mod utils;

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::Hypervisor;

#[derive(Debug)]
pub enum NetworkConfig {
    NetworkResourceWithNetNs(NetworkWithNetNsConfig),
}

#[async_trait]
pub trait Network: Send + Sync {
    async fn setup(&self, h: &dyn Hypervisor) -> Result<()>;
    async fn interfaces(&self) -> Result<Vec<agent::Interface>>;
    async fn routes(&self) -> Result<Vec<agent::Route>>;
    async fn neighs(&self) -> Result<Vec<agent::ARPNeighbor>>;
}

pub async fn new(config: &NetworkConfig) -> Result<Arc<dyn Network>> {
    match config {
        NetworkConfig::NetworkResourceWithNetNs(c) => Ok(Arc::new(
            NetworkWithNetns::new(c)
                .await
                .context("new network with netns")?,
        )),
    }
}
