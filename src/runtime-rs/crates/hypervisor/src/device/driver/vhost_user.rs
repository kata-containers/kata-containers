// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device::Device;
use crate::device::DeviceType;
use crate::Hypervisor as hypervisor;
use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug, Clone, Default)]
/// VhostUserConfig represents data shared by most vhost-user devices
pub struct VhostUserConfig {
    /// Device id
    pub dev_id: String,
    /// Socket path
    pub socket_path: String,
    /// Mac_address is only meaningful for vhost user net device
    pub mac_address: String,
    /// These are only meaningful for vhost user fs devices
    pub tag: String,
    pub cache: String,
    pub device_type: String,
    /// Pci_addr is the PCI address used to identify the slot at which the drive is attached.
    pub pci_addr: Option<String>,
    /// Block index of the device if assigned
    pub index: u8,
    pub cache_size: u32,
    pub queue_siez: u32,
}

#[derive(Debug, Clone, Default)]
pub struct VhostUserDevice {
    pub device_id: String,
    pub config: VhostUserConfig,
}

#[async_trait]
impl Device for VhostUserConfig {
    async fn attach(&mut self, _h: &dyn hypervisor) -> Result<()> {
        todo!()
    }

    async fn detach(&mut self, _h: &dyn hypervisor) -> Result<Option<u64>> {
        todo!()
    }

    async fn get_device_info(&self) -> DeviceType {
        todo!()
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        todo!()
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        todo!()
    }
}
