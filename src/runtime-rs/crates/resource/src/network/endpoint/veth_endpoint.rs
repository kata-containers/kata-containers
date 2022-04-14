// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::{self, Error};

use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::{device::NetworkConfig, Device, Hypervisor};

use super::Endpoint;
use crate::network::{utils, NetworkPair};

#[derive(Debug)]
pub struct VethEndpoint {
    net_pair: NetworkPair,
}

impl VethEndpoint {
    pub async fn new(
        handle: &rtnetlink::Handle,
        name: &str,
        idx: u32,
        model: &str,
        queues: usize,
    ) -> Result<Self> {
        let net_pair = NetworkPair::new(handle, idx, name, model, queues)
            .await
            .context("new networkInterfacePair")?;
        Ok(VethEndpoint { net_pair })
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
impl Endpoint for VethEndpoint {
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
            .context("add network model")?;
        let config = self.get_network_config().context("get network config")?;
        h.add_device(Device::Network(config))
            .await
            .context("Error add device")?;
        Ok(())
    }

    async fn detach(&self, h: &dyn Hypervisor) -> Result<()> {
        self.net_pair
            .del_network_model()
            .await
            .context("del network model")?;
        let config = self.get_network_config().context("get network config")?;
        h.remove_device(Device::Network(config))
            .await
            .context("remove device")?;
        Ok(())
    }
}
