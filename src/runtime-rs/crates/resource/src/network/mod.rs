// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

mod dan;
mod endpoint;
pub use dan::{dan_config_path, Dan, DanNetworkConfig};
pub use endpoint::endpoint_persist::EndpointState;
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
pub use kata_sys_util::netns::{generate_netns_name, NetnsGuard};
use tokio::sync::RwLock;

use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::{device::device_manager::DeviceManager, Hypervisor};

#[derive(Debug)]
pub enum NetworkConfig {
    NetNs(NetworkWithNetNsConfig),
    Dan(DanNetworkConfig),
}

#[async_trait]
pub trait Network: Send + Sync {
    async fn setup(&self) -> Result<()>;
    async fn interfaces(&self) -> Result<Vec<agent::Interface>>;
    async fn routes(&self) -> Result<Vec<agent::Route>>;
    async fn neighs(&self) -> Result<Vec<agent::ARPNeighbor>>;
    async fn save(&self) -> Option<Vec<EndpointState>>;
    async fn remove(&self, h: &dyn Hypervisor) -> Result<()>;
}

pub async fn new(
    config: &NetworkConfig,
    d: Arc<RwLock<DeviceManager>>,
) -> Result<Arc<dyn Network>> {
    match config {
        NetworkConfig::NetNs(c) => Ok(Arc::new(
            NetworkWithNetns::new(c, d)
                .await
                .context("new network with netns")?,
        )),
        NetworkConfig::Dan(c) => Ok(Arc::new(
            Dan::new(c, d)
                .await
                .context("New directly attachable network")?,
        )),
    }
}
