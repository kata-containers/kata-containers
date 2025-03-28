// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};

use super::Volume;
use crate::share_fs::DEFAULT_KATA_GUEST_SANDBOX_DIR;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::get_mount_type;
use kata_types::mount::KATA_EPHEMERAL_VOLUME_TYPE;
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

#[derive(Debug)]
pub(crate) struct EphemeralVolume {
    mount: oci::Mount,
    storage: Option<agent::Storage>,
}

impl EphemeralVolume {
    pub(crate) fn new(m: &oci::Mount) -> Result<Self> {
        if m.source().is_none() {
            return Err(anyhow!(format!(
                "got a wrong volume without source: {:?}",
                m
            )));
        }
        // storage
        // its safe here to unwrap on m.source since it has been checked.
        let mount_path = Path::new(DEFAULT_KATA_GUEST_SANDBOX_DIR)
            .join(KATA_EPHEMERAL_VOLUME_TYPE)
            .join(m.source().as_ref().unwrap());
        let mount_path = mount_path
            .to_str()
            .ok_or(anyhow!("faild to parse volume's mount path"))?;

        let options = vec![
            String::from("noexec"),
            String::from("nosuid"),
            String::from("nodev"),
            String::from("mode=1777"),
        ];

        let storage = agent::Storage {
            driver: String::from(KATA_EPHEMERAL_VOLUME_TYPE),
            driver_options: Vec::new(),
            source: String::from("shm"),
            fs_type: String::from("tmpfs"),
            fs_group: None,
            options,
            mount_point: mount_path.to_string(),
        };

        let mut mount = oci::Mount::default();
        mount.set_destination(m.destination().clone());
        mount.set_typ(Some("bind".to_string()));
        mount.set_source(Some(PathBuf::from(&mount_path)));
        mount.set_options(Some(vec!["rbind".to_string()]));

        Ok(Self {
            mount,
            storage: Some(storage),
        })
    }
}

#[async_trait]
impl Volume for EphemeralVolume {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
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

    async fn cleanup(&self, _device_manager: &RwLock<DeviceManager>) -> Result<()> {
        // TODO: Clean up ShmVolume
        warn!(sl!(), "Cleaning up ShmVolume is still unimplemented.");
        Ok(())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

pub(crate) fn is_ephemeral_volume(m: &oci::Mount) -> bool {
    get_mount_type(m).as_str() == KATA_EPHEMERAL_VOLUME_TYPE
}
