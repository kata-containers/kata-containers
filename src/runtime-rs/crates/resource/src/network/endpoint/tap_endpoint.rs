// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::{do_handle_device, DeviceManager};
use hypervisor::device::{DeviceConfig, DeviceType};
use hypervisor::{Hypervisor, NetworkConfig, NetworkDevice};
use tokio::sync::RwLock;

use super::endpoint_persist::TapEndpointState;
use super::Endpoint;
use crate::network::network_pair::{get_link_by_name, NetworkInterface};
use crate::network::{utils, EndpointState};

/// TapEndpoint is used to attach to the hypervisor directly
#[derive(Debug)]
pub struct TapEndpoint {
    // Name of virt interface
    name: String,
    // Hardware address of virt interface
    guest_mac: String,
    // Tap interface on the host
    tap_iface: NetworkInterface,
    // Device manager
    dev_mgr: Arc<RwLock<DeviceManager>>,
    // Virtio queue num
    queue_num: usize,
    // Virtio queue size
    queue_size: usize,
}

impl TapEndpoint {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        handle: &rtnetlink::Handle,
        name: &str,
        tap_name: &str,
        guest_mac: &str,
        queue_num: usize,
        queue_size: usize,
        dev_mgr: &Arc<RwLock<DeviceManager>>,
    ) -> Result<Self> {
        let tap_link = get_link_by_name(handle, tap_name)
            .await
            .context("get link by name")?;
        let tap_hard_addr =
            utils::get_mac_addr(&tap_link.attrs().hardware_addr).context("Get mac addr of tap")?;

        Ok(TapEndpoint {
            name: name.to_owned(),
            guest_mac: guest_mac.to_owned(),
            tap_iface: NetworkInterface {
                name: tap_name.to_owned(),
                hard_addr: tap_hard_addr,
                ..Default::default()
            },
            dev_mgr: dev_mgr.clone(),
            queue_num,
            queue_size,
        })
    }

    fn get_network_config(&self) -> Result<NetworkConfig> {
        let guest_mac = utils::parse_mac(&self.guest_mac).context("Parse mac address")?;
        Ok(NetworkConfig {
            host_dev_name: self.tap_iface.name.clone(),
            virt_iface_name: self.name.clone(),
            guest_mac: Some(guest_mac),
            queue_num: self.queue_num,
            queue_size: self.queue_size,
            ..Default::default()
        })
    }
}

#[async_trait]
impl Endpoint for TapEndpoint {
    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn hardware_addr(&self) -> String {
        self.guest_mac.clone()
    }

    async fn attach(&self) -> Result<()> {
        let config = self.get_network_config().context("Get network config")?;
        do_handle_device(&self.dev_mgr, &DeviceConfig::NetworkCfg(config))
            .await
            .context("Handle device")?;
        Ok(())
    }

    async fn detach(&self, h: &dyn Hypervisor) -> Result<()> {
        let config = self.get_network_config().context("Get network config")?;
        h.remove_device(DeviceType::Network(NetworkDevice {
            config,
            ..Default::default()
        }))
        .await
        .context("Remove device")?;
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
