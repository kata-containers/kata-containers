// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use kata_types::mount::StorageDevice;
use protocols::agent::Storage;
use tracing::instrument;

use crate::{
    mount::{VERITY_DEVICE_MOUNT_OPTION, VERITY_DEVICE_MOUNT_PATH},
    storage::{common_storage_handler, new_device, StorageContext, StorageHandler},
};

#[derive(Debug)]
pub struct OverlayfsHandler {}

#[async_trait::async_trait]
impl StorageHandler for OverlayfsHandler {
    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let cid = ctx
            .cid
            .clone()
            .ok_or_else(|| anyhow!("No container id in rw overlay"))?;
        if storage
            .options
            .iter()
            .any(|e| e == "io.katacontainers.fs-opt.overlay-rw")
        {
            let cpath = Path::new(crate::rpc::CONTAINER_BASE).join(cid);
            let work = cpath.join("work");
            let upper = cpath.join("upper");

            fs::create_dir_all(&work).context("Creating overlay work directory")?;
            fs::create_dir_all(&upper).context("Creating overlay upper directory")?;

            storage.fstype = "overlay".into();
            storage
                .options
                .push(format!("upperdir={}", upper.to_string_lossy()));
            storage
                .options
                .push(format!("workdir={}", work.to_string_lossy()));
        } else if storage
            .options
            .iter()
            .any(|e| e == VERITY_DEVICE_MOUNT_OPTION)
        {
            let container_path = Path::new(VERITY_DEVICE_MOUNT_PATH);
            fs::create_dir_all(container_path.join(&cid).join("snapshotdir").join("fs"))
                .map_err(anyhow::Error::from)
                .context("Could not create upperdir")?;
            fs::create_dir_all(container_path.join(&cid).join("snapshotdir").join("work"))
                .map_err(anyhow::Error::from)
                .context("Could not create workdir")?;
        }

        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}

#[derive(Debug)]
pub struct Virtio9pHandler {}

#[async_trait::async_trait]
impl StorageHandler for Virtio9pHandler {
    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}

#[derive(Debug)]
pub struct VirtioFsHandler {}

#[async_trait::async_trait]
impl StorageHandler for VirtioFsHandler {
    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let path = common_storage_handler(ctx.logger, &storage)?;
        new_device(path)
    }
}
