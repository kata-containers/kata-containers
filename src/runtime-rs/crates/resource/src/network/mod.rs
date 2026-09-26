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
pub(crate) use network_with_netns::netns_has_interfaces;
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
    /// Returns the list of network endpoints. Used to resolve PCI paths
    /// via QMP before sending update_interface to the agent.
    async fn endpoints(&self) -> Vec<std::sync::Arc<dyn endpoint::Endpoint>> {
        vec![]
    }

    async fn has_passthrough_devices(&self) -> bool {
        for endpoint in self.endpoints().await {
            if endpoint.host_bdf().await.is_some() {
                return true;
            }
        }

        false
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[derive(Debug, Default)]
    struct TestEndpoint {
        bdf: Option<String>,
    }

    #[async_trait]
    impl endpoint::Endpoint for TestEndpoint {
        async fn name(&self) -> String {
            "eth0".to_owned()
        }
        async fn hardware_addr(&self) -> String {
            "02:00:ca:fe:00:04".to_owned()
        }
        async fn attach(&self) -> Result<Option<String>> {
            Ok(None)
        }
        async fn detach(&self, _hypervisor: &dyn Hypervisor) -> Result<()> {
            Ok(())
        }
        async fn save(&self) -> Option<EndpointState> {
            None
        }
        async fn host_bdf(&self) -> Option<String> {
            self.bdf.clone()
        }
    }

    struct TestNetwork {
        endpoints: Vec<Arc<dyn endpoint::Endpoint>>,
    }

    #[async_trait]
    impl Network for TestNetwork {
        async fn setup(&self) -> Result<()> {
            Ok(())
        }
        async fn interfaces(&self) -> Result<Vec<agent::Interface>> {
            Ok(vec![])
        }
        async fn routes(&self) -> Result<Vec<agent::Route>> {
            Ok(vec![])
        }
        async fn neighs(&self) -> Result<Vec<agent::ARPNeighbor>> {
            Ok(vec![])
        }
        async fn save(&self) -> Option<Vec<EndpointState>> {
            None
        }
        async fn remove(&self, _h: &dyn Hypervisor) -> Result<()> {
            Ok(())
        }
        async fn endpoints(&self) -> Vec<Arc<dyn endpoint::Endpoint>> {
            self.endpoints.clone()
        }
    }

    fn test_endpoint(bdf: Option<&str>) -> Arc<TestEndpoint> {
        Arc::new(TestEndpoint {
            bdf: bdf.map(|bdf| bdf.to_owned()),
        })
    }

    fn endpoint(bdf: Option<&str>) -> Arc<dyn endpoint::Endpoint> {
        test_endpoint(bdf)
    }

    #[tokio::test]
    async fn test_has_passthrough_devices() {
        let network = TestNetwork { endpoints: vec![] };
        assert!(!network.has_passthrough_devices().await);

        let network = TestNetwork {
            endpoints: vec![endpoint(None)],
        };
        assert!(!network.has_passthrough_devices().await);

        let network = TestNetwork {
            endpoints: vec![endpoint(None), endpoint(Some("0000:b5:09.7"))],
        };
        assert!(network.has_passthrough_devices().await);
    }
}
