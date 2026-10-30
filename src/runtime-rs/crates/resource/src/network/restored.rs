// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{device::driver, Hypervisor};
use kata_sys_util::netns::NetnsGuard;
use scopeguard::defer;

use super::network_pair::delete_link_by_name;
use super::{rebind_saved_physical_endpoints, EndpointState, Network};

/// The network of a sandbox restored after its shim went away.
///
/// A live network holds the netns and the endpoints it attached. A restored
/// one only has what state.json recorded about them, and what is left to do
/// with that is give passed-through devices back to their host drivers (the
/// VM is gone, so there is nothing to detach them from) and remove the TAPs
/// of endpoints whose attach never started from the namespace, when the
/// namespace is the caller's and so outlives the sandbox. Holding this as
/// the resource manager's network keeps one cleanup path for both, and the
/// endpoints stay in the state until that cleanup succeeded.
pub(crate) struct RestoredNetwork {
    endpoints: Vec<EndpointState>,
    netns_path: Option<String>,
}

impl RestoredNetwork {
    pub(crate) fn new(endpoints: Vec<EndpointState>, netns_path: Option<String>) -> Self {
        Self {
            endpoints,
            netns_path,
        }
    }

    /// Remove the TAPs of the endpoints whose attach never started. A
    /// namespace that is already gone took them with it.
    async fn remove_unattached_taps(&self) -> Result<()> {
        let taps: Vec<&str> = self
            .endpoints
            .iter()
            .filter(|e| e.construction_only)
            .filter_map(|e| e.tap_iface_name.as_deref())
            .collect();
        if taps.is_empty() {
            return Ok(());
        }
        let Some(netns_path) = self.netns_path.as_deref() else {
            return Ok(());
        };
        if matches!(
            std::fs::metadata(netns_path),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound
        ) {
            return Ok(());
        }
        let _netns_guard = NetnsGuard::new(netns_path).context("net netns guard")?;
        let (connection, handle, _) =
            rtnetlink::new_connection().context("new netlink connection")?;
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });
        for tap in taps {
            delete_link_by_name(&handle, tap)
                .await
                .with_context(|| format!("remove the created tap {tap}"))?;
        }
        Ok(())
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
        // The TAPs and the passed-through NICs are independent: a failure on
        // one side must not keep the other from being tried, only keep the
        // state around for a retry.
        let taps = self
            .remove_unattached_taps()
            .await
            .context("remove the taps of unattached endpoints");
        if let Err(err) = &taps {
            // A rebind failure below would otherwise be the only error told.
            warn!(sl!(), "failed to remove unattached taps: {err:#}");
        }
        rebind_saved_physical_endpoints(&self.endpoints, driver::bind_device_to_host)?;
        taps
    }

    async fn netns_path(&self) -> Option<String> {
        self.netns_path.clone()
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
        let network = RestoredNetwork::new(endpoints.clone(), Some("/run/netns/cni-1".into()));

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
