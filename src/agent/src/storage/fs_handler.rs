// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::storage::{common_storage_handler, new_device, StorageContext, StorageHandler};
use anyhow::{anyhow, Context, Result};
use kata_types::device::{DRIVER_9P_TYPE, DRIVER_OVERLAYFS_TYPE, DRIVER_VIRTIOFS_TYPE};
use kata_types::mount::StorageDevice;
use protocols::agent::Storage;
use tracing::instrument;

#[derive(Debug)]
pub struct OverlayfsHandler {}

#[async_trait::async_trait]
impl StorageHandler for OverlayfsHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_OVERLAYFS_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        mut storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        if storage
            .options
            .iter()
            .any(|e| e == "io.katacontainers.fs-opt.overlay-rw")
        {
            let cid = ctx
                .cid
                .clone()
                .ok_or_else(|| anyhow!("No container id in rw overlay"))?;
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
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_9P_TYPE]
    }

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
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_VIRTIOFS_TYPE]
    }

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
