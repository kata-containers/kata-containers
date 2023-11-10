// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use tokio::sync::RwLock;

use super::Volume;
use crate::share_fs::DEFAULT_KATA_GUEST_SANDBOX_DIR;

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

            // mount
            let mount = oci::Mount {
                r#type: "bind".to_string(),
                destination: m.destination.clone(),
                source: mount_path.to_string(),
                options: vec!["rbind".to_string()],
            };

            (Some(storage), mount)
        } else {
            let mount = oci::Mount {
                r#type: "tmpfs".to_string(),
                destination: m.destination.clone(),
                source: "shm".to_string(),
                options: [
                    "noexec",
                    "nosuid",
                    "nodev",
                    "mode=1777",
                    &format!("size={}", DEFAULT_SHM_SIZE),
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            };
            (None, mount)
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
    m.destination == "/dev/shm" && m.r#type != KATA_EPHEMERAL_DEV_TYPE
}
