// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::{self, Error};
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::{do_handle_device, DeviceManager};
use hypervisor::device::driver::NetworkConfig;
use hypervisor::device::{DeviceConfig, DeviceType};
use hypervisor::{Hypervisor, NetworkDevice};
use tokio::sync::RwLock;

use super::endpoint_persist::{EndpointState, VlanEndpointState};
use super::Endpoint;
use crate::network::network_model::TC_FILTER_NET_MODEL_STR;
use crate::network::{utils, NetworkPair};

#[derive(Debug)]
pub struct VlanEndpoint {
    pub(crate) net_pair: NetworkPair,
    pub(crate) d: Arc<RwLock<DeviceManager>>,
}

impl VlanEndpoint {
    pub async fn new(
        d: &Arc<RwLock<DeviceManager>>,
        handle: &rtnetlink::Handle,
        name: &str,
        idx: u32,
        queues: usize,
    ) -> Result<Self> {
        let net_pair = NetworkPair::new(handle, idx, name, TC_FILTER_NET_MODEL_STR, queues)
            .await
            .context("new network interface pair failed.")?;

        Ok(VlanEndpoint {
            net_pair,
            d: d.clone(),
        })
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
            host_dev_name: iface.name.clone(),
            virt_iface_name: self.net_pair.virt_iface.name.clone(),
            guest_mac: Some(guest_mac),
            ..Default::default()
        })
    }
}

#[async_trait]
impl Endpoint for VlanEndpoint {
    async fn name(&self) -> String {
        self.net_pair.virt_iface.name.clone()
    }

    async fn hardware_addr(&self) -> String {
        self.net_pair.tap.tap_iface.hard_addr.clone()
    }

    async fn attach(&self) -> Result<()> {
        self.net_pair
            .add_network_model()
            .await
            .context("add network model failed.")?;

        let config = self.get_network_config().context("get network config")?;
        do_handle_device(&self.d, &DeviceConfig::NetworkCfg(config))
            .await
            .context("do handle network Vlan endpoint device failed.")?;

        Ok(())
    }

    async fn detach(&self, h: &dyn Hypervisor) -> Result<()> {
        self.net_pair
            .del_network_model()
            .await
            .context("delete network model failed.")?;

        let config = self
            .get_network_config()
            .context("get network config failed.")?;
        h.remove_device(DeviceType::Network(NetworkDevice {
            config,
            ..Default::default()
        }))
        .await
        .context("remove Vlan endpoint device by hypervisor failed.")?;

        Ok(())
    }

    async fn save(&self) -> Option<EndpointState> {
        Some(EndpointState {
            vlan_endpoint: Some(VlanEndpointState {
                if_name: self.net_pair.virt_iface.name.clone(),
                network_qos: self.net_pair.network_qos,
            }),
            ..Default::default()
        })
    }
}
