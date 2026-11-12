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
        device_manager::{do_handle_device, get_block_device_info, DeviceManager},
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
    utils::{
        build_bind_mount_options, filter_block_storage_options, generate_shared_path,
        DEFAULT_VOLUME_FS_TYPE, KATA_MOUNT_BIND_TYPE,
    },
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

        let block_driver = get_block_device_info(d).await.block_device_driver;

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

        let oci_opts = get_mount_options(m.options());
        let mut storage_options: Vec<String> =
            filter_block_storage_options(if !mount_info.options.is_empty() {
                &mount_info.options
            } else {
                &oci_opts
            });

        if read_only && !storage_options.iter().any(|o| o == "ro") {
            storage_options.push("ro".to_string());
        }

        let mut storage = agent::Storage {
            options: storage_options,
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
        let guest_path = generate_shared_path(m.destination().clone(), &device_id, sid)
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
        oci_mount.set_typ(Some(KATA_MOUNT_BIND_TYPE.to_string()));
        oci_mount.set_source(Some(PathBuf::from(&guest_path)));
        oci_mount.set_options(Some(build_bind_mount_options(
            &mount_info.options,
            m.options(),
        )));

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

#[cfg(test)]
mod tests {
    use super::*;
    use hypervisor::device::device_manager::DeviceManager;
    use hypervisor::qemu::Qemu;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    async fn new_device_manager() -> Arc<RwLock<DeviceManager>> {
        Arc::new(RwLock::new(
            DeviceManager::new(Arc::new(Qemu::new()), None)
                .await
                .unwrap(),
        ))
    }

    fn new_mount_info(volume_type: &str, device: &str) -> DirectVolumeMountInfo {
        DirectVolumeMountInfo {
            volume_type: volume_type.to_string(),
            device: device.to_string(),
            fs_type: "ext4".to_string(),
            ..Default::default()
        }
    }

    // SPDKVolume is not Debug, so unwrap_err() is not available here.
    async fn volume_creation_error(
        d: &RwLock<DeviceManager>,
        mount_info: &DirectVolumeMountInfo,
    ) -> anyhow::Error {
        match SPDKVolume::new(d, &oci::Mount::default(), mount_info, false, "sid").await {
            Ok(_) => panic!("creating the SPDK volume should fail"),
            Err(e) => e,
        }
    }

    #[actix_rt::test]
    async fn test_spdk_volume_rejects_unknown_volume_type() {
        let d = new_device_manager().await;
        let mount_info = new_mount_info("directvol", "/dev/vdb");
        let err = volume_creation_error(&d, &mount_info).await;
        assert!(
            format!("{err:#}").contains("mountinfo.json is invalid"),
            "unexpected error: {err:#}"
        );
    }

    #[actix_rt::test]
    async fn test_spdk_volume_missing_socket_returns_error() {
        let d = new_device_manager().await;

        for volume_type in [KATA_SPDK_VOLUME_TYPE, KATA_SPOOL_VOLUME_TYPE] {
            // The SPDK backend is not running, so its vhost-user socket does
            // not exist. The volume must be rejected with an error instead of
            // leaving the caller waiting for a backend that will never answer.
            let mount_info = new_mount_info(volume_type, "/run/vhu/does-not-exist.sock");
            let err = volume_creation_error(&d, &mount_info).await;
            assert!(
                format!("{err:#}").contains("stat socket failed"),
                "unexpected error for {volume_type}: {err:#}"
            );

            // A device that already carries the type prefix must be handled
            // the same way, the prefix is not part of the socket path.
            let prefixed = format!("{volume_type}:///run/vhu/does-not-exist.sock");
            let mount_info = new_mount_info(volume_type, &prefixed);
            let err = volume_creation_error(&d, &mount_info).await;
            assert!(
                format!("{err:#}").contains("stat socket failed"),
                "unexpected error for prefixed {volume_type}: {err:#}"
            );
        }
    }

    #[actix_rt::test]
    async fn test_spdk_volume_rejects_non_socket_device() {
        let d = new_device_manager().await;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-socket");

        // A regular file is not a vhost-user backend socket.
        std::fs::write(&path, b"not a socket").unwrap();
        let mount_info = new_mount_info(KATA_SPDK_VOLUME_TYPE, path.to_str().unwrap());
        let err = volume_creation_error(&d, &mount_info).await;
        assert!(
            format!("{err:#}").contains("is not valid"),
            "unexpected error: {err:#}"
        );
    }
}
