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

#[derive(Debug, PartialEq)]
enum ImagePullAction<'a> {
    UnpackPauseImage,
    PullImage(&'a str),
}

impl ImagePullHandler {
    fn action(storage: &Storage, is_pod_sandbox: bool) -> ImagePullAction<'_> {
        if is_pod_sandbox {
            ImagePullAction::UnpackPauseImage
        } else {
            ImagePullAction::PullImage(storage.source())
        }
    }
}

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

        let image_name = match Self::action(&storage, ctx.is_pod_sandbox) {
            ImagePullAction::UnpackPauseImage => {
                let mount_path = unpack_pause_image(&cid)?;
                return new_device(mount_path);
            }
            ImagePullAction::PullImage(image_name) => image_name,
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_for_pod_sandbox() {
        let storage = Storage {
            source: "unused-for-sandbox".to_string(),
            ..Default::default()
        };

        assert_eq!(
            ImagePullHandler::action(&storage, true),
            ImagePullAction::UnpackPauseImage
        );
    }

    #[test]
    fn test_action_for_container() {
        let storage = Storage {
            source: "example.com/image:latest".to_string(),
            ..Default::default()
        };

        assert_eq!(
            ImagePullHandler::action(&storage, false),
            ImagePullAction::PullImage("example.com/image:latest")
        );
    }
}
