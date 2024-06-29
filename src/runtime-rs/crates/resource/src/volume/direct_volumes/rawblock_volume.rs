// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_driver, DeviceManager},
        DeviceConfig,
    },
    BlockConfig,
};
use kata_types::mount::DirectVolumeMountInfo;
use nix::sys::{stat, stat::SFlag};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

use crate::volume::{direct_volumes::KATA_DIRECT_VOLUME_TYPE, utils::handle_block_volume, Volume};

#[derive(Clone)]
pub(crate) struct RawblockVolume {
    storage: Option<agent::Storage>,
    mount: oci::Mount,
    device_id: String,
}

/// RawblockVolume for raw block volume
impl RawblockVolume {
    pub(crate) async fn new(
        d: &RwLock<DeviceManager>,
        m: &oci::Mount,
        mount_info: &DirectVolumeMountInfo,
        read_only: bool,
        sid: &str,
    ) -> Result<Self> {
        let block_driver = get_block_driver(d).await;

        // check volume type
        if mount_info.volume_type != KATA_DIRECT_VOLUME_TYPE {
            return Err(anyhow!(
                "volume type {:?} is invalid",
                mount_info.volume_type
            ));
        }

        let fstat = stat::stat(mount_info.device.as_str())
            .with_context(|| format!("stat volume device file: {}", mount_info.device.clone()))?;
        if SFlag::from_bits_truncate(fstat.st_mode) != SFlag::S_IFREG
            && SFlag::from_bits_truncate(fstat.st_mode) != SFlag::S_IFBLK
        {
            return Err(anyhow!(
                "invalid volume device {:?} for volume type {:?}",
                mount_info.device,
                mount_info.volume_type
            ));
        }

        let block_config = BlockConfig {
            path_on_host: mount_info.device.clone(),
            driver_option: block_driver,
            ..Default::default()
        };

        // create and insert block device into Kata VM
        let device_info = do_handle_device(d, &DeviceConfig::BlockCfg(block_config.clone()))
            .await
            .context("do handle device failed.")?;

        let block_volume = handle_block_volume(device_info, m, read_only, sid, &mount_info.fs_type)
            .await
            .context("do handle block volume failed")?;

        Ok(Self {
            storage: Some(block_volume.0),
            mount: block_volume.1,
            device_id: block_volume.2,
        })
    }
}

#[async_trait]
impl Volume for RawblockVolume {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        let s = if let Some(s) = self.storage.as_ref() {
            vec![s.clone()]
        } else {
            vec![]
        };

        Ok(s)
    }

    async fn cleanup(&self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        device_manager
            .write()
            .await
            .try_remove_device(&self.device_id)
            .await
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(Some(self.device_id.clone()))
    }
}
