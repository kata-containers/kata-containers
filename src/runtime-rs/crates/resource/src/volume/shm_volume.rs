// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};

use super::Volume;
use crate::share_fs::DEFAULT_KATA_GUEST_SANDBOX_DIR;
use anyhow::Result;
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::{get_mount_path, get_mount_type};
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

pub const SHM_DIR: &str = "shm";
// DEFAULT_SHM_SIZE is the default shm size to be used in case host
// IPC is used.
pub const DEFAULT_SHM_SIZE: u64 = 65536 * 1024;

// KATA_EPHEMERAL_DEV_TYPE creates a tmpfs backed volume for sharing files between containers.
pub const KATA_EPHEMERAL_DEV_TYPE: &str = "ephemeral";

#[derive(Debug)]
pub(crate) struct ShmVolume {
    mount: oci::Mount,
    storage: Option<agent::Storage>,
}

impl ShmVolume {
    pub(crate) fn new(m: &oci::Mount, shm_size: u64) -> Result<Self> {
        let (storage, mount) = if shm_size > 0 {
            // storage
            let mount_path = Path::new(DEFAULT_KATA_GUEST_SANDBOX_DIR).join(SHM_DIR);
            let mount_path = mount_path.to_str().unwrap();
            let option = format!("size={}", shm_size);

            let options = vec![
                String::from("noexec"),
                String::from("nosuid"),
                String::from("nodev"),
                String::from("mode=1777"),
                option,
            ];

            let storage = agent::Storage {
                driver: String::from(KATA_EPHEMERAL_DEV_TYPE),
                driver_options: Vec::new(),
                source: String::from("shm"),
                fs_type: String::from("tmpfs"),
                fs_group: None,
                options,
                mount_point: mount_path.to_string(),
            };

            let mut oci_mount = oci::Mount::default();
            oci_mount.set_destination(m.destination().clone());
            oci_mount.set_typ(Some("bind".to_string()));
            oci_mount.set_source(Some(PathBuf::from(&mount_path)));
            oci_mount.set_options(Some(vec!["rbind".to_string()]));

            (Some(storage), oci_mount)
        } else {
            let mut oci_mount = oci::Mount::default();
            oci_mount.set_destination(m.destination().clone());
            oci_mount.set_typ(Some("tmpfs".to_string()));
            oci_mount.set_source(Some(PathBuf::from("shm")));
            oci_mount.set_options(Some(
                [
                    "noexec",
                    "nosuid",
                    "nodev",
                    "mode=1777",
                    &format!("size={}", DEFAULT_SHM_SIZE),
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ));

            (None, oci_mount)
        };

        Ok(Self { storage, mount })
    }
}

#[async_trait]
impl Volume for ShmVolume {
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

pub(crate) fn is_shm_volume(m: &oci::Mount) -> bool {
    get_mount_path(&Some(m.destination().clone())).as_str() == "/dev/shm"
        && get_mount_type(m).as_str() != KATA_EPHEMERAL_DEV_TYPE
}
