// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hypervisor::{device::driver, Hypervisor};

use super::{rebind_saved_physical_endpoints, EndpointState, Network};

/// The network of a sandbox restored after its shim went away.
///
/// A live network holds the netns and the endpoints it attached. A restored
/// one only has what state.json recorded about them, and the only thing left
/// to do with that is give passed-through devices back to their host drivers:
/// the VM is gone, so there is nothing to detach them from. Holding this as
/// the resource manager's network keeps one cleanup path for both, and the
/// endpoints stay in the state until that cleanup succeeded.
pub(crate) struct RestoredNetwork {
    endpoints: Vec<EndpointState>,
}

impl RestoredNetwork {
    pub(crate) fn new(endpoints: Vec<EndpointState>) -> Self {
        Self { endpoints }
    }
}

#[async_trait]
impl Network for RestoredNetwork {
    async fn setup(&self) -> Result<()> {
        Err(anyhow!("a restored network cannot be set up again"))
    }

    async fn interfaces(&self) -> Result<Vec<agent::Interface>> {
        Err(anyhow!("a restored network has no live interfaces"))
    }

    async fn routes(&self) -> Result<Vec<agent::Route>> {
        Err(anyhow!("a restored network has no live routes"))
    }

    async fn neighs(&self) -> Result<Vec<agent::ARPNeighbor>> {
        Err(anyhow!("a restored network has no live neighbors"))
    }

    async fn save(&self) -> Option<Vec<EndpointState>> {
        Some(self.endpoints.clone())
    }

    async fn remove(&self, _h: &dyn Hypervisor) -> Result<()> {
        rebind_saved_physical_endpoints(&self.endpoints, driver::bind_device_to_host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::endpoint::endpoint_persist::PhysicalEndpointState;

    #[tokio::test]
    async fn a_restored_network_saves_what_it_was_restored_from() {
        let endpoints = vec![EndpointState {
            physical_endpoint: Some(PhysicalEndpointState {
                bdf: "0000:41:00.2".to_string(),
                driver: "mlx5_core".to_string(),
                vendor_id: "0x15b3".to_string(),
                device_id: "0x101e".to_string(),
                hard_addr: String::new(),
            }),
            ..Default::default()
        }];
        let network = RestoredNetwork::new(endpoints.clone());

        let saved = network.save().await.unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(
            saved[0].physical_endpoint.as_ref().unwrap().bdf,
            endpoints[0].physical_endpoint.as_ref().unwrap().bdf
        );
        assert!(network.setup().await.is_err());
        assert!(network.endpoints().await.is_empty());
    }
}
