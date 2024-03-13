// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt;

use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::device::topology::PCIeTopology;
use crate::device::{Device, DeviceType};
use crate::Hypervisor as hypervisor;

#[derive(Clone, Default)]
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

#[derive(Clone, Debug, Default)]
pub struct NetworkConfig {
    /// for detach, now it's default value 0.
    pub index: u64,

    /// Host level path for the guest network interface.
    pub host_dev_name: String,
    /// Guest iface name for the guest network interface.
    pub virt_iface_name: String,
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
    /// Allow duplicate mac
    pub allow_duplicate_mac: bool,
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
    async fn attach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<()> {
        h.add_device(DeviceType::Network(self.clone()))
            .await
            .context("add network device.")?;

        return Ok(());
    }

    async fn detach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<Option<u64>> {
        h.remove_device(DeviceType::Network(self.clone()))
            .await
            .context("remove network device.")?;

        Ok(Some(self.config.index))
    }

    async fn update(&mut self, _h: &dyn hypervisor) -> Result<()> {
        // There's no need to do update for network device
        Ok(())
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
