// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::{self, Error};

use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::{Device, Hypervisor, NetworkConfig};

use crate::network::{
    network_pair::{get_link_by_name, NetworkInterface},
    utils, EndpointState,
};

use super::{endpoint_persist::TapEndpointState, Endpoint};

// TapEndpoint is used to attach to the hypervisor directly
#[derive(Debug)]
pub struct TapEndpoint {
    // Name of virt interface
    name: String,
    // Hardware address of virt interface
    hard_addr: String,
    // Tap interface on the host
    tap_iface: NetworkInterface,
}

impl TapEndpoint {
    pub async fn new(handle: &rtnetlink::Handle, idx: u32, tap_name: &str) -> Result<Self> {
        let name = format!("eth{}", idx);
        let hard_addr = utils::generate_private_mac_addr().context("Generate priviate mac addr")?;

        let tap_link = get_link_by_name(handle, tap_name)
            .await
            .context("get link by name")?;
        let tap_hard_addr =
            utils::get_mac_addr(&tap_link.attrs().hardware_addr).context("Get mac addr of tap")?;

        Ok(TapEndpoint {
            name,
            hard_addr,
            tap_iface: NetworkInterface {
                name: tap_name.to_owned(),
                hard_addr: tap_hard_addr,
                ..Default::default()
            },
        })
    }

    fn get_network_config(&self) -> Result<NetworkConfig> {
        let guest_mac = utils::parse_mac(&self.hard_addr).ok_or_else(|| {
            Error::new(
                io::ErrorKind::InvalidData,
                format!("hard_addr {}", &self.hard_addr),
            )
        })?;
        Ok(NetworkConfig {
            id: self.name.clone(),
            host_dev_name: self.tap_iface.name.clone(),
            guest_mac: Some(guest_mac),
        })
    }
}

#[async_trait]
impl Endpoint for TapEndpoint {
    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn hardware_addr(&self) -> String {
        self.hard_addr.clone()
    }

    async fn attach(&self, h: &dyn Hypervisor) -> Result<()> {
        let config = self.get_network_config().context("Get network config")?;
        h.add_device(Device::Network(config))
            .await
            .context("Failed to add device")?;
        Ok(())
    }

    async fn detach(&self, h: &dyn Hypervisor) -> Result<()> {
        let config = self.get_network_config().context("Get network config")?;
        h.remove_device(Device::Network(config))
            .await
            .context("Failed to add device")?;
        Ok(())
    }

    async fn save(&self) -> Option<EndpointState> {
        Some(EndpointState {
            tap_endpoint: Some(TapEndpointState {
                if_name: self.name.clone(),
            }),
            ..Default::default()
        })
    }
}
