// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt;

use anyhow::{Context, Result};
use async_trait::async_trait;
use dbs_utils::net::MacAddr as DragonballMacAddr;
use dragonball::api::v1::{
    Backend as DragonballNetworkBackend, NetworkInterfaceConfig as DragonballNetworkConfig,
    VirtioConfig as DragonballVirtioConfig,
};

use crate::{
    device::{Device, DeviceType},
    Hypervisor as hypervisor,
};

#[derive(Clone)]
pub struct Address(pub [u8; 6]);

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let b = self.0;
        write!(
            f,
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5]
        )
    }
}

#[derive(Clone, Debug)]
pub enum NetworkBackend {
    Virtio(VirtioConfig),
    Vhost(VirtioConfig),
}

impl Default for NetworkBackend {
    fn default() -> Self {
        Self::Virtio(VirtioConfig::default())
    }
}

impl From<NetworkBackend> for DragonballNetworkBackend {
    fn from(value: NetworkBackend) -> Self {
        match value {
            NetworkBackend::Virtio(config) => Self::Virtio(DragonballVirtioConfig {
                iface_id: config.virt_iface_name.clone(),
                host_dev_name: config.host_dev_name.clone(),
                rx_rate_limiter: None,
                tx_rate_limiter: None,
                allow_duplicate_mac: config.allow_duplicate_mac,
            }),
            NetworkBackend::Vhost(config) => Self::Vhost(DragonballVirtioConfig {
                iface_id: config.virt_iface_name.clone(),
                host_dev_name: config.host_dev_name.clone(),
                rx_rate_limiter: None,
                tx_rate_limiter: None,
                allow_duplicate_mac: config.allow_duplicate_mac,
            }),
        }
    }
}

/// Virtio network backend config
#[derive(Clone, Debug, Default)]
pub struct VirtioConfig {
    /// Host level path for the guest network interface.
    pub host_dev_name: String,
    /// Guest iface name for the guest network interface.
    pub virt_iface_name: String,
    /// Allow duplicate mac
    pub allow_duplicate_mac: bool,
}

#[derive(Clone, Debug, Default)]
pub struct NetworkConfig {
    /// for detach, now it's default value 0.
    pub index: u64,

    /// Network device backend
    pub backend: NetworkBackend,
    /// Guest MAC address.
    pub guest_mac: Option<Address>,
    /// Virtio queue size
    pub queue_size: usize,
    /// Virtio queue num
    pub queue_num: usize,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

impl From<NetworkConfig> for DragonballNetworkConfig {
    fn from(value: NetworkConfig) -> Self {
        let r = &value;
        r.into()
    }
}

impl From<&NetworkConfig> for DragonballNetworkConfig {
    fn from(value: &NetworkConfig) -> Self {
        Self {
            num_queues: Some(value.queue_num),
            queue_size: Some(value.queue_size as u16),
            backend: value.backend.clone().into(),
            guest_mac: value.guest_mac.clone().map(|mac| {
                // We are safety since mac address is checked by endpoints.
                DragonballMacAddr::from_bytes(&mac.0).unwrap()
            }),
            use_shared_irq: value.use_shared_irq,
            use_generic_irq: value.use_generic_irq,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct NetworkDevice {
    /// Unique identifier of the device
    pub device_id: String,

    /// Network Device config info
    pub config: NetworkConfig,
}

impl NetworkDevice {
    // new creates a NetworkDevice
    pub fn new(device_id: String, config: &NetworkConfig) -> Self {
        Self {
            device_id,
            config: config.clone(),
        }
    }
}

#[async_trait]
impl Device for NetworkDevice {
    async fn attach(&mut self, h: &dyn hypervisor) -> Result<()> {
        h.add_device(DeviceType::Network(self.clone()))
            .await
            .context("add network device.")?;

        return Ok(());
    }

    async fn detach(&mut self, h: &dyn hypervisor) -> Result<Option<u64>> {
        h.remove_device(DeviceType::Network(self.clone()))
            .await
            .context("remove network device.")?;

        Ok(Some(self.config.index))
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::Network(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        // network devices will not be attached multiple times, Just return Ok(false)

        Ok(false)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        // network devices will not be detached multiple times, Just return Ok(false)

        Ok(false)
    }
}
