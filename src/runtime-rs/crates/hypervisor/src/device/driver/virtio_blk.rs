// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub const VIRTIO_BLOCK_MMIO: &str = "virtio-blk-mmio";
use crate::device::Device;
use crate::device::{DeviceConfig, DeviceType};
use crate::Hypervisor as hypervisor;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
/// VIRTIO_BLOCK_PCI indicates block driver is virtio-pci based
pub const VIRTIO_BLOCK_PCI: &str = "virtio-blk-pci";
pub const KATA_MMIO_BLK_DEV_TYPE: &str = "mmioblk";
pub const KATA_BLK_DEV_TYPE: &str = "blk";

#[derive(Debug, Clone, Default)]
pub struct BlockConfig {
    /// Path of the drive.
    pub path_on_host: String,

    /// If set to true, the drive is opened in read-only mode. Otherwise, the
    /// drive is opened as read-write.
    pub is_readonly: bool,

    /// Don't close `path_on_host` file when dropping the device.
    pub no_drop: bool,

    /// device index
    pub index: u64,

    /// driver type for block device
    pub driver_option: String,

    /// device path in guest
    pub virt_path: String,

    /// device attach count
    pub attach_count: u64,

    /// device major number
    pub major: i64,

    /// device minor number
    pub minor: i64,
}

#[derive(Debug, Clone, Default)]
pub struct BlockDevice {
    pub device_id: String,
    pub attach_count: u64,
    pub config: BlockConfig,
}

impl BlockDevice {
    // new creates a new VirtioBlkDevice
    pub fn new(device_id: String, config: BlockConfig) -> Self {
        BlockDevice {
            device_id,
            attach_count: 0,
            config,
        }
    }
}

#[async_trait]
impl Device for BlockDevice {
    async fn attach(&mut self, h: &dyn hypervisor) -> Result<()> {
        // increase attach count, skip attach the device if the device is already attached
        if self
            .increase_attach_count()
            .await
            .context("failed to increase attach count")?
        {
            return Ok(());
        }
        if let Err(e) = h.add_device(DeviceType::Block(self.clone())).await {
            self.decrease_attach_count().await?;
            return Err(e);
        }
        return Ok(());
    }

    async fn detach(&mut self, h: &dyn hypervisor) -> Result<Option<u64>> {
        // get the count of device detached, skip detach once it reaches the 0
        if self
            .decrease_attach_count()
            .await
            .context("failed to decrease attach count")?
        {
            return Ok(None);
        }
        if let Err(e) = h.remove_device(DeviceType::Block(self.clone())).await {
            self.increase_attach_count().await?;
            return Err(e);
        }
        Ok(Some(self.config.index))
    }

    async fn get_device_info(&self) -> DeviceConfig {
        DeviceConfig::BlockCfg(self.config.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        match self.attach_count {
            0 => {
                // do real attach
                self.attach_count += 1;
                Ok(false)
            }
            std::u64::MAX => Err(anyhow!("device was attached too many times")),
            _ => {
                self.attach_count += 1;
                Ok(true)
            }
        }
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        match self.attach_count {
            0 => Err(anyhow!("detaching a device that wasn't attached")),
            1 => {
                // do real wrok
                self.attach_count -= 1;
                Ok(false)
            }
            _ => {
                self.attach_count -= 1;
                Ok(true)
            }
        }
    }
}
