// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use kata_sys_util::mount::create_mount_destination;
use kata_types::dmverity::{create_verity_device, destroy_verity_device, DmVerityInfo};
use kata_types::mount::StorageDevice;
use kata_types::volume::{
    KATA_VOLUME_DMVERITY_OPTION_SOURCE_TYPE, KATA_VOLUME_DMVERITY_OPTION_VERITY_INFO,
    KATA_VOLUME_DMVERITY_SOURCE_TYPE_PMEM, KATA_VOLUME_DMVERITY_SOURCE_TYPE_SCSI,
    KATA_VOLUME_DMVERITY_SOURCE_TYPE_VIRTIO_CCW, KATA_VOLUME_DMVERITY_SOURCE_TYPE_VIRTIO_MMIO,
    KATA_VOLUME_DMVERITY_SOURCE_TYPE_VIRTIO_PCI,
};
use protocols::agent::Storage;
use slog::Logger;
use tracing::instrument;

use crate::storage::block_handler::{
    PmemHandler, ScsiHandler, VirtioBlkCcwHandler, VirtioBlkMmioHandler, VirtioBlkPciHandler,
};
use crate::storage::{common_storage_handler, StorageContext, StorageHandler};

use super::StorageDeviceGeneric;

#[derive(Debug)]
pub struct DmVerityHandler {}

impl DmVerityHandler {
    fn get_dm_verity_info(storage: &Storage) -> Result<DmVerityInfo> {
        for option in storage.driver_options.iter() {
            if let Some((key, value)) = option.split_once('=') {
                if key == KATA_VOLUME_DMVERITY_OPTION_VERITY_INFO {
                    let verity_info: DmVerityInfo = serde_json::from_str(value)?;
                    return Ok(verity_info);
                }
            }
        }

        Err(anyhow!(
            "missing DmVerity information for DmVerity volume in the Storage: {:?}",
            storage
        ))
    }

    async fn update_source_device(
        storage: &mut Storage,
        ctx: &mut StorageContext<'_>,
    ) -> Result<()> {
        for option in storage.driver_options.clone() {
            if let Some((key, value)) = option.split_once('=') {
                if key == KATA_VOLUME_DMVERITY_OPTION_SOURCE_TYPE {
                    match value {
                        KATA_VOLUME_DMVERITY_SOURCE_TYPE_VIRTIO_PCI => {
                            VirtioBlkPciHandler::update_device_path(storage, ctx).await?;
                        }
                        KATA_VOLUME_DMVERITY_SOURCE_TYPE_VIRTIO_MMIO => {
                            VirtioBlkMmioHandler::update_device_path(storage, ctx).await?;
                        }
                        KATA_VOLUME_DMVERITY_SOURCE_TYPE_VIRTIO_CCW => {
                            VirtioBlkCcwHandler::update_device_path(storage, ctx).await?;
                        }
                        KATA_VOLUME_DMVERITY_SOURCE_TYPE_SCSI => {
                            ScsiHandler::update_device_path(storage, ctx).await?;
                        }
                        KATA_VOLUME_DMVERITY_SOURCE_TYPE_PMEM => {
                            PmemHandler::update_device_path(storage, ctx).await?;
                        }
                        _ => {
                            return Err(anyhow!(
                                "Unsupported storage driver type for dm-verity {}.",
                                value
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl StorageHandler for DmVerityHandler {
    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        Self::update_source_device(&mut storage, ctx).await?;
        create_mount_destination(&storage.source, &storage.mount_point, "", &storage.fstype)
            .context("Could not create mountpoint")?;

        let verity_info = Self::get_dm_verity_info(&storage)?;
        let verity_device_path = create_verity_device(&verity_info, Path::new(storage.source()))
            .context("create device with dm-verity enabled")?;
        storage.source = verity_device_path;
        common_storage_handler(ctx.logger, &storage)?;

        Ok(Arc::new(DmVerityDevice {
            common: StorageDeviceGeneric::new(storage.mount_point),
            verity_device_path: storage.source,
            logger: ctx.logger.clone(),
        }))
    }
}

struct DmVerityDevice {
    common: StorageDeviceGeneric,
    verity_device_path: String,
    logger: Logger,
}

impl StorageDevice for DmVerityDevice {
    fn path(&self) -> Option<&str> {
        self.common.path()
    }

    fn cleanup(&self) -> Result<()> {
        self.common.cleanup().context("clean up dm-verity volume")?;
        let device_path = &self.verity_device_path;
        debug!(
            self.logger,
            "destroy verity device path = {:?}", device_path
        );
        destroy_verity_device(device_path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use kata_types::{dmverity::DmVerityInfo, volume::KATA_VOLUME_DMVERITY_OPTION_VERITY_INFO};
    use protocols::agent::Storage;

    use crate::storage::dm_verity::DmVerityHandler;

    #[test]
    fn test_get_dm_verity_info() {
        let verity_info = DmVerityInfo {
            hashtype: "sha256".to_string(),
            hash: "d86104eee715a1b59b62148641f4ca73edf1be3006c4d481f03f55ac05640570".to_string(),
            blocknum: 2361,
            blocksize: 512,
            hashsize: 4096,
            offset: 1212416,
        };

        let verity_info_str = serde_json::to_string(&verity_info);
        assert!(verity_info_str.is_ok());

        let storage = Storage {
            driver: KATA_VOLUME_DMVERITY_OPTION_VERITY_INFO.to_string(),
            driver_options: vec![format!("verity_info={}", verity_info_str.ok().unwrap())],
            ..Default::default()
        };

        match DmVerityHandler::get_dm_verity_info(&storage) {
            Ok(result) => {
                assert_eq!(verity_info, result);
            }
            Err(e) => panic!("err = {}", e),
        }
    }
}
