// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::PathBuf;

use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, DeviceManager},
        DeviceConfig, DeviceType,
    },
    get_vfio_device, VfioConfig,
};
use kata_sys_util::mount::get_mount_type;
use kata_types::mount::DirectVolumeMountInfo;
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

use crate::volume::{
    utils::{generate_shared_path, DEFAULT_VOLUME_FS_TYPE},
    Volume,
};

pub(crate) struct VfioVolume {
    storage: Option<agent::Storage>,
    mount: oci::Mount,
    device_id: String,
}

// VfioVolume: vfio device based block volume
impl VfioVolume {
    pub(crate) async fn new(
        d: &RwLock<DeviceManager>,
        m: &oci::Mount,
        mount_info: &DirectVolumeMountInfo,
        read_only: bool,
        sid: &str,
    ) -> Result<Self> {
        // support both /dev/vfio/X and BDF<DDDD:BB:DD.F> or BDF<BB:DD.F>
        let vfio_device =
            get_vfio_device(mount_info.device.clone()).context("get vfio device failed.")?;
        let vfio_dev_config = &mut VfioConfig {
            host_path: vfio_device.clone(),
            dev_type: "b".to_string(),
            hostdev_prefix: "vfio_vol".to_owned(),
            ..Default::default()
        };

        // create and insert block device into Kata VM
        let device_info = do_handle_device(d, &DeviceConfig::VfioCfg(vfio_dev_config.clone()))
            .await
            .context("do handle device failed.")?;

        let storage_options = if read_only {
            vec!["ro".to_string()]
        } else {
            Vec::new()
        };

        let mut storage = agent::Storage {
            options: storage_options,
            ..Default::default()
        };

        let mut device_id = String::new();
        if let DeviceType::Vfio(device) = device_info {
            device_id = device.device_id;
            storage.driver = device.driver_type;
            // safe here, device_info is correct and only unwrap it.
            storage.source = device.config.virt_path.unwrap().1;
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

        let mut oci_mount = oci::Mount::default();
        oci_mount.set_destination(m.destination().clone());
        oci_mount.set_typ(Some(mount_info.fs_type.clone()));
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
impl Volume for VfioVolume {
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
