// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::{Rootfs, ROOTFS};
use crate::share_fs::{do_get_guest_path, do_get_host_path};
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{
    device::{
        device_manager::{do_handle_device, get_block_driver, DeviceManager},
        DeviceConfig, DeviceType,
    },
    BlockConfig,
};
use kata_types::config::hypervisor::{
    VIRTIO_BLK_CCW, VIRTIO_BLK_MMIO, VIRTIO_BLK_PCI, VIRTIO_PMEM, VIRTIO_SCSI,
};
use kata_types::fs::VM_ROOTFS_FILESYSTEM_XFS;
use kata_types::mount::Mount;
use nix::sys::stat::{self, SFlag};
use oci_spec::runtime as oci;
use std::fs;
use tokio::sync::RwLock;

const BLOCKFILE_ROOTFS_FLAG: &str = "loop";

pub(crate) struct BlockRootfs {
    guest_path: String,
    device_id: String,
    mount: oci::Mount,
    storage: Option<agent::Storage>,
}

impl BlockRootfs {
    pub async fn new(
        d: &RwLock<DeviceManager>,
        sid: &str,
        cid: &str,
        dev_id: u64,
        rootfs: &Mount,
    ) -> Result<Self> {
        let container_path = do_get_guest_path(ROOTFS, cid, false, false);
        let host_path = do_get_host_path(ROOTFS, sid, cid, false, false);
        // Create rootfs dir on host to make sure mount point in guest exists, as readonly dir is
        // shared to guest via virtiofs, and guest is unable to create rootfs dir.
        fs::create_dir_all(&host_path)
            .map_err(|e| anyhow!("failed to create rootfs dir {}: {:?}", host_path, e))?;

        let block_driver = get_block_driver(d).await;

        let block_device_config = &mut BlockConfig {
            major: stat::major(dev_id) as i64,
            minor: stat::minor(dev_id) as i64,
            driver_option: block_driver.clone(),
            path_on_host: rootfs.source.clone(),
            ..Default::default()
        };

        // create and insert block device into Kata VM
        let device_info = do_handle_device(d, &DeviceConfig::BlockCfg(block_device_config.clone()))
            .await
            .context("do handle device failed.")?;

        let mut storage = Storage {
            fs_type: rootfs.fs_type.clone(),
            mount_point: container_path.clone(),
            options: vec![],
            ..Default::default()
        };

        // XFS rootfs: add 'nouuid' to avoid UUID conflicts when the same
        // disk image is mounted across multiple VMs/containers.
        // This allows mounting XFS volumes that share the same UUID.
        if rootfs.fs_type == VM_ROOTFS_FILESYSTEM_XFS {
            storage.options.push("nouuid".to_string());
        }

        let mut device_id: String = "".to_owned();
        if let DeviceType::Block(device) = device_info {
            storage.driver = device.config.driver_option;
            device_id = device.device_id;

            match block_driver.as_str() {
                VIRTIO_BLK_PCI => {
                    storage.source = device
                        .config
                        .pci_path
                        .ok_or("PCI path missing for pci block device")
                        .map_err(|e| anyhow!(e))?
                        .to_string();
                }
                VIRTIO_BLK_MMIO => {
                    storage.source = device.config.virt_path;
                }
                VIRTIO_SCSI | VIRTIO_BLK_CCW | VIRTIO_PMEM => {
                    return Err(anyhow!(
                        "Complete support for block driver {} has not been implemented yet",
                        block_driver
                    ));
                }
                _ => {
                    return Err(anyhow!("Unknown block driver : {}", block_driver));
                }
            }
        }

        Ok(Self {
            guest_path: container_path.clone(),
            device_id,
            mount: oci::Mount::default(),
            storage: Some(storage),
        })
    }
}

#[async_trait]
impl Rootfs for BlockRootfs {
    async fn get_guest_rootfs_path(&self) -> Result<String> {
        Ok(self.guest_path.clone())
    }

    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    async fn get_storage(&self) -> Option<Storage> {
        self.storage.clone()
    }

    async fn get_device_id(&self) -> Result<Option<String>> {
        Ok(Some(self.device_id.clone()))
    }

    async fn cleanup(&self, device_manager: &RwLock<DeviceManager>) -> Result<()> {
        device_manager
            .write()
            .await
            .try_remove_device(&self.device_id)
            .await
    }
}

pub(crate) fn is_block_rootfs(m: &Mount) -> Option<(u64, Mount)> {
    if m.source.is_empty() {
        return None;
    }

    match stat::stat(m.source.as_str()) {
        Ok(fstat) => {
            if SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFBLK {
                let dev_id = fstat.st_rdev;
                let mut volume = m.clone();

                //clear the volume resource thus the block device will use the dev_id
                //to find the device's host path;
                volume.source = String::new();
                return Some((dev_id, volume));
            }

            if SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFREG
                && m.options.contains(&BLOCKFILE_ROOTFS_FLAG.to_string())
            {
                //use the block file's inode as the device id, which can make sure it's unique.
                let dev_id = fstat.st_ino;
                let options = m
                    .options
                    .clone()
                    .into_iter()
                    .filter(|o| !o.eq(BLOCKFILE_ROOTFS_FLAG))
                    .collect();

                //discard the blockfile rootfs's mount option "loop"
                let mut volume = m.clone();
                volume.options = options;

                return Some((dev_id, volume));
            }
        }
        Err(_) => return None,
    };
    None
}
