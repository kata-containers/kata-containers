// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_driver, DeviceManager},
        DeviceConfig, DeviceType,
    },
    VhostUserConfig, VhostUserType,
};
use kata_sys_util::mount::{get_mount_options, get_mount_path, get_mount_type};
use kata_types::mount::DirectVolumeMountInfo;
use nix::sys::{stat, stat::SFlag};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

use crate::volume::{
    direct_volumes::{KATA_SPDK_VOLUME_TYPE, KATA_SPOOL_VOLUME_TYPE},
    utils::{generate_shared_path, DEFAULT_VOLUME_FS_TYPE},
    Volume,
};

/// SPDKVolume: spdk block device volume
#[derive(Clone)]
pub(crate) struct SPDKVolume {
    storage: Option<agent::Storage>,
    mount: oci::Mount,
    device_id: String,
}

impl SPDKVolume {
    pub(crate) async fn new(
        d: &RwLock<DeviceManager>,
        m: &oci::Mount,
        mount_info: &DirectVolumeMountInfo,
        read_only: bool,
        sid: &str,
    ) -> Result<Self> {
        let device = match mount_info.volume_type.as_str() {
            KATA_SPDK_VOLUME_TYPE => {
                if mount_info.device.starts_with("spdk://") {
                    mount_info.device.clone()
                } else {
                    format!("spdk://{}", mount_info.device.as_str())
                }
            }
            KATA_SPOOL_VOLUME_TYPE => {
                if mount_info.device.starts_with("spool://") {
                    mount_info.device.clone()
                } else {
                    format!("spool://{}", mount_info.device.as_str())
                }
            }
            _ => return Err(anyhow!("mountinfo.json is invalid")),
        };

        // device format: X:///x/y/z.sock,so just unwrap it.
        // if file is not S_IFSOCK, return error.
        {
            // device tokens: (Type, Socket)
            let device_tokens = device.split_once("://").unwrap();

            let fstat = stat::stat(device_tokens.1).context("stat socket failed")?;
            let s_flag = SFlag::from_bits_truncate(fstat.st_mode);
            if s_flag != SFlag::S_IFSOCK {
                return Err(anyhow!("device {:?} is not valid", device));
            }
        }

        let block_driver = get_block_driver(d).await;

        let vhu_blk_config = &mut VhostUserConfig {
            socket_path: device,
            device_type: VhostUserType::Blk,
            driver_option: block_driver,
            ..Default::default()
        };

        if let Some(num) = mount_info.metadata.get("num_queues") {
            vhu_blk_config.num_queues = num
                .parse::<usize>()
                .context("num queues parse usize failed.")?;
        }
        if let Some(size) = mount_info.metadata.get("queue_size") {
            vhu_blk_config.queue_size = size
                .parse::<u32>()
                .context("num queues parse u32 failed.")?;
        }

        // create and insert block device into Kata VM
        let device_info =
            do_handle_device(d, &DeviceConfig::VhostUserBlkCfg(vhu_blk_config.clone()))
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

        let mut device_id = String::new();
        if let DeviceType::VhostUserBlk(device) = device_info {
            // blk, mmioblk
            storage.driver = device.config.driver_option;
            // /dev/vdX
            storage.source = device.config.virt_path;
            device_id = device.device_id;
        }

        // generate host guest shared path
        let guest_path = generate_shared_path(m.destination().clone(), read_only, &device_id, sid)
            .await
            .context("generate host-guest shared path failed")?;
        storage.mount_point = guest_path.clone();

        if get_mount_type(m).as_str() != "bind" {
            storage.fs_type = mount_info.fs_type.clone();
        } else {
            storage.fs_type = DEFAULT_VOLUME_FS_TYPE.to_string();
        }

        if get_mount_path(&Some(m.destination().clone())).starts_with("/dev") {
            storage.fs_type = "bind".to_string();
            storage.options.append(&mut get_mount_options(m.options()));
        }

        storage.fs_group = None;
        let mut oci_mount = oci::Mount::default();
        oci_mount.set_destination(m.destination().clone());
        oci_mount.set_typ(Some(storage.fs_type.clone()));
        oci_mount.set_source(Some(PathBuf::from(&guest_path)));
        oci_mount.set_options(m.options().clone());

        Ok(Self {
            storage: Some(storage),
            mount: oci_mount,
            device_id,
        })
    }
}

#[async_trait]
impl Volume for SPDKVolume {
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
