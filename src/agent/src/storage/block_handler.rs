// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use kata_sys_util::mount::get_device_mounted_count;
use kata_types::mount::{KataVirtualVolume, StorageDevice};
use nix::mount::MsFlags;
use protocols::agent::Storage;
use tracing::instrument;

use crate::device::{
    get_scsi_device_name, get_virtio_blk_pci_device_name, get_virtio_mmio_device_name,
    wait_for_pmem_device,
};
use crate::mount::{VERITY_DEVICE_MOUNT_OPTION, VERITY_DEVICE_MOUNT_PATH};
use crate::pci;
use crate::storage::{common_storage_handler, new_device, StorageContext, StorageHandler};
#[cfg(target_arch = "s390x")]
use crate::{ccw, device::get_virtio_blk_ccw_device_name};

#[derive(Debug)]
pub struct VirtioBlkMmioHandler {}

#[async_trait::async_trait]
impl StorageHandler for VirtioBlkMmioHandler {
    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        if !Path::new(&storage.source).exists() {
            get_virtio_mmio_device_name(ctx.sandbox, &storage.source)
                .await
                .context("failed to get mmio device name")?;
        }
        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}

#[derive(Debug)]
pub struct VirtioBlkPciHandler {}

#[async_trait::async_trait]
impl StorageHandler for VirtioBlkPciHandler {
    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        // If hot-plugged, get the device node path based on the PCI path
        // otherwise use the virt path provided in Storage Source
        if storage.source.starts_with("/dev") {
            let metadata = fs::metadata(&storage.source)
                .context(format!("get metadata on file {:?}", &storage.source))?;
            let mode = metadata.permissions().mode();
            if mode & libc::S_IFBLK == 0 {
                return Err(anyhow!("Invalid device {}", &storage.source));
            }
        } else {
            let pcipath = pci::Path::from_str(&storage.source)?;
            let dev_path = get_virtio_blk_pci_device_name(ctx.sandbox, &pcipath).await?;
            storage.source = dev_path;
        }
        let options = storage.options();
        if options.contains(&VERITY_DEVICE_MOUNT_OPTION.to_string()) {
            let logger = ctx.logger;
            let cid = ctx
                .cid
                .clone()
                .ok_or_else(|| anyhow!("No container id in rw overlay"))?;
            let virt_volume_base64 = options[0].clone();
            let virt_volume: KataVirtualVolume =
                KataVirtualVolume::from_base64(&virt_volume_base64)?;

            let mount_path = format!("{}/{}/{}", VERITY_DEVICE_MOUNT_PATH, cid, "lowerdir");
            let mount_type = storage.fstype();
            if fs::metadata(&mount_path).is_err() {
                fs::create_dir_all(&mount_path)
                    .map_err(anyhow::Error::from)
                    .context("Could not create mountpath")?;
            }

            let verity_info = virt_volume
                .dm_verity
                .ok_or_else(|| anyhow!("failed to get dm verity info"))?;
            info!(
                logger,
                "virtio_blk_storage_handler verity_info = {:?}", verity_info
            );
            let verity_device_name = &verity_info.hash;
            let count =
                get_device_mounted_count(&format!("{}/{}", "/dev/mapper", verity_device_name))?;
            if count > 0 {
                nix::mount::mount(
                    Some(format!("/dev/mapper/{}", verity_device_name).as_str()),
                    mount_path.as_str(),
                    Some(mount_type),
                    MsFlags::MS_RDONLY,
                    None::<&str>,
                )?;
            } else {
                let info_json = serde_json::to_string(&verity_info)?;
                image_rs::verity::mount_image_block_with_integrity(
                    info_json.as_str(),
                    Path::new(&storage.source),
                    Path::new(&mount_path),
                    mount_type,
                )?;
            }
            return new_device(mount_path);
        }

        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}

#[derive(Debug)]
pub struct VirtioBlkCcwHandler {}

#[async_trait::async_trait]
impl StorageHandler for VirtioBlkCcwHandler {
    #[cfg(target_arch = "s390x")]
    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let ccw_device = ccw::Device::from_str(&storage.source)?;
        let dev_path = get_virtio_blk_ccw_device_name(ctx.sandbox, &ccw_device).await?;
        storage.source = dev_path;
        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }

    #[cfg(not(target_arch = "s390x"))]
    #[instrument]
    async fn create_device(
        &self,
        _storage: Storage,
        _ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        Err(anyhow!("CCW is only supported on s390x"))
    }
}

#[derive(Debug)]
pub struct ScsiHandler {}

#[async_trait::async_trait]
impl StorageHandler for ScsiHandler {
    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        // Retrieve the device path from SCSI address.
        let dev_path = get_scsi_device_name(ctx.sandbox, &storage.source).await?;
        storage.source = dev_path;

        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}

#[derive(Debug)]
pub struct PmemHandler {}

#[async_trait::async_trait]
impl StorageHandler for PmemHandler {
    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        // Retrieve the device for pmem storage
        wait_for_pmem_device(ctx.sandbox, &storage.source).await?;

        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}
