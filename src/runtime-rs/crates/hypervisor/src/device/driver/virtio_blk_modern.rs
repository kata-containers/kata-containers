// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;
use tokio::sync::Mutex;

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
        write!(f, "{to_string}")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum BlockDeviceFormat {
    #[default]
    Raw,
    Vmdk,
}

impl std::fmt::Display for BlockDeviceFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let to_string = match *self {
            BlockDeviceFormat::Raw => "raw".to_string(),
            BlockDeviceFormat::Vmdk => "vmdk".to_string(),
        };
        write!(f, "{to_string}")
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockConfigModern {
    /// Path of the drive.
    pub path_on_host: String,

    /// If set to true, the drive is opened in read-only mode. Otherwise, the
    /// drive is opened as read-write.
    pub is_readonly: bool,

    /// Enables discard/unmap support for this block device.
    pub discard_unmap: bool,

    /// Don't close `path_on_host` file when dropping the device.
    pub no_drop: bool,

    /// raw, vmdk, etc. And default to raw if not set.
    pub format: BlockDeviceFormat,

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

    /// CCW device address for virtio-blk-ccw on s390x (e.g., "0.0.0005")
    pub ccw_addr: Option<String>,

    /// device attach count
    pub attach_count: u64,

    /// device major number
    pub major: i64,

    /// device minor number
    pub minor: i64,

    /// virtio queue size. size: byte
    pub queue_size: u32,

    /// block device multi-queue
    pub num_queues: usize,

    /// Logical sector size in bytes reported to the guest. 0 means use hypervisor default.
    pub logical_sector_size: u32,

    /// Physical sector size in bytes reported to the guest. 0 means use hypervisor default.
    pub physical_sector_size: u32,

    /// Override the QEMU virtio serial for this device.
    /// When set, the device is discoverable in the guest via
    /// `/dev/disk/by-id/virtio-<serial>`.
    /// If empty, the default `image-{device_id}` serial is used.
    pub serial_override: String,
}

#[derive(Debug, Clone, Default)]
pub struct BlockDeviceModern {
    pub device_id: String,
    pub attach_count: u64,
    pub config: BlockConfigModern,
}

#[derive(Debug, Clone)]
pub struct BlockDeviceModernHandle {
    inner: Arc<Mutex<BlockDeviceModern>>,
}

impl BlockDeviceModernHandle {
    pub fn new(device_id: String, config: BlockConfigModern) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BlockDeviceModern {
                device_id,
                attach_count: 0,
                config,
            })),
        }
    }

    pub fn arc(&self) -> Arc<Mutex<BlockDeviceModern>> {
        self.inner.clone()
    }

    pub async fn snapshot_config(&self) -> BlockConfigModern {
        self.inner.lock().await.config.clone()
    }

    pub async fn device_id(&self) -> String {
        self.inner.lock().await.device_id.clone()
    }

    pub async fn attach_count(&self) -> u64 {
        self.inner.lock().await.attach_count
    }
}

#[async_trait]
impl Device for BlockDeviceModernHandle {
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

        if let Err(e) = h.add_device(DeviceType::BlockModern(self.arc())).await {
            error!(sl!(), "failed to attach block device: {:?}", e);
            self.decrease_attach_count().await?;

            return Err(e);
        }

        Ok(())
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
        if let Err(e) = h.remove_device(DeviceType::BlockModern(self.arc())).await {
            self.increase_attach_count().await?;
            return Err(e);
        }
        Ok(Some(self.snapshot_config().await.index))
    }

    async fn update(&mut self, _h: &dyn hypervisor) -> Result<()> {
        // There's no need to do update for virtio-blk
        Ok(())
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::BlockModern(self.inner.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        let mut guard = self.inner.lock().await;
        do_increase_count(&mut guard.attach_count)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        let mut guard = self.inner.lock().await;
        do_decrease_count(&mut guard.attach_count)
    }
}
