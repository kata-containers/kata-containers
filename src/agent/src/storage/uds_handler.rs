// Copyright (c) 2024 Ant Group
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
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use tokio::select;
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
                        if let Ok((stream, _addr)) = new_conn {
                            info!(logger, "share uds {} got new connect from container, try to connect to host", real_uds.as_str());
                            if let Ok(vstream) = VsockStream::connect(VMADDR_CID_HOST, vport).await {
                                info!(logger, "connect to host successfully!");
                                let nlogger = logger.clone();
                                tokio::spawn(async move {
                                    let _ = copy_bidirectional(stream, vstream, nlogger).await;
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

//since vsock didn't support the read/write half shutdown operation,
//thus we shouldn't use the tokio's copy_bidirectional and reimplemented
//one here to close the vsock fd instead of shutdown when meet the EOF.
async fn copy_bidirectional(
    stream1: UnixStream,
    stream2: VsockStream,
    logger: slog::Logger,
) -> Result<()> {
    let (mut reader1, mut writer1) = stream1.into_split();
    let (mut reader2, mut writer2) = stream2.split();

    let mut buf1 = vec![0u8; 1024];
    let mut buf2 = vec![0u8; 1024];

    loop {
        select! {
            read1 = reader1.read(&mut buf1) => {
                match read1 {
                    Ok(0) => {
                        debug!(logger, "guest io stream closed");
                        break;
                    }
                    Ok(n) => {
                        if writer2.write_all(&buf1[..n]).await.is_err() {
                            debug!(logger,"Failed to write to host");
                            break;
                        }
                    }
                    Err(e) => {
                        error!(logger,"Failed to read from guest: {:?}", e);
                        break;
                    }
                }
            },
            read2 = reader2.read(&mut buf2) => {
                match read2 {
                    Ok(0) => {
                        debug!(logger,"host stream closed");
                        break;
                    }
                    Ok(n) => {
                        if writer1.write_all(&buf2[..n]).await.is_err() {
                            debug!(logger,"Failed to write to guest");
                            break;
                        }
                    }
                    Err(e) => {
                        debug!(logger, "Failed to read from host: {:?}", e);
                        break;
                    }
                }
            }
        }
    }

    debug!(logger, "Copying stopped due to one side closing");
    Ok(())
}
