// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::sync::Mutex;

use agent::Storage;
use hypervisor::Hypervisor;
use kata_types::config::{default::DEFAULT_VIRTIOFS_DEVICE, hypervisor::SharedFsInfo};

use super::{
    share_virtio_fs::{
        prepare_virtiofs, setup_inline_virtiofs, FS_TYPE_VIRTIO_FS, KATA_VIRTIO_FS_DEV_TYPE,
        MOUNT_GUEST_TAG,
    },
    utils::parse_sharefs_special_volumes,
    ShareFs, *,
};

lazy_static! {
    pub(crate) static ref SHARED_DIR_VIRTIO_FS_OPTIONS: Vec::<String> = vec![String::from("nodev")];
}

#[derive(Clone, Debug, Default)]
pub struct ShareVirtioFsInlineConfig {
    /// sandbox id
    pub sandbox_id: String,

    /// (virtiofs_device, extra_virtiofs_info)
    pub multi_virtiofs: HashMap<String, Vec<String>>,
}

impl ShareVirtioFsInlineConfig {
    // multi-virtiofs annotation: "virtiofs_device_01:arg01,arg02,...;virtiofs_device_02:arg01,arg02,..."
    pub fn new(sid: &str, extra_virtiofs_devs: &Vec<String>) -> Result<ShareVirtioFsInlineConfig> {
        let mut multi_virtiofs: HashMap<String, Vec<String>> = HashMap::new();
        multi_virtiofs
            .entry(DEFAULT_VIRTIOFS_DEVICE.to_string())
            .or_insert(Vec::new());

        for ex_vfs in extra_virtiofs_devs {
            // virtiofs_device_01:arg01,arg02,...;
            let ex_virtiofs: Vec<&str> = ex_vfs.split(':').collect();
            if ex_virtiofs.len() != 2 {
                return Err(anyhow!("invalid extra virtiofs formats: {}", ex_vfs));
            }

            // virtiofs_device_01
            let virtiofs_name = String::from(ex_virtiofs[0]);
            let extra_virtiofs_args = ex_virtiofs[1].split(',').map(String::from).collect();

            multi_virtiofs.insert(virtiofs_name, extra_virtiofs_args);
        }

        Ok(ShareVirtioFsInlineConfig {
            sandbox_id: sid.to_owned(),
            multi_virtiofs,
        })
    }
}

pub struct ShareVirtioFsInline {
    config: ShareVirtioFsInlineConfig,
    share_fs_mount: Arc<dyn ShareFsMount>,
    mounted_info_set: Arc<Mutex<HashMap<String, MountedInfo>>>,
}

impl ShareVirtioFsInline {
    pub(crate) fn new(
        id: &str,
        config: &SharedFsInfo,
        special_volumes: Vec<String>,
    ) -> Result<Self> {
        let config = ShareVirtioFsInlineConfig::new(id, &config.extra_virtiofs)
            .context("parese extra virtiofs")?;
        let devices: HashSet<&str> = config.multi_virtiofs.keys().map(|x| x.as_str()).collect();
        let mountinfo_map = parse_sharefs_special_volumes(devices, special_volumes)
            .context("parse special volumes failed.")?;

        Ok(Self {
            config,
            share_fs_mount: Arc::new(VirtiofsShareMount::new(id)),
            mounted_info_set: Arc::new(Mutex::new(mountinfo_map)),
        })
    }
}

#[async_trait]
impl ShareFs for ShareVirtioFsInline {
    fn get_share_fs_mount(&self) -> Arc<dyn ShareFsMount> {
        self.share_fs_mount.clone()
    }

    async fn setup_device_before_start_vm(&self, h: &dyn Hypervisor) -> Result<()> {
        // do prepare for virtiofs, if not set, just skip it.
        for (name, device) in self.config.multi_virtiofs.iter() {
            let (virtiofs_args, virtiofs_dev) = if name != DEFAULT_VIRTIOFS_DEVICE {
                (device.clone(), Some(name))
            } else {
                (Vec::new(), None)
            };

            prepare_virtiofs(
                h,
                INLINE_VIRTIO_FS,
                &self.config.sandbox_id,
                "",
                virtiofs_args,
                virtiofs_dev.as_ref().map(|s| s.as_str()),
            )
            .await
            .context("prepare extra virtiofs")?;
        }

        Ok(())
    }

    async fn setup_device_after_start_vm(&self, h: &dyn Hypervisor) -> Result<()> {
        // do setup for default inline virtiofs
        setup_inline_virtiofs(&self.config.sandbox_id, None, h)
            .await
            .context("setup default inline virtiofs")?;

        // do setup for extra virtiofs, if not set, just skip it.
        for device in self.config.multi_virtiofs.keys() {
            let virtiofs_dev = if device != DEFAULT_VIRTIOFS_DEVICE {
                Some(device)
            } else {
                None
            };

            setup_inline_virtiofs(
                &self.config.sandbox_id,
                virtiofs_dev.as_ref().map(|s| s.as_str()),
                h,
            )
            .await
            .context("setup extra inline virtiofs")?;
        }

        Ok(())
    }

    async fn get_storages(&self) -> Result<Vec<Storage>> {
        // setup storage
        let mut storages: Vec<Storage> = Vec::new();

        for (device_name, _device) in self.config.multi_virtiofs.iter() {
            let mut shared_volume: Storage = Storage {
                driver: String::from(KATA_VIRTIO_FS_DEV_TYPE),
                driver_options: Vec::new(),
                fs_type: String::from(FS_TYPE_VIRTIO_FS),
                fs_group: None,
                options: SHARED_DIR_VIRTIO_FS_OPTIONS.clone(),
                ..Default::default()
            };

            if device_name == DEFAULT_VIRTIOFS_DEVICE {
                shared_volume.source = String::from(MOUNT_GUEST_TAG);
                shared_volume.mount_point = String::from(KATA_GUEST_SHARE_DIR);
            } else {
                // /run/kata-containers/shared/<device_name>/
                shared_volume.source = String::from(device_name);
                shared_volume.mount_point = format!("{}/{}", KATA_GUEST_SHARED, device_name);
            }

            storages.push(shared_volume);
        }

        Ok(storages)
    }

    fn mounted_info_set(&self) -> Arc<Mutex<HashMap<String, MountedInfo>>> {
        self.mounted_info_set.clone()
    }
}
