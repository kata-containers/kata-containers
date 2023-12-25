// Copyright (C) 2019-2023 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::{do_handle_device, DeviceManager};
use hypervisor::device::{DeviceConfig, DeviceType};
use hypervisor::{Hypervisor, VhostUserConfig, VhostUserNetDevice, VhostUserType};
use tokio::sync::RwLock;

use super::endpoint_persist::VhostUserEndpointState;
use super::Endpoint;
use crate::network::EndpointState;

/// VhostUserEndpoint uses vhost-user-net device, which supports DPDK, etc.
#[derive(Debug)]
pub struct VhostUserEndpoint {
    // Name of virt interface
    name: String,
    // Hardware address of virt interface
    guest_mac: String,
    // Vhost-user-net device's socket path
    socket_path: String,
    // Device manager
    dev_mgr: Arc<RwLock<DeviceManager>>,
    // Virtio queue num
    queue_num: usize,
    // Virtio queue size
    queue_size: usize,
}

impl VhostUserEndpoint {
    pub async fn new(
        dev_mgr: &Arc<RwLock<DeviceManager>>,
        name: &str,
        guest_mac: &str,
        socket_path: &str,
        queue_num: usize,
        queue_size: usize,
    ) -> Result<Self> {
        let sk_path = Path::new(socket_path);
        if sk_path.exists() {
            return Err(anyhow!("vhost-user-net socket path {} exists", socket_path));
        }

        Ok(VhostUserEndpoint {
            name: name.to_string(),
            guest_mac: guest_mac.to_string(),
            socket_path: socket_path.to_string(),
            dev_mgr: dev_mgr.clone(),
            queue_num,
            queue_size,
        })
    }

    fn get_network_config(&self) -> VhostUserConfig {
        VhostUserConfig {
            socket_path: self.socket_path.clone(),
            mac_address: self.guest_mac.clone(),
            device_type: VhostUserType::Net,
            queue_size: self.queue_size as u32,
            num_queues: self.queue_num,
            ..Default::default()
        }
    }
}

#[async_trait]
impl Endpoint for VhostUserEndpoint {
    async fn name(&self) -> String {
        self.name.clone()
    }

    async fn hardware_addr(&self) -> String {
        self.guest_mac.clone()
    }

    async fn attach(&self) -> Result<()> {
        let config = self.get_network_config();
        do_handle_device(&self.dev_mgr, &DeviceConfig::VhostUserNetworkCfg(config))
            .await
            .context("handle device")?;
        Ok(())
    }

    async fn detach(&self, h: &dyn Hypervisor) -> Result<()> {
        let config = self.get_network_config();
        h.remove_device(DeviceType::VhostUserNetwork(VhostUserNetDevice {
            config,
            ..Default::default()
        }))
        .await
        .context("remove device")?;
        Ok(())
    }

    async fn save(&self) -> Option<EndpointState> {
        Some(EndpointState {
            vhost_user_endpoint: Some(VhostUserEndpointState {
                if_name: self.name.clone(),
                socket_path: self.socket_path.clone(),
            }),
            ..Default::default()
        })
    }
}
