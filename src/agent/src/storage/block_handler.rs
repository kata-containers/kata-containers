// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
#[cfg(target_arch = "s390x")]
use kata_types::device::DRIVER_BLK_CCW_TYPE;
use kata_types::device::{
    DRIVER_BLK_MMIO_TYPE, DRIVER_BLK_PCI_TYPE, DRIVER_NVDIMM_TYPE, DRIVER_SCSI_TYPE,
};
use kata_types::mount::StorageDevice;
use nix::sys::stat::{major, minor};
use protocols::agent::Storage;
use tracing::instrument;

#[cfg(target_arch = "s390x")]
use crate::ccw;
#[cfg(target_arch = "s390x")]
use crate::device::block_device_handler::get_virtio_blk_ccw_device_name;
use crate::device::block_device_handler::{
    get_virtio_blk_mmio_device_name, get_virtio_blk_pci_device_name,
};
use crate::device::nvdimm_device_handler::wait_for_pmem_device;
use crate::device::scsi_device_handler::get_scsi_device_name;
use crate::storage::{common_storage_handler, new_device, StorageContext, StorageHandler};
use crate::{confidential_data_hub, pci, AGENT_CONFIG};
use slog::Logger;

fn get_device_number(dev_path: &str, metadata: Option<&fs::Metadata>) -> Result<String> {
    let dev_id = match metadata {
        Some(m) => m.rdev(),
        None => {
            let m =
                fs::metadata(dev_path).context(format!("get metadata on file {:?}", dev_path))?;
            m.rdev()
        }
    };
    Ok(format!("{}:{}", major(dev_id), minor(dev_id)))
}

async fn handle_block_storage(
    logger: &Logger,
    storage: &Storage,
    dev_num: &str,
) -> Result<Arc<dyn StorageDevice>> {
    let has_ephemeral_encryption = storage
        .driver_options
        .contains(&"encryption_key=ephemeral".to_string());

    if has_ephemeral_encryption {
        let integrity = AGENT_CONFIG.secure_storage_integrity.to_string();
        let options = std::collections::HashMap::from([
            ("deviceId".to_string(), dev_num.to_string()),
            ("encryptType".to_string(), "LUKS".to_string()),
            ("dataIntegrity".to_string(), integrity),
        ]);

        confidential_data_hub::secure_mount("BlockDevice", &options, vec![], &storage.mount_point)
            .await?;

        new_device(storage.mount_point.clone())
    } else {
        let path = common_storage_handler(logger, storage)?;
        new_device(path)
    }
}

#[derive(Debug)]
pub struct VirtioBlkMmioHandler {}

#[async_trait::async_trait]
impl StorageHandler for VirtioBlkMmioHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_BLK_MMIO_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        if !Path::new(&storage.source).exists() {
            get_virtio_blk_mmio_device_name(ctx.sandbox, &storage.source)
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
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_BLK_PCI_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let dev_num: String;

        // If hot-plugged, get the device node path based on the PCI path
        // otherwise use the virt path provided in Storage Source
        if storage.source.starts_with("/dev") {
            let metadata = fs::metadata(&storage.source)
                .context(format!("get metadata on file {:?}", &storage.source))?;
            let mode = metadata.permissions().mode();
            if mode & libc::S_IFBLK == 0 {
                return Err(anyhow!("Invalid device {}", &storage.source));
            }
            dev_num = get_device_number(&storage.source, Some(&metadata))?;
        } else {
            let pcipath = pci::Path::from_str(&storage.source)?;
            let dev_path = get_virtio_blk_pci_device_name(ctx.sandbox, &pcipath).await?;
            storage.source = dev_path;
            dev_num = get_device_number(&storage.source, None)?;
        }

        handle_block_storage(ctx.logger, &storage, &dev_num).await
    }
}

#[cfg(target_arch = "s390x")]
#[derive(Debug)]
pub struct VirtioBlkCcwHandler {}

#[cfg(target_arch = "s390x")]
#[async_trait::async_trait]
impl StorageHandler for VirtioBlkCcwHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_BLK_CCW_TYPE]
    }

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
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_SCSI_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        // Retrieve the device path from SCSI address.
        let dev_path = get_scsi_device_name(ctx.sandbox, &storage.source).await?;
        storage.source = dev_path.clone();

        let dev_num = get_device_number(&dev_path, None)?;
        handle_block_storage(ctx.logger, &storage, &dev_num).await
    }
}

#[derive(Debug)]
pub struct PmemHandler {}

#[async_trait::async_trait]
impl StorageHandler for PmemHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_NVDIMM_TYPE]
    }

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
