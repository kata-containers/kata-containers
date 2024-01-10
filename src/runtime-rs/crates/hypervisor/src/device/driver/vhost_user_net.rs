// Copyright (C) 2019-2023 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::device::topology::PCIeTopology;
use crate::device::{Device, DeviceType};
use crate::{Hypervisor, VhostUserConfig};

#[derive(Debug, Clone, Default)]
/// Vhost-user-net device for device manager.
pub struct VhostUserNetDevice {
    pub device_id: String,
    pub config: VhostUserConfig,
}

impl VhostUserNetDevice {
    pub fn new(device_id: String, config: VhostUserConfig) -> Self {
        Self { device_id, config }
    }
}

#[async_trait]
impl Device for VhostUserNetDevice {
    async fn attach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn Hypervisor,
    ) -> Result<()> {
        h.add_device(DeviceType::VhostUserNetwork(self.clone()))
            .await
            .context("add vhost-user-net device to hypervisor")?;
        Ok(())
    }

    async fn detach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn Hypervisor,
    ) -> Result<Option<u64>> {
        h.remove_device(DeviceType::VhostUserNetwork(self.clone()))
            .await
            .context("remove vhost-user-net device from hypervisor")?;
        Ok(Some(self.config.index))
    }

    async fn update(&mut self, _h: &dyn Hypervisor) -> Result<()> {
        // There's no need to do update for vhost-user-net
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::VhostUserNetwork(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        // Vhost-user-net devices will not be attached multiple times, just
        // return Ok(false)
        Ok(false)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        // Vhost-user-net devices will not be detached multiple times, just
        // return Ok(false)
        Ok(false)
    }
}
