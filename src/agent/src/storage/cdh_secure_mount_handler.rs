// Copyright (c) 2023 Intel
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use anyhow::{anyhow, Result};
use kata_types::mount::StorageDevice;
use protocols::agent::Storage;

use crate::cdh::CDHClient;
use crate::storage::{new_device, StorageContext, StorageHandler};

pub struct CDHSecureMountHandler {}

#[async_trait::async_trait]
impl StorageHandler for CDHSecureMountHandler {
    async fn create_device(
        &self,
        storage: Storage,
        _ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let secure_mount_resp = CDHClient::new()?
            .secure_mount_async(
                storage.driver,
                storage.driver_options,
                storage.source,
                storage.fstype,
                storage.options,
                storage.mount_point,
            )
            .await
            .map_err(|e| anyhow!("secure mount failed: {:?}", e))?;
        new_device(secure_mount_resp.mountPath)
    }
}
