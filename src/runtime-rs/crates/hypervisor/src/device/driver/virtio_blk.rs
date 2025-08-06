// Copyright (c) 2022-2023 Alibaba Cloud
// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::pci_path::PciPath;
use crate::device::topology::PCIeTopology;
use crate::device::util::do_decrease_count;
use crate::device::util::do_increase_count;
use crate::device::Device;
use crate::device::DeviceType;
use crate::Hypervisor as hypervisor;
use anyhow::{Context, Result};
use async_trait::async_trait;

/// VIRTIO_BLOCK_PCI indicates block driver is virtio-pci based
pub const VIRTIO_BLOCK_PCI: &str = "virtio-blk-pci";
pub const VIRTIO_BLOCK_MMIO: &str = "virtio-blk-mmio";
pub const VIRTIO_BLOCK_CCW: &str = "virtio-blk-ccw";
pub const VIRTIO_PMEM: &str = "virtio-pmem";
pub const KATA_MMIO_BLK_DEV_TYPE: &str = "mmioblk";
pub const KATA_BLK_DEV_TYPE: &str = "blk";
pub const KATA_CCW_DEV_TYPE: &str = "ccw";
pub const KATA_NVDIMM_DEV_TYPE: &str = "nvdimm";
pub const KATA_SCSI_DEV_TYPE: &str = "scsi";

#[derive(Clone, Copy, Debug, Default)]
pub enum BlockDeviceAio {
    // IoUring is the Linux io_uring I/O implementation.
    #[default]
    IoUring,

    // Native is the native Linux AIO implementation.
    Native,

    // Threads is the pthread asynchronous I/O implementation.
    Threads,
}

impl BlockDeviceAio {
    pub fn new(aio: &str) -> Self {
        match aio {
            "native" => BlockDeviceAio::Native,
            "threads" => BlockDeviceAio::Threads,
            _ => BlockDeviceAio::IoUring,
        }
    }
}

impl std::fmt::Display for BlockDeviceAio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let to_string = match *self {
            BlockDeviceAio::Native => "native".to_string(),
            BlockDeviceAio::Threads => "threads".to_string(),
            _ => "iouring".to_string(),
        };
        write!(f, "{}", to_string)
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockConfig {
    /// Path of the drive.
    pub path_on_host: String,

    /// If set to true, the drive is opened in read-only mode. Otherwise, the
    /// drive is opened as read-write.
    pub is_readonly: bool,

    /// Don't close `path_on_host` file when dropping the device.
    pub no_drop: bool,

    /// Specifies cache-related options for block devices.
    /// Denotes whether use of O_DIRECT (bypass the host page cache) is enabled.
    /// If not set, use configurarion block_device_cache_direct.
    pub is_direct: Option<bool>,

    /// device index
    pub index: u64,

    /// blkdev_aio defines the type of asynchronous I/O the block device should use.
    pub blkdev_aio: BlockDeviceAio,

    /// driver type for block device
    pub driver_option: String,

    /// device path in guest
    pub virt_path: String,

    /// pci path is the slot at which the drive is attached
    pub pci_path: Option<PciPath>,

    /// scsi_addr of the block device, in case the device is attached using SCSI driver
    /// scsi_addr is of the format SCSI-Id:LUN
    pub scsi_addr: Option<String>,

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

        match h.add_device(DeviceType::Block(self.clone())).await {
            Ok(dev) => {
                // Update device info with the one received from device attach
                if let DeviceType::Block(blk) = dev {
                    self.config = blk.config;
                }
                Ok(())
            }
            Err(e) => {
                self.decrease_attach_count().await?;
                return Err(e);
            }
        }
    }

    async fn detach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<Option<u64>> {
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

    async fn update(&mut self, _h: &dyn hypervisor) -> Result<()> {
        // There's no need to do update for virtio-blk
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::Block(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        do_increase_count(&mut self.attach_count)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        do_decrease_count(&mut self.attach_count)
    }
}
