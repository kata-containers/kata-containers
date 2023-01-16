// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::{
    ARPNeighbor, IPAddress as AgentIPAddress, IPFamily, Interface as AgentInterface,
    Route as AgentRoute,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;

use super::NetworkInfo;

#[derive(Clone, Debug, Deserialize)]
pub struct DirectlyAttachableNetworkInfo {
    interface: Interface,
    #[serde(default)]
    routes: Vec<Route>,
    #[serde(default)]
    neighbors: Vec<ARPNeighbor>,
}

impl DirectlyAttachableNetworkInfo {
    /// Set device name on the guest
    pub fn set_name(&mut self, name: &str) {
        self.interface.name = name.to_owned();
    }

    /// Set hardware address
    pub fn set_hard_addr(&mut self, hard_addr: &str) {
        self.interface.hard_addr = hard_addr.to_owned();
    }
}

#[async_trait]
impl NetworkInfo for DirectlyAttachableNetworkInfo {
    async fn interface(&self) -> Result<AgentInterface> {
        let mut ip_addresses: Vec<AgentIPAddress> = vec![];
        for addr in self.interface.ip_addresses.iter() {
            let agent_ip_addr = AgentIPAddress {
                family: match addr.family.as_ref() {
                    "v4" => IPFamily::V4,
                    "v6" => IPFamily::V6,
                    _ => {
                        return Err(anyhow!(
                            "Parsing IP address {} failed due to its unsupported IP family {}",
                            addr.address,
                            addr.family
                        ));
                    }
                },
                address: addr.address.clone(),
                mask: addr.mask.clone(),
            };
            ip_addresses.push(agent_ip_addr)
        }
        Ok(AgentInterface {
            device: self.interface.name.clone(),
            name: self.interface.name.clone(),
            ip_addresses,
            hw_addr: self.interface.hard_addr.clone(),
            mtu: self.interface.mtu,
            pci_addr: self.interface.pci_addr.clone(),
            field_type: self.interface.field_type.clone(),
            raw_flags: self.interface.raw_flags,
        })
    }

    async fn routes(&self) -> Result<Vec<AgentRoute>> {
        let mut routes = vec![];
        for route in self.routes.iter() {
            routes.push(AgentRoute {
                dest: route.dest.clone(),
                source: route.source.clone(),
                gateway: route.gateway.clone(),
                device: self.interface.name.clone(),
                scope: route.scope,
                family: match route.family.as_ref() {
                    "v4" => IPFamily::V4,
                    "v6" => IPFamily::V6,
                    _ => return Err(anyhow!("IPFamily {} is unsupported", route.family)),
                },
            });
        }
        Ok(routes)
    }

    async fn neighs(&self) -> Result<Vec<ARPNeighbor>> {
        Ok(self.neighbors.clone())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct IPAddress {
    // IP family, the possible values are "v4" and "v6"
    #[serde(default = "default_ip_family")]
    pub family: String,
    // IP address
    pub address: String,
    // Mask of a IP address, e.g. "24"
    pub mask: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Interface {
    // Device name on the guest
    #[serde(default)]
    name: String,
    #[serde(default)]
    hard_addr: String,
    ip_addresses: Vec<IPAddress>,
    #[serde(default = "default_mtu")]
    mtu: u64,
    #[serde(default)]
    pub pci_addr: String,
    #[serde(default)]
    pub field_type: String,
    #[serde(default)]
    pub raw_flags: u32,
}

fn default_mtu() -> u64 {
    1500
}

#[derive(Clone, Debug, Deserialize)]
pub struct Route {
    #[serde(default)]
    // Destination(CIDR), an empty string denotes no destination
    pub dest: String,
    #[serde(default)]
    // Gateway(IP Address), an empty string denotes no gateway
    pub gateway: String,
    // Source(IP Address), an empty string denotes no gateway
    #[serde(default)]
    pub source: String,
    // Scope
    #[serde(default)]
    pub scope: u32,
    // IP family, the possible values are "v4" and "v6"
    #[serde(default = "default_ip_family")]
    pub family: String,
}

fn default_ip_family() -> String {
    "v4".to_owned()
}
