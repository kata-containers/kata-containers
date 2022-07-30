// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::{self, Error};

use super::endpoint_persist::{EndpointState, IpVlanEndpointState};
use anyhow::{Context, Result};
use async_trait::async_trait;

use super::Endpoint;
use crate::network::network_model::TC_FILTER_NET_MODEL_STR;
use crate::network::{utils, NetworkPair};
use hypervisor::{device::NetworkConfig, Device, Hypervisor};

// IPVlanEndpoint is the endpoint bridged to VM
#[derive(Debug)]
pub struct IPVlanEndpoint {
    pub(crate) net_pair: NetworkPair,
}

impl IPVlanEndpoint {
    pub async fn new(
        handle: &rtnetlink::Handle,
        name: &str,
        idx: u32,
        queues: usize,
    ) -> Result<Self> {
        // tc filter network model is the only one works for ipvlan
        let net_pair = NetworkPair::new(handle, idx, name, TC_FILTER_NET_MODEL_STR, queues)
            .await
            .context("error creating new NetworkPair")?;
        Ok(IPVlanEndpoint { net_pair })
    }

    fn get_network_config(&self) -> Result<NetworkConfig> {
        let iface = &self.net_pair.tap.tap_iface;
        let guest_mac = utils::parse_mac(&iface.hard_addr).ok_or_else(|| {
            Error::new(
                io::ErrorKind::InvalidData,
                format!("hard_addr {}", &iface.hard_addr),
            )
        })?;
        Ok(NetworkConfig {
            id: self.net_pair.virt_iface.name.clone(),
            host_dev_name: iface.name.clone(),
            guest_mac: Some(guest_mac),
        })
    }
}

#[async_trait]
impl Endpoint for IPVlanEndpoint {
    async fn name(&self) -> String {
        self.net_pair.virt_iface.name.clone()
    }

    async fn hardware_addr(&self) -> String {
        self.net_pair.tap.tap_iface.hard_addr.clone()
    }

    async fn attach(&self, h: &dyn Hypervisor) -> Result<()> {
        self.net_pair
            .add_network_model()
            .await
            .context("error adding network model")?;
        let config = self.get_network_config().context("get network config")?;
        h.add_device(Device::Network(config))
            .await
            .context("error adding device by hypervisor")?;

        Ok(())
    }

    async fn detach(&self, h: &dyn Hypervisor) -> Result<()> {
        self.net_pair
            .del_network_model()
            .await
            .context("error deleting network model")?;
        let config = self
            .get_network_config()
            .context("error getting network config")?;
        h.remove_device(Device::Network(config))
            .await
            .context("error removing device by hypervisor")?;

        Ok(())
    }

    async fn save(&self) -> Option<EndpointState> {
        Some(EndpointState {
            ipvlan_endpoint: Some(IpVlanEndpointState {
                if_name: self.net_pair.virt_iface.name.clone(),
                network_qos: self.net_pair.network_qos,
            }),
            ..Default::default()
        })
    }
}
