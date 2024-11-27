// Copyright (c) 2024 Ant Financial
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fs;
use std::path::Path;
use std::sync::Arc;

use crate::storage::{StorageContext, StorageHandler};
use anyhow::{anyhow, Context, Result};
use kata_types::device::DRIVER_UDS_TYPE;
use kata_types::mount::StorageDevice;
#[cfg(target_os = "linux")]
use libc::VMADDR_CID_HOST;
use protocols::agent::Storage;
use tokio::sync::Notify;
use tokio_vsock::VsockStream;
use tracing::instrument;

#[derive(Debug)]
pub struct UdsHandler {}

pub struct UdsStorage {
    notifier: Arc<Notify>,
}

impl StorageDevice for UdsStorage {
    fn path(&self) -> Option<&str> {
        None
    }

    /// Clean up resources related to the storage device.
    fn cleanup(&self) -> Result<()> {
        self.notifier.notify_waiters();
        Ok(())
    }
}

#[async_trait::async_trait]
impl StorageHandler for UdsHandler {
    #[instrument]
    fn driver_types(&self) -> &[&str] {
        &[DRIVER_UDS_TYPE]
    }

    #[instrument]
    async fn create_device(
        &self,
        storage: Storage,
        ctx: &mut StorageContext,
    ) -> Result<Arc<dyn StorageDevice>> {
        let target_dir = Path::new(&storage.mount_point);
        if let Some(dir) = target_dir.parent() {
            fs::create_dir_all(dir).context(format!("failed to create dir all {:?}", dir))?;
        }

        if storage.driver_options.len() != 1 {
            return Err(anyhow!("got the wrong uds storage, missing vport"));
        }

        let vport: u32 = storage.driver_options[0].parse()?;

        let listener = tokio::net::UnixListener::bind(storage.mount_point.as_str())
            .context(format!("bind to {:?}", storage.mount_point.as_str()))?;

        let notifier = Arc::new(Notify::new());

        let uds_device = UdsStorage {
            notifier: notifier.clone(),
        };

        let real_uds = storage.mount_point.clone();
        let logger = ctx.logger.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    new_conn = listener.accept() => {
                        if let Ok((mut stream, _addr)) = new_conn {
                            info!(logger, "share uds {} got new connect from container, try to connect to host", real_uds.as_str());
                            if let Ok(mut vstream) = VsockStream::connect(VMADDR_CID_HOST, vport).await {
                                info!(logger, "connect to host successfully!");

                                tokio::spawn(async move {
                                    let _ = tokio::io::copy_bidirectional(&mut stream, &mut vstream).await;
                                });
                            }
                        }
                    }
                    _ = notifier.notified() => {
                        info!(logger, "destroy the uds share {} in guest", real_uds.as_str());
                        break
                    }
                }
            }
        });

        Ok(Arc::new(uds_device))
    }
}
