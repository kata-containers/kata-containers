// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use async_trait::async_trait;

use crate::device::{hypervisor, topology::PCIeTopology, Device, DeviceType};

#[derive(Copy, Clone, Debug, Default)]
pub enum ShareFsMountOperation {
    #[default]
    Mount,
    Umount,
    Update,
}

#[derive(Debug, Default, Clone)]
pub enum ShareFsMountType {
    #[default]
    PASSTHROUGH,
    RAFS,
}

/// ShareFsMountConfig: share fs mount config
#[derive(Clone, Debug, Default)]
pub struct ShareFsMountConfig {
    /// source: the passthrough fs exported dir or rafs meta file of rafs
    pub source: String,

    /// fstype: specifies the type of this sub-fs, could be passthrough-fs or rafs
    pub fstype: ShareFsMountType,

    /// mount_point: the mount point inside guest
    pub mount_point: String,

    /// config: the rafs backend config file
    pub config: Option<String>,

    /// tag: is the tag used inside the kata guest.
    pub tag: String,

    /// op: the operation to take, e.g. mount, umount or update
    pub op: ShareFsMountOperation,

    /// prefetch_list_path: path to file that contains file lists that should be prefetched by rafs
    pub prefetch_list_path: Option<String>,
}

/// ShareFsConfig: Sharefs config for virtio-fs devices and their corresponding mount configurations,
/// facilitating mount/umount/update operations.
#[derive(Clone, Debug, Default)]
pub struct ShareFsConfig {
    /// host_shared_path: the upperdir of the passthrough fs exported dir or rafs meta file of rafs
    pub host_shared_path: String,

    /// fs_type: virtiofs or inline-virtiofs
    pub fs_type: String,

    /// socket_path: socket path for virtiofs
    pub sock_path: String,

    /// mount_tag: a label used as a hint to the guest.
    pub mount_tag: String,

    /// queue_size: queue size
    pub queue_size: u64,

    /// queue_num: queue number
    pub queue_num: u64,

    /// options: virtiofs device's config options.
    pub options: Vec<String>,

    /// mount config for sharefs mount/umount/update
    pub mount_config: Option<ShareFsMountConfig>,
}

#[derive(Debug, Default, Clone)]
pub struct ShareFsDevice {
    /// device id for sharefs device in device manager
    pub device_id: String,

    /// config for sharefs device
    pub config: ShareFsConfig,
}

impl ShareFsDevice {
    // new creates a share-fs device
    pub fn new(device_id: &str, config: &ShareFsConfig) -> Self {
        Self {
            device_id: device_id.to_string(),
            config: config.clone(),
        }
    }
}

#[async_trait]
impl Device for ShareFsDevice {
    async fn attach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        h: &dyn hypervisor,
    ) -> Result<()> {
        h.add_device(DeviceType::ShareFs(self.clone()))
            .await
            .context("add share-fs device.")?;

        Ok(())
    }

    async fn detach(
        &mut self,
        _pcie_topo: &mut Option<&mut PCIeTopology>,
        _h: &dyn hypervisor,
    ) -> Result<Option<u64>> {
        // no need to detach share-fs device

        Ok(None)
    }

    async fn update(&mut self, h: &dyn hypervisor) -> Result<()> {
        h.update_device(DeviceType::ShareFs(self.clone()))
            .await
            .context("update share-fs device.")
    }

    async fn get_device_info(&self) -> DeviceType {
        DeviceType::ShareFs(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        // share-fs devices will not be attached multiple times, Just return Ok(false)

        Ok(false)
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        // share-fs devices will not be detached multiple times, Just return Ok(false)

        Ok(false)
    }
}
