// Copyright (c) 2019 Ant Financial
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use kata_types::mount::StorageDevice;
use protocols::agent::Storage;
use std::iter;
use std::sync::Arc;
use tracing::instrument;

use crate::storage::{new_device, StorageContext, StorageHandler};

#[derive(Debug)]
pub struct BindWatcherHandler {}

#[async_trait::async_trait]
impl StorageHandler for BindWatcherHandler {
    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        if let Some(cid) = ctx.cid {
            ctx.sandbox
                .lock()
                .await
                .bind_watcher
                .add_container(cid.to_string(), iter::once(storage.clone()), ctx.logger)
                .await?;
        }
        new_device("".to_string())
    }
}
