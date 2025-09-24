// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};

use super::Volume;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::{get_mount_path, get_mount_type};
use kata_types::mount::{kata_guest_sandbox_dir, KATA_EPHEMERAL_VOLUME_TYPE};
use nix::sys::stat::stat;
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

        // refer to the golang `handleEphemeralStorage` code at
        // https://github.com/kata-containers/kata-containers/blob/9516286f6dd5cfd6b138810e5d7c9e01cf6fc043/src/runtime/virtcontainers/kata_agent.go#L1354

        let source = &get_mount_path(m.source());
        let file_stat =
            stat(Path::new(source)).with_context(|| format!("mount source {}", source))?;

        // if volume's gid isn't root group(default group), this means there's
        // an specific fsGroup is set on this local volume, then it should pass
        // to guest.
        let dir_options = if file_stat.st_gid != 0 {
            vec![format!("fsgid={}", file_stat.st_gid)]
        } else {
            vec![]
        };

        let file_name = Path::new(source)
            .file_name()
            .context(format!("get file name from {:?}", &m.source()))?;
        let source = Path::new(kata_guest_sandbox_dir().as_str())
            .join(KATA_EPHEMERAL_VOLUME_TYPE)
            .join(file_name)
            .into_os_string()
            .into_string()
            .map_err(|e| anyhow!("failed to get ephemeral path {:?}", e))?;

        // Create a storage struct so that kata agent is able to create
        // tmpfs backed volume inside the VM
        let ephemeral_storage = agent::Storage {
            driver: String::from(KATA_EPHEMERAL_VOLUME_TYPE),
            driver_options: Vec::new(),
            source: String::from("tmpfs"),
            fs_type: String::from("tmpfs"),
            fs_group: None,
            options: dir_options,
            mount_point: source.clone(),
        };

        let mut mount = oci::Mount::default();
        mount.set_destination(m.destination().clone());
        mount.set_typ(Some("bind".to_string()));
        mount.set_source(Some(PathBuf::from(&source)));
        mount.set_options(Some(vec!["rbind".to_string()]));

        Ok(Self {
            mount,
            storage: Some(ephemeral_storage),
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
        // TODO: Clean up EphemeralVolume
        warn!(sl!(), "Cleaning up EphemeralVolume is still unimplemented.");
        Ok(())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

pub(crate) fn is_ephemeral_volume(m: &oci::Mount) -> bool {
    get_mount_type(m).as_str() == KATA_EPHEMERAL_VOLUME_TYPE
}
