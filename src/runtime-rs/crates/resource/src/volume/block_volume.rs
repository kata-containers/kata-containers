// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use nix::sys::{stat, stat::SFlag};
use tokio::sync::RwLock;

use super::Volume;
use crate::volume::utils::{
    generate_shared_path, get_direct_volume_path, volume_mount_info, DEFAULT_VOLUME_FS_TYPE,
    KATA_DIRECT_VOLUME_TYPE, KATA_MOUNT_BIND_TYPE,
};
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_driver, DeviceManager},
        DeviceConfig, DeviceType,
    },
    BlockConfig,
};

#[derive(Clone)]
pub(crate) struct BlockVolume {
    storage: Option<agent::Storage>,
    mount: oci::Mount,
    device_id: String,
}

/// BlockVolume for bind-mount block volume and direct block volume
impl BlockVolume {
    pub(crate) async fn new(
        d: &RwLock<DeviceManager>,
        m: &oci::Mount,
        read_only: bool,
        sid: &str,
    ) -> Result<Self> {
        let mnt_src: &str = &m.source;
        // default block device fs type: ext4.
        let mut blk_dev_fstype = DEFAULT_VOLUME_FS_TYPE.to_string();

        let block_driver = get_block_driver(d).await;

        let block_device_config = match m.r#type.as_str() {
            KATA_MOUNT_BIND_TYPE => {
                let fstat = stat::stat(mnt_src).context(format!("stat {}", m.source))?;

                BlockConfig {
                    major: stat::major(fstat.st_rdev) as i64,
                    minor: stat::minor(fstat.st_rdev) as i64,
                    driver_option: block_driver,
                    ..Default::default()
                }
            }
            KATA_DIRECT_VOLUME_TYPE => {
                // get volume mountinfo from mountinfo.json
                let v = volume_mount_info(mnt_src)
                    .context("deserde information from mountinfo.json")?;
                // check volume type
                if v.volume_type != KATA_DIRECT_VOLUME_TYPE {
                    return Err(anyhow!("volume type {:?} is invalid", v.volume_type));
                }

                let fstat = stat::stat(v.device.as_str())
                    .with_context(|| format!("stat volume device file: {}", v.device.clone()))?;
                if SFlag::from_bits_truncate(fstat.st_mode) != SFlag::S_IFREG
                    && SFlag::from_bits_truncate(fstat.st_mode) != SFlag::S_IFBLK
                {
                    return Err(anyhow!(
                        "invalid volume device {:?} for volume type {:?}",
                        v.device,
                        v.volume_type
                    ));
                }

                blk_dev_fstype = v.fs_type.clone();

                BlockConfig {
                    path_on_host: v.device,
                    driver_option: block_driver,
                    ..Default::default()
                }
            }
            _ => {
                return Err(anyhow!(
                    "unsupport direct block volume r#type: {:?}",
                    m.r#type.as_str()
                ))
            }
        };

        // create and insert block device into Kata VM
        let device_info = do_handle_device(d, &DeviceConfig::BlockCfg(block_device_config.clone()))
            .await
            .context("do handle device failed.")?;

        // storage
        let mut storage = agent::Storage {
            options: if read_only {
                vec!["ro".to_string()]
            } else {
                Vec::new()
            },
            ..Default::default()
        };

        // As the true Block Device wrapped in DeviceType, we need to
        // get it out from the wrapper, and the device_id will be for
        // BlockVolume.
        // safe here, device_info is correct and only unwrap it.
        let mut device_id = String::new();
        if let DeviceType::Block(device) = device_info {
            // blk, mmioblk
            storage.driver = device.config.driver_option;
            // /dev/vdX
            storage.source = device.config.virt_path;
            device_id = device.device_id;
        }

        // generate host guest shared path
        let guest_path = generate_shared_path(m.destination.clone(), read_only, &device_id, sid)
            .await
            .context("generate host-guest shared path failed")?;
        storage.mount_point = guest_path.clone();

        // In some case, dest is device /dev/xxx
        if m.destination.clone().starts_with("/dev") {
            storage.fs_type = "bind".to_string();
            storage.options.append(&mut m.options.clone());
        } else {
            // usually, the dest is directory.
            storage.fs_type = blk_dev_fstype;
        }

        let mount = oci::Mount {
            destination: m.destination.clone(),
            r#type: storage.fs_type.clone(),
            source: guest_path,
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

pub(crate) fn is_block_volume(m: &oci::Mount) -> Result<bool> {
    let vol_types = [KATA_MOUNT_BIND_TYPE, KATA_DIRECT_VOLUME_TYPE];
    if !vol_types.contains(&m.r#type.as_str()) {
        return Ok(false);
    }

    let source = if m.r#type.as_str() == KATA_DIRECT_VOLUME_TYPE {
        get_direct_volume_path(&m.source).context("get direct volume path failed")?
    } else {
        m.source.clone()
    };

    let fstat =
        stat::stat(source.as_str()).context(format!("stat mount source {} failed.", source))?;
    let s_flag = SFlag::from_bits_truncate(fstat.st_mode);

    match m.r#type.as_str() {
        // case: mount bind and block device
        KATA_MOUNT_BIND_TYPE if s_flag == SFlag::S_IFBLK => Ok(true),
        // case: directvol and directory
        KATA_DIRECT_VOLUME_TYPE if s_flag == SFlag::S_IFDIR => Ok(true),
        // else: unsupported or todo for other volume type.
        _ => Ok(false),
    }
}
