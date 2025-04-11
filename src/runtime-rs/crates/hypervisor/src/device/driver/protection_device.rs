// Copyright (c) 2025 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{
    device::{topology::PCIeTopology, Device, DeviceType},
    Hypervisor as hypervisor,
};
use anyhow::{Context, Result};
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub enum ProtectionDeviceConfig {
    SevSnp(SevSnpConfig),
    Se,
}

#[derive(Debug, Clone)]
pub struct SevSnpConfig {
    pub is_snp: bool,
    pub cbitpos: u32,
    pub firmware: String,
}

#[derive(Debug, Clone)]
pub struct ProtectionDevice {
    pub device_id: String,
    pub config: ProtectionDeviceConfig,
}

impl ProtectionDevice {
    pub fn new(device_id: String, config: &ProtectionDeviceConfig) -> Self {
        Self {
            device_id: device_id.clone(),
            config: config.clone(),
        }
    }
}

#[async_trait]
impl Device for ProtectionDevice {
    async fn attach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<()> {
        h.add_device(DeviceType::Protection(self.clone()))
            .await
            .context("add protection device.")?;

        return Ok(());
    }

    // Except for attach() and get_device_info(), the rest of Device operations
    // don't seem to make sense for proctection device.
    async fn detach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        _h: &dyn hypervisor,
    ) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn update(&mut self, _h: &dyn hypervisor) -> Result<()> {
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::Protection(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        Ok(false)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        Ok(false)
    }
}
