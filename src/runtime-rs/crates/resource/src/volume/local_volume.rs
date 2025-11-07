// Copyright (c) 2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::{Path, PathBuf};

use crate::share_fs::{kata_guest_share_dir, PASSTHROUGH_FS_DIR};

use super::Volume;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::device::device_manager::DeviceManager;
use kata_sys_util::mount::{get_mount_path, get_mount_type};
use kata_types::mount::KATA_K8S_LOCAL_STORAGE_TYPE;
use nix::sys::stat::stat;
use oci_spec::runtime as oci;
use tokio::sync::RwLock;

/// Allocating an FSGroup that owns the pod's volumes
const FS_GID: &str = "fsgid";

#[derive(Debug)]
pub(crate) struct LocalStorage {
    mounts: Vec<oci::Mount>,
    storage: Option<agent::Storage>,
}

impl LocalStorage {
    pub(crate) fn new(m: &oci::Mount, sid: &str, cid: &str) -> Result<Self> {
        if m.source().is_none() {
            return Err(anyhow!(format!(
                "got a wrong volume without source: {:?}",
                m
            )));
        }

        let source = &get_mount_path(m.source());
        let file_stat = stat(Path::new(source))?;
        let mut dir_options = vec!["mode=0777".to_string()];

        // if volume's gid isn't root group(default group), this means there's
        // an specific fsGroup is set on this local volume, then it should pass
        // to guest.
        if file_stat.st_gid != 0 {
            dir_options.push(format!("{}={}", FS_GID, file_stat.st_gid));
        }

        let file_name = Path::new(source)
            .file_name()
            .context(format!("get file name from {:?}", &m.source()))?;

        // Set the mount source path to a the desired directory point in the VM.
        // In this case it is located in the sandbox directory.
        // We rely on the fact that the first container in the VM has the same ID as the sandbox ID.
        // In Kubernetes, this is usually the pause container and we depend on it existing for
        // local directories to work.
        let source = Path::new(&kata_guest_share_dir())
            .join(PASSTHROUGH_FS_DIR)
            .join(sid)
            .join("rootfs")
            .join(KATA_K8S_LOCAL_STORAGE_TYPE)
            .join(file_name)
            .into_os_string()
            .into_string()
            .map_err(|e| anyhow!("failed to get local path {:?}", e))?;

        // Create a storage struct so that kata agent is able to create
        // tmpfs backed volume inside the VM
        let local_storage = agent::Storage {
            driver: String::from(KATA_K8S_LOCAL_STORAGE_TYPE),
            driver_options: Vec::new(),
            source: String::from(KATA_K8S_LOCAL_STORAGE_TYPE),
            fs_type: String::from(KATA_K8S_LOCAL_STORAGE_TYPE),
            fs_group: None,
            options: dir_options,
            mount_point: source.clone(),
        };

        let mounts: Vec<oci::Mount> = if sid != cid {
            let mut mount = oci::Mount::default();
            mount.set_destination(m.destination().clone());
            mount.set_typ(Some("bind".to_string()));
            mount.set_source(Some(PathBuf::from(&source)));
            mount.set_options(m.options().clone());
            vec![mount]
        } else {
            vec![]
        };

        Ok(Self {
            mounts,
            storage: Some(local_storage),
        })
    }
}

#[async_trait]
impl Volume for LocalStorage {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        Ok(self.mounts.clone())
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
        // TODO: Clean up LocalStorage
        warn!(sl!(), "Cleaning up LocalStorage is no need.");
        Ok(())
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

pub(crate) fn is_local_volume(m: &oci::Mount) -> bool {
    get_mount_type(m).as_str() == KATA_K8S_LOCAL_STORAGE_TYPE
}
