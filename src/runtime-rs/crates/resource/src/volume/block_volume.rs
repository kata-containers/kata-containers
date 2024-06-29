// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::Volume;
use crate::volume::utils::{handle_block_volume, DEFAULT_VOLUME_FS_TYPE, KATA_MOUNT_BIND_TYPE};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_driver, DeviceManager},
        DeviceConfig,
    },
    BlockConfig,
};
use kata_sys_util::mount::get_mount_path;
use nix::sys::{stat, stat::SFlag};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

#[derive(Clone)]
pub(crate) struct BlockVolume {
    storage: Option<agent::Storage>,
    mount: oci::Mount,
    device_id: String,
}

/// BlockVolume for bind-mount block volume
impl BlockVolume {
    pub(crate) async fn new(
        d: &RwLock<DeviceManager>,
        m: &oci::Mount,
        read_only: bool,
        sid: &str,
    ) -> Result<Self> {
        let mnt_src = match m.source() {
            Some(path) => path,
            None => return Err(anyhow!("mount source path is empty")),
        };
        let block_driver = get_block_driver(d).await;
        let fstat = stat::stat(mnt_src).context(format!("stat {}", mnt_src.display()))?;
        let block_device_config = BlockConfig {
            major: stat::major(fstat.st_rdev) as i64,
            minor: stat::minor(fstat.st_rdev) as i64,
            driver_option: block_driver,
            ..Default::default()
        };

        // create and insert block device into Kata VM
        let device_info = do_handle_device(d, &DeviceConfig::BlockCfg(block_device_config.clone()))
            .await
            .context("do handle device failed.")?;

        let block_volume =
            handle_block_volume(device_info, m, read_only, sid, DEFAULT_VOLUME_FS_TYPE)
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
impl Volume for BlockVolume {
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

pub(crate) fn is_block_volume(m: &oci::Mount) -> bool {
    let mnt_type: Option<String> = m.typ().clone();

    if mnt_type.clone().is_none() || mnt_type.unwrap().as_str() != KATA_MOUNT_BIND_TYPE {
        return false;
    }

    match stat::stat(get_mount_path(m.source()).as_str()) {
        Ok(fstat) => SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFBLK,
        Err(_) => false,
    }
}
