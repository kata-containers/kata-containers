// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::PathBuf;

use super::Volume;
use anyhow::Result;
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::{get_mount_path, get_mount_type};
use kata_types::mount::{
    DEFAULT_KATA_GUEST_SANDBOX_DIR, KATA_EPHEMERAL_VOLUME_TYPE, SHM_DEVICE, SHM_DIR,
};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

#[derive(Debug)]
pub(crate) struct ShmVolume {
    mount: oci::Mount,
}

impl ShmVolume {
    pub(crate) fn new(m: &oci::Mount) -> Result<Self> {
        let mut mount = oci::Mount::default();
        mount.set_destination(m.destination().clone());
        mount.set_typ(Some("bind".to_string()));
        mount.set_source(Some(
            PathBuf::from(DEFAULT_KATA_GUEST_SANDBOX_DIR).join(SHM_DIR),
        ));
        mount.set_options(Some(vec!["rbind".to_string()]));

        Ok(Self { mount })
    }
}

#[async_trait]
impl Volume for ShmVolume {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(vec![])
    }

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        // No cleanup is required for ShmVolume because it is a mount in guest which
        // does not require explicit unmounting or deletion in host side.
        Ok(())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

pub(crate) fn is_shm_volume(m: &oci::Mount) -> bool {
    get_mount_path(&Some(m.destination().clone())).as_str() == SHM_DEVICE
        && get_mount_type(m).as_str() != KATA_EPHEMERAL_VOLUME_TYPE
}
