// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use async_trait::async_trait;
use std::{collections::HashMap, fs, path::Path};

use crate::share_fs::{do_get_guest_path, do_get_host_path};

use super::{share_fs_volume::generate_mount_path, Volume};
use agent::Storage;
use anyhow::{anyhow, Context};
use hypervisor::{
    device::{device_manager::DeviceManager, DeviceConfig},
    BlockConfig,
};
use nix::sys::stat::{self, SFlag};
use tokio::sync::RwLock;
#[derive(Debug)]
pub(crate) struct BlockVolume {
    storage: Option<agent::Storage>,
    mount: oci::Mount,
    device_id: String,
}

/// BlockVolume: block device volume
impl BlockVolume {
    pub(crate) async fn new(
        d: &RwLock<DeviceManager>,
        m: &oci::Mount,
        read_only: bool,
        cid: &str,
        sid: &str,
    ) -> Result<Self> {
        let fstat = stat::stat(m.source.as_str()).context(format!("stat {}", m.source))?;
        info!(sl!(), "device stat: {:?}", fstat);
        let mut options = HashMap::new();
        if read_only {
            options.insert("read_only".to_string(), "true".to_string());
        }

        let block_device_config = &mut BlockConfig {
            major: stat::major(fstat.st_rdev) as i64,
            minor: stat::minor(fstat.st_rdev) as i64,
            ..Default::default()
        };

        let device_id = d
            .write()
            .await
            .new_device(&DeviceConfig::BlockCfg(block_device_config.clone()))
            .await
            .context("failed to create deviec")?;

        d.write()
            .await
            .try_add_device(device_id.as_str())
            .await
            .context("failed to add deivce")?;

        let file_name = Path::new(&m.source).file_name().unwrap().to_str().unwrap();
        let file_name = generate_mount_path(cid, file_name);
        let guest_path = do_get_guest_path(&file_name, cid, true, false);
        let host_path = do_get_host_path(&file_name, sid, cid, true, read_only);
        fs::create_dir_all(&host_path)
            .map_err(|e| anyhow!("failed to create rootfs dir {}: {:?}", host_path, e))?;

        // get complete device information
        let dev_info = d
            .read()
            .await
            .get_device_info(&device_id)
            .await
            .context("failed to get device info")?;

        // storage
        let mut storage = Storage::default();

        if let DeviceConfig::BlockCfg(config) = dev_info {
            storage.driver = config.driver_option;
            storage.source = config.virt_path;
        }

        storage.options = if read_only {
            vec!["ro".to_string()]
        } else {
            Vec::new()
        };

        storage.mount_point = guest_path.clone();

        // If the volume had specified the filesystem type, use it. Otherwise, set it
        // to ext4 since but right now we only support it.
        if m.r#type != "bind" {
            storage.fs_type = m.r#type.clone();
        } else {
            storage.fs_type = "ext4".to_string();
        }

        // mount
        let mount = oci::Mount {
            destination: m.destination.clone(),
            r#type: m.r#type.clone(),
            source: guest_path.clone(),
            options: m.options.clone(),
        };

        Ok(Self {
            storage: Some(storage),
            mount,
            device_id,
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
    if m.r#type != "bind" {
        return false;
    }
    if let Ok(fstat) = stat::stat(m.source.as_str()).context(format!("stat {}", m.source)) {
        info!(sl!(), "device stat: {:?}", fstat);
        return SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFBLK;
    }
    false
}
