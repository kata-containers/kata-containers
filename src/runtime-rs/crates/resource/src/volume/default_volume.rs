// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use hypervisor::device::device_manager::DeviceManager;
use tokio::sync::RwLock;

use super::Volume;
use anyhow::Result;
use async_trait::async_trait;
use oci_spec::runtime as oci;

#[derive(Debug)]
pub(crate) struct DefaultVolume {
    mount: oci::Mount,
}

/// DefaultVolume: passthrough the mount to guest
impl DefaultVolume {
    pub fn new(mount: &oci::Mount) -> Result<Self> {
        Ok(Self {
            mount: mount.clone(),
        })
    }
}

#[async_trait]
impl Volume for DefaultVolume {
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
