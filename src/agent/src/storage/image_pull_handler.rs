// Copyright (c) 2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use super::new_device;
use crate::confidential_data_hub;
use crate::confidential_data_hub::image::unpack_pause_image;
use crate::rpc::CONTAINER_BASE;
use crate::storage::{StorageContext, StorageHandler};
use anyhow::{anyhow, Result};
use kata_types::mount::StorageDevice;
use kata_types::mount::KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL;
use protocols::agent::Storage;
use safe_path::scoped_join;
use std::sync::Arc;
use tracing::instrument;

#[derive(Debug)]
pub struct ImagePullHandler {}

#[async_trait::async_trait]
impl StorageHandler for ImagePullHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[KATA_VIRTUAL_VOLUME_IMAGE_GUEST_PULL]
    }

    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let cid = ctx
            .cid
            .clone()
            .ok_or_else(|| anyhow!("failed to get container id"))?;

        if cid == ctx.sandbox.lock().await.id {
            // The sandbox container receives the built-in pause image.
            let mount_path = unpack_pause_image(&cid)?;
            return new_device(mount_path);
        }

        let image_name = storage.source();
        debug!(ctx.logger, "image_name = {:?}", image_name);

        // generated bundles with rootfs and config.json will store under CONTAINER_BASE/cid/images.
        let bundle_path = scoped_join(CONTAINER_BASE, &cid)?;
        let bundle_path = match confidential_data_hub::pull_image(image_name, bundle_path).await {
            Ok(path) => {
                info!(
                    ctx.logger,
                    "pull and unpack image {image_name}, cid: {cid} succeeded."
                );
                path
            }
            Err(e) => {
                error!(
                    ctx.logger,
                    "pull and unpack image {image_name}, cid: {cid} failed with {:?}.",
                    e.to_string()
                );
                return Err(e);
            }
        };

        new_device(bundle_path)
    }
}
