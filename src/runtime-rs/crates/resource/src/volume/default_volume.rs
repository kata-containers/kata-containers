// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use hypervisor::device::device_manager::DeviceManager;
use tokio::sync::RwLock;

use anyhow::Result;
use async_trait::async_trait;

use super::{generate_volume_id, Volume};

#[derive(Debug)]
pub(crate) struct DefaultVolume {
    id: String,
    mount: oci::Mount,
}

/// DefaultVolume: passthrough the mount to guest
impl DefaultVolume {
    pub fn new(mount: &oci::Mount) -> Result<Self> {
        let id = generate_volume_id();
        Ok(Self {
            id,
            mount: mount.clone(),
        })
    }
}

#[async_trait]
impl Volume for DefaultVolume {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(vec![])
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        // TODO: Clean up DefaultVolume
        warn!(sl!(), "Cleaning up DefaultVolume is still unimplemented.");
        Ok(())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }
}
