// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use async_trait::async_trait;

use super::VhostUserConfig;
use crate::{
    device::{
        topology::PCIeTopology,
        util::{do_decrease_count, do_increase_count},
        Device, DeviceType,
    },
    Hypervisor as hypervisor,
};

#[derive(Debug, Clone, Default)]
pub struct VhostUserBlkDevice {
    pub device_id: String,

    /// If set to true, the drive is opened in read-only mode. Otherwise, the
    /// drive is opened as read-write.
    pub is_readonly: bool,

    /// Don't close `path_on_host` file when dropping the device.
    pub no_drop: bool,

    /// driver type for block device
    pub driver_option: String,

    pub attach_count: u64,
    pub config: VhostUserConfig,
}

impl VhostUserBlkDevice {
    // new creates a new VhostUserBlkDevice
    pub fn new(device_id: String, config: VhostUserConfig) -> Self {
        VhostUserBlkDevice {
            device_id,
            attach_count: 0,
            config,
            ..Default::default()
        }
    }
}

#[async_trait]
impl Device for VhostUserBlkDevice {
    async fn attach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<()> {
        // increase attach count, skip attach the device if the device is already attached
        if self
            .increase_attach_count()
            .await
            .context("failed to increase attach count")?
        {
            return Ok(());
        }

        if let Err(e) = h.add_device(DeviceType::VhostUserBlk(self.clone())).await {
            self.decrease_attach_count().await?;

            return Err(e);
        }

        return Ok(());
    }

    async fn detach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<Option<u64>> {
        // get the count of device detached, and detach once it reaches 0
        if self
            .decrease_attach_count()
            .await
            .context("failed to decrease attach count")?
        {
            return Ok(None);
        }

        if let Err(e) = h
            .remove_device(DeviceType::VhostUserBlk(self.clone()))
            .await
        {
            self.increase_attach_count().await?;

            return Err(e);
        }

        Ok(Some(self.config.index))
    }

    async fn update(&mut self, _h: &dyn hypervisor) -> Result<()> {
        // There's no need to do update for vhost-user-blk
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::VhostUserBlk(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        do_increase_count(&mut self.attach_count)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        do_decrease_count(&mut self.attach_count)
    }
}
